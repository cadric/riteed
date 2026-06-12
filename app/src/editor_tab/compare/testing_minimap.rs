use gtk4::prelude::*;

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

    pub(crate) fn compare_minimap_scrollbar_policies_for_tests(
        &self,
    ) -> (gtk4::PolicyType, gtk4::PolicyType, gtk4::PolicyType) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_minimap.scrollbar_policy_for_tests(),
                        compare.right_minimap.scrollbar_policy_for_tests(),
                        compare.unified_minimap.scrollbar_policy_for_tests(),
                    )
                })
            })
            .unwrap_or((
                gtk4::PolicyType::Automatic,
                gtk4::PolicyType::Automatic,
                gtk4::PolicyType::Automatic,
            ))
    }

    pub(crate) fn compare_scroll_past_end_padding_for_tests(&self) -> (i32, i32, i32) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_view.bottom_margin(),
                        compare.right_view.bottom_margin(),
                        compare.unified_view.bottom_margin(),
                    )
                })
            })
            .unwrap_or_default()
    }

    pub(crate) fn set_compare_viewport_page_sizes_for_tests(&self, page_size: f64) {
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare
                .left_minimap
                .set_viewport_page_size_for_tests(page_size);
            compare
                .right_minimap
                .set_viewport_page_size_for_tests(page_size);
            compare
                .unified_minimap
                .set_viewport_page_size_for_tests(page_size);
        }
    }
}
