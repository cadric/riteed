use gtk4::{gdk, prelude::*};
use libadwaita as adw;

use super::display::{CompareDisplayModel, CompareDisplayRow, DisplayContentRow};
use super::model::{DiffRowKind, DiffSide};
use super::presentation::{DiffPresentation, PresentationSide};

const COLOR_PROBE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/compare.css";

pub(super) struct CompareTags {
    reference_removed: gtk4::TextTag,
    reference_inline: gtk4::TextTag,
    reference_placeholder: gtk4::TextTag,
    current_added: gtk4::TextTag,
    current_inline: gtk4::TextTag,
    current_placeholder: gtk4::TextTag,
}

impl CompareTags {
    pub(super) fn new(
        left_buffer: &sourceview5::Buffer,
        right_buffer: &sourceview5::Buffer,
    ) -> Self {
        let tags = Self {
            reference_removed: gtk4::TextTag::new(None),
            reference_inline: gtk4::TextTag::new(None),
            reference_placeholder: gtk4::TextTag::new(None),
            current_added: gtk4::TextTag::new(None),
            current_inline: gtk4::TextTag::new(None),
            current_placeholder: gtk4::TextTag::new(None),
        };
        let left_table = left_buffer.tag_table();
        let right_table = right_buffer.tag_table();
        let _added = left_table.add(&tags.reference_removed);
        let _added = left_table.add(&tags.reference_inline);
        let _added = left_table.add(&tags.reference_placeholder);
        let _added = right_table.add(&tags.current_added);
        let _added = right_table.add(&tags.current_inline);
        let _added = right_table.add(&tags.current_placeholder);
        tags
    }

    pub(super) fn apply_colors(&self, view: &sourceview5::View) {
        let palette = ComparePalette::from_view(view);
        self.apply_palette(&palette);
    }

    fn apply_palette(&self, palette: &ComparePalette) {
        self.reference_removed.set_background_rgba(None);
        self.reference_removed
            .set_paragraph_background_rgba(Some(&palette.removed));
        self.current_added.set_background_rgba(None);
        self.current_added
            .set_paragraph_background_rgba(Some(&palette.added));
        self.reference_inline
            .set_background_rgba(Some(&palette.reference_inline));
        self.current_inline
            .set_background_rgba(Some(&palette.current_inline));
        self.reference_placeholder
            .set_foreground_rgba(Some(&palette.placeholder));
        self.current_placeholder
            .set_foreground_rgba(Some(&palette.placeholder));
    }

    #[cfg(test)]
    pub(super) fn semantic_colors_available() -> bool {
        ComparePalette::fallback().added != ComparePalette::fallback().removed
    }

    #[cfg(test)]
    pub(super) fn uses_full_row_backgrounds_for_tests(&self) -> bool {
        self.reference_removed.paragraph_background_rgba().is_some()
            && self.reference_removed.background_rgba().is_none()
            && self.current_added.paragraph_background_rgba().is_some()
            && self.current_added.background_rgba().is_none()
            && self.reference_inline.background_rgba().is_some()
            && self.reference_inline.paragraph_background_rgba().is_none()
            && self.current_inline.background_rgba().is_some()
            && self.current_inline.paragraph_background_rgba().is_none()
    }
}

pub(super) fn apply_display_tags(
    left_buffer: &sourceview5::Buffer,
    right_buffer: &sourceview5::Buffer,
    display: &CompareDisplayModel,
    tags: &CompareTags,
) {
    for (row_index, row) in display.rows.iter().enumerate() {
        if let CompareDisplayRow::Content(row) = row {
            match row.kind {
                DiffRowKind::Equal => {}
                DiffRowKind::ReferenceOnly => {
                    apply_line_tag(left_buffer, row_index, &tags.reference_removed);
                }
                DiffRowKind::CurrentOnly => {
                    apply_line_tag(right_buffer, row_index, &tags.current_added);
                }
                DiffRowKind::Modify => {
                    apply_line_tag(left_buffer, row_index, &tags.reference_removed);
                    apply_line_tag(right_buffer, row_index, &tags.current_added);
                    apply_inline_ranges(left_buffer, right_buffer, row_index, row, tags);
                }
            }
        }
    }
}

