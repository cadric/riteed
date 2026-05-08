use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::prelude::*;

type ScopeChangedHandler = Rc<dyn Fn(SearchScope)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchScope {
    Document,
    Project,
}

pub(super) struct ScopeBar {
    root: gtk4::Box,
    document_button: gtk4::ToggleButton,
    project_button: gtk4::ToggleButton,
    syncing: Rc<Cell<bool>>,
    handler: Rc<RefCell<Option<ScopeChangedHandler>>>,
}

impl ScopeBar {
    #[must_use]
    pub(super) fn new() -> Self {
        let document_label = pgettext("search scope", "Document");
        let project_label = pgettext("search scope", "Project");
        let document_button = gtk4::ToggleButton::with_label(&document_label);
        let project_button = gtk4::ToggleButton::with_label(&project_label);
        project_button.set_group(Some(&document_button));
        document_button.update_property(&[Property::Label(&document_label)]);
        project_button.update_property(&[Property::Label(&project_label)]);
        document_button.set_tooltip_text(Some(&document_label));
        project_button.set_tooltip_text(Some(&project_label));
        document_button.set_active(true);

        let root = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        root.add_css_class("linked");
        root.append(&document_button);
        root.append(&project_button);

        let bar = Self {
            root,
            document_button,
            project_button,
            syncing: Rc::new(Cell::new(false)),
            handler: Rc::new(RefCell::new(None)),
        };
        bar.install_callbacks();
        bar
    }

    #[must_use]
    pub(super) fn widget(&self) -> &gtk4::Box {
        &self.root
    }

    pub(super) fn reset_to_document(&self) {
        self.set_scope_silent(SearchScope::Document);
    }

    pub(super) fn force_project(&self) {
        self.set_scope_silent(SearchScope::Project);
    }

    #[must_use]
    pub(crate) fn current_scope(&self) -> SearchScope {
        if self.project_button.is_active() {
            SearchScope::Project
        } else {
            SearchScope::Document
        }
    }

    pub(super) fn connect_scope_changed(&self, handler: ScopeChangedHandler) {
        self.handler.replace(Some(handler));
    }

    fn install_callbacks(&self) {
        let syncing = Rc::clone(&self.syncing);
        let handler = Rc::clone(&self.handler);
        self.document_button.connect_toggled(move |button| {
            if syncing.get() || !button.is_active() {
                return;
            }
            if let Some(handler) = handler.borrow().as_ref() {
                handler(SearchScope::Document);
            }
        });

        let syncing = Rc::clone(&self.syncing);
        let handler = Rc::clone(&self.handler);
        self.project_button.connect_toggled(move |button| {
            if syncing.get() || !button.is_active() {
                return;
            }
            if let Some(handler) = handler.borrow().as_ref() {
                handler(SearchScope::Project);
            }
        });
    }

    fn set_scope_silent(&self, scope: SearchScope) {
        self.syncing.set(true);
        match scope {
            SearchScope::Document => self.document_button.set_active(true),
            SearchScope::Project => self.project_button.set_active(true),
        }
        self.syncing.set(false);
    }
}
