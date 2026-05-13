use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk4::{gio, glib, pango, prelude::*};

use crate::git_status::{GitActionState, GitStatusEntry};
use crate::source_control::SourceControlState;
use crate::source_control::actions::{self, GitRowAction};
use crate::source_control::row_popover::{RowActionRunner, RowPopover};
use crate::source_control::row_widgets::{
    BoundRows, add_context_shortcut, bind_staged_marker, bind_status_badge, remember_bound_row,
    row_widget_for_entry,
};
use crate::source_control::tree_model::{
    SourceControlNode, build_root_store, file_basename, node_for_position, restore_expanded_paths,
    restore_selected_node, snapshot_expanded_paths, snapshot_selected_node,
};

pub(super) struct SourceControlTree {
    root_store: gio::ListStore,
    tree_model: gtk4::TreeListModel,
    selection: gtk4::SingleSelection,
    list_view: gtk4::ListView,
    state_weak: Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    _popover: Rc<RowPopover>,
    bound_rows: BoundRows,
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
        let bound_rows = Rc::new(RefCell::new(Vec::new()));
        let list_view = gtk4::ListView::new(Some(selection.clone()), None::<gtk4::ListItemFactory>);
        let popover = RowPopover::new(action_runner(&state_weak));
        popover.attach_to_list_view(&list_view);
        let factory = create_factory(&list_view, &popover, &bound_rows);
        list_view.set_factory(Some(&factory));
        list_view.set_single_click_activate(true);
        list_view.set_enable_rubberband(false);
        list_view.set_vexpand(true);
        install_keyboard_context_menu(&list_view, &selection, &tree_model, &popover, &bound_rows);

        Self {
            root_store,
            tree_model,
            selection,
            list_view,
            state_weak,
            _popover: popover,
            bound_rows,
        }
    }

    #[must_use]
    pub(super) fn widget(&self) -> gtk4::ListView {
        self.list_view.clone()
    }

    pub(super) fn connect_activation(&self, weak: Weak<RefCell<SourceControlState>>) {
        *self.state_weak.borrow_mut() = weak.clone();
        let model = self.tree_model.clone();
        self.list_view.connect_activate(move |_, position| {
            activate_position(&model, position, &weak);
        });
    }

    pub(super) fn rebuild(&self, entries: &[GitStatusEntry]) {
        let expanded = snapshot_expanded_paths(&self.tree_model);
        let selected = snapshot_selected_node(&self.selection);
        let next_store = build_root_store(entries);
        self.bound_rows.borrow_mut().clear();
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
    list_view: &gtk4::ListView,
    popover: &Rc<RowPopover>,
    bound_rows: &BoundRows,
) -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    let list_view = list_view.clone();
    let popover = Rc::clone(popover);
    factory.connect_setup(move |_, object| setup_row(object, &list_view, &popover));
    let bound_rows_for_bind = Rc::clone(bound_rows);
    factory.connect_bind(move |_, object| bind_row(object, &bound_rows_for_bind));
    let bound_rows = Rc::clone(bound_rows);
    factory.connect_unbind(move |_, object| unbind_row(object, &bound_rows));
    factory
}

fn setup_row(object: &glib::Object, list_view: &gtk4::ListView, popover: &Rc<RowPopover>) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let expander = gtk4::TreeExpander::new();
    expander.set_indent_for_depth(true);
    let expander_weak = expander.downgrade();

    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row_box.add_css_class("riteed-sidebar-row");
    row_box.set_hexpand(true);
    row_box.set_margin_top(3);
    row_box.set_margin_bottom(3);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);

    let icon = gtk4::Image::new();
    icon.set_pixel_size(16);
    row_box.append(&icon);

    let status = gtk4::Label::new(None);
    status.add_css_class("caption");
    status.add_css_class("riteed-git-status-badge");
    row_box.append(&status);

    let staged = gtk4::Label::new(Some("S"));
    staged.add_css_class("caption");
    staged.add_css_class("riteed-git-staged");
    row_box.append(&staged);

    let label = gtk4::Label::new(None);
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_ellipsize(pango::EllipsizeMode::End);
    row_box.append(&label);

    let gesture = gtk4::GestureClick::new();
    gesture.set_button(3);
    let list_view = list_view.clone();
    let popover = Rc::clone(popover);
    let row_widget = row_box.clone();
    gesture.connect_pressed(move |_, _, _, _| {
        let Some(entry) = entry_for_expander(&expander_weak) else {
            return;
        };
        popover.popup_for_row(&list_view, &row_widget, &entry);
    });
    row_box.add_controller(gesture);

    expander.set_child(Some(&row_box));
    list_item.set_child(Some(&expander));
}

