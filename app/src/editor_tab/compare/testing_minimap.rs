use super::EditorTab;

impl EditorTab {
    pub(crate) fn compare_minimaps_visible_for_tests(&self) -> (bool, bool, bool) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_minimap.visible_for_tests(),
                        compare.right_minimap.visible_for_tests(),
                        compare.unified_minimap.visible_for_tests(),
                    )
                })
            })
            .unwrap_or_default()
    }
}
