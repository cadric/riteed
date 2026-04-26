use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::{gio, glib, pango, prelude::*};

use crate::git_status::{GitActionState, GitStatusEntry};
use crate::source_control::SourceControlState;
use crate::source_control::actions::{self, GitRowAction};
use crate::source_control::tree_model::{
    SourceControlNode, build_root_store, file_basename, node_for_position, restore_expanded_paths,
    restore_selected_node, snapshot_expanded_paths, snapshot_selected_node,
};

pub(super) struct SourceControlTree {
    root_store: gio::ListStore,
    tree_model: gtk4::TreeListModel,
    selection: gtk4::SingleSelection,
    list_view: gtk4::ListView,
    activation_guard: Rc<Cell<bool>>,
    state_weak: Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
}

impl SourceControlTree {
    #[must_use]
    pub(super) fn new() -> Self {
        let root_store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let tree_model = gtk4::TreeListModel::new(root_store.clone(), false, false, move |item| {
            let Ok(item) = item.clone().downcast::<glib::BoxedAnyObject>() else {
                return None;
            };
            let Ok(borrowed) = item.try_borrow::<SourceControlNode>() else {
                return None;
            };
            match &*borrowed {
                SourceControlNode::Folder { children_store, .. } => {
                    Some(children_store.clone().upcast())
                }
                SourceControlNode::File { .. } => None,
            }
        });
        let selection = gtk4::SingleSelection::new(Some(tree_model.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let state_weak = Rc::new(RefCell::new(Weak::new()));
        let activation_guard = Rc::new(Cell::new(false));
        let factory = create_factory(&state_weak, &activation_guard);
        let list_view = gtk4::ListView::new(Some(selection.clone()), Some(factory));
        list_view.set_single_click_activate(true);
        list_view.set_enable_rubberband(false);
        list_view.set_vexpand(true);

        Self {
            root_store,
            tree_model,
            selection,
            list_view,
            activation_guard,
            state_weak,
        }
    }

    #[must_use]
    pub(super) fn widget(&self) -> gtk4::ListView {
        self.list_view.clone()
    }

    pub(super) fn connect_activation(&self, weak: Weak<RefCell<SourceControlState>>) {
        *self.state_weak.borrow_mut() = weak.clone();
        let model = self.tree_model.clone();
        let activation_guard = Rc::clone(&self.activation_guard);
        self.list_view.connect_activate(move |_, position| {
            if activation_guard.get() {
                activation_guard.set(false);
                return;
            }
            activate_position(&model, position, &weak);
        });
    }

    pub(super) fn rebuild(&self, entries: &[GitStatusEntry]) {
        let expanded = snapshot_expanded_paths(&self.tree_model);
        let selected = snapshot_selected_node(&self.selection);
        let next_store = build_root_store(entries);
        self.root_store.remove_all();
        for position in 0..next_store.n_items() {
            if let Some(item) = next_store.item(position) {
                self.root_store.append(&item);
            }
        }
        restore_expanded_paths(&self.tree_model, &expanded);
        restore_selected_node(&self.tree_model, &self.selection, selected);
    }

    #[cfg(test)]
    pub(super) fn row_count_for_tests(&self) -> usize {
        self.tree_model.n_items() as usize
    }

    #[cfg(test)]
    pub(super) fn activation_for_path_for_tests(
        &self,
        path: &str,
    ) -> Option<(gtk4::ListView, u32)> {
        for position in 0..self.tree_model.n_items() {
            let Some((_row, node)) = node_for_position(&self.tree_model, position) else {
                continue;
            };
            match node {
                SourceControlNode::File { entry } => {
                    if entry.path.as_utf8() == Some(path) {
                        return Some((self.list_view.clone(), position));
                    }
                }
                SourceControlNode::Folder { full_path, .. } => {
                    if full_path == path {
                        return Some((self.list_view.clone(), position));
                    }
                }
            }
        }
        None
    }
}

fn create_factory(
    state_weak: &Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    activation_guard: &Rc<Cell<bool>>,
) -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    let state_weak_for_setup = Rc::clone(state_weak);
    let activation_guard = Rc::clone(activation_guard);
    factory.connect_setup(move |_, object| {
        setup_row(object, &state_weak_for_setup, &activation_guard);
    });
    factory.connect_bind(move |_, object| bind_row(object));
    factory.connect_unbind(unbind_row);
    factory
}

fn setup_row(
    object: &glib::Object,
    state_weak: &Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    activation_guard: &Rc<Cell<bool>>,
) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let expander = gtk4::TreeExpander::new();
    expander.set_indent_for_depth(true);

    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row_box.add_css_class("riteed-git-row");
    row_box.set_margin_top(3);
    row_box.set_margin_bottom(3);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);

    let icon = gtk4::Image::new();
    icon.set_pixel_size(16);
    row_box.append(&icon);

    let label = gtk4::Label::new(None);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(pango::EllipsizeMode::End);
    row_box.append(&label);

    let staged = gtk4::Label::new(Some("S"));
    staged.add_css_class("caption");
    staged.add_css_class("riteed-git-staged");
    row_box.append(&staged);

    let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    actions_box.add_css_class("riteed-git-row-actions");
    actions_box.append(&action_button(
        "list-add-symbolic",
        &pgettext("git action tooltip", "Stage File"),
        state_weak,
        activation_guard,
        GitRowAction::Stage,
    ));
    actions_box.append(&action_button(
        "list-remove-symbolic",
        &pgettext("git action tooltip", "Unstage File"),
        state_weak,
        activation_guard,
        GitRowAction::Unstage,
    ));
    row_box.append(&actions_box);

    let status = gtk4::Label::new(None);
    status.add_css_class("caption");
    status.add_css_class("riteed-git-status-badge");
    row_box.append(&status);

    expander.set_child(Some(&row_box));
    list_item.set_child(Some(&expander));
}