fn bind_row(object: &glib::Object, bound_rows: &BoundRows) {
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
    bind_node(&widgets, node, bound_rows);
}

fn node_for_position_from_row(row: &gtk4::TreeListRow) -> Option<SourceControlNode> {
    crate::source_control::tree_model::node_for_row(row)
}

fn action_runner(state_weak: &Rc<RefCell<Weak<RefCell<SourceControlState>>>>) -> RowActionRunner {
    let state_weak = Rc::clone(state_weak);
    Rc::new(move |path, action| {
        if let Some(state) = state_weak.borrow().upgrade() {
            actions::run_path_action(&state, &path, action);
        }
    })
}

fn install_keyboard_context_menu(
    list_view: &gtk4::ListView,
    selection: &gtk4::SingleSelection,
    model: &gtk4::TreeListModel,
    popover: &Rc<RowPopover>,
    bound_rows: &BoundRows,
) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Local);
    let list_view_weak = list_view.downgrade();
    let selection = selection.clone();
    let model = model.clone();
    let popover = Rc::clone(popover);
    let bound_rows = Rc::clone(bound_rows);
    let popup: Rc<dyn Fn() -> bool> = Rc::new(move || {
        let position = selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return false;
        }
        let Some((_row, SourceControlNode::File { entry })) = node_for_position(&model, position)
        else {
            return false;
        };
        let Some(list_view) = list_view_weak.upgrade() else {
            return false;
        };
        let Some(row_widget) = row_widget_for_entry(&bound_rows, entry.path.raw()) else {
            return false;
        };
        popover.popup_for_row(&list_view, &row_widget, &entry);
        true
    });
    add_context_shortcut(&controller, "Menu", Rc::clone(&popup));
    add_context_shortcut(&controller, "<Shift>F10", popup);
    list_view.add_controller(controller);
}

fn bind_node(widgets: &RowWidgets, node: SourceControlNode, bound_rows: &BoundRows) {
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
            remember_bound_row(bound_rows, &entry, &widgets.row_box);
            bind_staged_marker(&widgets.staged, entry.staged);
            bind_status_badge(&widgets.status, &entry);
        }
    }
}

fn row_widgets(expander: &gtk4::TreeExpander) -> Option<RowWidgets> {
    let row_box = expander.child()?.downcast::<gtk4::Box>().ok()?;
    let icon = row_box.first_child()?.downcast::<gtk4::Image>().ok()?;
    let status = icon.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let staged = status.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let label = staged.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some(RowWidgets {
        row_box: row_box.clone(),
        icon,
        label,
        staged,
        status,
    })
}

struct RowWidgets {
    row_box: gtk4::Box,
    icon: gtk4::Image,
    label: gtk4::Label,
    staged: gtk4::Label,
    status: gtk4::Label,
}

fn entry_for_expander(expander: &glib::WeakRef<gtk4::TreeExpander>) -> Option<GitStatusEntry> {
    let expander = expander.upgrade()?;
    let row = expander.list_row()?;
    let SourceControlNode::File { entry } = crate::source_control::tree_model::node_for_row(&row)?
    else {
        return None;
    };
    Some(entry)
}

fn unbind_row(object: &glib::Object, bound_rows: &BoundRows) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let entry = list_item
        .item()
        .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
        .and_then(|row| crate::source_control::tree_model::node_for_row(&row))
        .and_then(|node| match node {
            SourceControlNode::File { entry } => Some(entry),
            SourceControlNode::Folder { .. } => None,
        });
    if let Some(entry) = entry {
        bound_rows
            .borrow_mut()
            .retain(|row| row.path != entry.path.raw());
    }
    if let Some(expander) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
    {
        expander.set_list_row(None);
    }
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
