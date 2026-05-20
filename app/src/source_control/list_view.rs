use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk4::{gio, glib, pango, prelude::*};

use crate::git_status::{GitStatusEntry, GitWorktreeMode};
use crate::source_control::SourceControlState;
use crate::source_control::actions::{self, GitRowAction};
use crate::source_control::row_popover::{RowActionRunner, RowPopover};
use crate::source_control::row_widgets::{
    BoundRows, add_context_shortcut, bind_staged_marker, bind_status_badge, remember_bound_row,
    row_widget_for_entry,
};
use crate::source_control::tree_model::file_basename;

pub(super) struct SourceControlList {
    store: gio::ListStore,
    selection: gtk4::SingleSelection,
    list_view: gtk4::ListView,
    state_weak: Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    _popover: Rc<RowPopover>,
    bound_rows: BoundRows,
}

impl SourceControlList {
    #[must_use]
    pub(super) fn new() -> Self {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk4::SingleSelection::new(Some(store.clone()));
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
        install_keyboard_context_menu(&list_view, &selection, &store, &popover, &bound_rows);
        Self {
            store,
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
        let model = self.store.clone();
        self.list_view.connect_activate(move |_, position| {
            activate_position(&model, position, &weak);
        });
    }

    pub(super) fn rebuild(&self, entries: &[GitStatusEntry]) {
        let selected = selected_path(&self.selection);
        self.bound_rows.borrow_mut().clear();
        self.store.remove_all();
        for entry in sorted_entries(entries) {
            self.store.append(&glib::BoxedAnyObject::new(entry));
        }
        restore_selected_path(&self.selection, &self.store, selected.as_deref());
    }

    #[cfg(test)]
    pub(super) fn row_count_for_tests(&self) -> usize {
        usize::try_from(self.store.n_items()).map_or(0, |count| count)
    }

    #[cfg(test)]
    pub(super) fn activation_for_path_for_tests(
        &self,
        path: &str,
    ) -> Option<(gtk4::ListView, u32)> {
        for position in 0..self.store.n_items() {
            let entry = entry_at(&self.store, position)?;
            if entry.path.as_utf8() == Some(path) {
                return Some((self.list_view.clone(), position));
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
    let bound_rows_for_unbind = Rc::clone(bound_rows);
    factory.connect_unbind(move |_, object| unbind_row(object, &bound_rows_for_unbind));
    factory
}

fn setup_row(object: &glib::Object, list_view: &gtk4::ListView, popover: &Rc<RowPopover>) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let list_item_weak = list_item.downgrade();
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row_box.add_css_class("riteed-sidebar-row");
    row_box.set_hexpand(true);
    row_box.set_margin_top(3);
    row_box.set_margin_bottom(3);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);

    let icon = gtk4::Image::from_icon_name("text-x-generic-symbolic");
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
        let Some(entry) = entry_for_list_item(&list_item_weak) else {
            return;
        };
        popover.popup_for_row(&list_view, &row_widget, &entry);
    });
    row_box.add_controller(gesture);

    list_item.set_child(Some(&row_box));
}

fn bind_row(object: &glib::Object, bound_rows: &BoundRows) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(entry) = list_item.item().and_then(|item| entry_from_object(&item)) else {
        return;
    };
    let Some(widgets) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::Box>().ok())
        .and_then(|row_box| row_widgets(&row_box))
    else {
        return;
    };
    widgets.icon.set_icon_name(Some(entry_icon_name(&entry)));
    widgets.label.set_label(&list_display_name(&entry));
    widgets.row_box.set_tooltip_text(Some(entry.path.display()));
    remember_bound_row(bound_rows, &entry, &widgets.row_box);
    bind_staged_marker(&widgets.staged, entry.staged);
    bind_status_badge(&widgets.status, &entry);
}

