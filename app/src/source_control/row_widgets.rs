use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::{glib, prelude::*};

use crate::git_status::GitStatusEntry;
use crate::source_control::status_style;

pub(super) type BoundRows = Rc<RefCell<Vec<BoundRow>>>;

pub(super) struct BoundRow {
    pub(super) path: Vec<u8>,
    pub(super) widget: glib::WeakRef<gtk4::Widget>,
}

pub(super) const ACTIVE_ROW_CSS_CLASS: &str = "riteed-source-control-active-row";

pub(super) fn remember_bound_row(
    bound_rows: &BoundRows,
    entry: &GitStatusEntry,
    row_box: &gtk4::Box,
) {
    let mut rows = bound_rows.borrow_mut();
    rows.retain(|row| row.path != entry.path.raw());
    let widget: gtk4::Widget = row_box.clone().upcast();
    rows.push(BoundRow {
        path: entry.path.raw().to_vec(),
        widget: widget.downgrade(),
    });
}

pub(super) fn row_widget_for_entry(bound_rows: &BoundRows, path: &[u8]) -> Option<gtk4::Widget> {
    bound_rows
        .borrow()
        .iter()
        .rev()
        .find(|row| row.path == path)
        .and_then(|row| row.widget.upgrade())
}

pub(super) fn mark_active_row(bound_rows: &BoundRows, path: Option<&[u8]>) {
    for bound in bound_rows.borrow().iter() {
        let Some(widget) = bound.widget.upgrade() else {
            continue;
        };
        widget.remove_css_class(ACTIVE_ROW_CSS_CLASS);
        if path.is_some_and(|active| bound.path.as_slice() == active) {
            widget.add_css_class(ACTIVE_ROW_CSS_CLASS);
        }
    }
}

#[cfg(test)]
pub(super) fn active_row_path_for_tests(bound_rows: &BoundRows) -> Option<Vec<u8>> {
    bound_rows.borrow().iter().find_map(|bound| {
        let widget = bound.widget.upgrade()?;
        widget
            .has_css_class(ACTIVE_ROW_CSS_CLASS)
            .then(|| bound.path.clone())
    })
}

pub(super) fn add_context_shortcut(
    controller: &gtk4::ShortcutController,
    trigger: &str,
    popup: Rc<dyn Fn() -> bool>,
) {
    let Some(trigger) = gtk4::ShortcutTrigger::parse_string(trigger) else {
        return;
    };
    controller.add_shortcut(gtk4::Shortcut::new(
        Some(trigger),
        Some(gtk4::CallbackAction::new(move |_, _| {
            if popup() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        })),
    ));
}

pub(super) fn bind_staged_marker(staged: &gtk4::Label, visible: bool) {
    let staged_label = pgettext("git status", "Staged");
    staged.set_tooltip_text(Some(&staged_label));
    staged.update_property(&[Property::Label(&staged_label)]);
    staged.set_visible(visible);
}

pub(super) fn bind_status_badge(status: &gtk4::Label, entry: &GitStatusEntry) {
    let status_label = entry.status.label();
    status.set_label(entry.status.badge());
    status.set_tooltip_text(Some(&status_label));
    status.update_property(&[Property::Label(&status_label)]);
    status.set_visible(true);
    for class in status_style::STATUS_CLASSES {
        status.remove_css_class(class);
    }
    status.add_css_class(status_style::status_class_for(entry.status));
    if status_style::status_is_dim(entry.status) {
        status.add_css_class("dim-label");
    }
}
