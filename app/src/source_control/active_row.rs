use std::rc::Rc;

use super::{SourceControlController, SourceStateRef, path_target};

impl SourceControlController {
    pub(crate) fn set_active_uri(&self, uri: Option<String>) {
        self.state.borrow_mut().active_uri = uri;
        apply_active_row(&self.state);
    }
}

pub(super) fn apply_active_row(state: &SourceStateRef) {
    let (views, raw) = {
        let state = state.borrow();
        let raw = state.active_uri.as_deref().and_then(|uri| {
            let repo = state.repo.as_ref()?;
            path_target::raw_path_for_uri(repo, uri)
        });
        (Rc::clone(&state.views), raw)
    };
    views.mark_active_row(raw.as_deref());
}
