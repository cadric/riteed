use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use libadwaita as adw;

use crate::editor_tab::EditorTab;

#[derive(Clone)]
pub enum CloseMode {
    Tab(adw::TabPage),
    Window,
    OtherTabs,
}

struct CloseState {
    mode: CloseMode,
    queue: VecDeque<Rc<EditorTab>>,
    discarded: Vec<DiscardedRevision>,
}

struct DiscardedRevision {
    tab: Weak<EditorTab>,
    uri: Option<String>,
    dirty_generation: u64,
}

pub struct CloseCoordinator {
    state: RefCell<CloseState>,
}

impl CloseCoordinator {
    #[must_use]
    pub fn for_tab(page: &adw::TabPage, tab: Rc<EditorTab>) -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(CloseState {
                mode: CloseMode::Tab(page.clone()),
                queue: VecDeque::from([tab]),
                discarded: Vec::new(),
            }),
        })
    }

    #[must_use]
    pub fn for_window(queue: Vec<Rc<EditorTab>>) -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(CloseState {
                mode: CloseMode::Window,
                queue: VecDeque::from(queue),
                discarded: Vec::new(),
            }),
        })
    }

    #[must_use]
    pub fn for_other_tabs(queue: Vec<Rc<EditorTab>>) -> Rc<Self> {
        Rc::new(Self {
            state: RefCell::new(CloseState {
                mode: CloseMode::OtherTabs,
                queue: VecDeque::from(queue),
                discarded: Vec::new(),
            }),
        })
    }

    #[must_use]
    pub fn current_tab(&self) -> Option<Rc<EditorTab>> {
        self.state.borrow().queue.front().cloned()
    }

    #[must_use]
    pub(crate) fn contains_page(&self, page: &adw::TabPage) -> bool {
        self.state.borrow().queue.iter().any(|tab| {
            tab.page()
                .as_ref()
                .is_some_and(|candidate| candidate == page)
        })
    }

    pub fn advance(&self) {
        let _removed = self.state.borrow_mut().queue.pop_front();
    }

    pub(crate) fn record_discard(&self, tab: &Rc<EditorTab>) {
        let revision = DiscardedRevision {
            tab: Rc::downgrade(tab),
            uri: tab.document_uri(),
            dirty_generation: tab.dirty_generation(),
        };
        self.state.borrow_mut().discarded.push(revision);
    }

    #[must_use]
    pub(crate) fn permits_discard(&self, tab: &Rc<EditorTab>) -> bool {
        let uri = tab.document_uri();
        let generation = tab.dirty_generation();
        self.state.borrow().discarded.iter().any(|revision| {
            revision.tab.ptr_eq(&Rc::downgrade(tab))
                && revision.uri == uri
                && revision.dirty_generation == generation
        })
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state.borrow().queue.is_empty()
    }

    #[must_use]
    pub fn pending_page(&self) -> Option<adw::TabPage> {
        match self.state.borrow().mode.clone() {
            CloseMode::Tab(page) => Some(page),
            CloseMode::OtherTabs => self.current_tab().and_then(|tab| tab.page()),
            CloseMode::Window => None,
        }
    }

    #[must_use]
    pub fn matches_page(&self, candidate: &adw::TabPage) -> bool {
        self.pending_page()
            .as_ref()
            .is_some_and(|page| page == candidate)
    }

    #[must_use]
    pub fn is_tab_close(&self) -> bool {
        matches!(self.state.borrow().mode, CloseMode::Tab(_))
    }

    #[must_use]
    pub fn is_other_tabs_close(&self) -> bool {
        matches!(self.state.borrow().mode, CloseMode::OtherTabs)
    }
}

#[cfg(test)]
mod tests {
    use super::CloseCoordinator;

    #[test]
    fn empty_window_coordinator_is_complete() {
        let coordinator = CloseCoordinator::for_window(Vec::new());
        assert!(coordinator.is_complete());
    }
}
