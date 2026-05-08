use gettextrs::{gettext, pgettext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::git_process::{GitCommitSummary, GitLogState};
use crate::source_control::{SourceStateRef, git_error_is_cancelled};

const RECENT_COMMIT_LIMIT: usize = 25;

pub(super) struct SourceControlHistory {
    root: gtk4::Box,
    list: gtk4::ListBox,
    status: gtk4::Label,
    loaded_head_oid: RefCell<Option<String>>,
    needs_refresh: Cell<bool>,
}

pub(super) fn refresh(state: &SourceStateRef, head_oid: Option<&str>) {
    let head_oid_owned = head_oid.map(str::to_string);
    let (process, cancellable, should_refresh) = {
        let state = state.borrow();
        let Some(process) = state.process.clone() else {
            state.history.clear();
            return;
        };
        let Some(cancellable) = state.cancellable.clone() else {
            state.history.clear();
            return;
        };
        let should_refresh = state.history.should_refresh(head_oid);
        if should_refresh {
            state.history.set_loading();
        }
        (process, cancellable, should_refresh)
    };
    if !should_refresh {
        return;
    }
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    process.recent_commits(
        RECENT_COMMIT_LIMIT,
        &cancellable,
        Rc::new(move |result| {
            if cancellable_for_callback.is_cancelled() {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(log_state) => state
                    .borrow()
                    .history
                    .set_state(&log_state, head_oid_owned.clone()),
                Err(error) if git_error_is_cancelled(&error) => {}
                Err(_error) => state.borrow().history.set_error(),
            }
        }),
    );
}

impl SourceControlHistory {
    fn should_refresh(&self, head_oid: Option<&str>) -> bool {
        should_refresh_history(
            self.loaded_head_oid.borrow().as_deref(),
            self.needs_refresh.get(),
            head_oid,
        )
    }

    #[must_use]
    pub(super) fn new() -> Self {
        let root = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        root.set_vexpand(true);
        let title = gtk4::Label::new(Some(&pgettext("git history", "Recent Commits")));
        title.add_css_class("caption-heading");
        title.set_xalign(0.0);
        root.append(&title);

        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        list.add_css_class("boxed-list");
        let scroller = gtk4::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();
        scroller.set_vexpand(true);
        root.append(&scroller);

        let status = gtk4::Label::new(None);
        status.add_css_class("dim-label");
        status.set_xalign(0.0);
        status.set_wrap(true);
        root.append(&status);

        let history = Self {
            root,
            list,
            status,
            loaded_head_oid: RefCell::new(None),
            needs_refresh: Cell::new(true),
        };
        history.clear();
        history
    }

    #[must_use]
    pub(super) fn widget(&self) -> gtk4::Box {
        self.root.clone()
    }

    pub(super) fn clear(&self) {
        self.clear_rows();
        self.loaded_head_oid.borrow_mut().take();
        self.needs_refresh.set(true);
        self.status.set_label("");
        self.status.set_visible(false);
        self.root.set_visible(false);
    }

    pub(super) fn set_loading(&self) {
        self.clear_rows();
        self.root.set_visible(true);
        self.status
            .set_label(&ellipsis_label(gettext("Loading recent commits")));
        self.status.set_visible(true);
    }

    pub(super) fn set_state(&self, state: &GitLogState, head_oid: Option<String>) {
        self.loaded_head_oid.replace(head_oid);
        self.needs_refresh.set(false);
        match state {
            GitLogState::Commits(commits) if commits.is_empty() => self.set_no_history(),
            GitLogState::Commits(commits) => self.set_commits(commits),
            GitLogState::NoHistory => self.set_no_history(),
        }
    }

    pub(super) fn set_error(&self) {
        self.needs_refresh.set(true);
        self.clear_rows();
        self.root.set_visible(true);
        self.status
            .set_label(&gettext("Unable to read recent commits."));
        self.status.set_visible(true);
    }

    #[cfg(test)]
    pub(super) fn row_count_for_tests(&self) -> usize {
        let count = self.list.observe_children().n_items();
        usize::try_from(count).map_or(0, |count| count)
    }

    fn set_commits(&self, commits: &[GitCommitSummary]) {
        self.clear_rows();
        self.root.set_visible(true);
        self.status.set_visible(false);
        for commit in commits {
            self.list.append(&commit_row(commit));
        }
    }

    fn set_no_history(&self) {
        self.clear_rows();
        self.root.set_visible(true);
        self.status.set_label(&gettext("No commits yet."));
        self.status.set_visible(true);
    }

    fn clear_rows(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

fn commit_row(commit: &GitCommitSummary) -> adw::ActionRow {
    let subtitle = format!(
        "{} · {} · {}",
        commit.author, commit.date, commit.short_hash
    );
    let row = adw::ActionRow::builder()
        .title(&commit.subject)
        .subtitle(&subtitle)
        .tooltip_text(&commit.full_hash)
        .build();
    row.set_activatable(false);
    row
}

fn should_refresh_history(
    loaded_head_oid: Option<&str>,
    needs_refresh: bool,
    head_oid: Option<&str>,
) -> bool {
    needs_refresh || loaded_head_oid != head_oid
}

#[cfg(test)]
mod tests {
    use super::should_refresh_history;

    #[test]
    fn history_skips_index_only_refreshes_for_same_head() {
        assert!(!should_refresh_history(Some("abc"), false, Some("abc")));
    }

    #[test]
    fn history_refreshes_after_failure_or_head_change() {
        assert!(should_refresh_history(Some("abc"), true, Some("abc")));
        assert!(should_refresh_history(Some("abc"), false, Some("def")));
    }
}
