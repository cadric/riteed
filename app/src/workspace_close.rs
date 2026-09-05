use std::rc::Rc;

use gettextrs::gettext;
use gtk4::prelude::*;

use crate::close_flow::CloseCoordinator;
use crate::dialogs::{self, UnsavedResponse};
use crate::editor_tab::{EditorTab, SaveResult};
use crate::workspace::Workspace;

#[cfg(test)]
#[path = "gtk_tests_document_close_lifecycle.rs"]
pub(crate) mod tests;

pub(crate) fn handle_window_close_request(workspace: &Rc<Workspace>) -> gtk4::glib::Propagation {
    if workspace.state.borrow().allow_window_close {
        return gtk4::glib::Propagation::Proceed;
    }
    if workspace.state.borrow().close_flow.is_some() {
        return gtk4::glib::Propagation::Stop;
    }

    let dirty_tabs =
        dirty_tabs_for_window_close(workspace.selected_tab(), workspace.ordered_tabs());
    if dirty_tabs.is_empty() {
        return gtk4::glib::Propagation::Proceed;
    }

    workspace.state.borrow_mut().close_flow =
        Some(crate::close_flow::CloseCoordinator::for_window(dirty_tabs));
    present_close_dialog(workspace);
    gtk4::glib::Propagation::Stop
}

pub(crate) fn on_close_page(
    workspace: &Rc<Workspace>,
    page: &libadwaita::TabPage,
) -> gtk4::glib::Propagation {
    let coordinator = workspace.state.borrow().close_flow.clone();
    if let Some(coordinator) = coordinator {
        if !coordinator.is_other_tabs_close() || !coordinator.matches_page(page) {
            workspace.tab_view.close_page_finish(page, false);
            return gtk4::glib::Propagation::Stop;
        }
        return handle_expected_page_close(workspace, page);
    }

    handle_single_page_close(workspace, page)
}

pub(crate) fn request_close_other_tabs(workspace: &Rc<Workspace>, keep_page: &libadwaita::TabPage) {
    if workspace.state.borrow().close_flow.is_some() {
        return;
    }
    let queue = workspace
        .ordered_tabs()
        .into_iter()
        .filter(|tab| tab.page().as_ref().is_some_and(|page| page != keep_page))
        .collect::<Vec<_>>();
    if queue.is_empty() {
        return;
    }
    workspace.state.borrow_mut().close_flow =
        Some(crate::close_flow::CloseCoordinator::for_other_tabs(queue));
    workspace.sync_tab_action_state();
    close_next_other_tab(workspace);
}

fn handle_single_page_close(
    workspace: &Rc<Workspace>,
    page: &libadwaita::TabPage,
) -> gtk4::glib::Propagation {
    let Some(tab) = workspace.find_tab_by_page(page) else {
        return gtk4::glib::Propagation::Proceed;
    };

    if !tab.is_dirty() {
        workspace.tab_view.close_page_finish(page, true);
        return gtk4::glib::Propagation::Stop;
    }

    workspace.state.borrow_mut().close_flow =
        Some(crate::close_flow::CloseCoordinator::for_tab(page, tab));
    present_close_dialog(workspace);
    gtk4::glib::Propagation::Stop
}

fn handle_expected_page_close(
    workspace: &Rc<Workspace>,
    page: &libadwaita::TabPage,
) -> gtk4::glib::Propagation {
    let Some(tab) = workspace.find_tab_by_page(page) else {
        return gtk4::glib::Propagation::Stop;
    };

    if !tab.is_dirty() {
        workspace.tab_view.close_page_finish(page, true);
        return gtk4::glib::Propagation::Stop;
    }

    present_close_dialog(workspace);
    gtk4::glib::Propagation::Stop
}

