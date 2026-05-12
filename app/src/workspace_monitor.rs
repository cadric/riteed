use std::rc::Rc;

use gettextrs::gettext;
use gtk4::prelude::GtkWindowExt;

use crate::dialogs::{self, ExternalReloadResponse};
use crate::editor_monitor::PendingExternalState;
use crate::editor_tab::{BannerActionKind, EditorTab, ReloadCause, ReloadResult, SaveResult};
use crate::workspace::Workspace;

pub(crate) fn install_tab_hooks(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    tab.set_external_state_handler(Rc::new(move || {
        if let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) {
            on_tab_external_state_changed(&workspace, &tab);
        }
    }));

    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    tab.set_external_action_handler(Rc::new(move || {
        if let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) {
            on_banner_action(&workspace, &tab);
        }
    }));
}

pub(crate) fn on_selected_tab_changed(workspace: &Rc<Workspace>) {
    if let Some(tab) = workspace.selected_tab() {
        sync_selected_tab(workspace, &tab);
    }
}

fn on_tab_external_state_changed(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let selected = is_selected_tab(workspace, tab);
    let window_active = workspace.shell.is_active();

    if matches!(
        tab.pending_external_state(),
        PendingExternalState::ContentPossiblyChanged {
            acknowledged: false
        }
    ) && tab.should_auto_reload(selected, window_active)
    {
        request_reload(workspace, tab, true);
        return;
    }

    tab.sync_external_banner(selected, window_active);
    workspace.refresh_selected_state();
    workspace.persist_session_state_if_needed();
    if selected {
        sync_selected_tab(workspace, tab);
    }
}

fn sync_selected_tab(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let window_active = workspace.shell.is_active();
    tab.sync_external_banner(true, window_active);
    if window_active && tab.should_present_dirty_reload_prompt() {
        tab.mark_external_prompt_active(true);
        let weak_workspace = Rc::downgrade(workspace);
        let weak_tab = Rc::downgrade(tab);
        dialogs::confirm_external_reload(&workspace.shell, &tab.title(), move |response| {
            if let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) {
                tab.mark_external_prompt_active(false);
                match response {
                    ExternalReloadResponse::Compare => {
                        request_compare_with_disk(&workspace, &tab);
                    }
                    ExternalReloadResponse::Reload => request_reload(&workspace, &tab, false),
                    ExternalReloadResponse::KeepCurrent => {
                        tab.acknowledge_pending_external();
                        workspace.refresh_selected_state();
                    }
                }
            }
        });
    }
}

fn on_banner_action(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    match tab.banner_action_kind() {
        Some(BannerActionKind::Reload) => request_reload(workspace, tab, false),
        Some(BannerActionKind::Save) => {
            let weak_workspace = Rc::downgrade(workspace);
            workspace.request_save_tab(
                tab,
                false,
                Rc::new(move |result| {
                    if let Some(workspace) = weak_workspace.upgrade()
                        && matches!(result, SaveResult::Saved(_))
                    {
                        workspace.refresh_selected_state();
                    }
                }),
            );
        }
        Some(BannerActionKind::SaveAs) => {
            workspace.request_save_tab(tab, true, Rc::new(|_result| {}));
        }
        Some(BannerActionKind::RefreshReview) => {
            workspace.refresh_review_tab(tab);
            workspace.refresh_selected_state();
        }
        None => {}
    }
}

fn request_reload(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>, automatic: bool) {
    let cause = if automatic {
        ReloadCause::Automatic
    } else {
        ReloadCause::UserRequested
    };
    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    let should_apply = Rc::new(move || {
        let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) else {
            return false;
        };
        if automatic {
            let selected = is_selected_tab(&workspace, &tab);
            !selected || !workspace.shell.is_active()
        } else {
            true
        }
    });

    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    tab.reload_from_disk(
        cause,
        should_apply,
        Rc::new(move |result| {
            if let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) {
                match result {
                    Ok(ReloadResult::Applied) => {
                        workspace.refresh_selected_state();
                        if workspace.shell.is_active() {
                            workspace.show_toast(&gettext("The File Was Reloaded."));
                        }
                    }
                    Ok(ReloadResult::Deferred) => {
                        let selected = is_selected_tab(&workspace, &tab);
                        let window_active = workspace.shell.is_active();
                        tab.sync_external_banner(selected, window_active);
                        workspace.refresh_selected_state();
                        workspace.persist_session_state_if_needed();
                        if selected {
                            sync_selected_tab(&workspace, &tab);
                        } else if !automatic {
                            workspace.refresh_selected_state();
                        }
                    }
                    Err(error) => dialogs::present_error(&workspace.shell, &error),
                }
            }
        }),
    );
}

fn request_compare_with_disk(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    tab.start_compare_with_disk(Rc::new(move |result| {
        if let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) {
            match result {
                Ok(()) => {
                    tab.acknowledge_pending_external();
                    workspace.refresh_selected_state();
                }
                Err(error) => dialogs::present_error(&workspace.shell, &error),
            }
        }
    }));
}

fn is_selected_tab(workspace: &Workspace, tab: &EditorTab) -> bool {
    workspace
        .selected_tab()
        .and_then(|selected| selected.page())
        .zip(tab.page())
        .is_some_and(|(left, right)| left == right)
}
