use std::time::Duration;

use gtk4::{glib, prelude::*};

use super::EditorTab;
use super::compare::{MinimapRow, MinimapRowKind, compute_minimap_rows};
use super::minimap_palette::Palette;

const TAG_ADDED: &str = "riteed-scm-minimap-added";
const TAG_MODIFIED: &str = "riteed-scm-minimap-modified";
const TAG_DELETED: &str = "riteed-scm-minimap-deleted";
const STALE_DEBOUNCE_MS: u64 = 150;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MinimapDiffBandKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MinimapDiffBand {
    pub(crate) kind: MinimapDiffBandKind,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TextFingerprint {
    pub(crate) hash: u64,
    pub(crate) len: usize,
    pub(crate) chars: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MinimapDiffFingerprint {
    source: String,
    text: TextFingerprint,
}

pub(crate) struct MinimapDiffInput {
    pub(crate) source: String,
    pub(crate) reference_text: String,
    pub(crate) all_deleted: bool,
}

#[derive(Default)]
pub(super) struct MinimapDiffAttachment {
    pub(super) applied: Option<MinimapDiffFingerprint>,
    pub(super) pending_source: Option<String>,
    pub(super) stale: bool,
    pub(super) debounce: Option<glib::SourceId>,
    pub(super) generation: u64,
}

impl EditorTab {
    pub(crate) fn apply_source_control_minimap_diff(
        self: &std::rc::Rc<Self>,
        input: MinimapDiffInput,
    ) {
        if !self.is_document() {
            return;
        }
        if self.is_dirty() {
            self.mark_source_control_minimap_pending(input.source);
            return;
        }
        let current_text = self.buffer_text();
        let fingerprint = MinimapDiffFingerprint::new(input.source, &current_text);
        let already_current = {
            let state = self.state.borrow();
            state.ui.minimap_diff.applied.as_ref() == Some(&fingerprint)
                && !state.ui.minimap_diff.stale
        };
        if already_current {
            return;
        }
        let bands = if input.all_deleted {
            all_deleted_bands(&current_text)
        } else {
            bands_for_texts(&input.reference_text, &current_text)
        };
        self.apply_minimap_diff_bands(&bands, fingerprint);
    }

    pub(crate) fn mark_source_control_minimap_pending(&self, source: String) {
        {
            let mut state = self.state.borrow_mut();
            state.ui.minimap_diff.pending_source = Some(source);
        }
        self.refresh_source_control_minimap_stale_state();
    }

    pub(crate) fn clear_source_control_minimap_diff(&self) {
        self.clear_minimap_diff_tags();
        let mut state = self.state.borrow_mut();
        if let Some(source) = state.ui.minimap_diff.debounce.take() {
            source.remove();
        }
        state.ui.minimap_diff = MinimapDiffAttachment::default();
    }

    pub(crate) fn schedule_source_control_minimap_stale_check(self: &std::rc::Rc<Self>) {
        let generation = {
            let mut state = self.state.borrow_mut();
            if state.ui.minimap_diff.applied.is_none() {
                return;
            }
            if let Some(source) = state.ui.minimap_diff.debounce.take() {
                source.remove();
            }
            state.ui.minimap_diff.generation = state.ui.minimap_diff.generation.saturating_add(1);
            state.ui.minimap_diff.generation
        };
        let weak = std::rc::Rc::downgrade(self);
        let source =
            glib::timeout_add_local_once(Duration::from_millis(STALE_DEBOUNCE_MS), move || {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                if tab.state.borrow().ui.minimap_diff.generation == generation {
                    tab.refresh_source_control_minimap_stale_state();
                }
            });
        self.state.borrow_mut().ui.minimap_diff.debounce = Some(source);
    }

    pub(crate) fn refresh_source_control_minimap_colors(&self) {
        let stale = self.state.borrow().ui.minimap_diff.stale;
        apply_tag_colors(&self.text_buffer, &self.text_view, stale);
    }

    #[must_use]
    pub(crate) fn source_control_minimap_content_type(&self) -> Option<String> {
        self.state.borrow().document.content_type.clone()
    }

    #[cfg(test)]
    pub(crate) fn source_control_minimap_tag_counts_for_tests(&self) -> (usize, usize, usize) {
        (
            tagged_line_count(&self.text_buffer, TAG_ADDED),
            tagged_line_count(&self.text_buffer, TAG_MODIFIED),
            tagged_line_count(&self.text_buffer, TAG_DELETED),
        )
    }

    #[cfg(test)]
    pub(crate) fn source_control_minimap_stale_for_tests(&self) -> bool {
        self.state.borrow().ui.minimap_diff.stale
    }

    #[cfg(test)]
    pub(crate) fn source_control_minimap_tags_compose_for_tests(&self) -> bool {
        let table = self.text_buffer.tag_table();
        let (Some(added), Some(modified), Some(deleted)) = (
            table.lookup(TAG_ADDED),
            table.lookup(TAG_MODIFIED),
            table.lookup(TAG_DELETED),
        ) else {
            return false;
        };
        added.priority() == 0
            && modified.priority() == 1
            && deleted.priority() == 2
            && [added, modified, deleted].into_iter().all(|tag| {
                tag.foreground_rgba().is_none()
                    && tag.background_rgba().is_none()
                    && tag.paragraph_background_rgba().is_some()
            })
    }

    fn refresh_source_control_minimap_stale_state(&self) {
        let applied = self
            .state
            .borrow()
            .ui
            .minimap_diff
            .applied
            .as_ref()
            .map(|applied| applied.text);
        let is_stale = applied.is_some_and(|fingerprint| {
            let buffer_chars = usize::try_from(self.text_buffer.char_count()).unwrap_or_default();
            buffer_chars != fingerprint.chars
                || fingerprint != text_fingerprint(&self.buffer_text())
        });
        let changed = {
            let mut state = self.state.borrow_mut();
            state.ui.minimap_diff.debounce = None;
            if state.ui.minimap_diff.stale == is_stale {
                false
            } else {
                state.ui.minimap_diff.stale = is_stale;
                true
            }
        };
        if changed {
            apply_tag_colors(&self.text_buffer, &self.text_view, is_stale);
        }
    }

    fn apply_minimap_diff_bands(
        &self,
        bands: &[MinimapDiffBand],
        fingerprint: MinimapDiffFingerprint,
    ) {
        let modified = self.text_buffer.is_modified();
        self.clear_minimap_diff_tags();
        let tags = ensure_tags(&self.text_buffer);
        apply_tag_colors(&self.text_buffer, &self.text_view, false);
        for band in bands {
            apply_band(&self.text_buffer, &tags.tag_for(band.kind), band);
        }
        self.text_buffer.set_modified(modified);
        let mut state = self.state.borrow_mut();
        state.ui.minimap_diff.applied = Some(fingerprint);
        state.ui.minimap_diff.pending_source = None;
        state.ui.minimap_diff.stale = false;
    }

    fn clear_minimap_diff_tags(&self) {
        let modified = self.text_buffer.is_modified();
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        for name in [TAG_ADDED, TAG_MODIFIED, TAG_DELETED] {
            if let Some(tag) = self.text_buffer.tag_table().lookup(name) {
                self.text_buffer.remove_tag(&tag, &start, &end);
            }
        }
        self.text_buffer.set_modified(modified);
    }
}

impl MinimapDiffFingerprint {
    fn new(source: String, text: &str) -> Self {
        Self {
            source,
            text: text_fingerprint(text),
        }
    }
}

pub(crate) fn text_fingerprint(text: &str) -> TextFingerprint {
    let mut hash = FNV_OFFSET;
    let mut chars = 0_usize;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
        if (*byte & 0xC0) != 0x80 {
            chars = chars.saturating_add(1);
        }
    }
    TextFingerprint {
        hash,
        len: text.len(),
        chars,
    }
}

pub(crate) fn decode_minimap_text(
    bytes: Vec<u8>,
) -> Result<String, crate::git_process::GitProcessError> {
    if bytes.contains(&0) {
        return Err(crate::git_process::GitProcessError::BinaryContent);
    }
    String::from_utf8(bytes).map_err(|_| crate::git_process::GitProcessError::ParseFailed)
}

pub(crate) fn bands_for_texts(reference_text: &str, current_text: &str) -> Vec<MinimapDiffBand> {
    let rows = compute_minimap_rows(reference_text, current_text);
    if rows.skip_reason.is_some() {
        return Vec::new();
    }
    bands_for_rows(&rows.rows)
}

fn bands_for_rows(rows: &[MinimapRow]) -> Vec<MinimapDiffBand> {
    let mut bands = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let band = match row.kind {
            MinimapRowKind::Equal => None,
            MinimapRowKind::Added => row
                .current_line
                .map(|line| (MinimapDiffBandKind::Added, line)),
            MinimapRowKind::Modified => row
                .current_line
                .map(|line| (MinimapDiffBandKind::Modified, line)),
            MinimapRowKind::Removed => {
                deletion_target(rows, index).map(|line| (MinimapDiffBandKind::Deleted, line))
            }
        };
        if let Some((kind, line)) = band {
            push_band(&mut bands, kind, line);
        }
    }
    bands
}