pub(crate) fn on_page_detached(workspace: &Rc<Workspace>, page: &libadwaita::TabPage) {
    let should_continue_other_tabs;
    let mut rejected_page = None;
    {
        let Ok(mut state) = workspace.state.try_borrow_mut() else {
            let weak = Rc::downgrade(workspace);
            let detached_page = page.clone();
            gtk4::glib::idle_add_local_once(move || {
                if let Some(workspace) = weak.upgrade() {
                    on_page_detached(&workspace, &detached_page);
                }
            });
            return;
        };
        let is_transfer = workspace.is_transferring_page(page);
        for tab in &state.tabs {
            if !is_transfer && tab.page().as_ref().is_some_and(|item| item == page) {
                tab.cancel_io();
                tab.clear_large_file_surface();
                tab.clear_zoom_style();
                tab.clear_monitor();
            }
        }
        state
            .tabs
            .retain(|tab| tab.page().as_ref().is_none_or(|item| item != page));
        let close_flow = state.close_flow.clone();
        should_continue_other_tabs = close_flow.as_ref().is_some_and(|coordinator| {
            coordinator.is_other_tabs_close() && coordinator.matches_page(page)
        });
        if should_continue_other_tabs {
            if let Some(coordinator) = close_flow {
                coordinator.advance();
                if coordinator.is_complete() {
                    state.close_flow = None;
                }
            }
        } else if state.close_flow.as_ref().is_some_and(|coordinator| {
            coordinator.contains_page(page)
                || (coordinator.is_tab_close() && coordinator.matches_page(page))
        }) {
            rejected_page = state
                .close_flow
                .as_ref()
                .and_then(|flow| flow.pending_page());
            state.close_flow = None;
        }
    }
    if let Some(page) = rejected_page
        && workspace.find_tab_by_page(&page).is_some()
    {
        workspace.tab_view.close_page_finish(&page, false);
    }
    workspace.clear_transfer_guard(page);
    workspace.handle_selected_tab_changed();
    workspace.persist_session_state_if_needed();
    if should_continue_other_tabs {
        close_next_other_tab(workspace);
    } else {
        workspace.sync_tab_action_state();
    }
    if workspace.tab_view.n_pages() == 0 {
        workspace.shell.close();
    }
}

fn present_close_dialog(workspace: &Rc<Workspace>) {
    let coordinator = workspace.state.borrow().close_flow.clone();
    let Some(coordinator) = coordinator else {
        return;
    };
    let Some(tab) = coordinator.current_tab() else {
        return;
    };

    let weak = Rc::downgrade(workspace);
    let weak_coordinator = Rc::downgrade(&coordinator);
    let weak_tab = Rc::downgrade(&tab);
    dialogs::confirm_unsaved_changes(&workspace.shell, &tab.title(), move |response| {
        if let (Some(workspace), Some(coordinator), Some(tab)) = (
            weak.upgrade(),
            weak_coordinator.upgrade(),
            weak_tab.upgrade(),
        ) {
            handle_close_response(&workspace, &coordinator, &tab, response);
        }
    });
}

fn handle_close_response(
    workspace: &Rc<Workspace>,
    coordinator: &Rc<CloseCoordinator>,
    tab: &Rc<EditorTab>,
    response: UnsavedResponse,
) {
    if !is_current_close_target(workspace, coordinator, tab) {
        return;
    }

    match response {
        UnsavedResponse::Cancel => cancel_close_flow(workspace, coordinator),
        UnsavedResponse::Discard => {
            coordinator.record_discard(tab);
            advance_close_flow(workspace, coordinator);
        }
        UnsavedResponse::Save => {
            let weak = Rc::downgrade(workspace);
            let weak_coordinator = Rc::downgrade(coordinator);
            let weak_tab = Rc::downgrade(tab);
            workspace.request_save_tab(
                tab,
                false,
                Rc::new(move |result| {
                    if let (Some(workspace), Some(coordinator), Some(tab)) = (
                        weak.upgrade(),
                        weak_coordinator.upgrade(),
                        weak_tab.upgrade(),
                    ) {
                        handle_close_save_result(&workspace, &coordinator, &tab, &result);
                    }
                }),
            );
        }
    }
}

fn handle_close_save_result(
    workspace: &Rc<Workspace>,
    coordinator: &Rc<CloseCoordinator>,
    tab: &Rc<EditorTab>,
    result: &SaveResult,
) {
    if !is_current_close_target(workspace, coordinator, tab) {
        return;
    }
    match result {
        SaveResult::Saved(outcome) => {
            if tab.is_dirty() || tab.document_uri().as_deref() != Some(outcome.new_uri.as_str()) {
                cancel_close_for_edits(workspace, coordinator);
            } else {
                advance_close_flow(workspace, coordinator);
            }
        }
        SaveResult::CancelledByUser | SaveResult::Failed(_) => {
            cancel_close_flow(workspace, coordinator);
        }
    }
}

