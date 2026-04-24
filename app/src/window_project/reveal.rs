use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

use crate::project_tree_model::ProjectTreeItem;

use super::ProjectState;

pub(super) struct PendingReveal {
    pub(super) generation: u64,
    pub(super) cancellable: gio::Cancellable,
    chain: Vec<gio::File>,
    index: usize,
    attempts_left: usize,
}

pub(super) fn cancel_reveal(state: &Rc<RefCell<ProjectState>>) {
    let mut state = state.borrow_mut();
    if let Some(pending) = state.pending_reveal.take() {
        pending.cancellable.cancel();
    }
}

pub(super) fn schedule_restore_expanded(state: &Rc<RefCell<ProjectState>>, expanded: Vec<String>) {
    if expanded.is_empty() {
        return;
    }

    let state = Rc::downgrade(state);
    let expanded = Rc::new(expanded);
    for attempt in 0..8u64 {
        let state = state.clone();
        let expanded = Rc::clone(&expanded);
        let delay = Duration::from_millis(20 * (attempt + 1));
        let _source = glib::timeout_add_local_once(delay, move || {
            let Some(state) = state.upgrade() else {
                return;
            };
            let state_borrow = state.borrow();
            let tree = state_borrow.browser.tree().model();
            tree.restore_expanded_uris(&expanded);
        });
    }
}

pub(super) fn sync_reveal_for_selection(state: &Rc<RefCell<ProjectState>>) {
    let (root, selected_uri) = {
        let state = state.borrow();
        let root = state.root.as_ref().map(|root| root.file.clone());
        let selected_uri = state
            .workspace
            .upgrade()
            .and_then(|workspace| workspace.selected_tab().and_then(|tab| tab.uri()));
        (root, selected_uri)
    };

    let Some(root) = root else {
        state.borrow().browser.tree().clear_selection();
        cancel_reveal(state);
        return;
    };

    let Some(selected_uri) = selected_uri else {
        state.borrow().browser.tree().clear_selection();
        cancel_reveal(state);
        return;
    };
    let selected_file = gio::File::for_uri(&selected_uri);
    if !file_is_under_root(&root, &selected_file) {
        state.borrow().browser.tree().clear_selection();
        cancel_reveal(state);
        return;
    }

    start_reveal(state, &root, &selected_file);
}

fn start_reveal(state: &Rc<RefCell<ProjectState>>, root: &gio::File, target: &gio::File) {
    cancel_reveal(state);

    let Some(relative) = root.relative_path(target) else {
        state.borrow().browser.tree().clear_selection();
        return;
    };

    let mut chain = Vec::new();
    let mut current = root.clone();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        current = current.child(name);
        chain.push(current.clone());
    }
    if chain.is_empty() {
        state.borrow().browser.tree().clear_selection();
        return;
    }

    let generation = {
        let mut state = state.borrow_mut();
        state.reveal_generation += 1;
        let generation = state.reveal_generation;
        let cancellable = gio::Cancellable::new();
        state.pending_reveal = Some(PendingReveal {
            generation,
            cancellable,
            chain,
            index: 0,
            attempts_left: 160,
        });
        generation
    };

    let weak = Rc::downgrade(state);
    let _source = glib::timeout_add_local(Duration::from_millis(20), move || {
        let Some(state) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let mut state_mut = state.borrow_mut();
        let (wanted, is_last) = {
            let Some(pending) = state_mut.pending_reveal.as_mut() else {
                return glib::ControlFlow::Break;
            };
            if pending.cancellable.is_cancelled() || pending.generation != generation {
                state_mut.pending_reveal = None;
                return glib::ControlFlow::Break;
            }
            if pending.attempts_left == 0 {
                state_mut.pending_reveal = None;
                state_mut.browser.tree().clear_selection();
                return glib::ControlFlow::Break;
            }
            pending.attempts_left -= 1;

            let Some(wanted) = pending.chain.get(pending.index).cloned() else {
                state_mut.pending_reveal = None;
                return glib::ControlFlow::Break;
            };
            let is_last = pending.index >= pending.chain.len().saturating_sub(1);
            (wanted, is_last)
        };

        let model = state_mut.browser.tree().model().model();
        for position in 0..model.n_items() {
            let Some(item) = model.item(position) else {
                continue;
            };
            let Ok(row) = item.downcast::<gtk4::TreeListRow>() else {
                continue;
            };
            let Some(row_item) = row.item() else {
                continue;
            };
            let Ok(boxed) = row_item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            if entry.file.equal(&wanted) {
                if is_last {
                    state_mut.browser.tree().selection().set_selected(position);
                    state_mut.pending_reveal = None;
                    return glib::ControlFlow::Break;
                }
                if row.is_expandable() && !row.is_expanded() {
                    row.set_expanded(true);
                }
                if let Some(pending) = state_mut.pending_reveal.as_mut()
                    && !pending.cancellable.is_cancelled()
                    && pending.generation == generation
                {
                    pending.index = pending.index.saturating_add(1);
                }
                break;
            }
        }

        glib::ControlFlow::Continue
    });
}

fn file_is_under_root(root: &gio::File, child: &gio::File) -> bool {
    let Some(root_scheme) = root.uri_scheme() else {
        return false;
    };
    let Some(child_scheme) = child.uri_scheme() else {
        return false;
    };
    if root_scheme.is_empty() || child_scheme.is_empty() || root_scheme != child_scheme {
        return false;
    }
    child.has_prefix(root)
}

#[cfg(test)]
mod tests {
    use gtk4::gio;

    use super::file_is_under_root;

    #[test]
    fn containment_uses_gfile_boundaries() {
        let root = gio::File::for_path("/tmp/riteed-root");
        assert!(file_is_under_root(
            &root,
            &gio::File::for_path("/tmp/riteed-root/child.txt"),
        ));
        assert!(!file_is_under_root(
            &root,
            &gio::File::for_path("/tmp/riteed-root-sibling"),
        ));
    }

    #[test]
    fn containment_rejects_scheme_mismatch_and_handles_encoded_uri() {
        let root = gio::File::for_uri("file:///tmp/riteed%20root");
        let child = gio::File::for_uri("file:///tmp/riteed%20root/child.txt");
        let remote = gio::File::for_uri("https://example.com/child.txt");
        assert!(file_is_under_root(&root, &child));
        assert!(!file_is_under_root(&root, &remote));
    }
}