pub(super) fn apply_presentation(
    left_buffer: &sourceview5::Buffer,
    right_buffer: &sourceview5::Buffer,
    presentation: &DiffPresentation,
) {
    left_buffer.set_text(&presentation.reference_text);
    right_buffer.set_text(&presentation.current_text);
    left_buffer.set_modified(false);
    right_buffer.set_modified(false);
}

pub(super) fn apply_placeholder_tags(
    left_buffer: &sourceview5::Buffer,
    right_buffer: &sourceview5::Buffer,
    presentation: &DiffPresentation,
    tags: &CompareTags,
) {
    raise_tag_priority(left_buffer, &tags.reference_placeholder);
    raise_tag_priority(right_buffer, &tags.current_placeholder);
    for row in 0..presentation.reference_line_numbers.len() {
        if presentation.is_metadata_row(row) {
            apply_line_tag(left_buffer, row, &tags.reference_placeholder);
            apply_line_tag(right_buffer, row, &tags.current_placeholder);
            continue;
        }
        if presentation
            .placeholder_marker(PresentationSide::Reference, row)
            .is_some()
        {
            apply_line_tag(left_buffer, row, &tags.reference_placeholder);
        }
        if presentation
            .placeholder_marker(PresentationSide::Current, row)
            .is_some()
        {
            apply_line_tag(right_buffer, row, &tags.current_placeholder);
        }
    }
}

pub(super) fn clear_tags(
    left_buffer: &sourceview5::Buffer,
    right_buffer: &sourceview5::Buffer,
    tags: &CompareTags,
) {
    remove_buffer_tag(left_buffer, &tags.reference_removed);
    remove_buffer_tag(left_buffer, &tags.reference_inline);
    remove_buffer_tag(left_buffer, &tags.reference_placeholder);
    remove_buffer_tag(right_buffer, &tags.current_added);
    remove_buffer_tag(right_buffer, &tags.current_inline);
    remove_buffer_tag(right_buffer, &tags.current_placeholder);
}

#[cfg(test)]
pub(super) fn highlight_count(buffer: &sourceview5::Buffer) -> usize {
    let mut iter = buffer.start_iter();
    let end = buffer.end_iter();
    let mut count = 0;
    while iter < end {
        count += iter
            .tags()
            .iter()
            .filter(|tag| {
                tag.background_rgba().is_some() || tag.paragraph_background_rgba().is_some()
            })
            .count();
        if !iter.forward_char() {
            break;
        }
    }
    count
}

fn apply_inline_ranges(
    left_buffer: &sourceview5::Buffer,
    right_buffer: &sourceview5::Buffer,
    row_index: usize,
    row: &DisplayContentRow,
    tags: &CompareTags,
) {
    for range in &row.inline_ranges {
        let (buffer, tag) = match range.side {
            DiffSide::Reference => (left_buffer, &tags.reference_inline),
            DiffSide::Current => (right_buffer, &tags.current_inline),
        };
        let Some((start, end)) = line_offset_bounds(buffer, row_index, range.start, range.end)
        else {
            continue;
        };
        buffer.apply_tag(tag, &start, &end);
    }
}

fn apply_line_tag(buffer: &sourceview5::Buffer, row: usize, tag: &gtk4::TextTag) {
    let Some((start, end)) = line_bounds(buffer, row) else {
        return;
    };
    buffer.apply_tag(tag, &start, &end);
}

fn raise_tag_priority(buffer: &sourceview5::Buffer, tag: &gtk4::TextTag) {
    tag.set_priority(buffer.tag_table().size().saturating_sub(1));
}

fn remove_buffer_tag(buffer: &sourceview5::Buffer, tag: &gtk4::TextTag) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.remove_tag(tag, &start, &end);
}

