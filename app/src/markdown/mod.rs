use std::path::Path;

mod frontmatter;
pub(crate) mod model;
mod parser;
#[cfg(test)]
mod parser_tests;
mod render;
#[cfg(test)]
pub(crate) mod render_tests;
mod unsupported;

pub(crate) use parser::parse_document;
pub(crate) use render::{
    RenderedLink, link_target_at, render_document, render_large_document_fallback,
};

#[must_use]
pub(crate) fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_markdown_path;

    #[test]
    fn detects_markdown_extensions_only() {
        assert!(is_markdown_path(Path::new("notes.md")));
        assert!(is_markdown_path(Path::new("notes.MARKDOWN")));
        assert!(!is_markdown_path(Path::new("notes.mdx")));
        assert!(!is_markdown_path(Path::new("notes.txt")));
    }

    #[test]
    fn flatpak_manifest_keeps_preview_sandboxed() {
        let manifest = include_str!("../../build-aux/io.github.cadric.Riteed.yml");
        let cargo = include_str!("../../Cargo.toml");
        assert!(!manifest.contains("--share=network"));
        assert!(!manifest.contains("--filesystem=host"));
        assert!(!manifest.contains("--filesystem=home"));
        assert!(!cargo.to_ascii_lowercase().contains("webkit"));
        assert!(!cargo.to_ascii_lowercase().contains("webview"));
        assert!(!manifest.to_ascii_lowercase().contains("webkit"));
        assert!(!manifest.to_ascii_lowercase().contains("webview"));
    }
}
