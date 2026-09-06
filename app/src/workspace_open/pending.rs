use std::rc::Rc;

use gtk4::{gio, prelude::*};

use crate::editor_tab::EditorTab;
use crate::workspace::{PendingOpenTarget, PendingOpenToken, Workspace};

#[cfg(test)]
#[path = "../gtk_tests_pending_open.rs"]
pub(crate) mod tests;

pub(super) fn acquire_open_target(workspace: &Rc<Workspace>) -> (Rc<EditorTab>, bool) {
    if workspace.tab_view.n_pages() == 1 {
        let existing = workspace.ordered_tabs();
        if let Some(tab) = existing.first()
            && tab.is_clean_untitled()
            && !tab.is_loading()
            && !is_pending_open_target(workspace, tab)
        {
            return (tab.clone(), false);
        }
    }
    (workspace.add_empty_tab(true), true)
}

pub(super) fn register_pending_open(
    workspace: &Workspace,
    file: &gio::File,
    tab: &Rc<EditorTab>,
) -> PendingOpenToken {
    let mut state = workspace.state.borrow_mut();
    state.pending_open_generation = state.pending_open_generation.saturating_add(1);
    let token = PendingOpenToken::new(state.pending_open_generation);
    state.pending_open_targets.push(PendingOpenTarget {
        uri: file.uri().to_string(),
        tab: Rc::downgrade(tab),
        token,
    });
    token
}

pub(super) fn clear_pending_open(
    workspace: &Workspace,
    file: &gio::File,
    tab: &Rc<EditorTab>,
    token: PendingOpenToken,
) {
    let uri = file.uri().to_string();
    workspace
        .state
        .borrow_mut()
        .pending_open_targets
        .retain(|pending| {
            pending.token != token
                || pending.uri != uri
                || pending
                    .tab
                    .upgrade()
                    .is_none_or(|owner| !Rc::ptr_eq(&owner, tab))
        });
}

pub(super) fn find_tab_by_file(workspace: &Workspace, file: &gio::File) -> Option<Rc<EditorTab>> {
    workspace
        .ordered_tabs()
        .into_iter()
        .find(|tab| {
            tab.session_uri()
                .as_deref()
                .is_some_and(|uri| file.equal(&gio::File::for_uri(uri)))
        })
        .or_else(|| {
            workspace
                .state
                .borrow()
                .pending_open_targets
                .iter()
                .filter(|pending| file.equal(&gio::File::for_uri(&pending.uri)))
                .filter_map(|pending| pending.tab.upgrade())
                .find(|tab| tab_is_attached(workspace, tab))
        })
}

fn is_pending_open_target(workspace: &Workspace, tab: &Rc<EditorTab>) -> bool {
    workspace
        .state
        .borrow()
        .pending_open_targets
        .iter()
        .filter_map(|pending| pending.tab.upgrade())
        .any(|owner| Rc::ptr_eq(&owner, tab))
}

fn tab_is_attached(workspace: &Workspace, tab: &Rc<EditorTab>) -> bool {
    tab.page()
        .and_then(|page| workspace.find_tab_by_page(&page))
        .is_some_and(|attached| Rc::ptr_eq(&attached, tab))
}
