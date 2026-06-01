use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;
use crate::workspace::Workspace;

pub(crate) type PrintRunner = Rc<dyn Fn(&adw::ApplicationWindow, &sourceview5::View, &str)>;

pub(crate) struct DocumentToolsController {
    parent: adw::ApplicationWindow,
    workspace: Rc<Workspace>,
    statistics_action: gio::SimpleAction,
    print_action: gio::SimpleAction,
    print_runner: RefCell<PrintRunner>,
}

impl DocumentToolsController {
    #[must_use]
    pub(crate) fn new(parent: &adw::ApplicationWindow, workspace: &Rc<Workspace>) -> Rc<Self> {
        let statistics_action = gio::SimpleAction::new("document-statistics", None);
        let print_action = gio::SimpleAction::new("print", None);
        parent.add_action(&statistics_action);
        parent.add_action(&print_action);

        let controller = Rc::new(Self {
            parent: parent.clone(),
            workspace: Rc::clone(workspace),
            statistics_action,
            print_action,
            print_runner: RefCell::new(default_print_runner()),
        });
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

    fn print_document(&self) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if !tab.is_document() || tab.is_loading() || tab.is_compare_active() {
            return;
        }
        let runner = self.print_runner.borrow().clone();
        let view = tab.text_view();
        let title = tab.title();
        runner(&self.parent, &view, &title);
    }

    #[cfg(test)]
    pub(crate) fn set_print_runner_for_tests(&self, runner: PrintRunner) {
        self.print_runner.replace(runner);
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
