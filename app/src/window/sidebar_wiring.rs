use std::rc::Rc;

use crate::find_in_files::FindInFilesController;
use crate::settings::AppSettings;
use crate::sidebar_host::SidebarHost;
use crate::source_control::SourceControlController;
use crate::window_project::WindowProjectController;
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

use super::{git_actions, search_coordinator};

pub(super) struct WindowSidebarControllers {
    pub(super) sidebar_host: Rc<SidebarHost>,
    pub(super) find_in_files: Rc<FindInFilesController>,
    pub(super) source_control: SourceControlController,
    pub(super) git_actions: Rc<git_actions::WindowGitActions>,
}

pub(super) fn install(
    shell: &WindowShell,
    settings: &AppSettings,
    workspace: &Rc<Workspace>,
    project: &WindowProjectController,
) -> WindowSidebarControllers {
    let source_control = SourceControlController::new(&shell.window, settings, workspace);
    let find_in_files =
        FindInFilesController::new(&shell.window, workspace, settings.project_show_hidden());
    let sidebar_host = Rc::new(SidebarHost::new(
        &project.sidebar_widget(),
        &source_control.widget(),
        &find_in_files.widget(),
    ));
    shell
        .project_split_view
        .set_start_child(Some(sidebar_host.widget()));
    shell
        .project_split_view
        .set_end_child(Some(&shell.workspace_box));

    let source_root_handler = source_control.root_change_handler();
    let find_root_handler = find_in_files.root_change_handler();
    let find_in_files_for_root = Rc::clone(&find_in_files);
    let sidebar_host_for_root = Rc::clone(&sidebar_host);
    project.set_root_change_handler(Rc::new(move |root| {
        source_root_handler(root.clone());
        find_root_handler(root);
        find_in_files_for_root.clear();
        sidebar_host_for_root.set_search_results_visible(false);
    }));
    project.set_filter_change_handler(find_in_files.show_hidden_handler());
    project.set_sidebar_visibility_handler(find_in_files.sidebar_visibility_handler());
    source_control.set_status_handler(project.git_status_handler());
    workspace.set_save_notification_handler(source_control.save_notification_handler());
    workspace.set_review_refresh_handler(source_control.review_refresh_handler());
    let project_dirty = project.dirty_uris_handler();
    let workspace_for_dirty = Rc::downgrade(workspace);
    workspace.set_dirty_state_handler(Rc::new(move || {
        if let Some(workspace) = workspace_for_dirty.upgrade() {
            project_dirty(workspace.dirty_session_uris());
        }
    }));

    let project_for_search = project.clone();
    search_coordinator::install(
        &workspace.search,
        Rc::clone(&find_in_files),
        Rc::clone(&sidebar_host),
        Rc::new(move || project_for_search.current_root_file()),
    );

    let git_actions = git_actions::install(shell, source_control.clone(), Rc::clone(workspace));
    let git_actions_for_workspace = Rc::downgrade(&git_actions);
    let source_control_for_workspace = source_control.downgrade();
    workspace.set_git_action_sync_handler(Rc::new(move |tab| {
        let (Some(git_actions), Some(source_control)) = (
            git_actions_for_workspace.upgrade(),
            source_control_for_workspace.upgrade(),
        ) else {
            return;
        };
        git_actions.recompute_visibility();
        source_control.set_active_uri(tab.as_ref().and_then(|tab| tab.session_uri()));
        source_control.refresh_editor_minimap_diff_for_tab(tab);
    }));
    let git_actions_for_source = Rc::downgrade(&git_actions);
    let source_control_for_source = source_control.downgrade();
    source_control.set_state_change_handler(Rc::new(move || {
        let (Some(git_actions), Some(source_control)) = (
            git_actions_for_source.upgrade(),
            source_control_for_source.upgrade(),
        ) else {
            return;
        };
        git_actions.recompute_visibility();
        source_control.refresh_editor_minimap_diffs();
    }));

    WindowSidebarControllers {
        sidebar_host,
        find_in_files,
        source_control,
        git_actions,
    }
}
