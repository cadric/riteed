use crate::markdown::model::{MarkdownDiagnosticKind, MdBlock, MdInline};
use crate::markdown::parse_document;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

fn bounded_proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::SourceParallel(
            ".proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

#[test]
fn parses_commonmark_blocks_and_inlines() {
    let document = parse_document(
        "# Title\n\nParagraph with *emphasis*, **strong**, [link](https://example.test), ![alt](image.png).\n\n- one\n- two\n\n> quote\n\n```rust\nlet value = 1;\n```\n\n---\n",
    );
    assert!(matches!(
        document.body.blocks.first(),
        Some(MdBlock::Heading { level: 1, .. })
    ));
    assert!(
        document
            .body
            .blocks
            .iter()
            .any(|block| matches!(block, MdBlock::List { .. }))
    );
    assert!(
        document
            .body
            .blocks
            .iter()
            .any(|block| matches!(block, MdBlock::BlockQuote { .. }))
    );
    assert!(document.body.blocks.iter().any(|block| {
        matches!(
            block,
            MdBlock::CodeBlock { language_hint: Some(language), .. } if language == "rust"
        )
    }));
    assert!(
        document
            .body
            .blocks
            .iter()
            .any(|block| matches!(block, MdBlock::ThematicBreak { .. }))
    );
}

#[test]
fn parses_tight_list_item_text() {
    let document = parse_document(
        "- First unordered item\n- Second unordered item\n  - Nested unordered item\n\n1. First ordered item\n2. Second ordered item\n   1. Nested ordered item\n",
    );
    for expected in [
        "First unordered item",
        "Second unordered item",
        "Nested unordered item",
        "First ordered item",
        "Second ordered item",
        "Nested ordered item",
    ] {
        assert!(
            document
                .body
                .blocks
                .iter()
                .any(|block| block_contains_text(block, expected))
        );
    }
}

fn block_contains_text(block: &MdBlock, expected: &str) -> bool {
    match block {
        MdBlock::Paragraph { inlines, .. } | MdBlock::Heading { inlines, .. } => inlines
            .iter()
            .any(|inline| inline_contains_text(inline, expected)),
        MdBlock::BlockQuote { blocks, .. } => blocks
            .iter()
            .any(|nested| block_contains_text(nested, expected)),
        MdBlock::List { items, .. } => items.iter().any(|item| {
            item.blocks
                .iter()
                .any(|nested| block_contains_text(nested, expected))
        }),
        MdBlock::CodeBlock { text, .. } | MdBlock::Html { raw: text, .. } => {
            text.contains(expected)
        }
        MdBlock::ThematicBreak { .. } => false,
    }
}

fn inline_contains_text(inline: &MdInline, expected: &str) -> bool {
    match inline {
        MdInline::Text(text, _) | MdInline::Code(text, _) | MdInline::Html(text, _) => {
            text.contains(expected)
        }
        MdInline::Emphasis(children, _) | MdInline::Strong(children, _) => children
            .iter()
            .any(|child| inline_contains_text(child, expected)),
        MdInline::Link { children, .. } => children
            .iter()
            .any(|child| inline_contains_text(child, expected)),
        MdInline::Image { alt, .. } => alt
            .iter()
            .any(|child| inline_contains_text(child, expected)),
        MdInline::SoftBreak(_) | MdInline::HardBreak(_) => false,
    }
}

proptest! {
    #![proptest_config(bounded_proptest_config())]

    #[test]
    fn proptest_parse_document_no_panic(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let input = String::from_utf8_lossy(&bytes);
        let document = parse_document(&input);

        prop_assert!(document.body.source_offset <= input.len());
        prop_assert!(document.diagnostics.len() <= input.len().saturating_add(16));
    }
}

#[test]
fn frontmatter_is_not_body() {
    let document = parse_document("---\ntitle: Test\n---\n# Body\n");
    assert!(document.frontmatter.is_some());
    assert_eq!(document.body.source_offset, 20);
    assert!(matches!(
        document.body.blocks.first(),
        Some(MdBlock::Heading { .. })
    ));
}

#[test]
fn raw_html_is_literal_and_diagnostic() {
    let document = parse_document("<script>alert(1)</script>\n\nText <span>literal</span>");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|item| matches!(item.kind, MarkdownDiagnosticKind::RawHtmlLiteral))
    );
    assert!(
        document
            .body
            .blocks
            .iter()
            .any(|block| matches!(block, MdBlock::Html { .. }))
    );
    assert!(document.body.blocks.iter().any(|block| matches!(
        block,
        MdBlock::Paragraph { inlines, .. }
            if inlines.iter().any(|inline| matches!(inline, MdInline::Html(_, _)))
    )));
}

#[test]
fn image_references_get_blocked_diagnostics() {
    let document = parse_document(
        "![remote](https://example.test/a.png)\n![file](file:///tmp/a.png)\n![data](data:image/png;base64,aa)\n![local](a.png)\n",
    );
    for kind in [
        MarkdownDiagnosticKind::RemoteImageBlocked,
        MarkdownDiagnosticKind::FileUriImageBlocked,
        MarkdownDiagnosticKind::DataUriImageBlocked,
        MarkdownDiagnosticKind::LocalImageUnavailable,
    ] {
        assert!(document.diagnostics.iter().any(|item| item.kind == kind));
    }
}

#[test]
fn extension_markers_are_not_enabled_as_ast_nodes() {
    let document = parse_document("| a | b |\n|---|---|\n- [x] task\n~~gone~~\n");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|item| matches!(item.kind, MarkdownDiagnosticKind::UnsupportedSyntax(_)))
    );
    assert!(!document.body.blocks.iter().any(|block| {
        matches!(block, MdBlock::List { items, .. } if items.iter().any(|item| {
            item.blocks.iter().any(|block| matches!(
                block,
                MdBlock::Paragraph { inlines, .. }
                    if inlines.iter().any(|inline| matches!(inline, MdInline::Image { .. }))
            ))
        }))
    }));
}
