use std::time::{Duration, Instant};

use similar::{Algorithm, ChangeTag, DiffTag, TextDiff, capture_diff_slices_deadline};

use super::model::DiffSide;

const GRAPHEME_LIMIT: usize = 2_000;
const WORD_LIMIT: usize = 10_000;
const GRAPHEME_TIMEOUT_MS: u64 = 2;
const WORD_TIMEOUT_MS: u64 = 1;
const TOKEN_TIMEOUT_MS: u64 = 4;
const TOTAL_INLINE_BUDGET_MS: u64 = 12;
const MIN_INLINE_BUDGET_US: u64 = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct InlineRange {
    pub(super) side: DiffSide,
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug)]
pub(super) struct InlineBudget {
    deadline: Instant,
}

impl InlineBudget {
    pub(super) fn new() -> Self {
        Self {
            deadline: Instant::now() + Duration::from_millis(TOTAL_INLINE_BUDGET_MS),
        }
    }

    #[cfg(test)]
    fn expired_for_tests() -> Self {
        Self {
            deadline: Instant::now(),
        }
    }

    #[cfg(test)]
    fn with_deadline_for_tests(deadline: Instant) -> Self {
        Self { deadline }
    }

    fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn has_work_budget(&self) -> bool {
        self.remaining() >= Duration::from_micros(MIN_INLINE_BUDGET_US)
    }

    fn diff_deadline(&self) -> Option<Instant> {
        if self.has_work_budget() {
            Some(self.deadline)
        } else {
            None
        }
    }

    fn effective_timeout(&self, max_millis: u64) -> Option<Duration> {
        let remaining = self.remaining();
        if remaining < Duration::from_micros(MIN_INLINE_BUDGET_US) {
            return None;
        }
        Some(remaining.min(Duration::from_millis(max_millis)))
    }
}

pub(super) fn ranges_for_modify<'a>(
    reference: &'a str,
    current: &'a str,
    budget: &InlineBudget,
) -> Vec<InlineRange> {
    if !budget.has_work_budget() {
        return Vec::new();
    }
    let reference_len = reference.chars().count();
    let current_len = current.chars().count();
    if reference_len > WORD_LIMIT || current_len > WORD_LIMIT {
        return Vec::new();
    }
    if reference_len <= GRAPHEME_LIMIT && current_len <= GRAPHEME_LIMIT {
        let ranges = token_ranges(reference, current, budget);
        if !ranges.is_empty() {
            return ranges;
        }
        if !budget.has_work_budget() {
            return Vec::new();
        }
        return grapheme_ranges(reference, current, budget);
    }
    let Some(timeout) = budget.effective_timeout(WORD_TIMEOUT_MS) else {
        return Vec::new();
    };
    let diff = TextDiff::configure()
        .timeout(timeout)
        .diff_words(reference, current);
    collect_ranges(&diff)
}

fn grapheme_ranges<'a>(
    reference: &'a str,
    current: &'a str,
    budget: &InlineBudget,
) -> Vec<InlineRange> {
    let Some(timeout) = budget.effective_timeout(GRAPHEME_TIMEOUT_MS) else {
        return Vec::new();
    };
    let diff = TextDiff::configure()
        .timeout(timeout)
        .diff_graphemes(reference, current);
    collect_ranges(&diff)
}

fn token_ranges(reference: &str, current: &str, budget: &InlineBudget) -> Vec<InlineRange> {
    let reference_tokens = tokenize_for_inline(reference);
    let current_tokens = tokenize_for_inline(current);
    if reference_tokens.len() <= 1 || current_tokens.len() <= 1 {
        return Vec::new();
    }
    let Some(deadline) = budget.diff_deadline() else {
        return Vec::new();
    };
    let reference_values = token_values(&reference_tokens);
    let current_values = token_values(&current_tokens);
    let deadline = deadline.min(Instant::now() + Duration::from_millis(TOKEN_TIMEOUT_MS));
    let ops = capture_diff_slices_deadline(
        Algorithm::Myers,
        &reference_values,
        &current_values,
        Some(deadline),
    );
    let mut ranges = Vec::new();
    for op in ops {
        let (tag, reference_range, current_range) = op.as_tag_tuple();
        match tag {
            DiffTag::Equal => {}
            DiffTag::Delete => push_token_ranges(
                &mut ranges,
                DiffSide::Reference,
                &reference_tokens[reference_range],
            ),
            DiffTag::Insert => push_token_ranges(
                &mut ranges,
                DiffSide::Current,
                &current_tokens[current_range],
            ),
            DiffTag::Replace => push_replace_token_ranges(
                &mut ranges,
                &reference_tokens[reference_range],
                &current_tokens[current_range],
            ),
        }
    }
    ranges
}

