use std::ops::Range;

pub(crate) type SourceRange = Range<usize>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownDocument {
    pub(crate) frontmatter: Option<Frontmatter>,
    pub(crate) body: MarkdownBody,
    pub(crate) diagnostics: Vec<MarkdownDiagnostic>,
}

impl MarkdownDocument {
    #[cfg(debug_assertions)]
    pub(crate) fn debug_validate_source_ranges(&self) {
        if let Some(frontmatter) = self.frontmatter.as_ref() {
            validate_range(&frontmatter.source_range);
            debug_assert!(frontmatter.document_count == 0 || !frontmatter.raw.trim().is_empty());
        }
        for diagnostic in &self.diagnostics {
            if let Some(source_range) = diagnostic.source_range.as_ref() {
                validate_range(source_range);
            }
        }
        for block in &self.body.blocks {
            validate_block(block);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Frontmatter {
    pub(crate) raw: String,
    pub(crate) source_range: SourceRange,
    pub(crate) document_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownBody {
    pub(crate) blocks: Vec<MdBlock>,
    pub(crate) source_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MdBlock {
    Paragraph {
        inlines: Vec<MdInline>,
        source_range: SourceRange,
    },
    Heading {
        level: u8,
        inlines: Vec<MdInline>,
        source_range: SourceRange,
    },
    BlockQuote {
        blocks: Vec<MdBlock>,
        source_range: SourceRange,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        items: Vec<MdListItem>,
        source_range: SourceRange,
    },
    CodeBlock {
        language_hint: Option<String>,
        text: String,
        source_range: SourceRange,
    },
    ThematicBreak {
        source_range: SourceRange,
    },
    Html {
        raw: String,
        source_range: SourceRange,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MdListItem {
    pub(crate) blocks: Vec<MdBlock>,
    pub(crate) source_range: SourceRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MdInline {
    Text(String, SourceRange),
    Emphasis(Vec<MdInline>, SourceRange),
    Strong(Vec<MdInline>, SourceRange),
    Code(String, SourceRange),
    Link {
        target: String,
        title: String,
        children: Vec<MdInline>,
        source_range: SourceRange,
    },
    Image {
        target: String,
        title: String,
        alt: Vec<MdInline>,
        source_range: SourceRange,
    },
    Html(String, SourceRange),
    SoftBreak(SourceRange),
    HardBreak(SourceRange),
}

impl MdInline {
    #[must_use]
    pub(crate) fn source_range(&self) -> &SourceRange {
        match self {
            Self::Text(_, range)
            | Self::Emphasis(_, range)
            | Self::Strong(_, range)
            | Self::Code(_, range)
            | Self::Html(_, range)
            | Self::SoftBreak(range)
            | Self::HardBreak(range)
            | Self::Link {
                source_range: range,
                ..
            }
            | Self::Image {
                source_range: range,
                ..
            } => range,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownDiagnostic {
    pub(crate) kind: MarkdownDiagnosticKind,
    pub(crate) severity: DiagnosticSeverity,
    pub(crate) source_range: Option<SourceRange>,
}

impl MarkdownDiagnostic {
    #[must_use]
    pub(crate) fn new(
        kind: MarkdownDiagnosticKind,
        severity: DiagnosticSeverity,
        source_range: Option<SourceRange>,
    ) -> Self {
        Self {
            kind,
            severity,
            source_range,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticSeverity {
    Info,
    Warning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownDiagnosticKind {
    InvalidFrontmatter,
    UnclosedFrontmatter,
    UnsupportedSyntax(UnsupportedMarkdownFeature),
    RemoteImageBlocked,
    LocalImageUnavailable,
    FileUriImageBlocked,
    DataUriImageBlocked,
    RawHtmlLiteral,
    LargeDocumentPreviewDisabled,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum UnsupportedMarkdownFeature {
    Table,
    TaskList,
    Footnote,
    Strikethrough,
    Math,
    HeadingAttribute,
    WikiLink,
    DefinitionList,
    Subscript,
    Superscript,
    GfmAdmonition,
}

#[cfg(debug_assertions)]
fn validate_blocks(blocks: &[MdBlock]) {
    for block in blocks {
        validate_block(block);
    }
}

#[cfg(debug_assertions)]
fn validate_block(block: &MdBlock) {
    match block {
        MdBlock::Paragraph {
            inlines,
            source_range,
        }
        | MdBlock::Heading {
            inlines,
            source_range,
            ..
        } => {
            validate_range(source_range);
            validate_inlines(inlines);
        }
        MdBlock::BlockQuote {
            blocks,
            source_range,
        } => {
            validate_range(source_range);
            validate_blocks(blocks);
        }
        MdBlock::List {
            items,
            source_range,
            ..
        } => {
            validate_range(source_range);
            for item in items {
                validate_range(&item.source_range);
                validate_blocks(&item.blocks);
            }
        }
        MdBlock::CodeBlock { source_range, .. }
        | MdBlock::ThematicBreak { source_range }
        | MdBlock::Html { source_range, .. } => validate_range(source_range),
    }
}

#[cfg(debug_assertions)]
fn validate_inlines(inlines: &[MdInline]) {
    for inline in inlines {
        validate_inline(inline);
    }
}

#[cfg(debug_assertions)]
fn validate_inline(inline: &MdInline) {
    match inline {
        MdInline::Text(_, source_range)
        | MdInline::Code(_, source_range)
        | MdInline::Html(_, source_range)
        | MdInline::SoftBreak(source_range)
        | MdInline::HardBreak(source_range) => validate_range(source_range),
        MdInline::Emphasis(children, source_range) | MdInline::Strong(children, source_range) => {
            validate_range(source_range);
            validate_inlines(children);
        }
        MdInline::Link {
            title,
            children,
            source_range,
            ..
        } => {
            validate_range(source_range);
            debug_assert!(title.is_char_boundary(title.len()));
            validate_inlines(children);
        }
        MdInline::Image {
            title,
            alt,
            source_range,
            ..
        } => {
            validate_range(source_range);
            debug_assert!(title.is_char_boundary(title.len()));
            validate_inlines(alt);
        }
    }
}

#[cfg(debug_assertions)]
fn validate_range(range: &SourceRange) {
    debug_assert!(range.start <= range.end);
}
