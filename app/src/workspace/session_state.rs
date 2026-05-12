use super::Workspace;

impl Workspace {
    pub(crate) fn persist_session_state_if_needed(&self) {
        let snapshot = crate::session::session_snapshot(
            &self
                .ordered_tabs()
                .into_iter()
                .map(|tab| tab.document_uri())
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .filter(|uri| crate::document_limits::uri_supports_session_restore(uri))
        .collect::<Vec<_>>();
        let selected = self
            .selected_tab()
            .and_then(|tab| tab.document_uri())
            .filter(|uri| crate::document_limits::uri_supports_session_restore(uri));
        let selected = crate::session::selected_session_value(selected);

        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if state.restoring_session {
            return;
        }
        if !state.persist_session {
            return;
        }
        if crate::session::list_changed(&state.stored_session_files, &snapshot) {
            self.settings.set_session_files(&snapshot);
            state.stored_session_files = snapshot;
        }
        if crate::session::string_changed(&state.stored_selected_file, &selected) {
            self.settings.set_session_selected_file(&selected);
            state.stored_selected_file = selected;
        }
    }
}
