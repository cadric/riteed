use std::collections::BTreeSet;

use crate::markdown::model::{
    DiagnosticSeverity, MarkdownDiagnostic, MarkdownDiagnosticKind, UnsupportedMarkdownFeature,
};

#[must_use]
pub(crate) fn diagnostics_for(body: &str, body_offset: usize) -> Vec<MarkdownDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();
    let lines = indexed_lines(body, body_offset);
    let mut fence = None;
    let mut in_indented_code = false;

    for (index, line) in lines.iter().enumerate() {
        if let Some(open_fence) = fence {
            if is_closing_fence(line.text, open_fence) {
                fence = None;
            }
            continue;
        }

        if let Some(open_fence) = opening_fence(line.text) {
            fence = Some(open_fence);
            in_indented_code = false;
            continue;
        }

        if in_indented_code {
            if line.text.trim().is_empty() || is_indented_code_line(line.text) {
                continue;
            }
            in_indented_code = false;
        }

        if is_indented_code_line(line.text) {
            in_indented_code = true;
            continue;
        }

        let trimmed = line.text.trim_start();
        if is_task_list(trimmed) {
            push_once(
                &mut diagnostics,
                &mut seen,
                UnsupportedMarkdownFeature::TaskList,
                line.range(),
            );
        }
        if trimmed.starts_with("> [!") {
            push_once(
                &mut diagnostics,
                &mut seen,
                UnsupportedMarkdownFeature::GfmAdmonition,
                line.range(),
            );
        }
        if is_heading_attribute(trimmed) {
            push_once(
                &mut diagnostics,
                &mut seen,
                UnsupportedMarkdownFeature::HeadingAttribute,
                line.range(),
            );
        }
        if is_definition_list_marker(trimmed) {
            push_once(
                &mut diagnostics,
                &mut seen,
                UnsupportedMarkdownFeature::DefinitionList,
                line.range(),
            );
        }
        scan_inline_markers(line, &mut diagnostics, &mut seen);
        if index + 1 < lines.len()
            && line.text.contains('|')
            && !is_indented_code_line(lines[index + 1].text)
            && is_table_separator(lines[index + 1].text)
        {
            push_once(
                &mut diagnostics,
                &mut seen,
                UnsupportedMarkdownFeature::Table,
                line.range(),
            );
        }
    }

    diagnostics
}

fn push_once(
    diagnostics: &mut Vec<MarkdownDiagnostic>,
    seen: &mut BTreeSet<UnsupportedMarkdownFeature>,
    feature: UnsupportedMarkdownFeature,
    source_range: std::ops::Range<usize>,
) {
    if seen.insert(feature) {
        diagnostics.push(MarkdownDiagnostic::new(
            MarkdownDiagnosticKind::UnsupportedSyntax(feature),
            DiagnosticSeverity::Info,
            Some(source_range),
        ));
    }
}

fn scan_inline_markers(
    line: &IndexedLine<'_>,
    diagnostics: &mut Vec<MarkdownDiagnostic>,
    seen: &mut BTreeSet<UnsupportedMarkdownFeature>,
) {
    if line.text.contains("[^") {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::Footnote,
            line.range(),
        );
    }
    if line.text.contains("~~") {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::Strikethrough,
            line.range(),
        );
    }
    if line.text.contains("$$") {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::Math,
            line.range(),
        );
    }
    if line.text.contains("[[") && line.text.contains("]]") {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::WikiLink,
            line.range(),
        );
    }
    if contains_wrapped_marker(line.text, '~') {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::Subscript,
            line.range(),
        );
    }
    if contains_wrapped_marker(line.text, '^') {
        push_once(
            diagnostics,
            seen,
            UnsupportedMarkdownFeature::Superscript,
            line.range(),
        );
    }
}

#[derive(Clone, Copy)]
struct FenceState {
    marker: char,
    length: usize,
}

fn opening_fence(line: &str) -> Option<FenceState> {
    let (spaces, rest) = trim_up_to_three_leading_spaces(line)?;
    if spaces > 3 {
        return None;
    }
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = marker_run(rest, marker);
    if length < 3 {
        return None;
    }
    Some(FenceState { marker, length })
}

fn is_closing_fence(line: &str, fence: FenceState) -> bool {
    let Some((_spaces, rest)) = trim_up_to_three_leading_spaces(line) else {
        return false;
    };
    if !rest.starts_with(fence.marker) {
        return false;
    }
    let length = marker_run(rest, fence.marker);
    length >= fence.length && rest[length..].trim().is_empty()
}

fn trim_up_to_three_leading_spaces(line: &str) -> Option<(usize, &str)> {
    let spaces = line.chars().take_while(|item| *item == ' ').count();
    if spaces > 3 {
        return None;
    }
    Some((spaces, &line[spaces..]))
}

fn marker_run(line: &str, marker: char) -> usize {
    line.chars().take_while(|item| *item == marker).count()
}

