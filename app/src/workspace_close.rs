use std::rc::Rc;

use gtk4::prelude::*;

use crate::dialogs::{self, UnsavedResponse};
use crate::editor_tab::SaveResult;
use crate::workspace::Workspace;

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
    if workspace.state.borrow().close_flow.is_some() {
        return gtk4::glib::Propagation::Stop;
    }

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

pub(crate) fn on_page_detached(workspace: &Rc<Workspace>, page: &libadwaita::TabPage) {
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
        for tab in &state.tabs {
            if tab.page().as_ref().is_some_and(|item| item == page) {
                tab.clear_monitor();
            }
        }
        state
            .tabs
            .retain(|tab| tab.page().as_ref().is_none_or(|item| item != page));
        if state
            .close_flow
            .as_ref()
            .is_some_and(|coordinator| coordinator.matches_page(page))
        {
            state.close_flow = None;
        }
    }
    workspace.handle_selected_tab_changed();
    workspace.persist_session_state_if_needed();
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
    dialogs::confirm_unsaved_changes(&workspace.shell, &tab.title(), move |response| {
        if let Some(workspace) = weak.upgrade() {
            handle_close_response(&workspace, response);
        }
    });
}

fn handle_close_response(workspace: &Rc<Workspace>, response: UnsavedResponse) {
    let coordinator = workspace.state.borrow().close_flow.clone();
    let Some(coordinator) = coordinator else {
        return;
    };

    match response {
        UnsavedResponse::Cancel => {
            if let Some(page) = coordinator.pending_page() {
                workspace.tab_view.close_page_finish(&page, false);
            }
            workspace.state.borrow_mut().close_flow = None;
        }
        UnsavedResponse::Discard => advance_close_flow(workspace, &coordinator),
        UnsavedResponse::Save => {
            let Some(tab) = coordinator.current_tab() else {
                workspace.state.borrow_mut().close_flow = None;
                return;
            };
            let weak = Rc::downgrade(workspace);
            workspace.request_save_tab(
                &tab,
                false,
                Rc::new(move |result| {
                    if let Some(workspace) = weak.upgrade() {
                        handle_close_save_result(&workspace, &result);
                    }
                }),
            );
        }
    }
}

fn handle_close_save_result(workspace: &Rc<Workspace>, result: &SaveResult) {
    match result {
        SaveResult::Saved(_) => {
            if let Some(coordinator) = workspace.state.borrow().close_flow.clone() {
                advance_close_flow(workspace, &coordinator);
            }
        }
        SaveResult::CancelledByUser | SaveResult::Failed(_) => {
            if let Some(coordinator) = workspace.state.borrow().close_flow.clone()
                && let Some(page) = coordinator.pending_page()
            {
                workspace.tab_view.close_page_finish(&page, false);
            }
            workspace.state.borrow_mut().close_flow = None;
        }
    }
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

    coordinator.advance();
    if coordinator.is_complete() {
        workspace.state.borrow_mut().close_flow = None;
        finish_window_close(workspace);
    } else {
        present_close_dialog(workspace);
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
