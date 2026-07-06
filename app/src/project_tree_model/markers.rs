use std::collections::{HashMap, HashSet};

use gtk4::{glib, prelude::*};

use super::ProjectTreeItem;

pub(super) fn update_visible_git_badges(
    model: &gtk4::TreeListModel,
    statuses: &HashMap<String, String>,
) {
    for position in 0..model.n_items() {
        let Some(row) = model.row(position) else {
            continue;
        };
        let Some(item) = row.item() else {
            continue;
        };
        let Ok(mut boxed) = item.downcast::<glib::BoxedAnyObject>() else {
            continue;
        };
        let changed = {
            let Ok(mut borrowed) = boxed.try_borrow_mut::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &mut *borrowed else {
                continue;
            };
            let next = statuses.get(&entry.uri).cloned();
            if entry.git_badge == next {
                false
            } else {
                entry.git_badge = next;
                true
            }
        };
        if changed {
            model.items_changed(position, 1, 1);
        }
    }
}

pub(super) fn update_visible_dirty_markers(
    model: &gtk4::TreeListModel,
    dirty_uris: &HashSet<String>,
) {
    for position in 0..model.n_items() {
        let Some(row) = model.row(position) else {
            continue;
        };
        let Some(item) = row.item() else {
            continue;
        };
        let Ok(mut boxed) = item.downcast::<glib::BoxedAnyObject>() else {
            continue;
        };
        let changed = {
            let Ok(mut borrowed) = boxed.try_borrow_mut::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &mut *borrowed else {
                continue;
            };
            let next = dirty_uris.contains(&entry.uri);
            if entry.dirty == next {
                false
            } else {
                entry.dirty = next;
                true
            }
        };
        if changed {
            model.items_changed(position, 1, 1);
        }
    }
}