fn is_indented_code_line(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

fn is_task_list(trimmed: &str) -> bool {
    let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    else {
        return false;
    };
    rest.starts_with("[ ] ") || rest.starts_with("[x] ") || rest.starts_with("[X] ")
}

fn is_heading_attribute(trimmed: &str) -> bool {
    trimmed.starts_with('#') && trimmed.contains("{#") && trimmed.ends_with('}')
}

fn is_definition_list_marker(trimmed: &str) -> bool {
    trimmed.starts_with(": ") || trimmed.starts_with(":\t")
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|')
        && trimmed
            .chars()
            .all(|item| matches!(item, '|' | '-' | ':' | ' ' | '\t'))
        && trimmed.chars().filter(|item| *item == '-').count() >= 3
}

fn contains_wrapped_marker(line: &str, marker: char) -> bool {
    let mut open = false;
    for part in line.split_whitespace() {
        let count = part.chars().filter(|item| *item == marker).count();
        if count >= 2 && !part.contains("~~") {
            open = true;
        }
    }
    open
}

#[derive(Clone, Copy)]
struct IndexedLine<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl IndexedLine<'_> {
    fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

fn indexed_lines(body: &str, offset: usize) -> Vec<IndexedLine<'_>> {
    let mut result = Vec::new();
    let mut start = 0;
    for line in body.split_inclusive('\n') {
        let line_end = start + line.len();
        let text_end = line.strip_suffix('\n').map_or(line.len(), str::len);
        result.push(IndexedLine {
            text: &line[..text_end],
            start: offset + start,
            end: offset + line_end,
        });
        start = line_end;
    }
    if start < body.len() {
        result.push(IndexedLine {
            text: &body[start..],
            start: offset + start,
            end: offset + body.len(),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::diagnostics_for;
    use crate::markdown::model::{MarkdownDiagnosticKind, UnsupportedMarkdownFeature};

    #[test]
    fn reports_disabled_extension_markers() {
        let diagnostics = diagnostics_for(
            "| a | b |\n|---|---|\n- [x] task\n~~gone~~\n[^note]\n$$x$$\n# H {#id}\n[[Page]]\n: term\n~sub~\n^sup^\n> [!NOTE]\n",
            0,
        );
        for feature in [
            UnsupportedMarkdownFeature::Table,
            UnsupportedMarkdownFeature::TaskList,
            UnsupportedMarkdownFeature::Strikethrough,
            UnsupportedMarkdownFeature::Footnote,
            UnsupportedMarkdownFeature::Math,
            UnsupportedMarkdownFeature::HeadingAttribute,
            UnsupportedMarkdownFeature::WikiLink,
            UnsupportedMarkdownFeature::DefinitionList,
            UnsupportedMarkdownFeature::Subscript,
            UnsupportedMarkdownFeature::Superscript,
            UnsupportedMarkdownFeature::GfmAdmonition,
        ] {
            assert!(diagnostics.iter().any(|item| {
                matches!(
                    item.kind,
                    MarkdownDiagnosticKind::UnsupportedSyntax(candidate) if candidate == feature
                )
            }));
        }
    }

    #[test]
    fn skips_unsupported_markers_inside_fenced_code_blocks() {
        let diagnostics = diagnostics_for(
            "```rust\nlet table = [[1, 2], [3, 4]];\n| a | b |\n|---|---|\n$$x$$\n```\n\n| a | b |\n|---|---|\n",
            0,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(has_feature(&diagnostics, UnsupportedMarkdownFeature::Table));
        assert!(!has_feature(
            &diagnostics,
            UnsupportedMarkdownFeature::WikiLink
        ));
        assert!(!has_feature(&diagnostics, UnsupportedMarkdownFeature::Math));
    }

    #[test]
    fn skips_unsupported_markers_inside_indented_code_blocks() {
        let diagnostics = diagnostics_for(
            "    | a | b |\n    |---|---|\n\n\t~~literal~~\n\n~~real~~\n",
            0,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(has_feature(
            &diagnostics,
            UnsupportedMarkdownFeature::Strikethrough
        ));
        assert!(!has_feature(
            &diagnostics,
            UnsupportedMarkdownFeature::Table
        ));
    }

    #[test]
    fn honors_commonmark_fence_close_rules_for_diagnostic_scanning() {
        let diagnostics = diagnostics_for(
            "````\n~~inside~~\n```\n~~still inside~~\n```` \n~~outside~~\n~~~\n[[inside tilde]]\n```\n[[still inside tilde]]\n~~bad close text\n~~~ nope\n~~~\n[[outside]]\n",
            0,
        );

        assert!(has_feature(
            &diagnostics,
            UnsupportedMarkdownFeature::Strikethrough
        ));
        assert!(has_feature(
            &diagnostics,
            UnsupportedMarkdownFeature::WikiLink
        ));
        assert_eq!(diagnostics.len(), 2);
    }

    fn has_feature(
        diagnostics: &[crate::markdown::model::MarkdownDiagnostic],
        feature: UnsupportedMarkdownFeature,
    ) -> bool {
        diagnostics.iter().any(|item| {
            matches!(
                item.kind,
                MarkdownDiagnosticKind::UnsupportedSyntax(candidate) if candidate == feature
            )
        })
    }
}