fn deletion_target(rows: &[MinimapRow], index: usize) -> Option<usize> {
    rows[..index]
        .iter()
        .rev()
        .find_map(|row| row.current_line)
        .or_else(|| {
            rows[index.saturating_add(1)..]
                .iter()
                .find_map(|row| row.current_line)
        })
}

fn all_deleted_bands(current_text: &str) -> Vec<MinimapDiffBand> {
    let lines = current_line_count(current_text);
    if lines == 0 {
        Vec::new()
    } else {
        vec![MinimapDiffBand {
            kind: MinimapDiffBandKind::Deleted,
            start_line: 0,
            end_line: lines,
        }]
    }
}

fn current_line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.split_inclusive('\n').count()
    }
}

fn push_band(bands: &mut Vec<MinimapDiffBand>, kind: MinimapDiffBandKind, line: usize) {
    if let Some(last) = bands.last_mut()
        && last.kind == kind
        && line <= last.end_line
    {
        last.end_line = last.end_line.max(line.saturating_add(1));
        return;
    }
    bands.push(MinimapDiffBand {
        kind,
        start_line: line,
        end_line: line.saturating_add(1),
    });
}

struct MinimapTags {
    added: gtk4::TextTag,
    modified: gtk4::TextTag,
    deleted: gtk4::TextTag,
}

