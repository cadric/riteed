use std::cell::RefCell;
use std::rc::Rc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

use crate::project_tree_model::ProjectTreeItem;

use super::ProjectState;

#[cfg(not(test))]
const REVEAL_WATCHDOG_MS: u64 = 3_200;
#[cfg(test)]
const REVEAL_WATCHDOG_MS: u64 = 160;

#[cfg(test)]
static REVEAL_VISIBLE_ROW_SCANS: AtomicUsize = AtomicUsize::new(0);

pub(super) struct PendingReveal {
    generation: u64,
    chain: Vec<gio::File>,
    index: usize,
    step_scheduled: bool,
    step_source: Option<glib::SourceId>,
    watchdog_source: Option<glib::SourceId>,
    items_changed_handler: Option<glib::SignalHandlerId>,
}

#[derive(Clone, Copy)]
enum SelectionCleanup {
    Keep,
    Clear,
}

enum RevealStep {
    Select(u32),
    Expand(gtk4::TreeListRow),
    Advance,
    Wait,
    Unreachable,
    Finish,
}

pub(super) fn cancel_reveal(state: &Rc<RefCell<ProjectState>>) {
    finish_reveal(state, SelectionCleanup::Keep);
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
        clear_tree_selection(state);
        cancel_reveal(state);
        return;
    };

    let Some(selected_uri) = selected_uri else {
        clear_tree_selection(state);
        cancel_reveal(state);
        return;
    };
    let selected_file = gio::File::for_uri(&selected_uri);
    if !file_is_under_root(&root, &selected_file) {
        clear_tree_selection(state);
        cancel_reveal(state);
        return;
    }

    start_reveal(state, &root, &selected_file);
}

fn start_reveal(state: &Rc<RefCell<ProjectState>>, root: &gio::File, target: &gio::File) {
    cancel_reveal(state);

    let Some(relative) = root.relative_path(target) else {
        clear_tree_selection(state);
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
        clear_tree_selection(state);
        return;
    }

    let generation = {
        let mut state = state.borrow_mut();
        state.reveal_generation += 1;
        let generation = state.reveal_generation;
        state.pending_reveal = Some(PendingReveal {
            generation,
            chain,
            index: 0,
            step_scheduled: false,
            step_source: None,
            watchdog_source: None,
            items_changed_handler: None,
        });
        generation
    };

    install_reveal_model_handler(state, generation);
    schedule_reveal_watchdog(state, generation);
    schedule_reveal_step(state, generation);
}

fn finish_reveal(state: &Rc<RefCell<ProjectState>>, selection: SelectionCleanup) {
    let (model, pending, selected) = {
        let Ok(mut state) = state.try_borrow_mut() else {
            schedule_finish_reveal(state, selection);
            return;
        };
        state.reveal_generation = state.reveal_generation.saturating_add(1);
        let model = state.browser.tree().model().model().clone();
        let selected = state.browser.tree().selection().clone();
        (model, state.pending_reveal.take(), selected)
    };
    if let Some(mut pending) = pending {
        pending.disconnect(&model);
    }
    if matches!(selection, SelectionCleanup::Clear) {
        selected.set_selected(gtk4::INVALID_LIST_POSITION);
    }
}

fn schedule_finish_reveal(state: &Rc<RefCell<ProjectState>>, selection: SelectionCleanup) {
    let weak = Rc::downgrade(state);
    let _source = glib::idle_add_local_once(move || {
        let Some(state) = weak.upgrade() else {
            return;
        };
        finish_reveal(&state, selection);
    });
}

fn clear_tree_selection(state: &Rc<RefCell<ProjectState>>) {
    let selected = state.borrow().browser.tree().selection().clone();
    selected.set_selected(gtk4::INVALID_LIST_POSITION);
}

impl PendingReveal {
    fn disconnect(&mut self, model: &gtk4::TreeListModel) {
        if let Some(handler) = self.items_changed_handler.take() {
            model.disconnect(handler);
        }
        if let Some(source) = self.step_source.take() {
            source.remove();
        }
        if let Some(source) = self.watchdog_source.take() {
            source.remove();
        }
    }
}

fn install_reveal_model_handler(state: &Rc<RefCell<ProjectState>>, generation: u64) {
    let model = state.borrow().browser.tree().model().model().clone();
    let weak = Rc::downgrade(state);
    let handler = model.connect_items_changed(move |_, _, _, _| {
        let weak = weak.clone();
        let _source = glib::idle_add_local_once(move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            schedule_reveal_step(&state, generation);
        });
    });
    let mut handler = Some(handler);
    {
        let mut state = state.borrow_mut();
        if let Some(pending) = state.pending_reveal.as_mut()
            && pending.generation == generation
        {
            pending.items_changed_handler = handler.take();
        }
    }
    if let Some(handler) = handler {
        model.disconnect(handler);
    }
}

