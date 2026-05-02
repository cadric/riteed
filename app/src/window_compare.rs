mod dialog;

use std::cell::Cell;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::dialogs;
use crate::editor_tab::EditorTab;
use crate::error::AppError;
use crate::workspace::Workspace;

use dialog::CompareSlot;

pub(crate) struct WindowCompareController {
    shell: adw::ApplicationWindow,
    workspace: Rc<Workspace>,
    compare_action: gio::SimpleAction,
    refresh_reference_action: gio::SimpleAction,
    exit_action: gio::SimpleAction,
    next_action: gio::SimpleAction,
    prev_action: gio::SimpleAction,
    tab_compare_file_action: gio::SimpleAction,
    tab_compare_saved_action: gio::SimpleAction,
    tab_compare_pasted_text_action: gio::SimpleAction,
    compare_action_installed: Cell<bool>,
}

impl WindowCompareController {
    #[must_use]
    pub(crate) fn new(shell: &adw::ApplicationWindow, workspace: &Rc<Workspace>) -> Rc<Self> {
        let controller = Rc::new(Self {
            shell: shell.clone(),
            workspace: Rc::clone(workspace),
            compare_action: gio::SimpleAction::new("compare", None),
            refresh_reference_action: gio::SimpleAction::new("compare-refresh-reference", None),
            exit_action: gio::SimpleAction::new("compare-exit", None),
            next_action: gio::SimpleAction::new("diff-next", None),
            prev_action: gio::SimpleAction::new("diff-prev", None),
            tab_compare_file_action: gio::SimpleAction::new("tab-compare-with-file", None),
            tab_compare_saved_action: gio::SimpleAction::new(
                "tab-compare-with-saved-version",
                None,
            ),
            tab_compare_pasted_text_action: gio::SimpleAction::new(
                "tab-compare-with-pasted-text",
                None,
            ),
            compare_action_installed: Cell::new(false),
        });
        controller.install_actions();
        controller.install_callbacks();
        let weak = Rc::downgrade(&controller);
        workspace.set_compare_action_sync_handler(Rc::new(move |selected| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_actions(selected.as_deref());
            }
        }));
        controller.sync_actions(workspace.selected_tab().as_deref());
        controller
    }

    pub(crate) fn refresh_reference(self: &Rc<Self>) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        let weak = Rc::downgrade(self);
        tab.refresh_compare_reference(Rc::new(move |result| {
            if let Some(controller) = weak.upgrade() {
                controller.handle_compare_result(result);
            }
        }));
    }

    pub(crate) fn exit_compare(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.exit_compare();
        }
        self.workspace.refresh_selected_state();
    }

    pub(crate) fn next_diff(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_next_diff();
        }
    }

    pub(crate) fn previous_diff(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_previous_diff();
        }
    }

    pub(crate) fn refresh_action_state(&self) {
        self.sync_actions(self.workspace.selected_tab().as_deref());
    }

    #[cfg(test)]
    pub(crate) fn compare_two_files_for_tests(
        self: &Rc<Self>,
        editable: &gio::File,
        reference: &gio::File,
    ) {
        let callback: Rc<dyn Fn(Result<(), AppError>)> = Rc::new(|_result| {});
        self.open_two_file_compare(editable, reference, &callback);
    }

    #[cfg(test)]
    pub(crate) fn action_states_for_tests(&self) -> (bool, bool, bool, bool, bool) {
        (
            self.compare_action_installed.get(),
            self.refresh_reference_action.is_enabled(),
            self.exit_action.is_enabled(),
            self.next_action.is_enabled(),
            self.prev_action.is_enabled(),
        )
    }

    #[cfg(test)]
    pub(crate) fn tab_compare_action_states_for_tests(&self) -> (bool, bool, bool) {
        (
            self.tab_compare_file_action.is_enabled(),
            self.tab_compare_saved_action.is_enabled(),
            self.tab_compare_pasted_text_action.is_enabled(),
        )
    }

    fn install_actions(&self) {
        self.shell.add_action(&self.refresh_reference_action);
        self.shell.add_action(&self.exit_action);
        self.shell.add_action(&self.next_action);
        self.shell.add_action(&self.prev_action);
        self.shell.add_action(&self.tab_compare_file_action);
        self.shell.add_action(&self.tab_compare_saved_action);
        self.shell.add_action(&self.tab_compare_pasted_text_action);
        self.shell.add_action(&self.compare_action);
        self.compare_action_installed.set(true);
    }

    fn handle_compare_result(&self, result: Result<(), AppError>) {
        self.workspace.refresh_selected_state();
        if let Err(error) = result {
            dialogs::present_error(&self.shell, &error);
        }
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.compare_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.present_compare_dialog();
            }
        });
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
    }

    fn sync_actions(&self, selected: Option<&EditorTab>) {
        let active = selected.is_some_and(EditorTab::is_compare_active);
        let can_start = selected.is_some_and(|tab| !tab.is_compare_active());
        self.set_compare_action_visible(!active);
        self.tab_compare_file_action.set_enabled(can_start);
        self.tab_compare_pasted_text_action.set_enabled(can_start);
        self.tab_compare_saved_action
            .set_enabled(self.can_compare_with_saved(selected));
        self.refresh_reference_action
            .set_enabled(selected.is_some_and(EditorTab::compare_reference_is_refreshable));
        self.exit_action.set_enabled(active);
        self.next_action.set_enabled(active);
        self.prev_action.set_enabled(active);
    }

    fn can_compare_with_saved(&self, selected: Option<&EditorTab>) -> bool {
        selected.is_some_and(|tab| {
            !tab.is_compare_active()
                && tab.has_saved_local_uri()
                && tab.is_dirty()
                && !self.workspace.settings.autosave_enabled()
        })
    }

    fn set_compare_action_visible(&self, visible: bool) {
        if visible {
            if !self.compare_action_installed.get() {
                self.shell.add_action(&self.compare_action);
                self.compare_action_installed.set(true);
            }
            return;
        }
        if self.compare_action_installed.get() {
            self.shell.remove_action("compare");
            self.compare_action_installed.set(false);
        }
    }

    fn present_compare_dialog(self: &Rc<Self>) {
        dialog::present_compare_dialog(self);
    }

    fn present_tab_compare_file(self: &Rc<Self>) {
        if !self.tab_compare_file_action.is_enabled() {
            return;
        }
        let weak = Rc::downgrade(self);
        dialog::choose_file(
            &self.shell,
            &pgettext("file dialog title", "Choose a File"),
            Rc::new(move |file| {
                if let Some(controller) = weak.upgrade()
                    && let Some(file) = file
                {
                    controller.start_compare_for_selected_tab(CompareSlot::File(file));
                }
            }),
        );
    }

    fn compare_selected_with_saved(self: &Rc<Self>) {
        if self.tab_compare_saved_action.is_enabled() {
            self.start_compare_for_selected_tab(CompareSlot::SavedVersion);
        }
    }

    fn present_tab_compare_pasted_text(self: &Rc<Self>) {
        if !self.tab_compare_pasted_text_action.is_enabled() {
            return;
        }
        let weak = Rc::downgrade(self);
        dialog::show_paste_text_dialog(
            &self.shell,
            None,
            Rc::new(move |text| {
                if let Some(controller) = weak.upgrade()
                    && let Some(text) = text
                {
                    controller.start_compare_for_selected_tab(CompareSlot::Text(text));
                }
            }),
        );
    }

    fn start_compare_for_selected_tab(self: &Rc<Self>, right: CompareSlot) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        let callback = self.wrap_compare_callback(Rc::new(|_result| {}));
        self.start_compare_for_current_document(&tab, right, &callback);
    }

    fn start_compare_from_dialog(
        self: &Rc<Self>,
        left: CompareSlot,
        right: CompareSlot,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let callback = self.wrap_compare_callback(callback);
        match left {
            CompareSlot::CurrentDocument(tab) => {
                self.start_compare_for_current_document(&tab, right, &callback);
            }
            CompareSlot::File(file) => {
                self.start_compare_for_file(&file, right, &callback);
            }
            CompareSlot::Text(text) => {
                self.start_compare_for_text(&text, right, &callback);
            }
            CompareSlot::None | CompareSlot::SavedVersion => {
                callback(Err(AppError::Cancelled));
            }
        }
    }

    fn start_compare_for_current_document(
        self: &Rc<Self>,
        tab: &Rc<EditorTab>,
        right: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        match right {
            CompareSlot::SavedVersion => tab.start_compare_with_disk(Rc::clone(callback)),
            CompareSlot::File(file) => tab.start_compare_with_file(&file, Rc::clone(callback)),
            CompareSlot::Text(text) => tab.start_compare_with_text(&text, Rc::clone(callback)),
            CompareSlot::None | CompareSlot::CurrentDocument(_) => {
                callback(Err(AppError::Cancelled));
            }
        }
        self.workspace.refresh_selected_state();
    }

    fn start_compare_for_file(
        self: &Rc<Self>,
        editable: &gio::File,
        right: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        match right {
            CompareSlot::File(reference) => {
                self.open_two_file_compare(editable, &reference, callback);
            }
            CompareSlot::Text(text) => {
                self.open_file_text_compare(editable, &text, callback);
            }
            _ => {
                callback(Err(AppError::Cancelled));
            }
        }
    }

    fn start_compare_for_text(
        self: &Rc<Self>,
        editable_text: &str,
        right: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let tab = self
            .workspace
            .selected_tab()
            .filter(|tab| tab.is_clean_untitled())
            .unwrap_or_else(|| self.workspace.add_empty_tab(true));
        tab.text_buffer().set_text(editable_text);
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        self.workspace.refresh_selected_state();
        match right {
            CompareSlot::File(file) => tab.start_compare_with_file(&file, Rc::clone(callback)),
            CompareSlot::Text(text) => tab.start_compare_with_text(&text, Rc::clone(callback)),
            _ => callback(Err(AppError::Cancelled)),
        }
        self.workspace.refresh_selected_state();
    }

    fn open_two_file_compare(
        self: &Rc<Self>,
        editable: &gio::File,
        reference: &gio::File,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let tab = if self.workspace.tab_view.n_pages() == 1 {
            self.workspace
                .ordered_tabs()
                .first()
                .filter(|tab| tab.is_clean_untitled())
                .cloned()
                .unwrap_or_else(|| self.workspace.add_empty_tab(true))
        } else {
            self.workspace.add_empty_tab(true)
        };
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        let weak = Rc::downgrade(self);
        let reference = reference.clone();
        let tab_for_failure = tab.clone();
        let callback_for_open = Rc::clone(callback);
        tab.clone().load_file(
            &self.shell,
            editable,
            Rc::new(move |result| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(uri) => {
                        controller.workspace.remember_recent_uri(&uri);
                        controller.workspace.persist_session_state_if_needed();
                        tab.start_compare_with_file(&reference, Rc::clone(&callback_for_open));
                        controller.workspace.refresh_selected_state();
                    }
                    Err(error) => {
                        if tab_for_failure.is_clean_untitled() {
                            controller.workspace.close_tab_if_clean(&tab_for_failure);
                        }
                        controller.workspace.refresh_selected_state();
                        callback_for_open(Err(error));
                    }
                }
            }),
        );
    }

    fn open_file_text_compare(
        self: &Rc<Self>,
        editable: &gio::File,
        reference_text: &str,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let tab = self.workspace.add_empty_tab(true);
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        let weak = Rc::downgrade(self);
        let reference_text = reference_text.to_string();
        let tab_for_failure = tab.clone();
        let callback_for_open = Rc::clone(callback);
        tab.clone().load_file(
            &self.shell,
            editable,
            Rc::new(move |result| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                match result {
                    Ok(uri) => {
                        controller.workspace.remember_recent_uri(&uri);
                        controller.workspace.persist_session_state_if_needed();
                        tab.start_compare_with_text(&reference_text, Rc::clone(&callback_for_open));
                        controller.workspace.refresh_selected_state();
                    }
                    Err(error) => {
                        if tab_for_failure.is_clean_untitled() {
                            controller.workspace.close_tab_if_clean(&tab_for_failure);
                        }
                        controller.workspace.refresh_selected_state();
                        callback_for_open(Err(error));
                    }
                }
            }),
        );
    }

    fn wrap_compare_callback(
        self: &Rc<Self>,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) -> Rc<dyn Fn(Result<(), AppError>)> {
        let weak = Rc::downgrade(self);
        Rc::new(move |result| {
            if let Some(controller) = weak.upgrade() {
                controller.workspace.refresh_selected_state();
                if let Err(error) = &result {
                    dialogs::present_error(&controller.shell, error);
                }
            }
            callback(result);
        })
    }
}
