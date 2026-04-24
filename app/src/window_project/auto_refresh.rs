use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;

use super::{ProjectState, reveal};

const AUTO_REFRESH_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone)]
pub(super) struct ProjectAutoRefresh {
    pending: Rc<Cell<bool>>,
    on_refresh: Rc<dyn Fn()>,
}

impl ProjectAutoRefresh {
    pub(super) fn new(on_refresh: Rc<dyn Fn()>) -> Self {
        Self {
            pending: Rc::new(Cell::new(false)),
            on_refresh,
        }
    }

    pub(super) fn schedule(&self) {
        if self.pending.replace(true) {
            return;
        }
        let pending = self.pending.clone();
        let on_refresh = self.on_refresh.clone();
        let _source = glib::timeout_add_local_once(AUTO_REFRESH_DELAY, move || {
            pending.set(false);
            on_refresh();
        });
    }
}

pub(super) fn refresh_tree(state: &Rc<std::cell::RefCell<ProjectState>>) {
    let expanded = {
        let state = state.borrow();
        if state.root.is_none() {
            return;
        }
        let tree = state.browser.tree().model();
        let expanded = tree.snapshot_expanded_uris();
        tree.refresh();
        expanded
    };

    reveal::schedule_restore_expanded(state, expanded);
    reveal::sync_reveal_for_selection(state);
}
