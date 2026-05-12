use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gio, glib, pango, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;

use super::review_session::ReviewChangeListItem;
use crate::editor_tab::EditorTab;

pub(in crate::editor_tab) fn present(tab: &Rc<EditorTab>, items: &[ReviewChangeListItem]) {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for item in items {
        store.append(&glib::BoxedAnyObject::new(item.clone()));
    }
    let selection = gtk4::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(true);
    selection.set_can_unselect(false);
    let list_view = gtk4::ListView::new(Some(selection), Some(create_factory()));
    list_view.set_enable_rubberband(false);
    list_view.set_vexpand(true);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_height(320)
        .child(&list_view)
        .build();

    let close_button = gtk4::Button::with_label(&pgettext("dialog button", "Close"));
    close_button.set_halign(gtk4::Align::End);

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    content.append(&scrolled);
    content.append(&close_button);

    let dialog = adw::Dialog::builder()
        .title(gettext("Change List"))
        .content_width(460)
        .content_height(420)
        .can_close(true)
        .child(&content)
        .build();
    dialog.update_property(&[
        Property::Label(&gettext("Change List")),
        Property::Description(&gettext("List of changed files and hunks in this review.")),
    ]);

    let weak_dialog = dialog.downgrade();
    close_button.connect_clicked(move |_| {
        if let Some(dialog) = weak_dialog.upgrade() {
            dialog.close();
        }
    });

    let weak_dialog = dialog.downgrade();
    let weak_tab = Rc::downgrade(tab);
    list_view.connect_activate(move |_, position| {
        let Some(item) = item_at(&store, position) else {
            return;
        };
        if let Some(dialog) = weak_dialog.upgrade() {
            dialog.close();
        }
        if let Some(tab) = weak_tab.upgrade() {
            tab.scroll_review_to_target(item.target);
        }
    });

    dialog.present(Some(&tab.root));
}

fn create_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
            return;
        };
        list_item.set_child(Some(&row_widget()));
    });
    factory.connect_bind(|_, object| {
        let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
            return;
        };
        let Some(item) = list_item
            .item()
            .and_then(|object| item_from_object(&object))
        else {
            return;
        };
        let Some(row) = list_item.child().and_downcast::<gtk4::Box>() else {
            return;
        };
        bind_row(&row, &item);
    });
    factory
}

fn row_widget() -> gtk4::Box {
    let title = gtk4::Label::builder()
        .ellipsize(pango::EllipsizeMode::Middle)
        .xalign(0.0)
        .build();
    let description = gtk4::Label::builder()
        .ellipsize(pango::EllipsizeMode::End)
        .xalign(0.0)
        .build();
    description.add_css_class("dim-label");
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(3)
        .margin_top(9)
        .margin_bottom(9)
        .margin_start(12)
        .margin_end(12)
        .build();
    row.append(&title);
    row.append(&description);
    row
}

fn bind_row(row: &gtk4::Box, item: &ReviewChangeListItem) {
    if let Some(title) = row.first_child().and_downcast::<gtk4::Label>() {
        title.set_label(&item.label);
    }
    if let Some(description) = row.last_child().and_downcast::<gtk4::Label>() {
        description.set_label(&item.description);
    }
    row.update_property(&[Property::Label(&format!(
        "{} {}",
        item.label, item.description
    ))]);
}

fn item_at(store: &gio::ListStore, position: u32) -> Option<ReviewChangeListItem> {
    store
        .item(position)
        .and_then(|object| item_from_object(&object))
}

fn item_from_object(object: &glib::Object) -> Option<ReviewChangeListItem> {
    let boxed = object.clone().downcast::<glib::BoxedAnyObject>().ok()?;
    let borrowed = boxed.try_borrow::<ReviewChangeListItem>().ok()?;
    Some((*borrowed).clone())
}

#[cfg(test)]
mod tests {
    use super::item_at;
    use crate::editor_tab::ReviewScrollTarget;
    use crate::editor_tab::compare::review_session::ReviewChangeListItem;

    #[test]
    fn list_store_roundtrips_change_targets() {
        let item = item("file.txt", "Changed line", 12);
        let store = gtk4::gio::ListStore::new::<gtk4::glib::BoxedAnyObject>();
        store.append(&gtk4::glib::BoxedAnyObject::new(item.clone()));

        assert_eq!(item_at(&store, 0), Some(item));
        assert_eq!(item_at(&store, 1), None);
    }

    fn item(label: &str, description: &str, line_index: usize) -> ReviewChangeListItem {
        ReviewChangeListItem {
            label: label.to_string(),
            description: description.to_string(),
            target: ReviewScrollTarget { line_index },
        }
    }
}