impl MinimapTags {
    fn tag_for(&self, kind: MinimapDiffBandKind) -> gtk4::TextTag {
        match kind {
            MinimapDiffBandKind::Added => self.added.clone(),
            MinimapDiffBandKind::Modified => self.modified.clone(),
            MinimapDiffBandKind::Deleted => self.deleted.clone(),
        }
    }
}

fn ensure_tags(buffer: &sourceview5::Buffer) -> MinimapTags {
    let added = ensure_tag(buffer, TAG_ADDED);
    let modified = ensure_tag(buffer, TAG_MODIFIED);
    let deleted = ensure_tag(buffer, TAG_DELETED);
    added.set_priority(0);
    modified.set_priority(1);
    deleted.set_priority(2);
    MinimapTags {
        added,
        modified,
        deleted,
    }
}

fn ensure_tag(buffer: &sourceview5::Buffer, name: &str) -> gtk4::TextTag {
    if let Some(tag) = buffer.tag_table().lookup(name) {
        return tag;
    }
    let tag = gtk4::TextTag::builder().name(name).build();
    let _added = buffer.tag_table().add(&tag);
    tag
}

fn apply_tag_colors(buffer: &sourceview5::Buffer, view: &sourceview5::View, stale: bool) {
    let tags = ensure_tags(buffer);
    let palette = Palette::from_view(view, stale);
    for tag in [&tags.added, &tags.modified, &tags.deleted] {
        tag.set_foreground_rgba(None);
        tag.set_background_rgba(None);
    }
    tags.added
        .set_paragraph_background_rgba(Some(&palette.added));
    tags.modified
        .set_paragraph_background_rgba(Some(&palette.modified));
    tags.deleted
        .set_paragraph_background_rgba(Some(&palette.deleted));
}

