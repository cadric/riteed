use gtk4::prelude::*;

use crate::markdown::parse_document;
use crate::markdown::render::{link_target_at, render_document, render_large_document_fallback};

pub(crate) fn exercise_markdown_renderer() {
    renders_commonmark_to_text_buffer();
    renders_heading_levels_with_distinct_tags();
    renders_block_markdown_as_preview_text();
    renders_paragraph_and_line_break_spacing();
    renders_compact_diagnostics();
    renders_thematic_break_as_tagged_separator();
    renders_indented_code_block_with_visual_padding();
    renders_large_document_fallback();
}

fn renders_commonmark_to_text_buffer() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document(
        "---\ntitle: Test\n---\n# Title\n\nText with *emphasis* and **strong**.\n\n![Alt](https://example.test/image.png)\n\n[Open](https://example.test)\n",
    );
    let output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("Markdown Frontmatter Metadata"));
    assert!(text.contains("Title"));
    assert!(text.contains("Markdown Image Placeholder: Alt"));
    assert!(!text.contains("---"));
    assert_eq!(
        link_target_at(
            &output.links,
            output.links.first().map_or(0, |link| link.start)
        ),
        Some(String::from("https://example.test"))
    );
}

fn renders_heading_levels_with_distinct_tags() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document("# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6\n");
    let _output = render_document(&buffer, &document);
    let table = buffer.tag_table();

    for level in 1..=6 {
        let tag_name = format!("markdown-heading-{level}");
        assert!(table.lookup(&tag_name).is_some());
        assert!(buffer_has_tag(&buffer, &tag_name));
    }

    let h5 = table.lookup("markdown-heading-5");
    let h6 = table.lookup("markdown-heading-6");
    if let (Some(h5), Some(h6)) = (h5, h6) {
        assert!((h5.scale() - h6.scale()).abs() < f64::EPSILON);
        assert!(h5.foreground_rgba().is_none());
        assert!(h6.foreground_rgba().is_some());
    }
}

fn renders_block_markdown_as_preview_text() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document(
        "- First unordered item\n- Second unordered item\n  - Nested unordered item\n\n1. First ordered item\n2. Second ordered item\n   1. Nested ordered item\n\nParagraph two has a soft\nline break.\n\nHard break with a backslash\\\ncontinues here.\n\n> Quote paragraph.\n>\n> - Quote list item\n\n```rust\nfn main() {}\n```\n\n---\n",
    );
    let _output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("• First unordered item"));
    assert!(text.contains("◦ Nested unordered item"));
    assert!(text.contains("1. First ordered item"));
    assert!(text.contains("1. Nested ordered item"));
    assert!(text.contains("Paragraph two has a soft line break."));
    assert!(text.contains("Hard break with a backslash\ncontinues here."));
    assert!(text.contains("▏ Quote paragraph."));
    assert!(text.contains("◦ Quote list item"));
    assert!(!text.contains("rust"));
    assert!(text.contains("  fn main() {}"));
    assert!(!text.contains("```"));
    assert!(!text.contains("-----"));
    assert!(!text.contains("                "));
}

fn renders_paragraph_and_line_break_spacing() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document(
        "First paragraph is split across\nsource lines.\n\nSecond paragraph follows a blank line.\n\n   \n\nThird paragraph follows a spaces-only blank line.\n\nSoft break stays\nin one paragraph.\n\nHard break via spaces:  \nNext hard-break line.\n\nHard break via backslash:\\\nNext backslash line.\n",
    );
    let _output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("First paragraph is split across source lines."));
    assert!(text.contains("source lines.\n\nSecond paragraph"));
    assert!(text.contains("blank line.\n\nThird paragraph"));
    assert!(text.contains("Soft break stays in one paragraph."));
    assert!(text.contains("Hard break via spaces:\nNext hard-break line."));
    assert!(text.contains("Hard break via backslash:\nNext backslash line."));
}

fn renders_compact_diagnostics() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document(
        "| a | b |\n|---|---|\n- [x] task\n~~gone~~\n\n![Alt](https://example.test/a.png)\n\n<span>raw</span>\n",
    );
    let _output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("unsupported extensions omitted"));
    assert!(text.contains("tables"));
    assert!(text.contains("task lists"));
    assert!(text.contains("strikethrough"));
    assert!(text.contains("images are shown as placeholders"));
    assert!(text.contains("raw HTML is shown as literal text"));
    assert!(!text.contains("Markdown Preview Diagnostics"));
    assert!(!text.contains("Markdown diagnostic information"));
}

fn renders_thematic_break_as_tagged_separator() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document("Before\n\n---\n\nAfter\n");
    let _output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("Before"));
    assert!(text.contains("After"));
    assert!(!text.contains("-----"));
    assert!(!text.contains("                "));
    assert!(text.contains("──"));

    let table = buffer.tag_table();
    let rule_tag = table.lookup("markdown-rule");
    assert!(rule_tag.is_some());
    if let Some(rule_tag) = rule_tag {
        assert!(rule_tag.paragraph_background_rgba().is_none());
    }
    assert!(buffer_has_tag(&buffer, "markdown-rule"));
}

fn renders_indented_code_block_with_visual_padding() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let document = parse_document("\tTabbed indentation creates an indented code block.\n");
    let _output = render_document(&buffer, &document);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();

    assert!(text.contains("  Tabbed indentation creates an indented code block."));
}

fn renders_large_document_fallback() {
    let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
    let _output = render_large_document_fallback(&buffer);
    let text = buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string();
    assert!(text.contains("Markdown Preview Disabled"));
}

fn buffer_has_tag(buffer: &gtk4::TextBuffer, tag_name: &str) -> bool {
    let mut iter = buffer.start_iter();
    loop {
        if iter
            .tags()
            .iter()
            .any(|tag| tag.name().as_deref() == Some(tag_name))
        {
            return true;
        }
        if !iter.forward_char() {
            return false;
        }
    }
}
