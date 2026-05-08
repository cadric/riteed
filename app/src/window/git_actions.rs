use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gio, prelude::*};

use crate::git_status::{GitActionState, GitStatusEntry};
use crate::source_control::SourceControlController;
use crate::source_control::action_widgets::bind_action_state;
use crate::source_control::actions::GitRowAction;
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

pub(super) struct WindowGitActions {
    group: gtk4::Box,
    diff: gtk4::Button,
    stage: gtk4::Button,
    unstage: gtk4::Button,
    discard: gtk4::Button,
    source_control: SourceControlController,
    workspace: Rc<Workspace>,
}

pub(crate) struct GitGroupVisibility {
    pub(crate) group_visible: bool,
    pub(crate) stage: GitActionState,
    pub(crate) unstage: GitActionState,
    pub(crate) discard: GitActionState,
    pub(crate) diff: GitActionState,
}

#[derive(Clone, Copy)]
pub(crate) struct TabState<'a> {
    pub(crate) uri: Option<&'a str>,
    pub(crate) is_compare_mode: bool,
    pub(crate) is_dirty: bool,
}

impl GitGroupVisibility {
    #[must_use]
    pub(crate) fn hidden() -> Self {
        let reason = gettext("No Git action is available for the active tab.");
        Self {
            group_visible: false,
            stage: GitActionState::Disabled(reason.clone()),
            unstage: GitActionState::Disabled(reason.clone()),
            discard: GitActionState::Disabled(reason.clone()),
            diff: GitActionState::Disabled(reason),
        }
    }
}

#[must_use]
pub(crate) fn derive_git_button_state(
    entry: Option<&GitStatusEntry>,
    tab_state: TabState<'_>,
) -> GitGroupVisibility {
    let Some(entry) = entry else {
        return GitGroupVisibility::hidden();
    };
    if tab_state.is_compare_mode || tab_state.uri.is_none() {
        return GitGroupVisibility::hidden();
    }

    let mut visibility = GitGroupVisibility {
        group_visible: true,
        stage: entry.stage_action.clone(),
        unstage: entry.unstage_action.clone(),
        discard: entry.discard_action.clone(),
        diff: entry.diff_action.clone(),
    };
    if tab_state.is_dirty {
        let reason = gettext("Save the open document before using Git actions.");
        visibility.stage = GitActionState::Disabled(reason.clone());
        visibility.unstage = GitActionState::Disabled(reason.clone());
        visibility.discard = GitActionState::Disabled(reason.clone());
        visibility.diff = GitActionState::Disabled(reason);
    }
    visibility
}

pub(super) fn install(
    shell: &WindowShell,
    source_control: SourceControlController,
    workspace: Rc<Workspace>,
) -> Rc<WindowGitActions> {
    let actions = Rc::new(WindowGitActions {
        group: shell.git_actions_group.clone(),
        diff: shell.git_diff_button.clone(),
        stage: shell.git_stage_button.clone(),
        unstage: shell.git_unstage_button.clone(),
        discard: shell.git_discard_button.clone(),
        source_control,
        workspace,
    });
    actions.configure_buttons();
    actions.install_actions(&shell.window);
    actions.recompute_visibility();
    actions
}

impl WindowGitActions {
    pub(super) fn recompute_visibility(&self) {
        let selected = self.workspace.selected_tab();
        let uri = selected.as_ref().and_then(|tab| tab.uri());
        let entry = uri
            .as_deref()
            .and_then(|uri| self.source_control.entry_for_uri(uri));
        let tab_state = TabState {
            uri: uri.as_deref(),
            is_compare_mode: selected.as_ref().is_some_and(|tab| tab.is_compare_active()),
            is_dirty: selected.as_ref().is_some_and(|tab| tab.is_dirty()),
        };
        let visibility = derive_git_button_state(entry.as_ref(), tab_state);
        self.apply_visibility(&visibility);
    }

    fn configure_buttons(&self) {
        configure_button(&self.diff, &diff_label());
        configure_button(&self.stage, &stage_label());
        configure_button(&self.unstage, &unstage_label());
        configure_button(&self.discard, &discard_label());
    }

