use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::markdown::frontmatter;
use crate::markdown::model::{
    DiagnosticSeverity, MarkdownBody, MarkdownDiagnostic, MarkdownDiagnosticKind, MarkdownDocument,
    MdBlock, MdInline, MdListItem, SourceRange,
};
use crate::markdown::normalize::parser_input;

#[must_use]
// PARSER-BOUNDARY: id=markdown_parse
pub(crate) fn parse_document(input: &str) -> MarkdownDocument {
    let split = frontmatter::split(input);
    let mut diagnostics = split.diagnostics;
    diagnostics.extend(crate::markdown::unsupported::diagnostics_for(
        split.body,
        split.body_offset,
    ));
    let mut body = parse_body(split.body, split.body_offset, &mut diagnostics);
    body.source_offset = split.body_offset;
    MarkdownDocument {
        frontmatter: split.frontmatter,
        body,
        diagnostics,
    }
}

fn parse_body(
    body: &str,
    body_offset: usize,
    diagnostics: &mut Vec<MarkdownDiagnostic>,
) -> MarkdownBody {
    let parser_body = parser_input(body);
    let parser = Parser::new_ext(&parser_body, Options::empty()).into_offset_iter();
    let mut frames = vec![Frame::Document { blocks: Vec::new() }];
    for (event, range) in parser {
        let source_range = absolute_range(range, body_offset);
        match event {
            Event::Start(tag) => start_tag(&mut frames, tag, source_range),
            Event::End(tag) => end_tag(&mut frames, tag, source_range, diagnostics),
            Event::Text(text) => append_text(&mut frames, text.as_ref(), source_range),
            Event::Code(code) => {
                append_inline(&mut frames, MdInline::Code(code.to_string(), source_range));
            }
            Event::Html(html) => append_html(&mut frames, html.as_ref(), source_range, diagnostics),
            Event::InlineHtml(html) => {
                append_inline_html(&mut frames, html.as_ref(), source_range, diagnostics);
            }
            Event::SoftBreak => append_inline(&mut frames, MdInline::SoftBreak(source_range)),
            Event::HardBreak => append_inline(&mut frames, MdInline::HardBreak(source_range)),
            Event::Rule => append_block(&mut frames, MdBlock::ThematicBreak { source_range }),
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                append_text(&mut frames, math.as_ref(), source_range.clone());
                diagnostics.push(MarkdownDiagnostic::new(
                    MarkdownDiagnosticKind::UnsupportedSyntax(
                        crate::markdown::model::UnsupportedMarkdownFeature::Math,
                    ),
                    DiagnosticSeverity::Info,
                    Some(source_range),
                ));
            }
            Event::FootnoteReference(label) => {
                append_text(&mut frames, label.as_ref(), source_range);
            }
            Event::TaskListMarker(done) => {
                let marker = if done { "[x] " } else { "[ ] " };
                append_text(&mut frames, marker, source_range);
            }
        }
    }
    unwind_frames(&mut frames);
    MarkdownBody {
        blocks: document_blocks(frames),
        source_offset: body_offset,
    }
}

