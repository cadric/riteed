mod tags;

use gettextrs::pgettext;
use gtk4::prelude::*;
use std::collections::BTreeSet;

use self::tags::MarkdownTags;
use crate::markdown::model::{
    MarkdownDiagnostic, MarkdownDiagnosticKind, MarkdownDocument, MdBlock, MdInline, MdListItem,
    UnsupportedMarkdownFeature,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenderedLink {
    pub(crate) start: i32,
    pub(crate) end: i32,
    pub(crate) target: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RenderOutput {
    pub(crate) links: Vec<RenderedLink>,
}

#[must_use]
pub(crate) fn render_document(
    buffer: &gtk4::TextBuffer,
    document: &MarkdownDocument,
) -> RenderOutput {
    document.debug_validate_source_ranges();
    buffer.set_text("");
    let tags = MarkdownTags::new(buffer);
    let mut renderer = Renderer {
        buffer,
        tags,
        links: Vec::new(),
    };
    renderer.render_frontmatter(document);
    renderer.render_diagnostics(&document.diagnostics);
    for block in &document.body.blocks {
        renderer.render_block(block, 0);
    }
    RenderOutput {
        links: renderer.links,
    }
}

#[must_use]
pub(crate) fn render_large_document_fallback(buffer: &gtk4::TextBuffer) -> RenderOutput {
    buffer.set_text("");
    let tags = MarkdownTags::new(buffer);
    let mut renderer = Renderer {
        buffer,
        tags,
        links: Vec::new(),
    };
    let heading_tag = renderer.tags.heading.clone();
    renderer.insert_tagged_line(
        &pgettext("markdown preview", "Markdown Preview Disabled"),
        &[heading_tag],
    );
    let diagnostic_tag = renderer.tags.diagnostic.clone();
    if let Some(message) = diagnostic_message(MarkdownDiagnosticKind::LargeDocumentPreviewDisabled)
    {
        renderer.insert_tagged_line(&message, &[diagnostic_tag]);
    }
    RenderOutput::default()
}

pub(crate) fn link_target_at(links: &[RenderedLink], offset: i32) -> Option<String> {
    links
        .iter()
        .find(|link| offset >= link.start && offset <= link.end)
        .map(|link| link.target.clone())
}

struct Renderer<'a> {
    buffer: &'a gtk4::TextBuffer,
    tags: MarkdownTags,
    links: Vec<RenderedLink>,
}

impl Renderer<'_> {
    fn render_frontmatter(&mut self, document: &MarkdownDocument) {
        let Some(frontmatter) = document.frontmatter.as_ref() else {
            return;
        };
        let metadata_tag = self.tags.metadata.clone();
        self.insert_tagged_line(
            &pgettext("markdown preview", "Markdown Frontmatter Metadata"),
            &[metadata_tag],
        );
        if !frontmatter.raw.trim().is_empty() {
            let code_tag = self.tags.code.clone();
            self.insert_tagged_line(frontmatter.raw.trim_end(), &[code_tag]);
        }
        self.insert_blank_line();
    }

    fn render_diagnostics(&mut self, diagnostics: &[MarkdownDiagnostic]) {
        if diagnostics.is_empty() {
            return;
        }
        for line in diagnostic_lines(diagnostics) {
            let diagnostic_tag = self.tags.diagnostic.clone();
            self.insert_tagged_line(&line, &[diagnostic_tag]);
        }
        self.insert_blank_line();
    }

    fn render_block(&mut self, block: &MdBlock, depth: usize) {
        match block {
            MdBlock::Paragraph { inlines, .. } => {
                self.render_inlines(inlines);
                self.insert_blank_line();
                self.insert_blank_line();
            }
            MdBlock::Heading { level, inlines, .. } => {
                let start = self.current_offset();
                self.render_inlines(inlines);
                let end = self.current_offset();
                self.apply_tag(&self.tags.heading_for_level(*level), start, end);
                self.insert_blank_line();
            }
            MdBlock::BlockQuote { blocks, .. } => {
                let start = self.current_offset();
                for nested in blocks {
                    self.insert_indent(depth);
                    let quote_marker_tag = self.tags.quote_marker.clone();
                    self.insert_tagged("▏ ", &[quote_marker_tag]);
                    self.render_block(nested, depth + 1);
                }
                let end = self.current_offset();
                self.apply_tag(&self.tags.quote, start, end);
            }
            MdBlock::List {
                ordered,
                start,
                items,
                ..
            } => {
                self.render_list(*ordered, *start, items, depth);
                self.insert_blank_line();
            }
            MdBlock::CodeBlock { text, .. } => {
                let code_block_tag = self.tags.code_block.clone();
                self.insert_tagged_line(&padded_code_block_text(text), &[code_block_tag]);
                self.insert_blank_line();
            }
            MdBlock::ThematicBreak { .. } => {
                let rule_tag = self.tags.rule.clone();
                self.insert_tagged_line("────────────────────────────────", &[rule_tag]);
                self.insert_blank_line();
            }
            MdBlock::Html { raw, .. } => {
                let code_tag = self.tags.code.clone();
                self.insert_tagged_line(raw.trim_end(), &[code_tag]);
                self.insert_blank_line();
            }
        }
    }

    fn render_list(
        &mut self,
        ordered: bool,
        start: Option<u64>,
        items: &[MdListItem],
        depth: usize,
    ) {
        let first = start.unwrap_or(1);
        for (index, item) in items.iter().enumerate() {
            let marker = if ordered {
                let offset = u64::try_from(index).map_or(u64::MAX, |value| value);
                format!("{}.", first.saturating_add(offset))
            } else {
                unordered_marker(depth).to_string()
            };
            self.insert_indent(depth);
            let marker_tag = self.tags.list_marker.clone();
            self.insert_tagged(&marker, &[marker_tag]);
            self.insert_plain(" ");
            self.render_list_item(item, depth);
        }
    }

    fn render_list_item(&mut self, item: &MdListItem, depth: usize) {
        let mut first_block = true;
        for block in &item.blocks {
            if !first_block {
                self.insert_indent(depth + 1);
            }
            match block {
                MdBlock::Paragraph { inlines, .. } => {
                    self.render_inlines(inlines);
                    self.insert_plain("\n");
                }
                other => self.render_block(other, depth + 1),
            }
            first_block = false;
        }
    }

    fn render_inlines(&mut self, inlines: &[MdInline]) {
        for inline in inlines {
            self.render_inline(inline);
        }
    }

    fn render_inline(&mut self, inline: &MdInline) {
        match inline {
            MdInline::Text(text, _) => {
                self.insert_plain(text);
            }
            MdInline::Emphasis(children, _) => {
                let start = self.current_offset();
                self.render_inlines(children);
                let end = self.current_offset();
                self.apply_tag(&self.tags.emphasis, start, end);
            }
            MdInline::Strong(children, _) => {
                let start = self.current_offset();
                self.render_inlines(children);
                let end = self.current_offset();
                self.apply_tag(&self.tags.strong, start, end);
            }
            MdInline::Code(code, _) => {
                let code_tag = self.tags.code.clone();
                self.insert_tagged(code, &[code_tag]);
            }
            MdInline::Link {
                target, children, ..
            } => {
                let start = self.current_offset();
                self.render_inlines(children);
                let end = self.current_offset();
                self.apply_tag(&self.tags.link, start, end);
                if end > start {
                    self.links.push(RenderedLink {
                        start,
                        end,
                        target: target.clone(),
                    });
                }
            }
            MdInline::Image { target, alt, .. } => {
                self.render_image(target, alt);
            }
            MdInline::Html(html, _) => {
                let code_tag = self.tags.code.clone();
                self.insert_tagged(html, &[code_tag]);
            }
            MdInline::SoftBreak(_) => {
                self.insert_plain(" ");
            }
            MdInline::HardBreak(_) => {
                self.insert_plain("\n");
            }
        }
    }

    fn render_image(&mut self, target: &str, alt: &[MdInline]) {
        let alt_text = inline_plain_text(alt);
        let text = if alt_text.is_empty() {
            format!(
                "[{}: {target}]",
                pgettext("markdown preview", "Markdown Image Placeholder")
            )
        } else {
            format!(
                "[{}: {alt_text} - {target}]",
                pgettext("markdown preview", "Markdown Image Placeholder")
            )
        };
        let metadata_tag = self.tags.metadata.clone();
        self.insert_tagged(&text, &[metadata_tag]);
    }

    fn insert_indent(&mut self, depth: usize) {
        for _ in 0..depth {
            self.insert_plain("  ");
        }
    }

    fn insert_blank_line(&mut self) {
        self.insert_plain("\n");
    }

    fn insert_tagged_line(&mut self, text: &str, tags: &[gtk4::TextTag]) {
        self.insert_tagged(text, tags);
        self.insert_plain("\n");
    }

    fn insert_plain(&self, text: &str) {
        let mut iter = self.buffer.end_iter();
        self.buffer.insert(&mut iter, text);
    }

    fn insert_tagged(&self, text: &str, tags: &[gtk4::TextTag]) {
        let start = self.current_offset();
        self.insert_plain(text);
        let end = self.current_offset();
        for tag in tags {
            self.apply_tag(tag, start, end);
        }
    }

    fn current_offset(&self) -> i32 {
        self.buffer.end_iter().offset()
    }

    fn apply_tag(&self, tag: &gtk4::TextTag, start: i32, end: i32) {
        if end <= start {
            return;
        }
        let start_iter = self.buffer.iter_at_offset(start);
        let end_iter = self.buffer.iter_at_offset(end);
        self.buffer.apply_tag(tag, &start_iter, &end_iter);
    }
}

