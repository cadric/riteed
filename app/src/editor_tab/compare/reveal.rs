#[derive(Clone, Copy)]
pub(super) enum RevealScope {
    Above,
    Below,
    All,
}

pub(super) fn reveal_rows(
    hidden_start: usize,
    hidden_end: usize,
    context_lines: i32,
    scope: RevealScope,
) -> Vec<usize> {
    let hidden_count = hidden_end.saturating_sub(hidden_start);
    if hidden_count == 0 {
        return Vec::new();
    }
    let context = usize::try_from(context_lines)
        .map_or(3, |value| value)
        .clamp(1, 10)
        .min(hidden_count);
    let range = match scope {
        RevealScope::Above => hidden_start..hidden_start.saturating_add(context),
        RevealScope::Below => hidden_end.saturating_sub(context)..hidden_end,
        RevealScope::All => hidden_start..hidden_end,
    };
    range.collect()
}
