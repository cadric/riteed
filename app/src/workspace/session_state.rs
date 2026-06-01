use std::rc::Rc;

use gtk4::glib;

use super::Workspace;

impl Workspace {
    pub(crate) fn finish_session_restore_without_startup_write(self: &Rc<Self>) {
        Self::schedule_session_restore_finish(self);
    }

    fn schedule_session_restore_finish(workspace: &Rc<Self>) {
        let weak = Rc::downgrade(workspace);
        glib::idle_add_local_once(move || {
            if let Some(workspace) = weak.upgrade() {
                workspace.mark_session_restore_finished();
            }
        });
    }

    fn mark_session_restore_finished(self: &Rc<Self>) {
        if self.try_mark_session_restore_finished() {
            return;
        }
        Self::schedule_session_restore_finish(self);
    }

    fn try_mark_session_restore_finished(&self) -> bool {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return false;
        };
        state.restoring_session = false;
        true
    }

    pub(crate) fn persist_session_state_if_needed(&self) {
        let snapshot = crate::session::session_snapshot(
            &self
                .ordered_tabs()
                .into_iter()
                .map(|tab| tab.session_uri())
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .filter(|uri| crate::document_limits::uri_supports_session_restore(uri))
        .collect::<Vec<_>>();
        let selected = self
            .selected_tab()
            .and_then(|tab| tab.session_uri())
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