fn schedule_reveal_watchdog(state: &Rc<RefCell<ProjectState>>, generation: u64) {
    let weak = Rc::downgrade(state);
    let source =
        glib::timeout_add_local_once(Duration::from_millis(REVEAL_WATCHDOG_MS), move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let should_finish = {
                let mut state = state.borrow_mut();
                let Some(pending) = state.pending_reveal.as_mut() else {
                    return;
                };
                if pending.generation != generation {
                    return;
                }
                pending.watchdog_source = None;
                true
            };
            if should_finish {
                finish_reveal(&state, SelectionCleanup::Clear);
            }
        });
    let mut source = Some(source);
    {
        let mut state = state.borrow_mut();
        if let Some(pending) = state.pending_reveal.as_mut()
            && pending.generation == generation
        {
            pending.watchdog_source = source.take();
        }
    }
    if let Some(source) = source {
        source.remove();
    }
}

fn schedule_reveal_step(state: &Rc<RefCell<ProjectState>>, generation: u64) {
    {
        let Ok(mut state) = state.try_borrow_mut() else {
            let weak = Rc::downgrade(state);
            let _source = glib::idle_add_local_once(move || {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                schedule_reveal_step(&state, generation);
            });
            return;
        };
        let Some(pending) = state.pending_reveal.as_mut() else {
            return;
        };
        if pending.generation != generation || pending.step_scheduled {
            return;
        }
        pending.step_scheduled = true;
    }

    let weak = Rc::downgrade(state);
    let source = glib::idle_add_local_once(move || {
        let Some(state) = weak.upgrade() else {
            return;
        };
        {
            let mut state = state.borrow_mut();
            let Some(pending) = state.pending_reveal.as_mut() else {
                return;
            };
            if pending.generation != generation {
                return;
            }
            pending.step_scheduled = false;
            pending.step_source = None;
        }
        drive_reveal_step(&state, generation);
    });
    let mut source = Some(source);
    {
        let mut state = state.borrow_mut();
        if let Some(pending) = state.pending_reveal.as_mut()
            && pending.generation == generation
        {
            pending.step_source = source.take();
        }
    }
    if let Some(source) = source {
        source.remove();
    }
}

fn drive_reveal_step(state: &Rc<RefCell<ProjectState>>, generation: u64) {
    let step = reveal_step(state, generation);
    match step {
        RevealStep::Select(position) => {
            let selected = state.borrow().browser.tree().selection().clone();
            selected.set_selected(position);
            finish_reveal(state, SelectionCleanup::Keep);
        }
        RevealStep::Expand(row) => {
            row.set_expanded(true);
            if advance_reveal_index(state, generation) {
                schedule_reveal_step(state, generation);
            }
        }
        RevealStep::Advance => {
            if advance_reveal_index(state, generation) {
                schedule_reveal_step(state, generation);
            }
        }
        RevealStep::Wait => {}
        RevealStep::Unreachable => finish_reveal(state, SelectionCleanup::Clear),
        RevealStep::Finish => finish_reveal(state, SelectionCleanup::Keep),
    }
}

fn reveal_step(state: &Rc<RefCell<ProjectState>>, generation: u64) -> RevealStep {
    let state = state.borrow();
    let Some(pending) = state.pending_reveal.as_ref() else {
        return RevealStep::Finish;
    };
    if pending.generation != generation {
        return RevealStep::Finish;
    }
    let Some(wanted) = pending.chain.get(pending.index) else {
        return RevealStep::Finish;
    };
    let is_last = pending.index >= pending.chain.len().saturating_sub(1);
    let model = state.browser.tree().model().model();
    let Some((position, row)) = visible_reveal_row(model, wanted) else {
        return RevealStep::Wait;
    };
    if is_last {
        return RevealStep::Select(position);
    }
    if !row.is_expandable() {
        return RevealStep::Unreachable;
    }
    if row.is_expanded() {
        RevealStep::Advance
    } else {
        RevealStep::Expand(row)
    }
}

fn advance_reveal_index(state: &Rc<RefCell<ProjectState>>, generation: u64) -> bool {
    let mut state = state.borrow_mut();
    let Some(pending) = state.pending_reveal.as_mut() else {
        return false;
    };
    if pending.generation != generation {
        return false;
    }
    pending.index = pending.index.saturating_add(1);
    true
}

fn visible_reveal_row(
    model: &gtk4::TreeListModel,
    wanted: &gio::File,
) -> Option<(u32, gtk4::TreeListRow)> {
    #[cfg(test)]
    REVEAL_VISIBLE_ROW_SCANS.fetch_add(1, Ordering::Relaxed);

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
        let matches = {
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            entry.file.equal(wanted)
        };
        if matches {
            return Some((position, row));
        }
    }
    None
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
pub(super) fn reset_visible_row_scan_count_for_tests() {
    REVEAL_VISIBLE_ROW_SCANS.store(0, Ordering::Relaxed);
}

#[cfg(test)]
pub(super) fn visible_row_scan_count_for_tests() -> usize {
    REVEAL_VISIBLE_ROW_SCANS.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(super) fn reveal_pending_for_tests(state: &Rc<RefCell<ProjectState>>) -> bool {
    state.borrow().pending_reveal.is_some()
}

#[cfg(test)]
pub(super) fn reveal_file_for_tests(state: &Rc<RefCell<ProjectState>>, target: &gio::File) {
    let root = state.borrow().root.as_ref().map(|root| root.file.clone());
    if let Some(root) = root {
        start_reveal(state, &root, target);
    }
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