fn line_bounds(
    buffer: &sourceview5::Buffer,
    row: usize,
) -> Option<(gtk4::TextIter, gtk4::TextIter)> {
    let row = i32::try_from(row).ok()?;
    let start = buffer.iter_at_line(row)?;
    let mut end = start;
    if !end.forward_line() {
        end = buffer.end_iter();
    }
    Some((start, end))
}

fn line_offset_bounds(
    buffer: &sourceview5::Buffer,
    row: usize,
    start: usize,
    end: usize,
) -> Option<(gtk4::TextIter, gtk4::TextIter)> {
    let row = i32::try_from(row).ok()?;
    let start_offset = i32::try_from(start).ok()?;
    let end_offset = i32::try_from(end).ok()?;
    let start_iter = buffer.iter_at_line_offset(row, start_offset)?;
    let end_iter = buffer.iter_at_line_offset(row, end_offset)?;
    Some((start_iter, end_iter))
}

pub(super) struct ComparePalette {
    pub(super) added: gdk::RGBA,
    pub(super) removed: gdk::RGBA,
    pub(super) reference_inline: gdk::RGBA,
    pub(super) current_inline: gdk::RGBA,
    pub(super) placeholder: gdk::RGBA,
}

impl ComparePalette {
    pub(super) fn from_view(view: &sourceview5::View) -> Self {
        let fallback = Self::fallback();
        let foreground = view.color();
        let removed =
            resolve_probe_color(view, "riteed-diff-reference-color-probe", &fallback.removed);
        let added = resolve_probe_color(view, "riteed-diff-current-color-probe", &fallback.added);
        Self::from_semantic(&removed, &added, &foreground)
    }

    pub(super) fn fallback() -> Self {
        let foreground = gdk::RGBA::new(0.0, 0.0, 0.0, 1.0);
        Self::from_semantic(
            &adw::AccentColor::Red.to_rgba(),
            &adw::AccentColor::Green.to_rgba(),
            &foreground,
        )
    }

    fn from_semantic(removed: &gdk::RGBA, added: &gdk::RGBA, foreground: &gdk::RGBA) -> Self {
        let manager = adw::StyleManager::default();
        let (line_alpha, inline_alpha) = if manager.is_high_contrast() {
            (0.30, 0.55)
        } else {
            (0.16, 0.34)
        };
        Self {
            added: with_alpha(added, line_alpha),
            removed: with_alpha(removed, line_alpha),
            reference_inline: with_alpha(removed, inline_alpha),
            current_inline: with_alpha(added, inline_alpha),
            placeholder: placeholder_color(foreground),
        }
    }
}

fn placeholder_color(foreground: &gdk::RGBA) -> gdk::RGBA {
    let manager = adw::StyleManager::default();
    let alpha = if manager.is_high_contrast() {
        1.0
    } else {
        0.74
    };
    with_alpha(foreground, alpha)
}

fn with_alpha(color: &gdk::RGBA, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(color.red(), color.green(), color.blue(), alpha)
}

fn resolve_probe_color(
    view: &sourceview5::View,
    css_class: &str,
    fallback: &gdk::RGBA,
) -> gdk::RGBA {
    let base = view.color();
    let display = view.display();
    let provider = gtk4::CssProvider::new();
    provider.load_from_resource(COLOR_PROBE_CSS_RESOURCE);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    view.add_css_class(css_class);
    let resolved = view.color();
    view.remove_css_class(css_class);
    gtk4::style_context_remove_provider_for_display(&display, &provider);
    if rgba_close(&resolved, &base) {
        *fallback
    } else {
        resolved
    }
}

fn rgba_close(left: &gdk::RGBA, right: &gdk::RGBA) -> bool {
    (left.red() - right.red()).abs() < f32::EPSILON
        && (left.green() - right.green()).abs() < f32::EPSILON
        && (left.blue() - right.blue()).abs() < f32::EPSILON
        && (left.alpha() - right.alpha()).abs() < f32::EPSILON
}
