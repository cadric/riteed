use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::editor_tab::EditorTab;
use crate::settings::AppSettings;
use crate::workspace::Workspace;

pub(crate) type PrintRunner = Rc<dyn Fn(&crate::document_print::PrintJob<'_>)>;
pub(crate) type PreviewRunner = Rc<dyn Fn(&adw::ApplicationWindow, &sourceview5::View, &str, &str)>;

pub(crate) struct DocumentToolsController {
    parent: adw::ApplicationWindow,
    workspace: Rc<Workspace>,
    settings: AppSettings,
    print_session: crate::document_print::PrintSession,
    statistics_action: gio::SimpleAction,
    print_action: gio::SimpleAction,
    preview_action: gio::SimpleAction,
    print_runner: RefCell<PrintRunner>,
    preview_runner: RefCell<PreviewRunner>,
}

impl DocumentToolsController {
    #[must_use]
    pub(crate) fn new(
        parent: &adw::ApplicationWindow,
        workspace: &Rc<Workspace>,
        settings: &AppSettings,
    ) -> Rc<Self> {
        let statistics_action = gio::SimpleAction::new("document-statistics", None);
        let print_action = gio::SimpleAction::new("print", None);
        let preview_action = gio::SimpleAction::new("print-preview", None);
        parent.add_action(&statistics_action);
        parent.add_action(&print_action);
        parent.add_action(&preview_action);

        let controller = Rc::new(Self {
            parent: parent.clone(),
            workspace: Rc::clone(workspace),
            settings: settings.clone(),
            print_session: crate::document_print::PrintSession::default(),
            statistics_action,
            print_action,
            preview_action,
            print_runner: RefCell::new(default_print_runner()),
            preview_runner: RefCell::new(Rc::new(|_, _, _, _| {})),
        });
        controller
            .preview_runner
            .replace(default_preview_runner(Rc::downgrade(&controller)));
        controller.install_callbacks();
        let selected = workspace.selected_tab();
        controller.sync_actions(selected.as_ref());
        let weak = Rc::downgrade(&controller);
        workspace.set_document_tools_sync_handler(Rc::new(move |selected| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_actions(selected.as_ref());
            }
        }));
        controller
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.statistics_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.present_statistics();
            }
        });

        let weak = Rc::downgrade(self);
        self.print_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.print_document();
            }
        });

        let weak = Rc::downgrade(self);
        self.preview_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.preview_document();
            }
        });
    }

    pub(crate) fn sync_current(&self) {
        let selected = self.workspace.selected_tab();
        self.sync_actions(selected.as_ref());
    }

    fn sync_actions(&self, selected: Option<&Rc<EditorTab>>) {
        let statistics_enabled = selected.is_some_and(|tab| tab.is_document() && !tab.is_loading());
        let print_enabled = selected
            .is_some_and(|tab| tab.is_document() && !tab.is_loading() && !tab.is_compare_active());
        self.statistics_action.set_enabled(statistics_enabled);
        self.print_action.set_enabled(print_enabled);
        self.preview_action.set_enabled(print_enabled);
    }

    fn present_statistics(&self) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if !tab.is_document() || tab.is_loading() {
            return;
        }
        crate::document_statistics::present(&self.parent, &tab);
    }

    fn print_document(self: &Rc<Self>) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if print_needs_markdown_confirmation(tab.is_markdown_preview_active()) {
            let dialog = adw::AlertDialog::new(
                Some(&gettext("Print Markdown Source?")),
                Some(&gettext(
                    "Printing the formatted preview is not supported yet. The raw Markdown source will be printed.",
                )),
            );
            dialog.add_response("cancel", &pgettext("dialog button", "Cancel"));
            dialog.add_response("print", &pgettext("dialog button", "Print Source"));
            dialog.set_default_response(Some("print"));
            dialog.set_close_response("cancel");
            let weak = Rc::downgrade(self);
            dialog.connect_response(Some("print"), move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.start_print();
                }
            });
            dialog.present(Some(&self.parent));
            return;
        }
        self.start_print();
    }

    fn start_print(&self) {
        let body_font = crate::document_print::print_body_font_name(&self.settings.editor_font());
        self.print_document_with_font(&body_font);
    }

    pub(crate) fn print_document_with_font(&self, body_font: &str) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if !tab.is_document() || tab.is_loading() || tab.is_compare_active() {
            return;
        }
        let runner = self.print_runner.borrow().clone();
        let view = tab.text_view();
        let title = tab.title();
        runner(&crate::document_print::PrintJob {
            parent: &self.parent,
            view: &view,
            title: &title,
            body_font,
            session: &self.print_session,
        });
    }

    fn preview_document(&self) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if !tab.is_document() || tab.is_loading() || tab.is_compare_active() {
            return;
        }
        let runner = self.preview_runner.borrow().clone();
        let view = tab.text_view();
        let title = tab.title();
        let body_font = crate::document_print::print_body_font_name(&self.settings.editor_font());
        runner(&self.parent, &view, &title, &body_font);
    }

    #[cfg(test)]
    pub(crate) fn set_print_runner_for_tests(&self, runner: PrintRunner) {
        self.print_runner.replace(runner);
    }

    #[cfg(test)]
    pub(crate) fn set_preview_runner_for_tests(&self, runner: PreviewRunner) {
        self.preview_runner.replace(runner);
    }

    #[cfg(test)]
    pub(crate) fn actions_enabled_for_tests(&self) -> (bool, bool) {
        (
            self.statistics_action.is_enabled(),
            self.print_action.is_enabled(),
        )
    }
}

fn default_print_runner() -> PrintRunner {
    Rc::new(crate::document_print::run_print)
}

fn print_needs_markdown_confirmation(preview_active: bool) -> bool {
    preview_active
}

fn default_preview_runner(controller: std::rc::Weak<DocumentToolsController>) -> PreviewRunner {
    Rc::new(move |parent, view, title, body_font| {
        let controller = controller.clone();
        crate::document_print_preview::present_preview(
            parent,
            view,
            title,
            body_font,
            Rc::new(move |chosen_font: &str| {
                if let Some(controller) = controller.upgrade() {
                    controller.print_document_with_font(chosen_font);
                }
            }),
        );
    })
}

#[cfg(test)]
mod print_confirmation_tests {
    #[test]
    fn preview_mode_requires_confirmation() {
        assert!(super::print_needs_markdown_confirmation(true));
        assert!(!super::print_needs_markdown_confirmation(false));
    }
}
