use crate::markdown::model::{
    DiagnosticSeverity, Frontmatter, MarkdownDiagnostic, MarkdownDiagnosticKind,
};

const UTF8_BOM: &str = "\u{feff}";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrontmatterSplit<'a> {
    pub(crate) frontmatter: Option<Frontmatter>,
    pub(crate) body: &'a str,
    pub(crate) body_offset: usize,
    pub(crate) diagnostics: Vec<MarkdownDiagnostic>,
}

#[must_use]
pub(crate) fn split(input: &str) -> FrontmatterSplit<'_> {
    let bom_len = if input.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else {
        0
    };
    let (first_line, first_end, first_next) = line_at(input, bom_len);
    if !delimiter_matches(first_line, "---") {
        return FrontmatterSplit {
            frontmatter: None,
            body: &input[bom_len..],
            body_offset: bom_len,
            diagnostics: Vec::new(),
        };
    }

    let mut cursor = first_next;
    while cursor < input.len() {
        let (line, _line_end, next) = line_at(input, cursor);
        if delimiter_matches(line, "---") || delimiter_matches(line, "...") {
            let raw = input[first_next..cursor].to_string();
            let mut diagnostics = Vec::new();
            let document_count = match yaml_rust2::YamlLoader::load_from_str(&raw) {
                Ok(documents) => documents.len(),
                Err(_error) => {
                    diagnostics.push(MarkdownDiagnostic::new(
                        MarkdownDiagnosticKind::InvalidFrontmatter,
                        DiagnosticSeverity::Warning,
                        Some(bom_len..next),
                    ));
                    0
                }
            };
            return FrontmatterSplit {
                frontmatter: Some(Frontmatter {
                    raw,
                    source_range: bom_len..next,
                    document_count,
                }),
                body: &input[next..],
                body_offset: next,
                diagnostics,
            };
        }
        if next == cursor {
            break;
        }
        cursor = next;
    }

    FrontmatterSplit {
        frontmatter: None,
        body: input,
        body_offset: 0,
        diagnostics: vec![MarkdownDiagnostic::new(
            MarkdownDiagnosticKind::UnclosedFrontmatter,
            DiagnosticSeverity::Warning,
            Some(bom_len..first_end),
        )],
    }
}

fn line_at(input: &str, start: usize) -> (&str, usize, usize) {
    let remaining = &input[start..];
    if let Some(relative_newline) = remaining.find('\n') {
        let line_end = start + relative_newline;
        let next = line_end + '\n'.len_utf8();
        (&input[start..line_end], line_end, next)
    } else {
        (remaining, input.len(), input.len())
    }
}

fn delimiter_matches(line: &str, delimiter: &str) -> bool {
    line.strip_suffix('\r').unwrap_or(line) == delimiter
}

#[cfg(test)]
mod tests {
    use super::split;
    use crate::markdown::model::MarkdownDiagnosticKind;
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
    fn frontmatter_is_split_from_body() {
        let split = split("---\ntitle: Test\n---\n# Heading\n");
        let frontmatter = split.frontmatter.as_ref();
        assert!(frontmatter.is_some());
        assert_eq!(
            frontmatter.map(|item| item.raw.as_str()),
            Some("title: Test\n")
        );
        assert_eq!(split.body, "# Heading\n");
        assert_eq!(split.body_offset, 20);
    }

    #[test]
    fn invalid_frontmatter_keeps_body_split() {
        let split = split("---\ninvalid: [\n---\nBody");
        assert!(split.frontmatter.is_some());
        assert_eq!(split.body, "Body");
        assert!(
            split
                .diagnostics
                .iter()
                .any(|item| { matches!(item.kind, MarkdownDiagnosticKind::InvalidFrontmatter) })
        );
    }

    #[test]
    fn unclosed_frontmatter_candidate_remains_body() {
        let split = split("---\ntitle: Test\n# Heading\n");
        assert!(split.frontmatter.is_none());
        assert_eq!(split.body, "---\ntitle: Test\n# Heading\n");
        assert!(
            split
                .diagnostics
                .iter()
                .any(|item| { matches!(item.kind, MarkdownDiagnosticKind::UnclosedFrontmatter) })
        );
    }

    proptest! {
        #![proptest_config(bounded_proptest_config())]

        #[test]
        fn proptest_split_terminates(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
            let input = String::from_utf8_lossy(&bytes);
            let split = split(&input);

            prop_assert!(split.body_offset <= input.len());
            prop_assert!(split.body.len() <= input.len());
            prop_assert!(split.diagnostics.len() <= 1);
        }
    }
}