fn action_button(
    icon_name: &str,
    label: &str,
    state_weak: &Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    activation_guard: &Rc<Cell<bool>>,
    action: GitRowAction,
) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(label)
        .build();
    button.add_css_class("flat");
    button.update_property(&[Property::Label(label)]);
    let state_weak = Rc::clone(state_weak);
    let activation_guard = Rc::clone(activation_guard);
    button.connect_clicked(move |button| {
        activation_guard.set(true);
        let Some(state) = state_weak.borrow().upgrade() else {
            let activation_guard = Rc::clone(&activation_guard);
            let _ = glib::idle_add_local_once(move || activation_guard.set(false));
            return;
        };
        let Some(path) = path_for_button(button) else {
            let activation_guard = Rc::clone(&activation_guard);
            let _ = glib::idle_add_local_once(move || activation_guard.set(false));
            return;
        };
        actions::run_path_action(&state, &path, action);
        let activation_guard = Rc::clone(&activation_guard);
        let _ = glib::idle_add_local_once(move || activation_guard.set(false));
    });
    button
}

fn bind_row(object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(row) = list_item
        .item()
        .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
    else {
        return;
    };
    let Some(expander) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
    else {
        return;
    };
    expander.set_list_row(Some(&row));
    let Some(widgets) = row_widgets(&expander) else {
        return;
    };
    let Some(node) = node_for_position_from_row(&row) else {
        return;
    };
    bind_node(&widgets, node);
}

fn node_for_position_from_row(row: &gtk4::TreeListRow) -> Option<SourceControlNode> {
    crate::source_control::tree_model::node_for_row(row)
}