fn inline_plain_text(inlines: &[MdInline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            MdInline::Text(value, _) | MdInline::Code(value, _) | MdInline::Html(value, _) => {
                text.push_str(value);
            }
            MdInline::Emphasis(children, _) | MdInline::Strong(children, _) => {
                text.push_str(&inline_plain_text(children));
            }
            MdInline::Link { children, .. } => text.push_str(&inline_plain_text(children)),
            MdInline::Image { alt, .. } => text.push_str(&inline_plain_text(alt)),
            MdInline::SoftBreak(_) | MdInline::HardBreak(_) => text.push(' '),
        }
    }
    text
}

fn padded_code_block_text(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return String::from("  ");
    }
    trimmed
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn diagnostic_message(kind: MarkdownDiagnosticKind) -> Option<String> {
    match kind {
        MarkdownDiagnosticKind::InvalidFrontmatter => Some(pgettext(
            "markdown diagnostic",
            "The YAML frontmatter could not be parsed.",
        )),
        MarkdownDiagnosticKind::UnclosedFrontmatter => Some(pgettext(
            "markdown diagnostic",
            "The opening frontmatter delimiter has no closing delimiter.",
        )),
        MarkdownDiagnosticKind::UnsupportedSyntax(_) => None,
        MarkdownDiagnosticKind::RemoteImageBlocked => Some(pgettext(
            "markdown diagnostic",
            "A remote image was blocked and rendered as a placeholder.",
        )),
        MarkdownDiagnosticKind::LocalImageUnavailable => Some(pgettext(
            "markdown diagnostic",
            "A local image reference was not loaded automatically.",
        )),
        MarkdownDiagnosticKind::FileUriImageBlocked => Some(pgettext(
            "markdown diagnostic",
            "A file URI image reference was blocked.",
        )),
        MarkdownDiagnosticKind::DataUriImageBlocked => Some(pgettext(
            "markdown diagnostic",
            "A data URI image reference was blocked.",
        )),
        MarkdownDiagnosticKind::RawHtmlLiteral => Some(pgettext(
            "markdown diagnostic",
            "Raw HTML is shown as literal text.",
        )),
        MarkdownDiagnosticKind::LargeDocumentPreviewDisabled => Some(pgettext(
            "markdown diagnostic",
            "The document is too large for live Markdown preview.",
        )),
    }
}

