use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::{
    ProjectRoot, ProjectState, RootChangeHandler, RootChangeOrigin, reveal, sidebar_state,
};
use crate::error::AppError;
use crate::workspace::OpenSource;

const ROOT_QUERY_ATTRIBUTES: &str = "standard::type,standard::display-name";

struct RootApplyPlan {
    settings: crate::settings::AppSettings,
    show_hidden_action: gio::SimpleAction,
    handler: Option<RootChangeHandler>,
    uri: String,
    display_name: String,
    origin: RootChangeOrigin,
}

pub(super) fn apply_folder_filter(dialog: &gtk4::FileDialog) {
    let folder_filter = gtk4::FileFilter::new();
    folder_filter.set_name(Some(&pgettext("file filter", "Folders")));
    folder_filter.add_mime_type("inode/directory");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&folder_filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&folder_filter));
}

pub(super) fn open_file_from_tree(state: &Rc<RefCell<ProjectState>>, file: gio::File) {
    let workspace = state.borrow().workspace.upgrade();
    if let Some(workspace) = workspace {
        workspace.request_open_files(vec![file], OpenSource::ProjectTree);
    }
}

pub(super) fn begin_root_change(
    state: &Rc<RefCell<ProjectState>>,
    folder: &gio::File,
    origin: RootChangeOrigin,
) {
    reveal::cancel_reveal(state);
    let (generation, cancellable) = {
        let mut state_mut = state.borrow_mut();
        if let Some(cancellable) = state_mut.root_cancellable.take() {
            cancellable.cancel();
        }
        if let Some(cancellable) = state_mut.symlink_cancellable.take() {
            cancellable.cancel();
        }
        state_mut.symlink_generation += 1;
        state_mut.root_generation += 1;
        let generation = state_mut.root_generation;
        let cancellable = gio::Cancellable::new();
        state_mut.root_cancellable = Some(cancellable.clone());
        (generation, cancellable)
    };

    let state_for_callback = Rc::clone(state);
    let folder_for_callback = folder.clone();
    folder.query_info_async(
        ROOT_QUERY_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        Some(&cancellable),
        move |result| {
            if state_for_callback.borrow().root_generation != generation {
                return;
            }
            state_for_callback.borrow_mut().root_cancellable = None;
            match result {
                Ok(info) => finish_query(&state_for_callback, &folder_for_callback, origin, &info),
                Err(error) => {
                    let mut state_mut = state_for_callback.borrow_mut();
                    finish_query_error(&state_for_callback, &mut state_mut, origin, error);
                }
            }
        },
    );
}

fn finish_query(
    state: &Rc<RefCell<ProjectState>>,
    folder: &gio::File,
    origin: RootChangeOrigin,
    info: &gio::FileInfo,
) {
    let mut state_mut = state.borrow_mut();
    if info.file_type() != gio::FileType::Directory {
        if origin == RootChangeOrigin::Restore {
            state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
                "The previous project folder could not be restored.",
            )));
            drop(state_mut);
            sync_root_none(state, false);
        } else {
            state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
                "That folder could not be opened.",
            )));
        }
        return;
    }

    let plan = apply_root_change(&mut state_mut, folder, info, origin);
    drop(state_mut);
    let sidebar_visible = finish_root_change(state, folder, plan);
    sidebar_state::set_sidebar_visible_for_root(state, sidebar_visible);
    sidebar_state::sync_actions_for_root(state);
    reveal::sync_reveal_for_selection(state);
}

fn finish_query_error(
    state: &Rc<RefCell<ProjectState>>,
    state_mut: &mut ProjectState,
    origin: RootChangeOrigin,
    error: glib::Error,
) {
    if error.matches(gio::IOErrorEnum::Cancelled) {
        return;
    }
    if origin == RootChangeOrigin::Restore {
        state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
            "The previous project folder could not be restored.",
        )));
        sync_root_none(state, false);
        return;
    }
    state_mut
        .toast_overlay
        .add_toast(adw::Toast::new(&AppError::from(error).body()));
}

fn apply_root_change(
    state: &mut ProjectState,
    folder: &gio::File,
    info: &gio::FileInfo,
    origin: RootChangeOrigin,
) -> RootApplyPlan {
    let uri = folder.uri().to_string();
    let display_name = resolve_display_name(folder, info);
    state.root = Some(ProjectRoot {
        file: folder.clone(),
    });
    state.toast_keys.clear();

    RootApplyPlan {
        settings: state.settings.clone(),
        show_hidden_action: state.show_hidden_action.clone(),
        handler: state.root_change_handler.clone(),
        uri,
        display_name,
        origin,
    }
}

fn finish_root_change(
    state: &Rc<RefCell<ProjectState>>,
    folder: &gio::File,
    plan: RootApplyPlan,
) -> bool {
    plan.settings.set_project_folder_uri(&plan.uri);
    plan.settings
        .set_project_folder_display_name(&plan.display_name);

    let sidebar_visible = match plan.origin {
        RootChangeOrigin::UserOpen | RootChangeOrigin::AppOpen => {
            plan.settings.set_project_sidebar_visible(true);
            true
        }
        RootChangeOrigin::Restore => plan.settings.project_sidebar_visible(),
    };

    let show_hidden = plan.settings.project_show_hidden();
    plan.show_hidden_action.set_state(&show_hidden.to_variant());
    {
        let state = state.borrow();
        state.browser.set_title(&plan.display_name);
        state.browser.tree().set_root(Some(folder.clone()));
        state.browser.tree().model().set_show_hidden(show_hidden);
    }
    if let Some(handler) = plan.handler {
        handler(Some(folder.clone()));
    }
    sidebar_visible
}

fn resolve_display_name(folder: &gio::File, info: &gio::FileInfo) -> String {
    let display_name = info.display_name().to_string();
    if !display_name.is_empty() {
        return display_name;
    }

    folder.basename().map_or_else(
        || folder.uri().to_string(),
        |name| name.to_string_lossy().to_string(),
    )
}

pub(super) fn close_root(state: &Rc<RefCell<ProjectState>>) {
    reveal::cancel_reveal(state);
    {
        let mut state = state.borrow_mut();
        if let Some(cancellable) = state.root_cancellable.take() {
            cancellable.cancel();
        }
        if let Some(cancellable) = state.symlink_cancellable.take() {
            cancellable.cancel();
        }
        state.symlink_generation += 1;
        state.root = None;
        state.settings.set_project_folder_uri("");
        state.settings.set_project_folder_display_name("");
        state.settings.set_project_sidebar_visible(false);
        state.browser.set_title(&gettext("Project"));
        state.browser.tree().set_root(None);
        state.toast_keys.clear();
        if let Some(handler) = state.root_change_handler.as_ref() {
            handler(None);
        }
    }
    sidebar_state::sync_actions_for_root(state);
}

pub(super) fn sync_root_none(state: &Rc<RefCell<ProjectState>>, clear_settings: bool) {
    reveal::cancel_reveal(state);
    {
        let mut state = state.borrow_mut();
        if let Some(cancellable) = state.symlink_cancellable.take() {
            cancellable.cancel();
        }
        state.symlink_generation += 1;
        state.root = None;
        state.browser.set_title(&gettext("Project"));
        state.browser.tree().set_root(None);
        if let Some(handler) = state.root_change_handler.as_ref() {
            handler(None);
        }
        if clear_settings {
            state.settings.set_project_folder_uri("");
            state.settings.set_project_folder_display_name("");
            state.settings.set_project_sidebar_visible(false);
        }
    }
    sidebar_state::sync_actions_for_root(state);
}