fn is_current_close_target(
    workspace: &Workspace,
    coordinator: &Rc<CloseCoordinator>,
    tab: &Rc<EditorTab>,
) -> bool {
    let current = workspace.state.borrow().close_flow.clone();
    current.is_some_and(|current| Rc::ptr_eq(&current, coordinator))
        && coordinator
            .current_tab()
            .is_some_and(|current| Rc::ptr_eq(&current, tab))
        && tab
            .page()
            .and_then(|page| workspace.find_tab_by_page(&page))
            .is_some_and(|current| Rc::ptr_eq(&current, tab))
}

pub(crate) fn is_close_target(workspace: &Workspace, tab: &Rc<EditorTab>) -> bool {
    let coordinator = workspace.state.borrow().close_flow.clone();
    coordinator
        .and_then(|coordinator| coordinator.current_tab())
        .is_some_and(|current| Rc::ptr_eq(&current, tab))
}

fn cancel_close_flow(workspace: &Workspace, coordinator: &Rc<CloseCoordinator>) {
    let owns_flow = workspace
        .state
        .borrow()
        .close_flow
        .as_ref()
        .is_some_and(|current| Rc::ptr_eq(current, coordinator));
    if !owns_flow {
        return;
    }
    let pending_page = coordinator.pending_page();
    workspace.state.borrow_mut().close_flow = None;
    if let Some(page) = pending_page
        && workspace.find_tab_by_page(&page).is_some()
    {
        workspace.tab_view.close_page_finish(&page, false);
    }
    workspace.sync_tab_action_state();
}

fn cancel_close_for_edits(workspace: &Workspace, coordinator: &Rc<CloseCoordinator>) {
    cancel_close_flow(workspace, coordinator);
    workspace.show_toast(&gettext(
        "Closing was cancelled because the document has newer unsaved changes.",
    ));
}

fn advance_close_flow(
    workspace: &Rc<Workspace>,
    coordinator: &Rc<crate::close_flow::CloseCoordinator>,
) {
    if coordinator.is_tab_close() {
        if let Some(page) = coordinator.pending_page() {
            coordinator.advance();
            workspace.tab_view.close_page_finish(&page, true);
        }
        return;
    }
    if coordinator.is_other_tabs_close() {
        if let Some(page) = coordinator.pending_page() {
            workspace.tab_view.close_page_finish(&page, true);
        }
        return;
    }

    coordinator.advance();
    if coordinator.is_complete() {
        if workspace
            .ordered_tabs()
            .iter()
            .any(|tab| tab.is_dirty() && !coordinator.permits_discard(tab))
        {
            cancel_close_for_edits(workspace, coordinator);
            return;
        }
        workspace.state.borrow_mut().close_flow = None;
        workspace.sync_tab_action_state();
        finish_window_close(workspace);
    } else {
        present_close_dialog(workspace);
    }
}

fn close_next_other_tab(workspace: &Rc<Workspace>) {
    loop {
        let coordinator = workspace.state.borrow().close_flow.clone();
        let Some(coordinator) = coordinator.filter(|coordinator| coordinator.is_other_tabs_close())
        else {
            workspace.sync_tab_action_state();
            return;
        };
        let Some(tab) = coordinator.current_tab() else {
            workspace.state.borrow_mut().close_flow = None;
            workspace.sync_tab_action_state();
            return;
        };
        let Some(page) = tab.page() else {
            coordinator.advance();
            continue;
        };
        if workspace.find_tab_by_page(&page).is_some() {
            workspace.tab_view.close_page(&page);
            return;
        }
        coordinator.advance();
    }
}

fn finish_window_close(workspace: &Workspace) {
    workspace.state.borrow_mut().allow_window_close = true;
    workspace.shell.close();
}

fn dirty_tabs_for_window_close(
    selected: Option<Rc<crate::editor_tab::EditorTab>>,
    ordered: Vec<Rc<crate::editor_tab::EditorTab>>,
) -> Vec<Rc<crate::editor_tab::EditorTab>> {
    let mut dirty_tabs = Vec::new();
    if let Some(selected_tab) = selected {
        if selected_tab.is_dirty() {
            dirty_tabs.push(selected_tab.clone());
        }
        for tab in ordered {
            let same_uri = selected_tab
                .page()
                .zip(tab.page())
                .is_some_and(|(left, right)| left == right);
            if !same_uri && tab.is_dirty() {
                dirty_tabs.push(tab);
            }
        }
        return dirty_tabs;
    }

    ordered.into_iter().filter(|tab| tab.is_dirty()).collect()
}