fn bind_node(widgets: &RowWidgets, node: SourceControlNode) {
    match node {
        SourceControlNode::Folder {
            display_name,
            full_path,
            ..
        } => {
            widgets.icon.set_icon_name(Some("folder-symbolic"));
            widgets.label.set_label(&display_name);
            widgets.row_box.set_tooltip_text(Some(&full_path));
            widgets.staged.set_visible(false);
            widgets.actions_box.set_visible(false);
            widgets.status.set_visible(false);
        }
        SourceControlNode::File { entry } => {
            widgets.icon.set_icon_name(Some("text-x-generic-symbolic"));
            widgets.label.set_label(&file_basename(&entry));
            let tooltip = match &entry.diff_action {
                GitActionState::Enabled => entry.path.display().to_string(),
                GitActionState::Disabled(reason) => reason.clone(),
            };
            widgets.row_box.set_tooltip_text(Some(&tooltip));
            bind_staged_marker(&widgets.staged, entry.staged);
            widgets.actions_box.set_visible(true);
            bind_action_state(
                &widgets.stage_button,
                &entry.stage_action,
                &pgettext("git action tooltip", "Stage File"),
            );
            bind_action_state(
                &widgets.unstage_button,
                &entry.unstage_action,
                &pgettext("git action tooltip", "Unstage File"),
            );
            bind_status_badge(&widgets.status, &entry);
        }
    }
}

fn bind_staged_marker(staged: &gtk4::Label, visible: bool) {
    let staged_label = pgettext("git status", "Staged");
    staged.set_tooltip_text(Some(&staged_label));
    staged.update_property(&[Property::Label(&staged_label)]);
    staged.set_visible(visible);
}

fn bind_action_state(button: &gtk4::Button, action_state: &GitActionState, tooltip: &str) {
    button.set_sensitive(action_state.enabled());
    match action_state {
        GitActionState::Enabled => button.set_tooltip_text(Some(tooltip)),
        GitActionState::Disabled(reason) => button.set_tooltip_text(Some(reason)),
    }
}

fn bind_status_badge(status: &gtk4::Label, entry: &GitStatusEntry) {
    let status_label = entry.status.label();
    status.set_label(entry.status.badge());
    status.set_tooltip_text(Some(&status_label));
    status.update_property(&[Property::Label(&status_label)]);
    status.set_visible(true);
}

fn row_widgets(expander: &gtk4::TreeExpander) -> Option<RowWidgets> {
    let row_box = expander.child()?.downcast::<gtk4::Box>().ok()?;
    let icon = row_box.first_child()?.downcast::<gtk4::Image>().ok()?;
    let label = icon.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let staged = label.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let actions_box = staged.next_sibling()?.downcast::<gtk4::Box>().ok()?;
    let stage_button = actions_box.first_child()?.downcast::<gtk4::Button>().ok()?;
    let unstage_button = stage_button
        .next_sibling()?
        .downcast::<gtk4::Button>()
        .ok()?;
    let status = actions_box.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some(RowWidgets {
        row_box,
        icon,
        label,
        staged,
        actions_box,
        stage_button,
        unstage_button,
        status,
    })
}

struct RowWidgets {
    row_box: gtk4::Box,
    icon: gtk4::Image,
    label: gtk4::Label,
    staged: gtk4::Label,
    actions_box: gtk4::Box,
    stage_button: gtk4::Button,
    unstage_button: gtk4::Button,
    status: gtk4::Label,
}

fn path_for_button(button: &gtk4::Button) -> Option<Vec<u8>> {
    let actions_box = button.parent()?;
    let row_box = actions_box.parent()?;
    let expander = row_box.parent()?.downcast::<gtk4::TreeExpander>().ok()?;
    let row = expander.list_row()?;
    let SourceControlNode::File { entry } = crate::source_control::tree_model::node_for_row(&row)?
    else {
        return None;
    };
    Some(entry.path.raw().to_vec())
}

fn unbind_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(expander) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
    else {
        return;
    };
    expander.set_list_row(None);
}

fn activate_position(
    model: &gtk4::TreeListModel,
    position: u32,
    weak: &Weak<RefCell<SourceControlState>>,
) {
    let Some((row, node)) = node_for_position(model, position) else {
        return;
    };
    match node {
        SourceControlNode::Folder { .. } => {
            row.set_expanded(!row.is_expanded());
        }
        SourceControlNode::File { entry } => {
            if let Some(state) = weak.upgrade() {
                actions::run_path_action(&state, entry.path.raw(), GitRowAction::Diff);
            }
        }
    }
}
