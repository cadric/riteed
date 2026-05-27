use std::rc::Rc;

use gtk4::prelude::*;

use crate::editor_tab::EditorTab;

use super::WindowCompareController;

impl WindowCompareController {
    pub(super) fn install_actions(&self) {
        self.shell.add_action(&self.refresh_reference_action);
        self.shell.add_action(&self.exit_action);
        self.shell.add_action(&self.next_action);
        self.shell.add_action(&self.prev_action);
        self.shell.add_action(&self.open_reviewed_file_action);
        self.shell.add_action(&self.change_list_action);
        self.shell.add_action(&self.reveal_above_action);
        self.shell.add_action(&self.reveal_below_action);
        self.shell.add_action(&self.reveal_all_action);
        self.shell.add_action(&self.refresh_review_action);
        self.compare_settings_actions.add_to_window(&self.shell);
        self.shell.add_action(&self.tab_compare_file_action);
        self.shell.add_action(&self.tab_compare_saved_action);
        self.shell.add_action(&self.tab_compare_pasted_text_action);
    }

    pub(super) fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.refresh_reference_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.refresh_reference();
            }
        });
        let weak = Rc::downgrade(self);
        self.exit_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.exit_compare();
            }
        });
        let weak = Rc::downgrade(self);
        self.next_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.next_diff();
            }
        });
        let weak = Rc::downgrade(self);
        self.prev_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.previous_diff();
            }
        });
        let weak = Rc::downgrade(self);
        self.open_reviewed_file_action
            .connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.open_reviewed_file();
                }
            });
        let weak = Rc::downgrade(self);
        self.change_list_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.present_change_list();
            }
        });
        let weak = Rc::downgrade(self);
        self.reveal_above_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.reveal_above();
            }
        });
        let weak = Rc::downgrade(self);
        self.reveal_below_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.reveal_below();
            }
        });
        let weak = Rc::downgrade(self);
        self.reveal_all_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.reveal_all();
            }
        });
        let weak = Rc::downgrade(self);
        self.refresh_review_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.refresh_review();
            }
        });
        let weak = Rc::downgrade(self);
        self.tab_compare_file_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.present_tab_compare_file();
            }
        });
        let weak = Rc::downgrade(self);
        self.tab_compare_saved_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.compare_selected_with_saved();
            }
        });
        let weak = Rc::downgrade(self);
        self.tab_compare_pasted_text_action
            .connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.present_tab_compare_pasted_text();
                }
            });
        self.install_compare_settings_callbacks();
    }

    pub(super) fn sync_actions(&self, selected: Option<&EditorTab>) {
        let active = selected.is_some_and(EditorTab::is_compare_active);
        let review =
            selected.is_some_and(|tab| tab.kind() == crate::editor_tab::TabKind::GitReview);
        let review_with_changes = review && selected.is_some_and(|tab| tab.review_file_count() > 0);
        let can_navigate = active || review_with_changes;
        let can_reveal = selected.is_some_and(|tab| {
            if tab.kind() == crate::editor_tab::TabKind::GitReview {
                tab.review_can_reveal_context()
            } else {
                tab.compare_can_reveal_context()
            }
        });
        let can_start = selected.is_some_and(|tab| !tab.is_compare_active());
        self.tab_compare_file_action.set_enabled(can_start);
        self.tab_compare_pasted_text_action.set_enabled(can_start);
        self.tab_compare_saved_action
            .set_enabled(self.can_compare_with_saved(selected));
        self.refresh_reference_action
            .set_enabled(selected.is_some_and(EditorTab::compare_reference_is_refreshable));
        self.exit_action.set_enabled(active);
        self.next_action.set_enabled(can_navigate);
        self.prev_action.set_enabled(can_navigate);
        self.reveal_above_action.set_enabled(can_reveal);
        self.reveal_below_action.set_enabled(can_reveal);
        self.reveal_all_action.set_enabled(can_reveal);
        self.change_list_action.set_enabled(review_with_changes);
        self.open_reviewed_file_action.set_enabled(
            review && selected.is_some_and(|tab| tab.current_review_open_target().is_some()),
        );
        self.refresh_review_action.set_enabled(review);
        self.sync_compare_settings_actions();
    }

    fn can_compare_with_saved(&self, selected: Option<&EditorTab>) -> bool {
        selected.is_some_and(|tab| {
            !tab.is_compare_active()
                && tab.has_saved_local_uri()
                && tab.is_dirty()
                && !self.workspace.settings.autosave_enabled()
        })
    }
}
