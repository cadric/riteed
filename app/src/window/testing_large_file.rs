use std::rc::{Rc, Weak};

use super::Window;
use crate::editor_tab::EditorTab;

impl Window {
    pub(crate) fn selected_tab_weak_for_tests(&self) -> Option<Weak<EditorTab>> {
        self.workspace
            .selected_tab()
            .map(|tab: Rc<EditorTab>| Rc::downgrade(&tab))
    }

    pub(crate) fn set_large_file_full_feature_limit_for_tests(&self, value: f64) {
        self.shell
            .large_file_full_feature_limit_row
            .set_value(value);
    }
}
