use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::{gdk, prelude::*};

use crate::git_status::GitStatusEntry;
use crate::source_control::action_widgets::bind_action_state;
use crate::source_control::actions::GitRowAction;

pub(crate) type RowActionRunner = Rc<dyn Fn(Vec<u8>, GitRowAction)>;

pub(crate) struct RowPopover {
    popover: gtk4::Popover,
    stage: gtk4::Button,
    unstage: gtk4::Button,
    discard: gtk4::Button,
    diff: gtk4::Button,
    action_runner: RowActionRunner,
    bound_path: RefCell<Option<Vec<u8>>>,
}

impl RowPopover {
    #[must_use]
    pub(crate) fn new(action_runner: RowActionRunner) -> Rc<Self> {
        let stage = action_button("list-add-symbolic", &stage_label());
        let unstage = action_button("list-remove-symbolic", &unstage_label());
        let discard = action_button("edit-delete-symbolic", &discard_label());
        let diff = action_button("view-dual-symbolic", &diff_label());
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content.add_css_class("toolbar");
        content.append(&diff);
        content.append(&stage);
        content.append(&unstage);
        content.append(&discard);

        let popover = gtk4::Popover::new();
        popover.set_child(Some(&content));

        let row_popover = Rc::new(Self {
            popover,
            stage,
            unstage,
            discard,
            diff,
            action_runner,
            bound_path: RefCell::new(None),
        });
        row_popover.connect_buttons();
        row_popover
    }

    pub(crate) fn attach_to_list_view(&self, list_view: &impl IsA<gtk4::Widget>) {
        self.popover.set_parent(list_view);
    }

    pub(crate) fn bind_to_entry(&self, entry: &GitStatusEntry) {
        bind_action_state(&self.stage, &entry.stage_action, &stage_label());
        bind_action_state(&self.unstage, &entry.unstage_action, &unstage_label());
        bind_action_state(&self.discard, &entry.discard_action, &discard_label());
        bind_action_state(&self.diff, &entry.diff_action, &diff_label());
        self.bound_path.replace(Some(entry.path.raw().to_vec()));
    }

    pub(crate) fn popup_for_row(
        &self,
        list_view: &impl IsA<gtk4::Widget>,
        row_widget: &impl IsA<gtk4::Widget>,
        entry: &GitStatusEntry,
    ) {
        self.bind_to_entry(entry);
        let Some(bounds) = row_widget.compute_bounds(list_view) else {
            return;
        };
        let bounds = bounds.round_extents();
        let rect = gdk::Rectangle::from(gtk4::cairo::Rectangle::new(
            f64::from(bounds.x()),
            f64::from(bounds.y()),
            f64::from(bounds.width()),
            f64::from(bounds.height()),
        ));
        self.popover.set_pointing_to(Some(&rect));
        self.popover.popup();
    }

    fn connect_buttons(self: &Rc<Self>) {
        self.connect_button(&self.stage, GitRowAction::Stage);
        self.connect_button(&self.unstage, GitRowAction::Unstage);
        self.connect_button(&self.discard, GitRowAction::Discard);
        self.connect_button(&self.diff, GitRowAction::Diff);
    }

    fn connect_button(self: &Rc<Self>, button: &gtk4::Button, action: GitRowAction) {
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            let Some(popover) = weak.upgrade() else {
                return;
            };
            let Some(path) = popover.bound_path.borrow().clone() else {
                return;
            };
            (popover.action_runner)(path, action);
            popover.popover.popdown();
        });
    }
}

fn action_button(icon_name: &str, label: &str) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(label)
        .build();
    button.add_css_class("flat");
    button.update_property(&[Property::Label(label)]);
    button
}

fn stage_label() -> String {
    pgettext("git action tooltip", "Stage File")
}

fn unstage_label() -> String {
    pgettext("git action tooltip", "Unstage File")
}

fn discard_label() -> String {
    pgettext("git action tooltip", "Discard Changes")
}

fn diff_label() -> String {
    pgettext("git action tooltip", "Compare With Git")
}
