use super::model::{DiffRowModel, build_row_model};

const MAX_COMPARE_BYTES: usize = 1_000_000;
const MAX_COMPARE_LINES: usize = 20_000;

pub(super) fn compute_diff_row_model(reference_text: &str, current_text: &str) -> DiffRowModel {
    if compare_too_large(reference_text, current_text) {
        return DiffRowModel::too_large();
    }
    build_row_model(reference_text, current_text)
}

fn compare_too_large(reference_text: &str, current_text: &str) -> bool {
    reference_text.len().saturating_add(current_text.len()) > MAX_COMPARE_BYTES
        || line_count(reference_text).saturating_add(line_count(current_text)) > MAX_COMPARE_LINES
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_COMPARE_BYTES, compute_diff_row_model};

    #[test]
    fn performance_guard_skips_large_inputs() {
        let large = "x".repeat(MAX_COMPARE_BYTES + 1);
        let model = compute_diff_row_model(&large, "");
        assert!(model.too_large);
        assert!(model.hunks.is_empty());
    }
}
