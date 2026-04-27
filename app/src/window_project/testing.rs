use gtk4::prelude::*;
use gtk4::{gio, glib::variant::ToVariant};

use super::{WindowProjectController, auto_refresh, close_root, symlink};

impl WindowProjectController {
    pub(crate) fn root_uri_for_tests(&self) -> Option<String> {
        self.state
            .borrow()
            .root
            .as_ref()
            .map(|root| root.file.uri().to_string())
    }

    pub(crate) fn action_states_for_tests(&self) -> (bool, bool, bool, bool) {
        let state = self.state.borrow();
        (
            state.sidebar_visible_action.is_enabled(),
            state.show_hidden_action.is_enabled(),
            state.refresh_action.is_enabled(),
            state.close_action.is_enabled(),
        )
    }

    pub(crate) fn tree_entry_names_for_tests(&self) -> Vec<String> {
        self.state
            .borrow()
            .browser
            .tree()
            .model()
            .visible_entry_names_for_tests()
    }

    pub(crate) fn project_monitor_count_for_tests(&self) -> usize {
        self.state
            .borrow()
            .browser
            .tree()
            .model()
            .monitor_count_for_tests()
    }

    pub(crate) fn trigger_project_auto_refresh_for_tests(&self) {
        auto_refresh::refresh_tree(&self.state);
    }

    pub(crate) fn expand_tree_entry_for_tests(&self, name: &str) -> bool {
        self.state
            .borrow()
            .browser
            .tree()
            .expand_entry_for_tests(name)
    }

    pub(crate) fn selected_tree_uri_for_tests(&self) -> Option<String> {
        self.state.borrow().browser.tree().selected_uri_for_tests()
    }

    pub(crate) fn close_for_tests(&self) {
        close_root(&self.state);
    }

    pub(crate) fn refresh_for_tests(&self) {
        auto_refresh::refresh_tree(&self.state);
    }

    pub(crate) fn set_show_hidden_for_tests(&self, show_hidden: bool) {
        let action = self.state.borrow().show_hidden_action.clone();
        action.change_state(&show_hidden.to_variant());
    }

    pub(crate) fn set_sidebar_visible_for_tests(&self, visible: bool) {
        let action = self.state.borrow().sidebar_visible_action.clone();
        action.change_state(&visible.to_variant());
    }

    pub(crate) fn resolve_symlink_for_tests(&self, file: &gio::File) {
        symlink::handle_symlink_activation(&self.state, file);
    }
}
