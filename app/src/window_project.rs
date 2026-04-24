use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::error::AppError;
use crate::project_browser::ProjectBrowser;
use crate::project_tree::ProjectTreeActivation;
use crate::settings::AppSettings;
use crate::window_shell::WindowShell;
use crate::workspace::{OpenSource, Workspace};

const ROOT_QUERY_ATTRIBUTES: &str = "standard::type,standard::display-name";

mod app_open;
mod reveal;
mod symlink;

#[derive(Clone, Debug)]
struct ProjectRoot {
    file: gio::File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootChangeOrigin {
    Restore,
    UserOpen,
    AppOpen,
}

type TreeActivationHandler = Rc<dyn Fn(ProjectTreeActivation)>;

struct ProjectState {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    split_view: adw::OverlaySplitView,
    settings: AppSettings,
    workspace: Weak<Workspace>,
    browser: ProjectBrowser,
    root: Option<ProjectRoot>,

    sidebar_visible_action: gio::SimpleAction,
    show_hidden_action: gio::SimpleAction,
    refresh_action: gio::SimpleAction,
    close_action: gio::SimpleAction,

    root_generation: u64,
    root_cancellable: Option<gio::Cancellable>,

    reveal_generation: u64,
    pending_reveal: Option<reveal::PendingReveal>,

    symlink_generation: u64,
    symlink_cancellable: Option<gio::Cancellable>,
    toast_keys: HashSet<String>,
}

pub struct WindowProjectController {
    state: Rc<RefCell<ProjectState>>,
}

impl WindowProjectController {
    #[must_use]
    pub fn new(shell: &WindowShell, settings: &AppSettings, workspace: &Rc<Workspace>) -> Self {
        let sidebar_visible_action =
            gio::SimpleAction::new_stateful("project-sidebar-visible", None, &false.to_variant());
        let show_hidden_action = gio::SimpleAction::new_stateful(
            "project-show-hidden",
            None,
            &settings.project_show_hidden().to_variant(),
        );
        let refresh_action = gio::SimpleAction::new("refresh-project-tree", None);
        let close_action = gio::SimpleAction::new("close-folder", None);

        shell.window.add_action(&sidebar_visible_action);
        shell.window.add_action(&show_hidden_action);
        shell.window.add_action(&refresh_action);
        shell.window.add_action(&close_action);

        let tree_activation_handler: Rc<RefCell<Option<TreeActivationHandler>>> =
            Rc::new(RefCell::new(None));
        let tree_activation_handler_cell = Rc::clone(&tree_activation_handler);

        let toast_overlay = shell.toast_overlay.clone();
        let browser = ProjectBrowser::new(Rc::new(move |activation| {
            let handler = tree_activation_handler_cell
                .borrow()
                .as_ref()
                .map(Rc::clone);
            if let Some(handler) = handler {
                handler(activation);
            }
        }));
        shell.project_split_view.set_sidebar(Some(browser.widget()));

        let state = Rc::new(RefCell::new(ProjectState {
            window: shell.window.clone(),
            toast_overlay,
            split_view: shell.project_split_view.clone(),
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            browser,
            root: None,
            sidebar_visible_action,
            show_hidden_action,
            refresh_action,
            close_action,
            root_generation: 0,
            root_cancellable: None,
            reveal_generation: 0,
            pending_reveal: None,
            symlink_generation: 0,
            symlink_cancellable: None,
            toast_keys: HashSet::new(),
        }));
        let state_weak = Rc::downgrade(&state);
        let handler: TreeActivationHandler = Rc::new(move |activation| {
            let Some(state) = state_weak.upgrade() else {
                return;
            };
            match activation {
                ProjectTreeActivation::RegularFile(file) => open_file_from_tree(&state, file),
                ProjectTreeActivation::Symlink(file) => {
                    symlink::handle_symlink_activation(&state, &file);
                }
            }
        });
        *tree_activation_handler.borrow_mut() = Some(handler);

        let controller = Self { state };

        controller.install_callbacks();
        controller.restore_from_settings();
        controller
    }

    pub fn request_open_folder_dialog(&self) {
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Open Folder"))
            .accept_label(pgettext("file dialog action", "Open"))
            .modal(true)
            .build();

        let state = Rc::downgrade(&self.state);
        dialog.select_folder(
            Some(&self.state.borrow().window),
            None::<&gio::Cancellable>,
            move |result| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                match result {
                    Ok(folder) => begin_root_change(&state, &folder, RootChangeOrigin::UserOpen),
                    Err(error) => {
                        if !error.matches(gtk4::DialogError::Dismissed) {
                            state
                                .borrow()
                                .toast_overlay
                                .add_toast(adw::Toast::new(&AppError::from(error).body()));
                        }
                    }
                }
            },
        );
    }

