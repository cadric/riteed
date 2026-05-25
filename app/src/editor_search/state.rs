use std::rc::Rc;

use crate::editor_tab::EditorTab;

use super::{SearchTarget, preview::PreviewSearchBinding};

pub(super) enum SearchBinding {
    Source { context: sourceview5::SearchContext },
    Preview(PreviewSearchBinding),
}

pub(super) struct SearchState {
    pub(super) active_tab: Option<Rc<EditorTab>>,
    pub(super) active_target: SearchTarget,
    pub(super) active_binding: Option<SearchBinding>,
    pub(super) manual_message: Option<String>,
}
