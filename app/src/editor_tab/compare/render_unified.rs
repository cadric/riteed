use gtk4::prelude::*;

use super::CompareController;
use super::display::CompareDisplayModel;
use super::render::ComparePalette;
use super::unified::{UnifiedLineSide, UnifiedPresentation, build_unified_presentation};

pub(super) struct UnifiedTags {
    removed: gtk4::TextTag,
    added: gtk4::TextTag,
    reference_inline: gtk4::TextTag,
    current_inline: gtk4::TextTag,
    metadata: gtk4::TextTag,
}

impl UnifiedTags {
    pub(super) fn new(buffer: &sourceview5::Buffer) -> Self {
        let tags = Self {
            removed: gtk4::TextTag::new(None),
            added: gtk4::TextTag::new(None),
            reference_inline: gtk4::TextTag::new(None),
            current_inline: gtk4::TextTag::new(None),
            metadata: gtk4::TextTag::new(None),
        };
        let table = buffer.tag_table();
        let _added = table.add(&tags.removed);
        let _added = table.add(&tags.added);
        let _added = table.add(&tags.reference_inline);
        let _added = table.add(&tags.current_inline);
        let _added = table.add(&tags.metadata);
        tags
    }

    pub(super) fn apply_colors(&self, view: &sourceview5::View) {
        let palette = ComparePalette::from_view(view);
        self.removed.set_background_rgba(None);
        self.removed
            .set_paragraph_background_rgba(Some(&palette.removed));
        self.added.set_background_rgba(None);
        self.added
            .set_paragraph_background_rgba(Some(&palette.added));
        self.reference_inline
            .set_background_rgba(Some(&palette.reference_inline));
        self.current_inline
            .set_background_rgba(Some(&palette.current_inline));
        self.metadata
            .set_foreground_rgba(Some(&palette.placeholder));
    }
}

impl CompareController {
    pub(super) fn apply_unified_display(
        &self,
        display: &CompareDisplayModel,
        hidden_trim_whitespace_differences: bool,
    ) {
        let presentation = build_unified_presentation(display);
        let mut lines = presentation
            .lines
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>();
        if hidden_trim_whitespace_differences {
            lines.push(gettextrs::gettext(
                "Only leading or trailing whitespace differences are hidden.",
            ));
        }
        self.unified_presentation
            .borrow_mut()
            .clone_from(&presentation);
        self.unified_buffer.set_text(&lines.join("\n"));
        apply_unified_tags(&self.unified_buffer, &presentation, &self.unified_tags);
        self.unified_buffer.set_modified(false);
        self.unified_gutter.refresh();
    }
}

fn apply_unified_tags(
    buffer: &sourceview5::Buffer,
    presentation: &UnifiedPresentation,
    tags: &UnifiedTags,
) {
    clear_unified_tags(buffer, tags);
    raise_tag_priority(buffer, &tags.reference_inline);
    raise_tag_priority(buffer, &tags.current_inline);
    raise_tag_priority(buffer, &tags.metadata);
    for (row, line) in presentation.lines.iter().enumerate() {
        match line.side {
            UnifiedLineSide::Removal => apply_line_tag(buffer, row, &tags.removed),
            UnifiedLineSide::Addition => apply_line_tag(buffer, row, &tags.added),
            UnifiedLineSide::Collapsed => apply_line_tag(buffer, row, &tags.metadata),
            UnifiedLineSide::Context => {}
        }
        for range in &line.inline_ranges {
            let tag = match line.side {
                UnifiedLineSide::Removal => &tags.reference_inline,
                UnifiedLineSide::Addition => &tags.current_inline,
                UnifiedLineSide::Context | UnifiedLineSide::Collapsed => continue,
            };
            let Some((start, end)) = line_offset_bounds(buffer, row, range.start, range.end) else {
                continue;
            };
            buffer.apply_tag(tag, &start, &end);
        }
    }
}

fn clear_unified_tags(buffer: &sourceview5::Buffer, tags: &UnifiedTags) {
    remove_buffer_tag(buffer, &tags.removed);
    remove_buffer_tag(buffer, &tags.added);
    remove_buffer_tag(buffer, &tags.reference_inline);
    remove_buffer_tag(buffer, &tags.current_inline);
    remove_buffer_tag(buffer, &tags.metadata);
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
