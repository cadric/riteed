use std::cell::{Cell, RefCell};
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
pub(super) const DEFAULT_PROJECT_SIDEBAR_WIDTH: i32 = 320;
pub(super) const MAX_PROJECT_SIDEBAR_WIDTH: i32 = 520;
pub(super) const MIN_PROJECT_SIDEBAR_WIDTH: i32 = 220;

mod app_open;
mod auto_refresh;
mod reveal;
mod sidebar_state;
mod symlink;
#[cfg(test)]
mod testing;

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
type RootChangeHandler = Rc<dyn Fn(Option<gio::File>)>;
type GitStatusHandler = Rc<dyn Fn(Vec<(String, String)>)>;

struct RootApplyPlan {
    settings: AppSettings,
    show_hidden_action: gio::SimpleAction,
    handler: Option<RootChangeHandler>,
    uri: String,
    display_name: String,
    origin: RootChangeOrigin,
}

struct ProjectState {
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    split_view: gtk4::Paned,
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
    sidebar_width: i32,
    sidebar_position_guard: Rc<Cell<bool>>,
    sidebar_animation: Option<adw::TimedAnimation>,

    reveal_generation: u64,
    pending_reveal: Option<reveal::PendingReveal>,

    symlink_generation: u64,
    symlink_cancellable: Option<gio::Cancellable>,
    toast_keys: HashSet<String>,
    root_change_handler: Option<RootChangeHandler>,
}

#[derive(Clone)]
pub struct WindowProjectController {
    state: Rc<RefCell<ProjectState>>,
}

impl WindowProjectController {
    #[must_use]
    pub fn new(
        shell: &WindowShell,
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
        restore_project: bool,
    ) -> Self {
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
            sidebar_width: DEFAULT_PROJECT_SIDEBAR_WIDTH,
            sidebar_position_guard: Rc::new(Cell::new(false)),
            sidebar_animation: None,
            symlink_cancellable: None,
            toast_keys: HashSet::new(),
            root_change_handler: None,
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

        let auto_refresh = auto_refresh::ProjectAutoRefresh::new({
            let state = Rc::downgrade(&state);
            Rc::new(move || {
                if let Some(state) = state.upgrade() {
                    auto_refresh::refresh_tree(&state);
                }
            })
        });
        let auto_refresh_for_model = auto_refresh.clone();
        state
            .borrow()
            .browser
            .tree()
            .model()
            .set_auto_refresh_handler(Rc::new(move || {
                auto_refresh_for_model.schedule();
            }));

        let controller = Self { state };

        controller.install_callbacks();
        if restore_project {
            controller.restore_from_settings();
        }
        controller
    }

    pub fn request_open_folder_dialog(&self) {
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Open Folder"))
            .accept_label(pgettext("file dialog action", "Open"))
            .modal(true)
            .build();
        apply_folder_filter(&dialog);

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

    #[must_use]
    pub(crate) fn sidebar_widget(&self) -> adw::ToolbarView {
        self.state.borrow().browser.widget().clone()
    }

    pub(crate) fn set_root_change_handler(&self, handler: RootChangeHandler) {
        self.state.borrow_mut().root_change_handler = Some(handler);
    }

    pub(crate) fn git_status_handler(&self) -> GitStatusHandler {
        let weak = Rc::downgrade(&self.state);
        Rc::new(move |statuses| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if let Ok(state) = state.try_borrow() {
                state.browser.set_git_statuses(statuses);
            }
        })
    }

    #[must_use]
    pub(crate) fn current_root_file(&self) -> Option<gio::File> {
        self.state
            .borrow()
            .root
            .as_ref()
            .map(|root| root.file.clone())
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
                let Ok(mut state) = state.try_borrow_mut() else {
                    return;
                };
                if state.root.is_none() {
                    return;
                }
                sidebar_state::set_sidebar_visibility(&mut state, value);
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
                let Some((settings, model, expanded)) = ({
                    let state = state.borrow();
                    if state.root.is_none() {
                        None
                    } else {
                        let model = state.browser.tree().model().clone();
                        let expanded = model.snapshot_expanded_uris();
                        Some((state.settings.clone(), model, expanded))
                    }
                }) else {
                    return;
                };
                settings.set_project_show_hidden(value);
                model.set_show_hidden(value);
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
                auto_refresh::refresh_tree(&state);
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
            .connect_position_notify(move |split_view| {
                let Some(state) = state.upgrade() else {
                    return;
                };
                let Ok(mut state) = state.try_borrow_mut() else {
                    return;
                };
                sidebar_state::set_sidebar_position_from_move(&mut state, split_view);
            });

        let state = Rc::downgrade(&self.state);
        let workspace = {
            let state = self.state.borrow();
            state.workspace.upgrade()
        };
        if let Some(workspace) = workspace {
            workspace.tab_view.connect_selected_page_notify(move |_| {
                if let Some(state) = state.upgrade() {
                    reveal::sync_reveal_for_selection(&state);
                }
            });
        }

        sidebar_state::sync_actions_for_root(&self.state);
    }
}

fn apply_folder_filter(dialog: &gtk4::FileDialog) {
    let folder_filter = gtk4::FileFilter::new();
    folder_filter.set_name(Some(&pgettext("file filter", "Folders")));
    folder_filter.add_mime_type("inode/directory");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&folder_filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&folder_filter));
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

                    let plan =
                        apply_root_change(&mut state_mut, &folder_for_callback, &info, origin);
                    drop(state_mut);
                    let sidebar_visible =
                        finish_root_change(&state_for_callback, &folder_for_callback, plan);
                    sidebar_state::set_sidebar_visible_for_root(
                        &state_for_callback,
                        sidebar_visible,
                    );
                    sidebar_state::sync_actions_for_root(&state_for_callback);
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

fn close_root(state: &Rc<RefCell<ProjectState>>) {
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

fn sync_root_none(state: &Rc<RefCell<ProjectState>>, clear_settings: bool) {
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