    pub(crate) fn handle_application_open(&self, files: Vec<gio::File>) {
        app_open::handle_application_open(&self.state, files);
    }

    pub(crate) fn restore_before_session(&self) {
        self.restore_from_settings();
    }

    #[cfg(test)]
    pub(crate) fn root_uri_for_tests(&self) -> Option<String> {
        self.state
            .borrow()
            .root
            .as_ref()
            .map(|root| root.file.uri().to_string())
    }

    #[cfg(test)]
    pub(crate) fn action_states_for_tests(&self) -> (bool, bool, bool, bool) {
        let state = self.state.borrow();
        (
            state.sidebar_visible_action.is_enabled(),
            state.show_hidden_action.is_enabled(),
            state.refresh_action.is_enabled(),
            state.close_action.is_enabled(),
        )
    }

    #[cfg(test)]
    pub(crate) fn tree_entry_names_for_tests(&self) -> Vec<String> {
        self.state
            .borrow()
            .browser
            .tree()
            .model()
            .visible_entry_names_for_tests()
    }

    #[cfg(test)]
    pub(crate) fn close_for_tests(&self) {
        close_root(&self.state);
    }

    #[cfg(test)]
    pub(crate) fn refresh_for_tests(&self) {
        refresh_tree(&self.state);
    }

    #[cfg(test)]
    pub(crate) fn set_show_hidden_for_tests(&self, show_hidden: bool) {
        let action = self.state.borrow().show_hidden_action.clone();
        action.change_state(&show_hidden.to_variant());
    }

    #[cfg(test)]
    pub(crate) fn resolve_symlink_for_tests(&self, file: &gio::File) {
        symlink::handle_symlink_activation(&self.state, file);
    }

    fn restore_from_settings(&self) {
        let uri = self.state.borrow().settings.project_folder_uri();
        if uri.is_empty() {
            sync_root_none(&self.state, false);
            return;
        }
        let folder = gio::File::for_uri(&uri);
        begin_root_change(&self.state, &folder, RootChangeOrigin::Restore);
    }

    fn install_callbacks(&self) {
        let state = Rc::downgrade(&self.state);
        self.state
            .borrow()
            .sidebar_visible_action
            .connect_change_state(move |action, value| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let Some(value) = value.and_then(glib::Variant::get::<bool>) else {
                    return;
                };
                action.set_state(&value.to_variant());
                let state = state.borrow();
                if state.root.is_none() {
                    return;
                }
                state.split_view.set_show_sidebar(value);
                state.settings.set_project_sidebar_visible(value);
            });

        let state = Rc::downgrade(&self.state);
        self.state
            .borrow()
            .show_hidden_action
            .connect_change_state(move |action, value| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let Some(value) = value.and_then(glib::Variant::get::<bool>) else {
                    return;
                };
                action.set_state(&value.to_variant());
                if state.borrow().root.is_none() {
                    return;
                }
                let expanded = state
                    .borrow()
                    .browser
                    .tree()
                    .model()
                    .snapshot_expanded_uris();
                state.borrow().settings.set_project_show_hidden(value);
                state.borrow().browser.tree().model().set_show_hidden(value);
                reveal::schedule_restore_expanded(&state, expanded);
                reveal::sync_reveal_for_selection(&state);
            });

        let state = Rc::downgrade(&self.state);
        self.state
            .borrow()
            .refresh_action
            .connect_activate(move |_, _| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                refresh_tree(&state);
            });

        let state = Rc::downgrade(&self.state);
        self.state
            .borrow()
            .close_action
            .connect_activate(move |_, _| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                close_root(&state);
            });

        let state = Rc::downgrade(&self.state);
        self.state
            .borrow()
            .split_view
            .connect_show_sidebar_notify(move |split_view| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let state = state.borrow();
                if state.root.is_none() {
                    if split_view.shows_sidebar() {
                        split_view.set_show_sidebar(false);
                    }
                    return;
                }
                let value = split_view.shows_sidebar();
                state.settings.set_project_sidebar_visible(value);
                state.sidebar_visible_action.set_state(&value.to_variant());
            });

        let state = Rc::downgrade(&self.state);
        if let Some(workspace) = self.state.borrow().workspace.upgrade() {
            workspace.tab_view.connect_selected_page_notify(move |_| {
                if let Some(state) = state.upgrade() {
                    reveal::sync_reveal_for_selection(&state);
                }
            });
        }

        sync_actions_for_root(&self.state);
    }
}

fn sync_actions_for_root(state: &Rc<RefCell<ProjectState>>) {
    let state = state.borrow();
    let has_root = state.root.is_some();
    state.sidebar_visible_action.set_enabled(has_root);
    state.show_hidden_action.set_enabled(has_root);
    state.refresh_action.set_enabled(has_root);
    state.close_action.set_enabled(has_root);
    if !has_root {
        state.sidebar_visible_action.set_state(&false.to_variant());
        state.split_view.set_show_sidebar(false);
    }
}

