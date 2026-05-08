use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::prelude::*;

use crate::git_status::GitStatusEntry;
use crate::settings::{AppSettings, SourceControlViewMode};
use crate::source_control::SourceControlState;
#[cfg(test)]
use crate::source_control::SourceStateRef;
use crate::source_control::list_view::SourceControlList;
use crate::source_control::tree_view::SourceControlTree;

pub(super) struct SourceControlViews {
    root: gtk4::Box,
    stack: gtk4::Stack,
    tree: SourceControlTree,
    list: SourceControlList,
    tree_button: gtk4::ToggleButton,
    list_button: gtk4::ToggleButton,
    syncing: Rc<Cell<bool>>,
    last_entries: RefCell<Vec<GitStatusEntry>>,
}

impl SourceControlViews {
    #[must_use]
    pub(super) fn new(settings: &AppSettings) -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        root.set_vexpand(true);
        let tree_button = view_button(&pgettext("source control view", "Tree"));
        let list_button = view_button(&pgettext("source control view", "List"));
        list_button.set_group(Some(&tree_button));
        let switcher = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        switcher.add_css_class("linked");
        switcher.append(&tree_button);
        switcher.append(&list_button);
        root.append(&switcher);

        let tree = SourceControlTree::new();
        let list = SourceControlList::new();
        let tree_view = tree.widget();
        let list_view = list.widget();
        let tree_scroller = scrollable_view(&tree_view);
        let list_scroller = scrollable_view(&list_view);
        let stack = gtk4::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&tree_scroller, Some("tree"));
        stack.add_named(&list_scroller, Some("list"));
        root.append(&stack);

        let views = Self {
            root,
            stack,
            tree,
            list,
            tree_button,
            list_button,
            syncing: Rc::new(Cell::new(false)),
            last_entries: RefCell::new(Vec::new()),
        };
        views.set_mode(settings.source_control_view_mode());
        views.install_mode_callbacks(settings);
        views
    }

    #[must_use]
    pub(super) fn widget(&self) -> gtk4::Box {
        self.root.clone()
    }

    pub(super) fn connect_activation(&self, weak: Weak<RefCell<SourceControlState>>) {
        self.tree.connect_activation(weak.clone());
        self.list.connect_activation(weak);
    }

    pub(super) fn rebuild(&self, entries: &[GitStatusEntry]) {
        let mut last_entries = self.last_entries.borrow_mut();
        if last_entries.as_slice() == entries {
            return;
        }
        *last_entries = entries.to_vec();
        self.tree.rebuild(entries);
        self.list.rebuild(entries);
    }

    #[cfg(test)]
    pub(super) fn row_count_for_tests(&self) -> usize {
        match self.mode_for_tests() {
            SourceControlViewMode::Tree => self.tree.row_count_for_tests(),
            SourceControlViewMode::List => self.list.row_count_for_tests(),
        }
    }

    #[cfg(test)]
    pub(super) fn activate_path_for_tests(&self, path: &str) -> Option<(gtk4::ListView, u32)> {
        match self.mode_for_tests() {
            SourceControlViewMode::Tree => self.tree.activation_for_path_for_tests(path),
            SourceControlViewMode::List => self.list.activation_for_path_for_tests(path),
        }
    }

    #[cfg(test)]
    pub(super) fn set_mode_for_tests(state: &SourceStateRef, mode: SourceControlViewMode) {
        let state = state.borrow();
        state.settings.set_source_control_view_mode(mode);
        state.views.set_mode(mode);
    }

    #[cfg(test)]
    pub(super) fn mode_for_tests(&self) -> SourceControlViewMode {
        if self.stack.visible_child_name().as_deref() == Some("list") {
            SourceControlViewMode::List
        } else {
            SourceControlViewMode::Tree
        }
    }

    fn install_mode_callbacks(&self, settings: &AppSettings) {
        let settings_for_tree = settings.clone();
        let stack_for_tree = self.stack.clone();
        let syncing_for_tree = Rc::clone(&self.syncing);
        self.tree_button.connect_toggled(move |button| {
            if syncing_for_tree.get() || !button.is_active() {
                return;
            }
            stack_for_tree.set_visible_child_name("tree");
            settings_for_tree.set_source_control_view_mode(SourceControlViewMode::Tree);
        });

        let settings_for_list = settings.clone();
        let stack_for_list = self.stack.clone();
        let syncing_for_list = Rc::clone(&self.syncing);
        self.list_button.connect_toggled(move |button| {
            if syncing_for_list.get() || !button.is_active() {
                return;
            }
            stack_for_list.set_visible_child_name("list");
            settings_for_list.set_source_control_view_mode(SourceControlViewMode::List);
        });
    }

    fn set_mode(&self, mode: SourceControlViewMode) {
        self.syncing.set(true);
        match mode {
            SourceControlViewMode::Tree => {
                self.tree_button.set_active(true);
                self.stack.set_visible_child_name("tree");
            }
            SourceControlViewMode::List => {
                self.list_button.set_active(true);
                self.stack.set_visible_child_name("list");
            }
        }
        self.syncing.set(false);
    }
}

fn view_button(label: &str) -> gtk4::ToggleButton {
    let button = gtk4::ToggleButton::with_label(label);
    button.update_property(&[Property::Label(label)]);
    button
}

fn scrollable_view(child: &gtk4::ListView) -> gtk4::ScrolledWindow {
    let scroller = gtk4::ScrolledWindow::builder()
        .child(child)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();
    scroller.set_vexpand(true);
    scroller
}