fn start_tag(frames: &mut Vec<Frame>, tag: Tag<'_>, source_range: SourceRange) {
    match tag {
        Tag::Paragraph => frames.push(Frame::Paragraph {
            inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::Heading { level, .. } => frames.push(Frame::Heading {
            level: heading_level(level),
            inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::BlockQuote(_) => frames.push(Frame::BlockQuote {
            blocks: Vec::new(),
            start: source_range.start,
        }),
        Tag::List(start) => frames.push(Frame::List {
            ordered: start.is_some(),
            start_number: start,
            items: Vec::new(),
            start: source_range.start,
        }),
        Tag::Item => frames.push(Frame::Item {
            blocks: Vec::new(),
            pending_inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::CodeBlock(kind) => frames.push(Frame::CodeBlock {
            language_hint: code_language(kind),
            text: String::new(),
            start: source_range.start,
        }),
        Tag::HtmlBlock => frames.push(Frame::HtmlBlock {
            raw: String::new(),
            start: source_range.start,
        }),
        Tag::Emphasis => frames.push(Frame::Emphasis {
            inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::Strong => frames.push(Frame::Strong {
            inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::Link {
            dest_url, title, ..
        } => frames.push(Frame::Link {
            target: dest_url.to_string(),
            title: title.to_string(),
            inlines: Vec::new(),
            start: source_range.start,
        }),
        Tag::Image {
            dest_url, title, ..
        } => frames.push(Frame::Image {
            target: dest_url.to_string(),
            title: title.to_string(),
            inlines: Vec::new(),
            start: source_range.start,
        }),
        _ => {}
    }
}

fn end_tag(
    frames: &mut Vec<Frame>,
    tag: TagEnd,
    source_range: SourceRange,
    diagnostics: &mut Vec<MarkdownDiagnostic>,
) {
    match tag {
        TagEnd::Paragraph => close_paragraph(frames, source_range.end),
        TagEnd::Heading(_) => close_heading(frames, source_range.end),
        TagEnd::BlockQuote(_) => close_block_quote(frames, source_range.end),
        TagEnd::List(_) => close_list(frames, source_range.end),
        TagEnd::Item => close_item(frames, source_range.end),
        TagEnd::CodeBlock => close_code_block(frames, source_range.end),
        TagEnd::HtmlBlock => close_html_block(frames, source_range.end),
        TagEnd::Emphasis => close_emphasis(frames, source_range.end),
        TagEnd::Strong => close_strong(frames, source_range.end),
        TagEnd::Link => close_link(frames, source_range.end),
        TagEnd::Image => close_image(frames, source_range.end, diagnostics),
        _ => {}
    }
}

fn append_text(frames: &mut [Frame], text: &str, source_range: SourceRange) {
    if let Some(Frame::CodeBlock { text: code, .. } | Frame::HtmlBlock { raw: code, .. }) =
        frames.last_mut()
    {
        code.push_str(text);
        return;
    }
    append_inline(frames, MdInline::Text(text.to_string(), source_range));
}

fn append_html(
    frames: &mut [Frame],
    html: &str,
    source_range: SourceRange,
    diagnostics: &mut Vec<MarkdownDiagnostic>,
) {
    diagnostics.push(MarkdownDiagnostic::new(
        MarkdownDiagnosticKind::RawHtmlLiteral,
        DiagnosticSeverity::Info,
        Some(source_range.clone()),
    ));
    if let Some(Frame::HtmlBlock { raw, .. }) = frames.last_mut() {
        raw.push_str(html);
    } else {
        append_block(
            frames,
            MdBlock::Html {
                raw: html.to_string(),
                source_range,
            },
        );
    }
}

fn append_inline_html(
    frames: &mut [Frame],
    html: &str,
    source_range: SourceRange,
    diagnostics: &mut Vec<MarkdownDiagnostic>,
) {
    diagnostics.push(MarkdownDiagnostic::new(
        MarkdownDiagnosticKind::RawHtmlLiteral,
        DiagnosticSeverity::Info,
        Some(source_range.clone()),
    ));
    append_inline(frames, MdInline::Html(html.to_string(), source_range));
}

fn append_inline(frames: &mut [Frame], inline: MdInline) {
    for frame in frames.iter_mut().rev() {
        match frame {
            Frame::Paragraph { inlines, .. }
            | Frame::Heading { inlines, .. }
            | Frame::Emphasis { inlines, .. }
            | Frame::Strong { inlines, .. }
            | Frame::Link { inlines, .. }
            | Frame::Image { inlines, .. } => {
                inlines.push(inline);
                return;
            }
            Frame::Item {
                pending_inlines, ..
            } => {
                pending_inlines.push(inline);
                return;
            }
            _ => {}
        }
    }
}

fn append_block(frames: &mut [Frame], block: MdBlock) {
    for frame in frames.iter_mut().rev() {
        match frame {
            Frame::Document { blocks } | Frame::BlockQuote { blocks, .. } => {
                blocks.push(block);
                return;
            }
            Frame::Item {
                blocks,
                pending_inlines,
                ..
            } => {
                if let Some(paragraph) = take_pending_paragraph(pending_inlines) {
                    blocks.push(paragraph);
                }
                blocks.push(block);
                return;
            }
            _ => {}
        }
    }
}

fn close_paragraph(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Paragraph { inlines, start }) = frames.pop() {
        append_block(
            frames,
            MdBlock::Paragraph {
                inlines,
                source_range: start..end,
            },
        );
    }
}

fn close_heading(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Heading {
        level,
        inlines,
        start,
    }) = frames.pop()
    {
        append_block(
            frames,
            MdBlock::Heading {
                level,
                inlines,
                source_range: start..end,
            },
        );
    }
}

fn close_block_quote(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::BlockQuote { blocks, start }) = frames.pop() {
        append_block(
            frames,
            MdBlock::BlockQuote {
                blocks,
                source_range: start..end,
            },
        );
    }
}

fn close_list(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::List {
        ordered,
        start_number,
        items,
        start,
    }) = frames.pop()
    {
        append_block(
            frames,
            MdBlock::List {
                ordered,
                start: start_number,
                items,
                source_range: start..end,
            },
        );
    }
}

fn close_item(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Item {
        mut blocks,
        mut pending_inlines,
        start,
    }) = frames.pop()
        && let Some(Frame::List { items, .. }) = frames.last_mut()
    {
        if let Some(paragraph) = take_pending_paragraph(&mut pending_inlines) {
            blocks.push(paragraph);
        }
        items.push(MdListItem {
            blocks,
            source_range: start..end,
        });
    }
}

fn take_pending_paragraph(inlines: &mut Vec<MdInline>) -> Option<MdBlock> {
    if inlines.is_empty() {
        return None;
    }
    let start = inlines
        .first()
        .map_or(0, |inline| inline.source_range().start);
    let end = inlines
        .last()
        .map_or(start, |inline| inline.source_range().end);
    Some(MdBlock::Paragraph {
        inlines: std::mem::take(inlines),
        source_range: start..end,
    })
}

fn close_code_block(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::CodeBlock {
        language_hint,
        text,
        start,
    }) = frames.pop()
    {
        append_block(
            frames,
            MdBlock::CodeBlock {
                language_hint,
                text,
                source_range: start..end,
            },
        );
    }
}

fn close_html_block(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::HtmlBlock { raw, start }) = frames.pop() {
        append_block(
            frames,
            MdBlock::Html {
                raw,
                source_range: start..end,
            },
        );
    }
}

fn close_emphasis(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Emphasis { inlines, start }) = frames.pop() {
        append_inline(frames, MdInline::Emphasis(inlines, start..end));
    }
}

fn close_strong(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Strong { inlines, start }) = frames.pop() {
        append_inline(frames, MdInline::Strong(inlines, start..end));
    }
}

fn close_link(frames: &mut Vec<Frame>, end: usize) {
    if let Some(Frame::Link {
        target,
        title,
        inlines,
        start,
    }) = frames.pop()
    {
        append_inline(
            frames,
            MdInline::Link {
                target,
                title,
                children: inlines,
                source_range: start..end,
            },
        );
    }
}

fn close_image(frames: &mut Vec<Frame>, end: usize, diagnostics: &mut Vec<MarkdownDiagnostic>) {
    if let Some(Frame::Image {
        target,
        title,
        inlines,
        start,
    }) = frames.pop()
    {
        diagnostics.push(MarkdownDiagnostic::new(
            image_diagnostic_kind(&target),
            DiagnosticSeverity::Info,
            Some(start..end),
        ));
        append_inline(
            frames,
            MdInline::Image {
                target,
                title,
                alt: inlines,
                source_range: start..end,
            },
        );
    }
}

fn image_diagnostic_kind(target: &str) -> MarkdownDiagnosticKind {
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        MarkdownDiagnosticKind::RemoteImageBlocked
    } else if lower.starts_with("file://") {
        MarkdownDiagnosticKind::FileUriImageBlocked
    } else if lower.starts_with("data:") {
        MarkdownDiagnosticKind::DataUriImageBlocked
    } else {
        MarkdownDiagnosticKind::LocalImageUnavailable
    }
}

