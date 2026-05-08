use std::rc::Rc;

use crate::editor_tab::EditorTab;

pub(super) struct SearchBinding {
    pub(super) context: sourceview5::SearchContext,
}

pub(super) struct SearchState {
    pub(super) active_tab: Option<Rc<EditorTab>>,
    pub(super) active_binding: Option<SearchBinding>,
    pub(super) manual_message: Option<String>,
}