fn open_file_from_tree(state: &Rc<RefCell<ProjectState>>, file: gio::File) {
    let workspace = state.borrow().workspace.upgrade();
    if let Some(workspace) = workspace {
        workspace.request_open_files(vec![file], OpenSource::ProjectTree);
    }
}

fn begin_root_change(
    state: &Rc<RefCell<ProjectState>>,
    folder: &gio::File,
    origin: RootChangeOrigin,
) {
    let (generation, cancellable) = {
        let mut state_mut = state.borrow_mut();
        if let Some(cancellable) = state_mut.root_cancellable.take() {
            cancellable.cancel();
        }
        if let Some(pending) = state_mut.pending_reveal.take() {
            pending.cancellable.cancel();
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
            let mut state_mut = state_for_callback.borrow_mut();
            state_mut.root_cancellable = None;
            match result {
                Ok(info) => {
                    if info.file_type() != gio::FileType::Directory {
                        if origin == RootChangeOrigin::Restore {
                            state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
                                "The previous project folder could not be restored.",
                            )));
                            drop(state_mut);
                            sync_root_none(&state_for_callback, false);
                        } else {
                            state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
                                "That folder could not be opened.",
                            )));
                        }
                        return;
                    }

                    let sidebar_visible =
                        apply_root_change(&mut state_mut, &folder_for_callback, &info, origin);
                    drop(state_mut);
                    set_sidebar_visible_for_root(&state_for_callback, sidebar_visible);
                    sync_actions_for_root(&state_for_callback);
                    reveal::sync_reveal_for_selection(&state_for_callback);
                }
                Err(error) => {
                    if error.matches(gio::IOErrorEnum::Cancelled) {
                        return;
                    }
                    if origin == RootChangeOrigin::Restore {
                        state_mut.toast_overlay.add_toast(adw::Toast::new(&gettext(
                            "The previous project folder could not be restored.",
                        )));
                        drop(state_mut);
                        sync_root_none(&state_for_callback, false);
                        return;
                    }
                    state_mut
                        .toast_overlay
                        .add_toast(adw::Toast::new(&AppError::from(error).body()));
                }
            }
        },
    );
}

fn apply_root_change(
    state: &mut ProjectState,
    folder: &gio::File,
    info: &gio::FileInfo,
    origin: RootChangeOrigin,
) -> bool {
    let uri = folder.uri().to_string();
    let display_name = resolve_display_name(folder, info);
    state.root = Some(ProjectRoot {
        file: folder.clone(),
    });
    state.browser.set_title(&display_name);
    state.browser.tree().set_root(Some(folder.clone()));

    state.settings.set_project_folder_uri(&uri);
    state
        .settings
        .set_project_folder_display_name(&display_name);

    let sidebar_visible = match origin {
        RootChangeOrigin::UserOpen | RootChangeOrigin::AppOpen => {
            state.settings.set_project_sidebar_visible(true);
            true
        }
        RootChangeOrigin::Restore => state.settings.project_sidebar_visible(),
    };

    let show_hidden = state.settings.project_show_hidden();
    state
        .show_hidden_action
        .set_state(&show_hidden.to_variant());
    state.browser.tree().model().set_show_hidden(show_hidden);
    sidebar_visible
}

fn set_sidebar_visible_for_root(state: &Rc<RefCell<ProjectState>>, visible: bool) {
    let state = state.borrow();
    state.split_view.set_show_sidebar(visible);
    state
        .sidebar_visible_action
        .set_state(&visible.to_variant());
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

fn close_root(state: &Rc<RefCell<ProjectState>>) {
    {
        let mut state = state.borrow_mut();
        if let Some(cancellable) = state.root_cancellable.take() {
            cancellable.cancel();
        }
        if let Some(pending) = state.pending_reveal.take() {
            pending.cancellable.cancel();
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
    }
    sync_actions_for_root(state);
}

fn sync_root_none(state: &Rc<RefCell<ProjectState>>, clear_settings: bool) {
    {
        let mut state = state.borrow_mut();
        if let Some(pending) = state.pending_reveal.take() {
            pending.cancellable.cancel();
        }
        if let Some(cancellable) = state.symlink_cancellable.take() {
            cancellable.cancel();
        }
        state.symlink_generation += 1;
        state.root = None;
        state.browser.set_title(&gettext("Project"));
        state.browser.tree().set_root(None);
        if clear_settings {
            state.settings.set_project_folder_uri("");
            state.settings.set_project_folder_display_name("");
            state.settings.set_project_sidebar_visible(false);
        }
    }
    sync_actions_for_root(state);
}

fn refresh_tree(state: &Rc<RefCell<ProjectState>>) {
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
