use super::{SourceControlController, SourceControlState, path_target};

impl SourceControlController {
    pub(crate) fn set_active_uri(&self, uri: Option<String>) {
        self.state.borrow_mut().active_uri = uri;
        apply_active_row(&self.state.borrow());
    }
}

pub(super) fn apply_active_row(state: &SourceControlState) {
    let raw = state.active_uri.as_deref().and_then(|uri| {
        let repo = state.repo.as_ref()?;
        path_target::raw_path_for_uri(repo, uri)
    });
    state.views.mark_active_row(raw.as_deref());
}