fn unwind_frames(frames: &mut Vec<Frame>) {
    while frames.len() > 1 {
        let end = frame_start(frames.last());
        match frames.last() {
            Some(Frame::Paragraph { .. }) => close_paragraph(frames, end),
            Some(Frame::Heading { .. }) => close_heading(frames, end),
            Some(Frame::BlockQuote { .. }) => close_block_quote(frames, end),
            Some(Frame::List { .. }) => close_list(frames, end),
            Some(Frame::Item { .. }) => close_item(frames, end),
            Some(Frame::CodeBlock { .. }) => close_code_block(frames, end),
            Some(Frame::HtmlBlock { .. }) => close_html_block(frames, end),
            Some(Frame::Emphasis { .. }) => close_emphasis(frames, end),
            Some(Frame::Strong { .. }) => close_strong(frames, end),
            Some(Frame::Link { .. }) => close_link(frames, end),
            Some(Frame::Image { .. }) => {
                let mut diagnostics = Vec::new();
                close_image(frames, end, &mut diagnostics);
            }
            Some(Frame::Document { .. }) | None => break,
        }
    }
}

fn document_blocks(mut frames: Vec<Frame>) -> Vec<MdBlock> {
    match frames.pop() {
        Some(Frame::Document { blocks }) => blocks,
        _ => Vec::new(),
    }
}