fn diagnostic_lines(diagnostics: &[MarkdownDiagnostic]) -> Vec<String> {
    let mut unsupported = BTreeSet::new();
    let mut has_image_placeholder = false;
    let mut has_raw_html = false;
    let mut lines = Vec::new();

    for diagnostic in diagnostics {
        match diagnostic.kind {
            MarkdownDiagnosticKind::UnsupportedSyntax(feature) => {
                unsupported.insert(feature);
            }
            MarkdownDiagnosticKind::RemoteImageBlocked
            | MarkdownDiagnosticKind::LocalImageUnavailable
            | MarkdownDiagnosticKind::FileUriImageBlocked
            | MarkdownDiagnosticKind::DataUriImageBlocked => {
                has_image_placeholder = true;
            }
            MarkdownDiagnosticKind::RawHtmlLiteral => {
                has_raw_html = true;
            }
            other => {
                if let Some(message) = diagnostic_message(other) {
                    lines.push(message);
                }
            }
        }
    }

    if !unsupported.is_empty() {
        let features = unsupported
            .into_iter()
            .map(unsupported_feature_label)
            .collect::<Vec<_>>()
            .join(", ");
        lines.insert(
            0,
            pgettext(
                "markdown diagnostic",
                "Markdown preview: unsupported extensions omitted: {features}.",
            )
            .replace("{features}", &features),
        );
    }
    if has_image_placeholder {
        lines.push(pgettext(
            "markdown diagnostic",
            "Markdown preview: images are shown as placeholders.",
        ));
    }
    if has_raw_html {
        lines.push(pgettext(
            "markdown diagnostic",
            "Markdown preview: raw HTML is shown as literal text.",
        ));
    }

    lines
}

fn unsupported_feature_label(feature: UnsupportedMarkdownFeature) -> String {
    match feature {
        UnsupportedMarkdownFeature::Table => pgettext("markdown extension label", "tables"),
        UnsupportedMarkdownFeature::TaskList => pgettext("markdown extension label", "task lists"),
        UnsupportedMarkdownFeature::Footnote => pgettext("markdown extension label", "footnotes"),
        UnsupportedMarkdownFeature::Strikethrough => {
            pgettext("markdown extension label", "strikethrough")
        }
        UnsupportedMarkdownFeature::Math => pgettext("markdown extension label", "math"),
        UnsupportedMarkdownFeature::HeadingAttribute => {
            pgettext("markdown extension label", "heading attributes")
        }
        UnsupportedMarkdownFeature::WikiLink => pgettext("markdown extension label", "wiki links"),
        UnsupportedMarkdownFeature::DefinitionList => {
            pgettext("markdown extension label", "definition lists")
        }
        UnsupportedMarkdownFeature::Subscript => pgettext("markdown extension label", "subscript"),
        UnsupportedMarkdownFeature::Superscript => {
            pgettext("markdown extension label", "superscript")
        }
        UnsupportedMarkdownFeature::GfmAdmonition => {
            pgettext("markdown extension label", "GFM admonitions")
        }
    }
}

fn unordered_marker(depth: usize) -> &'static str {
    match depth {
        0 => "•",
        1 => "◦",
        _ => "▪",
    }
}
