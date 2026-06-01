use gettextrs::pgettext;
use libadwaita as adw;

use super::Workspace;

impl Workspace {
    pub(crate) fn handle_selected_tab_changed(self: &std::rc::Rc<Self>) {
        let selected = self.selected_tab();
        self.search.bind_tab(selected.clone());
        self.refresh_selected_state();
        crate::workspace_monitor::on_selected_tab_changed(self);
    }

    pub(crate) fn refresh_selected_state(&self) {
        self.sync_tab_action_state();
        let selected = self.selected_tab();
        self.status_bar.update(selected.as_deref());
        if let Some(callback) = self.format_preferences_handler.get() {
            callback(selected.clone());
        }
        if let Some(callback) = self.compare_action_sync_handler.get() {
            callback(selected.clone());
        }
        if let Some(callback) = self.document_tools_sync_handler.get() {
            callback(selected.clone());
        }
        if let Some(callback) = self.git_action_sync_handler.get() {
            callback(selected.clone());
        }

        if let Some(tab) = selected {
            self.title_widget.set_title(&tab.title());
            self.title_widget.set_subtitle("");
            self.save_action
                .set_enabled(tab.can_save_document() && tab.is_dirty());
            self.save_as_action.set_enabled(tab.can_save_document());
            self.close_action.set_enabled(true);
            return;
        }

        self.title_widget
            .set_title(&pgettext("document title", "Untitled"));
        self.title_widget
            .set_subtitle(&pgettext("document subtitle", "Plain Text Document"));
        self.save_action.set_enabled(false);
        self.save_as_action.set_enabled(false);
        self.close_action.set_enabled(false);
    }

    pub(crate) fn show_toast(&self, message: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(message));
    }
}