fn frame_start(frame: Option<&Frame>) -> usize {
    match frame {
        Some(
            Frame::Paragraph { start, .. }
            | Frame::Heading { start, .. }
            | Frame::BlockQuote { start, .. }
            | Frame::List { start, .. }
            | Frame::Item { start, .. }
            | Frame::CodeBlock { start, .. }
            | Frame::HtmlBlock { start, .. }
            | Frame::Emphasis { start, .. }
            | Frame::Strong { start, .. }
            | Frame::Link { start, .. }
            | Frame::Image { start, .. },
        ) => *start,
        Some(Frame::Document { .. }) | None => 0,
    }
}

fn absolute_range(range: std::ops::Range<usize>, offset: usize) -> SourceRange {
    offset + range.start..offset + range.end
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn code_language(kind: CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Indented => None,
        CodeBlockKind::Fenced(info) => {
            let first = info.split_whitespace().next();
            first.filter(|item| !item.is_empty()).map(ToOwned::to_owned)
        }
    }
}

enum Frame {
    Document {
        blocks: Vec<MdBlock>,
    },
    Paragraph {
        inlines: Vec<MdInline>,
        start: usize,
    },
    Heading {
        level: u8,
        inlines: Vec<MdInline>,
        start: usize,
    },
    BlockQuote {
        blocks: Vec<MdBlock>,
        start: usize,
    },
    List {
        ordered: bool,
        start_number: Option<u64>,
        items: Vec<MdListItem>,
        start: usize,
    },
    Item {
        blocks: Vec<MdBlock>,
        pending_inlines: Vec<MdInline>,
        start: usize,
    },
    CodeBlock {
        language_hint: Option<String>,
        text: String,
        start: usize,
    },
    HtmlBlock {
        raw: String,
        start: usize,
    },
    Emphasis {
        inlines: Vec<MdInline>,
        start: usize,
    },
    Strong {
        inlines: Vec<MdInline>,
        start: usize,
    },
    Link {
        target: String,
        title: String,
        inlines: Vec<MdInline>,
        start: usize,
    },
    Image {
        target: String,
        title: String,
        inlines: Vec<MdInline>,
        start: usize,
    },
}