fn apply_band(buffer: &sourceview5::Buffer, tag: &gtk4::TextTag, band: &MinimapDiffBand) {
    let Some(start) = i32::try_from(band.start_line).ok() else {
        return;
    };
    let Some(end) = i32::try_from(band.end_line).ok() else {
        return;
    };
    let Some(start_iter) = buffer.iter_at_line(start) else {
        return;
    };
    let end_iter = buffer
        .iter_at_line(end)
        .unwrap_or_else(|| buffer.end_iter());
    buffer.apply_tag(tag, &start_iter, &end_iter);
}

#[cfg(test)]
fn tagged_line_count(buffer: &sourceview5::Buffer, name: &str) -> usize {
    let Some(tag) = buffer.tag_table().lookup(name) else {
        return 0;
    };
    let mut count = 0_usize;
    let line_count = buffer.line_count();
    for line in 0..line_count {
        let Some(iter) = buffer.iter_at_line(line) else {
            continue;
        };
        if iter.tags().iter().any(|candidate| candidate == &tag) {
            count = count.saturating_add(1);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::{
        MinimapDiffBand, MinimapDiffBandKind, bands_for_texts, decode_minimap_text,
        text_fingerprint,
    };
    use crate::git_process::GitProcessError;

    #[test]
    fn bands_mark_added_and_modified_lines() {
        let bands = bands_for_texts("same\nold\n", "same\nnew\nadded\n");
        assert_eq!(
            bands,
            vec![
                MinimapDiffBand {
                    kind: MinimapDiffBandKind::Modified,
                    start_line: 1,
                    end_line: 2,
                },
                MinimapDiffBand {
                    kind: MinimapDiffBandKind::Added,
                    start_line: 2,
                    end_line: 3,
                },
            ]
        );
    }

    #[test]
    fn deletion_only_maps_to_previous_or_first_surviving_line() {
        assert_eq!(
            bands_for_texts("a\nb\nc\n", "a\nc\n"),
            vec![MinimapDiffBand {
                kind: MinimapDiffBandKind::Deleted,
                start_line: 0,
                end_line: 1,
            }]
        );
        assert_eq!(
            bands_for_texts("a\nb\nc\n", "b\nc\n"),
            vec![MinimapDiffBand {
                kind: MinimapDiffBandKind::Deleted,
                start_line: 0,
                end_line: 1,
            }]
        );
    }

    #[test]
    fn clean_and_too_large_inputs_have_no_bands() {
        assert!(bands_for_texts("same\n", "same\n").is_empty());
        let large = "x".repeat(1_000_001);
        assert!(bands_for_texts(&large, "").is_empty());
    }

    #[test]
    fn text_decoder_rejects_binary_and_invalid_utf8() {
        assert_eq!(decode_minimap_text(b"text".to_vec()).as_deref(), Ok("text"));
        assert_eq!(
            decode_minimap_text(b"a\0b".to_vec()),
            Err(GitProcessError::BinaryContent)
        );
        assert_eq!(
            decode_minimap_text(vec![0xff]),
            Err(GitProcessError::ParseFailed)
        );
    }

    #[test]
    fn fnv_fingerprint_is_deterministic_and_length_aware() {
        let first = text_fingerprint("abc");
        let second = text_fingerprint("abc");
        let different = text_fingerprint("abc\n");
        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_eq!(first.len, 3);
    }

    #[test]
    fn fingerprint_counts_characters_not_bytes() {
        assert_eq!(text_fingerprint("abc").chars, 3);
        assert_eq!(text_fingerprint("æøå").chars, 3);
        assert_eq!(text_fingerprint("æøå").len, 6);
        assert_eq!(text_fingerprint("").chars, 0);
    }
}
