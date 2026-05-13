use gtk4::{gdk, pango, prelude::*};

pub(super) struct MarkdownTags {
    pub(super) heading: gtk4::TextTag,
    heading_2: gtk4::TextTag,
    heading_3: gtk4::TextTag,
    heading_4: gtk4::TextTag,
    heading_5: gtk4::TextTag,
    heading_6: gtk4::TextTag,
    pub(super) strong: gtk4::TextTag,
    pub(super) emphasis: gtk4::TextTag,
    pub(super) code: gtk4::TextTag,
    pub(super) code_block: gtk4::TextTag,
    pub(super) quote: gtk4::TextTag,
    pub(super) quote_marker: gtk4::TextTag,
    pub(super) list_marker: gtk4::TextTag,
    pub(super) link: gtk4::TextTag,
    pub(super) metadata: gtk4::TextTag,
    pub(super) diagnostic: gtk4::TextTag,
    pub(super) rule: gtk4::TextTag,
}

impl MarkdownTags {
    pub(super) fn new(buffer: &gtk4::TextBuffer) -> Self {
        Self {
            heading: named_tag(buffer, "markdown-heading-1", configure_heading_1),
            heading_2: named_tag(buffer, "markdown-heading-2", configure_heading_2),
            heading_3: named_tag(buffer, "markdown-heading-3", configure_heading_3),
            heading_4: named_tag(buffer, "markdown-heading-4", configure_heading_4),
            heading_5: named_tag(buffer, "markdown-heading-5", configure_heading_5),
            heading_6: named_tag(buffer, "markdown-heading-6", configure_heading_6),
            strong: named_tag(buffer, "markdown-strong", configure_strong),
            emphasis: named_tag(buffer, "markdown-emphasis", configure_emphasis),
            code: named_tag(buffer, "markdown-code", configure_code),
            code_block: named_tag(buffer, "markdown-code-block", configure_code_block),
            quote: named_tag(buffer, "markdown-quote", configure_quote),
            quote_marker: named_tag(buffer, "markdown-quote-marker", configure_quote_marker),
            list_marker: named_tag(buffer, "markdown-list-marker", configure_list_marker),
            link: named_tag(buffer, "markdown-link", configure_link),
            metadata: named_tag(buffer, "markdown-metadata", configure_metadata),
            diagnostic: named_tag(buffer, "markdown-diagnostic", configure_diagnostic),
            rule: named_tag(buffer, "markdown-rule", configure_rule),
        }
    }

    pub(super) fn heading_for_level(&self, level: u8) -> gtk4::TextTag {
        match level {
            1 => self.heading.clone(),
            2 => self.heading_2.clone(),
            3 => self.heading_3.clone(),
            4 => self.heading_4.clone(),
            5 => self.heading_5.clone(),
            _ => self.heading_6.clone(),
        }
    }
}

fn named_tag(
    buffer: &gtk4::TextBuffer,
    name: &str,
    configure: fn(&gtk4::TextTag),
) -> gtk4::TextTag {
    let table = buffer.tag_table();
    if let Some(tag) = table.lookup(name) {
        return tag;
    }
    let tag = gtk4::TextTag::new(Some(name));
    configure(&tag);
    let _added = table.add(&tag);
    tag
}

fn configure_heading_1(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.55);
    tag.set_pixels_above_lines(14);
    tag.set_pixels_below_lines(8);
}

fn configure_heading_2(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.3);
    tag.set_pixels_above_lines(12);
    tag.set_pixels_below_lines(6);
}

fn configure_heading_3(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.18);
    tag.set_pixels_above_lines(10);
    tag.set_pixels_below_lines(5);
}

fn configure_heading_4(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.08);
    tag.set_pixels_above_lines(8);
    tag.set_pixels_below_lines(4);
}

fn configure_heading_5(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.0);
    tag.set_pixels_above_lines(7);
    tag.set_pixels_below_lines(3);
}

fn configure_heading_6(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_scale(1.0);
    tag.set_pixels_above_lines(7);
    tag.set_pixels_below_lines(3);
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.42, 0.45, 0.48, 1.0)));
}

fn configure_strong(tag: &gtk4::TextTag) {
    tag.set_weight(700);
}

fn configure_emphasis(tag: &gtk4::TextTag) {
    tag.set_style(pango::Style::Italic);
}

fn configure_code(tag: &gtk4::TextTag) {
    tag.set_family(Some("monospace"));
    tag.set_background_rgba(Some(&gdk::RGBA::new(0.92, 0.94, 0.96, 1.0)));
    tag.set_wrap_mode(gtk4::WrapMode::WordChar);
}

fn configure_code_block(tag: &gtk4::TextTag) {
    tag.set_family(Some("monospace"));
    tag.set_left_margin(18);
    tag.set_right_margin(18);
    tag.set_pixels_above_lines(7);
    tag.set_pixels_below_lines(7);
    tag.set_paragraph_background_rgba(Some(&gdk::RGBA::new(0.94, 0.95, 0.97, 1.0)));
    tag.set_wrap_mode(gtk4::WrapMode::WordChar);
}

fn configure_quote(tag: &gtk4::TextTag) {
    tag.set_left_margin(24);
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.38, 0.42, 0.48, 1.0)));
}

fn configure_quote_marker(tag: &gtk4::TextTag) {
    tag.set_weight(700);
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.78, 0.81, 0.84, 1.0)));
}

fn configure_list_marker(tag: &gtk4::TextTag) {
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.34, 0.38, 0.44, 1.0)));
}

fn configure_link(tag: &gtk4::TextTag) {
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.10, 0.35, 0.78, 1.0)));
    tag.set_underline(pango::Underline::Single);
}

fn configure_metadata(tag: &gtk4::TextTag) {
    tag.set_style(pango::Style::Italic);
}

fn configure_diagnostic(tag: &gtk4::TextTag) {
    tag.set_left_margin(12);
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.38, 0.42, 0.48, 1.0)));
}

fn configure_rule(tag: &gtk4::TextTag) {
    tag.set_pixels_above_lines(6);
    tag.set_pixels_below_lines(6);
    tag.set_foreground_rgba(Some(&gdk::RGBA::new(0.74, 0.78, 0.82, 1.0)));
}
