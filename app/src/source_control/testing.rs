use super::{SourceControlController, history};

impl SourceControlController {
    pub(crate) fn active_row_path_for_tests(&self) -> Option<String> {
        let raw = self.state.borrow().views.active_row_path_for_tests()?;
        String::from_utf8(raw).ok()
    }

    pub(crate) fn history_expanded_for_tests(&self) -> bool {
        self.state.borrow().history.expanded_for_tests()
    }

    pub(crate) fn history_content_revealed_for_tests(&self) -> bool {
        self.state.borrow().history.content_revealed_for_tests()
    }

    pub(crate) fn history_root_visible_for_tests(&self) -> bool {
        self.state.borrow().history.root_visible_for_tests()
    }

    pub(crate) fn toggle_history_for_tests(&self) -> bool {
        history::toggle(&self.state)
    }

    pub(crate) fn history_split_position_for_tests(&self) -> i32 {
        self.state.borrow().history_split.position()
    }

    pub(crate) fn set_history_split_position_for_tests(&self, position: i32) {
        self.state.borrow().history_split.set_position(position);
    }
}
