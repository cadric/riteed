use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::dialogs;
use crate::editor_tab::EditorTab;
use crate::error::AppError;
use crate::workspace::Workspace;

pub(crate) struct WindowCompareController {
    shell: adw::ApplicationWindow,
    workspace: Rc<Workspace>,
    compare_with_disk_action: gio::SimpleAction,
    compare_with_file_action: gio::SimpleAction,
    compare_two_files_action: gio::SimpleAction,
    refresh_reference_action: gio::SimpleAction,
    exit_action: gio::SimpleAction,
    next_action: gio::SimpleAction,
    prev_action: gio::SimpleAction,
}

impl WindowCompareController {
    #[must_use]
    pub(crate) fn new(shell: &adw::ApplicationWindow, workspace: &Rc<Workspace>) -> Rc<Self> {
        let controller = Rc::new(Self {
            shell: shell.clone(),
            workspace: Rc::clone(workspace),
            compare_with_disk_action: gio::SimpleAction::new("compare-with-disk", None),
            compare_with_file_action: gio::SimpleAction::new("compare-with-file", None),
            compare_two_files_action: gio::SimpleAction::new("compare-two-files", None),
            refresh_reference_action: gio::SimpleAction::new("compare-refresh-reference", None),
            exit_action: gio::SimpleAction::new("compare-exit", None),
            next_action: gio::SimpleAction::new("diff-next", None),
            prev_action: gio::SimpleAction::new("diff-prev", None),
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

    pub(crate) fn request_compare_with_disk(self: &Rc<Self>) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        let weak = Rc::downgrade(self);
        tab.start_compare_with_disk(Rc::new(move |result| {
            if let Some(controller) = weak.upgrade() {
                controller.handle_compare_result(result);
            }
        }));
    }

    pub(crate) fn request_compare_with_file_dialog(self: &Rc<Self>) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Compare With File"))
            .accept_label(pgettext("file dialog action", "Compare"))
            .modal(true)
            .build();
        apply_text_filters(&dialog);
        let parent = self.shell.clone();
        let weak = Rc::downgrade(self);
        dialog.open(Some(&parent), None::<&gio::Cancellable>, move |result| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(file) => controller.start_compare_with_file(&tab, &file),
                Err(error) if error.matches(gtk4::DialogError::Dismissed) => {}
                Err(error) => dialogs::present_error(&controller.shell, &AppError::from(error)),
            }
        });
    }

    pub(crate) fn request_compare_two_files_dialog(self: &Rc<Self>) {
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Compare Two Files"))
            .accept_label(pgettext("file dialog action", "Compare"))
            .modal(true)
            .build();
        apply_text_filters(&dialog);
        let parent = self.shell.clone();
        let weak = Rc::downgrade(self);
        dialog.open_multiple(Some(&parent), None::<&gio::Cancellable>, move |result| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(model) => {
                    let files = files_from_model(&model);
                    if files.len() < 2 {
                        dialogs::present_message(
                            &controller.shell,
                            &gettext("Choose Two Files to Compare"),
                            &gettext("Select two local text files before starting a comparison."),
                        );
                        return;
                    }
                    controller.open_two_file_compare(&files[0], &files[1]);
                }
                Err(error) if error.matches(gtk4::DialogError::Dismissed) => {}
                Err(error) => dialogs::present_error(&controller.shell, &AppError::from(error)),
            }
        });
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
        self.open_two_file_compare(editable, reference);
    }

    #[cfg(test)]
    pub(crate) fn action_states_for_tests(&self) -> (bool, bool, bool, bool, bool, bool, bool) {
        (
            self.compare_with_disk_action.is_enabled(),
            self.compare_with_file_action.is_enabled(),
            self.compare_two_files_action.is_enabled(),
            self.refresh_reference_action.is_enabled(),
            self.exit_action.is_enabled(),
            self.next_action.is_enabled(),
            self.prev_action.is_enabled(),
        )
    }

    fn install_actions(&self) {
        self.shell.add_action(&self.compare_with_disk_action);
        self.shell.add_action(&self.compare_with_file_action);
        self.shell.add_action(&self.compare_two_files_action);
        self.shell.add_action(&self.refresh_reference_action);
        self.shell.add_action(&self.exit_action);
        self.shell.add_action(&self.next_action);
        self.shell.add_action(&self.prev_action);
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.compare_with_disk_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.request_compare_with_disk();
            }
        });
        let weak = Rc::downgrade(self);
        self.compare_with_file_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.request_compare_with_file_dialog();
            }
        });
        let weak = Rc::downgrade(self);
        self.compare_two_files_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.request_compare_two_files_dialog();
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
    }

    fn sync_actions(&self, selected: Option<&EditorTab>) {
        let active = selected.is_some_and(EditorTab::is_compare_active);
        self.compare_with_disk_action
            .set_enabled(selected.is_some_and(|tab| {
                tab.has_saved_local_uri() && !tab.is_loading() && !tab.is_compare_active()
            }));
        self.compare_with_file_action
            .set_enabled(selected.is_some_and(|tab| !tab.is_loading() && !tab.is_compare_active()));
        self.compare_two_files_action.set_enabled(true);
        self.refresh_reference_action.set_enabled(active);
        self.exit_action.set_enabled(active);
        self.next_action.set_enabled(active);
        self.prev_action.set_enabled(active);
    }

    fn start_compare_with_file(&self, tab: &Rc<EditorTab>, file: &gio::File) {
        let weak_workspace = Rc::downgrade(&self.workspace);
        let shell = self.shell.clone();
        tab.start_compare_with_file(
            file,
            Rc::new(move |result| {
                if let Some(workspace) = weak_workspace.upgrade() {
                    workspace.refresh_selected_state();
                }
                if let Err(error) = result {
                    dialogs::present_error(&shell, &error);
                }
            }),
        );
    }

    fn open_two_file_compare(self: &Rc<Self>, editable: &gio::File, reference: &gio::File) {
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
                        controller.start_compare_with_file(&tab, &reference);
                    }
                    Err(error) => {
                        if tab_for_failure.is_clean_untitled() {
                            controller.workspace.close_tab_if_clean(&tab_for_failure);
                        }
                        dialogs::present_error(&controller.shell, &error);
                    }
                }
            }),
        );
    }

    fn handle_compare_result(&self, result: Result<(), AppError>) {
        self.workspace.refresh_selected_state();
        if let Err(error) = result {
            dialogs::present_error(&self.shell, &error);
        }
    }
}

fn files_from_model(model: &gio::ListModel) -> Vec<gio::File> {
    (0..model.n_items())
        .filter_map(|position| model.item(position).and_downcast::<gio::File>())
        .collect()
}

fn apply_text_filters(dialog: &gtk4::FileDialog) {
    let text_filter = gtk4::FileFilter::new();
    text_filter.set_name(Some(&pgettext("file filter", "Plain Text Files")));
    text_filter.add_mime_type("text/plain");
    text_filter.add_suffix("txt");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&any_filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&text_filter));
}
