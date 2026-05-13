mod actions;
mod dialog;
mod paste_text;
mod settings_actions;

use std::cell::Cell;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::dialogs;
use crate::editor_tab::EditorTab;
use crate::error::AppError;
use crate::workspace::{OpenSource, Workspace};

use dialog::CompareSlot;
use settings_actions::CompareSettingsActions;

pub(crate) struct WindowCompareController {
    shell: adw::ApplicationWindow,
    workspace: Rc<Workspace>,
    compare_action: gio::SimpleAction,
    refresh_reference_action: gio::SimpleAction,
    exit_action: gio::SimpleAction,
    next_action: gio::SimpleAction,
    prev_action: gio::SimpleAction,
    open_reviewed_file_action: gio::SimpleAction,
    change_list_action: gio::SimpleAction,
    reveal_above_action: gio::SimpleAction,
    reveal_below_action: gio::SimpleAction,
    reveal_all_action: gio::SimpleAction,
    refresh_review_action: gio::SimpleAction,
    compare_settings_actions: CompareSettingsActions,
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
            open_reviewed_file_action: gio::SimpleAction::new("open-reviewed-file", None),
            change_list_action: gio::SimpleAction::new("compare-change-list", None),
            reveal_above_action: gio::SimpleAction::new("compare-reveal-above", None),
            reveal_below_action: gio::SimpleAction::new("compare-reveal-below", None),
            reveal_all_action: gio::SimpleAction::new("compare-reveal-all", None),
            refresh_review_action: gio::SimpleAction::new("compare-refresh-review", None),
            compare_settings_actions: CompareSettingsActions::new(&workspace.settings),
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
        self.dispatch_to_selected_tab(EditorTab::review_next_change, EditorTab::compare_next_diff);
    }

    pub(crate) fn previous_diff(&self) {
        self.dispatch_to_selected_tab(
            EditorTab::review_previous_change,
            EditorTab::compare_previous_diff,
        );
    }

    pub(crate) fn reveal_above(&self) {
        self.dispatch_to_selected_tab(
            EditorTab::review_reveal_above,
            EditorTab::compare_reveal_above,
        );
    }

    pub(crate) fn reveal_below(&self) {
        self.dispatch_to_selected_tab(
            EditorTab::review_reveal_below,
            EditorTab::compare_reveal_below,
        );
    }

    pub(crate) fn reveal_all(&self) {
        self.dispatch_to_selected_tab(EditorTab::review_reveal_all, EditorTab::compare_reveal_all);
    }

    fn dispatch_to_selected_tab(&self, review: fn(&EditorTab), compare: fn(&EditorTab)) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if tab.kind() == crate::editor_tab::TabKind::GitReview {
            review(&tab);
        } else {
            compare(&tab);
        }
    }

    pub(crate) fn present_change_list(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.present_change_list();
        }
    }

    pub(crate) fn open_reviewed_file(self: &Rc<Self>) {
        let Some(file) = self
            .workspace
            .selected_tab()
            .and_then(|tab| tab.current_review_open_target())
        else {
            return;
        };
        self.workspace
            .request_open_files(vec![file], OpenSource::SourceControl);
    }

    pub(crate) fn refresh_review(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            self.workspace.refresh_review_tab(&tab);
        }
    }

    pub(crate) fn refresh_action_state(&self) {
        self.sync_actions(self.workspace.selected_tab().as_deref());
    }

    #[cfg(test)]
    pub(crate) fn compare_two_files_for_tests(
        self: &Rc<Self>,
        reference: &gio::File,
        current: &gio::File,
    ) {
        let callback: Rc<dyn Fn(Result<(), AppError>)> = Rc::new(|_result| {});
        self.open_two_file_compare(current, reference, &callback);
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

    fn handle_compare_result(&self, result: Result<(), AppError>) {
        self.workspace.refresh_selected_state();
        if let Err(error) = result {
            dialogs::present_error(&self.shell, &error);
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
        paste_text::show_paste_text_dialog(
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

    #[cfg(test)]
    pub(crate) fn present_compare_dialog_for_tests(self: &Rc<Self>) -> adw::Dialog {
        dialog::present_compare_dialog_for_tests(self)
    }

    #[cfg(test)]
    pub(crate) fn present_paste_text_dialog_for_tests(&self) -> adw::Dialog {
        paste_text::show_paste_text_dialog_for_tests(&self.shell)
    }

    fn start_compare_for_selected_tab(self: &Rc<Self>, reference: CompareSlot) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        let callback = self.wrap_compare_callback(Rc::new(|_result| {}));
        self.start_compare_for_current_document(&tab, reference, &callback);
    }

    fn start_compare_from_dialog(
        self: &Rc<Self>,
        left: CompareSlot,
        right: CompareSlot,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let callback = self.wrap_compare_callback(callback);
        match right {
            CompareSlot::CurrentDocument(tab) => {
                self.start_compare_for_current_document(&tab, left, &callback);
            }
            CompareSlot::File(file) => {
                self.start_compare_for_file(&file, left, &callback);
            }
            CompareSlot::Text(text) => {
                self.start_compare_for_text(&text, left, &callback);
            }
            CompareSlot::None | CompareSlot::SavedVersion => {
                callback(Err(AppError::Cancelled));
            }
        }
    }

    fn start_compare_for_current_document(
        self: &Rc<Self>,
        tab: &Rc<EditorTab>,
        reference: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        match reference {
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
        current: &gio::File,
        reference: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        match reference {
            CompareSlot::File(reference) => {
                self.open_two_file_compare(current, &reference, callback);
            }
            CompareSlot::Text(text) => {
                self.open_file_text_compare(current, &text, callback);
            }
            _ => {
                callback(Err(AppError::Cancelled));
            }
        }
    }

    fn start_compare_for_text(
        self: &Rc<Self>,
        current_text: &str,
        reference: CompareSlot,
        callback: &Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let tab = self
            .workspace
            .selected_tab()
            .filter(|tab| tab.is_clean_untitled())
            .unwrap_or_else(|| self.workspace.add_empty_tab(true));
        tab.text_buffer().set_text(current_text);
        if let Some(page) = tab.page() {
            self.workspace.tab_view.set_selected_page(&page);
        }
        self.workspace.refresh_selected_state();
        match reference {
            CompareSlot::File(file) => tab.start_compare_with_file(&file, Rc::clone(callback)),
            CompareSlot::Text(text) => tab.start_compare_with_text(&text, Rc::clone(callback)),
            _ => callback(Err(AppError::Cancelled)),
        }
        self.workspace.refresh_selected_state();
    }

    fn open_two_file_compare(
        self: &Rc<Self>,
        current: &gio::File,
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
            current,
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
        current: &gio::File,
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
            current,
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
