use crate::editor_tab::EditorTab;

impl EditorTab {
    pub(crate) fn compare_uses_full_row_backgrounds_for_tests(&self) -> bool {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.tags.uses_full_row_backgrounds_for_tests())
            })
            .unwrap_or(false)
    }
}