fn row_widgets(row_box: &gtk4::Box) -> Option<RowWidgets> {
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

fn entry_for_list_item(list_item: &glib::WeakRef<gtk4::ListItem>) -> Option<GitStatusEntry> {
    list_item
        .upgrade()
        .and_then(|item| item.item())
        .and_then(|item| entry_from_object(&item))
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
    store: &gio::ListStore,
    popover: &Rc<RowPopover>,
    bound_rows: &BoundRows,
) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Local);
    let list_view_weak = list_view.downgrade();
    let selection = selection.clone();
    let store = store.clone();
    let popover = Rc::clone(popover);
    let bound_rows = Rc::clone(bound_rows);
    let popup: Rc<dyn Fn() -> bool> = Rc::new(move || {
        let position = selection.selected();
        if position == gtk4::INVALID_LIST_POSITION {
            return false;
        }
        let Some(entry) = entry_at(&store, position) else {
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

fn unbind_row(object: &glib::Object, bound_rows: &BoundRows) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(entry) = list_item.item().and_then(|item| entry_from_object(&item)) else {
        return;
    };
    bound_rows
        .borrow_mut()
        .retain(|row| row.path != entry.path.raw());
}

fn list_display_name(entry: &GitStatusEntry) -> String {
    file_basename(entry)
}

fn entry_icon_name(entry: &GitStatusEntry) -> &'static str {
    match entry.worktree_mode {
        GitWorktreeMode::Directory | GitWorktreeMode::Gitlink => "folder-symbolic",
        GitWorktreeMode::Regular(_)
        | GitWorktreeMode::Symlink
        | GitWorktreeMode::Absent
        | GitWorktreeMode::Unsupported
        | GitWorktreeMode::Unknown => "text-x-generic-symbolic",
    }
}

fn activate_position(
    model: &gio::ListStore,
    position: u32,
    weak: &Weak<RefCell<SourceControlState>>,
) {
    let Some(entry) = entry_at(model, position) else {
        return;
    };
    if let Some(state) = weak.upgrade() {
        actions::run_path_action(&state, entry.path.raw(), GitRowAction::Diff);
    }
}

fn sorted_entries(entries: &[GitStatusEntry]) -> Vec<GitStatusEntry> {
    let mut keyed_entries = entries
        .iter()
        .cloned()
        .map(|entry| (entry.path.display().to_lowercase(), entry))
        .collect::<Vec<_>>();
    keyed_entries.sort_by(|(left_key, left), (right_key, right)| {
        left_key
            .cmp(right_key)
            .then_with(|| left.path.raw().cmp(right.path.raw()))
    });
    keyed_entries
        .into_iter()
        .map(|(_key, entry)| entry)
        .collect()
}

fn selected_path(selection: &gtk4::SingleSelection) -> Option<Vec<u8>> {
    let position = selection.selected();
    if position == gtk4::INVALID_LIST_POSITION {
        return None;
    }
    selection
        .model()
        .and_then(|model| model.item(position))
        .and_then(|item| entry_from_object(&item))
        .map(|entry| entry.path.raw().to_vec())
}

fn restore_selected_path(
    selection: &gtk4::SingleSelection,
    store: &gio::ListStore,
    selected: Option<&[u8]>,
) {
    let Some(selected) = selected else {
        selection.set_selected(gtk4::INVALID_LIST_POSITION);
        return;
    };
    for position in 0..store.n_items() {
        if entry_at(store, position).is_some_and(|entry| entry.path.raw() == selected) {
            selection.set_selected(position);
            return;
        }
    }
    selection.set_selected(gtk4::INVALID_LIST_POSITION);
}

fn entry_at(store: &gio::ListStore, position: u32) -> Option<GitStatusEntry> {
    store
        .item(position)
        .and_then(|item| entry_from_object(&item))
}

fn entry_from_object(item: &glib::Object) -> Option<GitStatusEntry> {
    let boxed = item.clone().downcast::<glib::BoxedAnyObject>().ok()?;
    let borrowed = boxed.try_borrow::<GitStatusEntry>().ok()?;
    Some((*borrowed).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_status::{GitFileStatus, GitPath};

    #[test]
    fn list_display_name_uses_file_basename() {
        let entry = GitStatusEntry::new(
            GitPath::from_bytes(b"app/build-aux/validation/i18n-review.v1.json"),
            GitFileStatus::Modified,
            None,
            None,
            false,
            true,
        );

        assert_eq!(list_display_name(&entry), "i18n-review.v1.json");
        assert_eq!(
            entry.path.display(),
            "app/build-aux/validation/i18n-review.v1.json"
        );
    }
}