fn collect_ranges<'a>(diff: &TextDiff<'a, 'a, '_, str>) -> Vec<InlineRange> {
    let mut left_offset = 0;
    let mut right_offset = 0;
    let mut ranges = Vec::new();
    for change in diff.iter_all_changes() {
        let len = change.value().chars().count();
        match change.tag() {
            ChangeTag::Delete => {
                push_range(&mut ranges, DiffSide::Reference, left_offset, len);
                left_offset += len;
            }
            ChangeTag::Insert => {
                push_range(&mut ranges, DiffSide::Current, right_offset, len);
                right_offset += len;
            }
            ChangeTag::Equal => {
                left_offset += len;
                right_offset += len;
            }
        }
    }
    ranges
}

fn push_range(ranges: &mut Vec<InlineRange>, side: DiffSide, start: usize, len: usize) {
    if len == 0 {
        return;
    }
    ranges.push(InlineRange {
        side,
        start,
        end: start + len,
    });
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct InlineToken {
    text: String,
    start: usize,
    end: usize,
}

fn tokenize_for_inline(line: &str) -> Vec<InlineToken> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        if is_separator(ch) {
            let end = if ch == ':' && chars.get(index + 1) == Some(&':') {
                index + 2
            } else {
                index + 1
            };
            tokens.push(token_from_chars(&chars, start, end));
            index = end;
            continue;
        }
        index += 1;
        while index < chars.len() && !chars[index].is_whitespace() && !is_separator(chars[index]) {
            index += 1;
        }
        tokens.push(token_from_chars(&chars, start, index));
    }
    tokens
}

fn is_separator(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | ';'
            | ':'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | '<'
            | '>'
            | '+'
            | '-'
            | '*'
            | '/'
            | '='
            | '%'
            | '!'
            | '?'
            | '&'
            | '|'
            | '^'
            | '~'
            | '"'
            | '\''
    )
}

fn token_from_chars(chars: &[char], start: usize, end: usize) -> InlineToken {
    InlineToken {
        text: chars[start..end].iter().collect(),
        start,
        end,
    }
}

fn token_values(tokens: &[InlineToken]) -> Vec<String> {
    tokens.iter().map(|token| token.text.clone()).collect()
}

fn push_token_ranges(ranges: &mut Vec<InlineRange>, side: DiffSide, tokens: &[InlineToken]) {
    for token in tokens {
        if token.start < token.end {
            ranges.push(InlineRange {
                side,
                start: token.start,
                end: token.end,
            });
        }
    }
}

fn push_replace_token_ranges(
    ranges: &mut Vec<InlineRange>,
    reference_tokens: &[InlineToken],
    current_tokens: &[InlineToken],
) {
    if reference_tokens.len() != current_tokens.len() {
        push_token_ranges(ranges, DiffSide::Reference, reference_tokens);
        push_token_ranges(ranges, DiffSide::Current, current_tokens);
        return;
    }
    for (reference, current) in reference_tokens.iter().zip(current_tokens.iter()) {
        if let Some((reference_range, current_range)) = refined_token_ranges(reference, current) {
            push_refined_range(ranges, DiffSide::Reference, reference, reference_range);
            push_refined_range(ranges, DiffSide::Current, current, current_range);
        } else {
            push_token_ranges(ranges, DiffSide::Reference, std::slice::from_ref(reference));
            push_token_ranges(ranges, DiffSide::Current, std::slice::from_ref(current));
        }
    }
}

fn refined_token_ranges(
    reference: &InlineToken,
    current: &InlineToken,
) -> Option<((usize, usize), (usize, usize))> {
    if reference.text == current.text
        || !is_identifier_token(reference)
        || !is_identifier_token(current)
    {
        return None;
    }
    let reference_chars = reference.text.chars().collect::<Vec<_>>();
    let current_chars = current.text.chars().collect::<Vec<_>>();
    let prefix = common_prefix_len(&reference_chars, &current_chars);
    let suffix = common_suffix_len(&reference_chars, &current_chars, prefix);
    let reference_end = reference_chars.len().saturating_sub(suffix);
    let current_end = current_chars.len().saturating_sub(suffix);
    if prefix >= reference_end && prefix >= current_end {
        return None;
    }
    Some(((prefix, reference_end), (prefix, current_end)))
}