    fn install_actions(self: &Rc<Self>, window: &impl IsA<gio::ActionMap>) {
        self.install_action(window, "scm-diff-active", GitRowAction::Diff);
        self.install_action(window, "scm-stage-active", GitRowAction::Stage);
        self.install_action(window, "scm-unstage-active", GitRowAction::Unstage);
        self.install_action(window, "scm-discard-active", GitRowAction::Discard);
    }

    fn install_action(
        self: &Rc<Self>,
        window: &impl IsA<gio::ActionMap>,
        name: &str,
        action: GitRowAction,
    ) {
        let simple_action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(self);
        simple_action.connect_activate(move |_, _| {
            if let Some(actions) = weak.upgrade() {
                actions.run_active_action(action);
            }
        });
        window.add_action(&simple_action);
    }

    fn run_active_action(&self, action: GitRowAction) {
        let Some(tab) = self.workspace.selected_tab() else {
            return;
        };
        if tab.is_compare_active() || tab.is_dirty() {
            return;
        }
        let Some(uri) = tab.uri() else {
            return;
        };
        let Some(state) = self.source_control.entry_action_state_for_uri(&uri, action) else {
            return;
        };
        if !state.enabled() {
            return;
        }
        self.source_control.run_action_for_uri(&uri, action);
    }

    fn apply_visibility(&self, visibility: &GitGroupVisibility) {
        self.group.set_visible(visibility.group_visible);
        bind_action_state(&self.diff, &visibility.diff, &diff_label());
        bind_action_state(&self.stage, &visibility.stage, &stage_label());
        bind_action_state(&self.unstage, &visibility.unstage, &unstage_label());
        bind_action_state(&self.discard, &visibility.discard, &discard_label());
    }
}

fn configure_button(button: &gtk4::Button, label: &str) {
    button.add_css_class("flat");
    button.update_property(&[Property::Label(label)]);
}

fn stage_label() -> String {
    pgettext("git action tooltip", "Stage File")
}

fn unstage_label() -> String {
    pgettext("git action tooltip", "Unstage File")
}

fn discard_label() -> String {
    pgettext("git action tooltip", "Discard Changes")
}

fn diff_label() -> String {
    pgettext("git action tooltip", "Compare With Git")
}

#[cfg(test)]
mod tests {
    use crate::git_status::{GitActionState, GitFileStatus, GitPath, GitStatusEntry};
    use crate::window::git_actions::{TabState, derive_git_button_state};

    #[test]
    fn git_button_state_hides_without_modified_entry() {
        let state = derive_git_button_state(
            None,
            TabState {
                uri: Some("file:///tmp/example.txt"),
                is_compare_mode: false,
                is_dirty: false,
            },
        );

        assert!(!state.group_visible);
    }

    #[test]
    fn git_button_state_hides_in_compare_mode() {
        let entry = modified_entry();
        let state = derive_git_button_state(
            Some(&entry),
            TabState {
                uri: Some("file:///tmp/example.txt"),
                is_compare_mode: true,
                is_dirty: false,
            },
        );

        assert!(!state.group_visible);
    }

    #[test]
    fn git_button_state_overlays_dirty_disable() {
        let entry = modified_entry();
        let state = derive_git_button_state(
            Some(&entry),
            TabState {
                uri: Some("file:///tmp/example.txt"),
                is_compare_mode: false,
                is_dirty: true,
            },
        );

        assert!(state.group_visible);
        assert!(!state.stage.enabled());
        assert!(matches!(state.diff, GitActionState::Disabled(reason) if reason.contains("Save")));
    }

    fn modified_entry() -> GitStatusEntry {
        let mut entry = GitStatusEntry::new(
            GitPath::from_bytes(b"example.txt"),
            GitFileStatus::Modified,
            Some(String::from("old")),
            Some(String::from("new")),
            false,
            true,
        );
        entry.stage_action = GitActionState::Enabled;
        entry.discard_action = GitActionState::Enabled;
        entry.diff_action = GitActionState::Enabled;
        entry
    }
}
