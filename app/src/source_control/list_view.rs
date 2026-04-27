use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::{gio, glib, pango, prelude::*};

use crate::git_status::{GitActionState, GitStatusEntry};
use crate::source_control::SourceControlState;
use crate::source_control::actions::{self, GitRowAction};
use crate::source_control::row_overlay::setup_action_overlay;
use crate::source_control::status_style;

pub(super) struct SourceControlList {
    store: gio::ListStore,
    selection: gtk4::SingleSelection,
    list_view: gtk4::ListView,
    activation_guard: Rc<Cell<bool>>,
    state_weak: Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
}

impl SourceControlList {
    #[must_use]
    pub(super) fn new() -> Self {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk4::SingleSelection::new(Some(store.clone()));
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
            store,
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
        let model = self.store.clone();
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
        let selected = selected_path(&self.selection);
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
    state_weak: &Rc<RefCell<Weak<RefCell<SourceControlState>>>>,
    activation_guard: &Rc<Cell<bool>>,
) -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    let state_weak = Rc::clone(state_weak);
    let activation_guard = Rc::clone(activation_guard);
    factory.connect_setup(move |_, object| setup_row(object, &state_weak, &activation_guard));
    factory.connect_bind(move |_, object| bind_row(object));
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
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
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

    let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    actions_box.add_css_class("riteed-git-row-actions");
    actions_box.set_halign(gtk4::Align::End);
    actions_box.set_valign(gtk4::Align::Center);
    actions_box.set_can_target(false);
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
    actions_box.append(&action_button(
        "edit-delete-symbolic",
        &pgettext("git action tooltip", "Discard Changes"),
        state_weak,
        activation_guard,
        GitRowAction::Discard,
    ));

    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("riteed-git-row");
    overlay.set_hexpand(true);
    overlay.set_child(Some(&row_box));
    overlay.add_overlay(&actions_box);
    overlay.set_measure_overlay(&actions_box, false);
    setup_action_overlay(&overlay, &actions_box);

    list_item.set_child(Some(&overlay));
}

fn bind_row(object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(entry) = list_item.item().and_then(|item| entry_from_object(&item)) else {
        return;
    };
    let Some(widgets) = list_item
        .child()
        .and_then(|child| child.downcast::<gtk4::Overlay>().ok())
        .and_then(|overlay| row_widgets(&overlay))
    else {
        return;
    };
    widgets.label.set_label(entry.path.display());
    widgets.row_box.set_tooltip_text(Some(entry.path.display()));
    bind_staged_marker(&widgets.staged, entry.staged);
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
    bind_action_state(
        &widgets.discard_button,
        &entry.discard_action,
        &pgettext("git action tooltip", "Discard Changes"),
    );
    bind_status_badge(&widgets.status, &entry);
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
        if let Some(state) = state_weak.borrow().upgrade()
            && let Some(path) = path_for_button(button)
        {
            actions::run_path_action(&state, path.as_bytes(), action);
        }
        let activation_guard = Rc::clone(&activation_guard);
        glib::idle_add_local_once(move || activation_guard.set(false));
    });
    button
}

fn row_widgets(overlay: &gtk4::Overlay) -> Option<RowWidgets> {
    let row_box = overlay.child()?.downcast::<gtk4::Box>().ok()?;
    let icon = row_box.first_child()?.downcast::<gtk4::Image>().ok()?;
    let status = icon.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let staged = status.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let label = staged.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    let actions_box = row_box.next_sibling()?.downcast::<gtk4::Box>().ok()?;
    let stage_button = actions_box.first_child()?.downcast::<gtk4::Button>().ok()?;
    let unstage_button = stage_button
        .next_sibling()?
        .downcast::<gtk4::Button>()
        .ok()?;
    let discard_button = unstage_button
        .next_sibling()?
        .downcast::<gtk4::Button>()
        .ok()?;
    Some(RowWidgets {
        row_box,
        label,
        staged,
        stage_button,
        unstage_button,
        discard_button,
        status,
    })
}

struct RowWidgets {
    row_box: gtk4::Box,
    label: gtk4::Label,
    staged: gtk4::Label,
    stage_button: gtk4::Button,
    unstage_button: gtk4::Button,
    discard_button: gtk4::Button,
    status: gtk4::Label,
}

fn path_for_button(button: &gtk4::Button) -> Option<String> {
    let actions_box = button.parent()?;
    let overlay = actions_box.parent()?.downcast::<gtk4::Overlay>().ok()?;
    let row_box = overlay.child()?.downcast::<gtk4::Box>().ok()?;
    let icon = row_box.first_child()?;
    let status = icon.next_sibling()?;
    let staged = status.next_sibling()?;
    let label = staged.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some(label.text().to_string())
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
    for class in status_style::STATUS_CLASSES {
        status.remove_css_class(class);
    }
    status.add_css_class(status_style::status_class_for(entry.status));
    if status_style::status_is_dim(entry.status) {
        status.add_css_class("dim-label");
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
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| {
        left.path
            .display()
            .to_lowercase()
            .cmp(&right.path.display().to_lowercase())
            .then_with(|| left.path.raw().cmp(right.path.raw()))
    });
    entries
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