fn is_identifier_token(token: &InlineToken) -> bool {
    token
        .text
        .chars()
        .all(|ch| ch == '_' || ch.is_alphanumeric())
}

fn common_prefix_len(reference: &[char], current: &[char]) -> usize {
    reference
        .iter()
        .zip(current.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(reference: &[char], current: &[char], prefix: usize) -> usize {
    let max_suffix = reference.len().min(current.len()).saturating_sub(prefix);
    reference
        .iter()
        .rev()
        .zip(current.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count()
}

fn push_refined_range(
    ranges: &mut Vec<InlineRange>,
    side: DiffSide,
    token: &InlineToken,
    range: (usize, usize),
) {
    if range.0 >= range.1 {
        return;
    }
    ranges.push(InlineRange {
        side,
        start: token.start + range.0,
        end: token.start + range.1,
    });
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{DiffSide, InlineBudget, WORD_LIMIT, ranges_for_modify, tokenize_for_inline};

    #[test]
    fn inline_ranges_use_grapheme_offsets() {
        let ranges = ranges_for_modify("hello\n", "hullo\n", &InlineBudget::new());
        assert!(ranges.contains(&super::InlineRange {
            side: DiffSide::Reference,
            start: 1,
            end: 2,
        }));
        assert!(ranges.contains(&super::InlineRange {
            side: DiffSide::Current,
            start: 1,
            end: 2,
        }));
    }

    #[test]
    fn inline_ranges_skip_very_long_lines() {
        let long = "x".repeat(WORD_LIMIT + 1);
        assert!(ranges_for_modify(&long, "y", &InlineBudget::new()).is_empty());
    }

    #[test]
    fn inline_ranges_skip_when_budget_expired() {
        let budget = InlineBudget::expired_for_tests();
        let ranges = ranges_for_modify("compare.left_buffer\n", "compare.right_buffer\n", &budget);
        assert!(ranges.is_empty());
    }

    #[test]
    fn inline_ranges_skip_when_remaining_budget_is_too_small() {
        let deadline = Instant::now() + Duration::from_micros(100);
        let budget = InlineBudget::with_deadline_for_tests(deadline);
        let ranges = ranges_for_modify("compare.left_buffer\n", "compare.right_buffer\n", &budget);
        assert!(ranges.is_empty());
    }

    #[test]
    fn tokenizer_keeps_snake_case_identifiers() {
        assert_eq!(token_texts("foo_bar"), ["foo_bar"]);
        assert_eq!(token_texts("foo.bar"), ["foo", ".", "bar"]);
        assert_eq!(token_texts("Vec<T>"), ["Vec", "<", "T", ">"]);
        assert_eq!(
            token_texts("obj::method()"),
            ["obj", "::", "method", "(", ")"]
        );
        assert!(tokenize_for_inline("").is_empty());
        assert!(tokenize_for_inline(" \t  ").is_empty());
    }

    #[test]
    fn tokenizer_reports_character_offsets() {
        let tokens = tokenize_for_inline(" æ::x");
        assert_eq!(tokens[0].start, 1);
        assert_eq!(tokens[0].end, 2);
        assert_eq!(tokens[1].start, 2);
        assert_eq!(tokens[1].end, 4);
    }

    #[test]
    fn token_ranges_highlight_changed_identifier_parts() {
        let ranges = ranges_for_modify(
            "compare.left_buffer\n",
            "compare.reference_buffer\n",
            &InlineBudget::new(),
        );
        assert!(ranges.contains(&super::InlineRange {
            side: DiffSide::Reference,
            start: 8,
            end: 12,
        }));
        assert!(ranges.contains(&super::InlineRange {
            side: DiffSide::Current,
            start: 8,
            end: 17,
        }));
    }

    fn token_texts(line: &str) -> Vec<String> {
        tokenize_for_inline(line)
            .into_iter()
            .map(|token| token.text)
            .collect()
    }
}
