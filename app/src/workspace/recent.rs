use super::Workspace;

impl Workspace {
    pub(crate) fn remember_recent_uri(&self, uri: &str) {
        if self
            .state
            .try_borrow()
            .map_or(true, |state| state.restoring_session)
        {
            return;
        }

        let current = self.settings.recent_files();
        let updated = crate::session::remember_recent(&current, uri);
        if crate::session::list_changed(&current, &updated) {
            self.settings.set_recent_files(&updated);
            if let Ok(mut state) = self.state.try_borrow_mut() {
                state.recent_files = updated;
            }
        }
    }

    pub(crate) fn prune_recent_uri(&self, uri: &str) {
        let current = self.settings.recent_files();
        let updated = crate::session::forget_recent(&current, uri);
        if crate::session::list_changed(&current, &updated) {
            self.settings.set_recent_files(&updated);
            if let Ok(mut state) = self.state.try_borrow_mut() {
                state.recent_files = updated;
            }
        }
    }
}
