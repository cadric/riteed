use std::path::PathBuf;
use std::rc::Rc;

use gtk4::gio;
use gtk4::prelude::*;

use super::{
    SourceControlState, SourceStateRef, cancel_minimap_requests, cancel_minimap_requests_for_tab,
    path_target, remove_minimap_cancellable, track_minimap_cancellable,
};
use crate::editor_tab::EditorTab;
use crate::editor_tab::minimap_diff::{MinimapDiffInput, decode_minimap_text};
use crate::git_process::{GitProcess, GitProcessError};
use crate::git_status::{GitAttrState, GitFileStatus, GitStatusEntry};

pub(super) fn refresh_open_tabs(state: &SourceStateRef) {
    cancel_minimap_requests(state);
    let tabs = state
        .borrow()
        .workspace
        .upgrade()
        .map(|workspace| workspace.ordered_tabs())
        .unwrap_or_default();
    for tab in tabs {
        refresh_tab_without_cancel(state, Some(tab));
    }
}

pub(super) fn refresh_tab(state: &SourceStateRef, tab: Option<Rc<EditorTab>>) {
    if let Some(tab) = tab.as_ref() {
        cancel_minimap_requests_for_tab(state, tab, None);
    }
    refresh_tab_without_cancel(state, tab);
}

fn refresh_tab_without_cancel(state: &SourceStateRef, tab: Option<Rc<EditorTab>>) {
    let Some(tab) = tab else {
        return;
    };
    if !tab.is_document() {
        tab.clear_source_control_minimap_diff();
        return;
    }
    if !tab.editor_heavy_features_enabled() {
        tab.clear_source_control_minimap_diff();
        return;
    }
    let Some(target) = target_for_tab(state, &tab) else {
        tab.clear_source_control_minimap_diff();
        return;
    };
    let Some(entry) = target.entry else {
        tab.clear_source_control_minimap_diff();
        return;
    };
    let content_type = tab.source_control_minimap_content_type();
    if entry_blocks_minimap(&entry, &target.attrs, content_type.as_deref()) {
        tab.clear_source_control_minimap_diff();
        return;
    }

    let source = source_token(&target.repo, &entry);
    if tab.is_dirty() {
        tab.mark_source_control_minimap_pending(source);
        return;
    }
    match reference_for_entry(&entry) {
        ReferenceInput::Empty => tab.apply_source_control_minimap_diff(MinimapDiffInput {
            source,
            reference_text: String::new(),
            all_deleted: false,
        }),
        ReferenceInput::AllDeleted => tab.apply_source_control_minimap_diff(MinimapDiffInput {
            source,
            reference_text: String::new(),
            all_deleted: true,
        }),
        ReferenceInput::Blob(oid) => {
            load_reference_blob(
                state,
                &tab,
                &target.process,
                target.repo,
                entry,
                oid.as_str(),
                source,
            );
        }
    }
}

struct MinimapTarget {
    repo: PathBuf,
    process: GitProcess,
    attrs: GitAttrState,
    entry: Option<GitStatusEntry>,
}

enum ReferenceInput {
    Empty,
    Blob(String),
    AllDeleted,
}

fn target_for_tab(state: &SourceStateRef, tab: &EditorTab) -> Option<MinimapTarget> {
    let uri = tab.document_uri()?;
    let state = state.borrow();
    if state.snapshot.too_large {
        return None;
    }
    let repo = state.repo.clone()?;
    let process = state.process.clone()?;
    let raw = path_target::raw_path_for_uri(&repo, uri.as_str())?;
    let entry = state
        .snapshot
        .entries
        .iter()
        .find(|entry| entry.path.raw() == raw.as_slice())
        .cloned();
    Some(MinimapTarget {
        repo,
        process,
        attrs: state.attrs.clone(),
        entry,
    })
}

fn reference_for_entry(entry: &GitStatusEntry) -> ReferenceInput {
    if entry.status == GitFileStatus::Deleted {
        return ReferenceInput::AllDeleted;
    }
    if matches!(
        entry.status,
        GitFileStatus::Added | GitFileStatus::Untracked
    ) {
        return ReferenceInput::Empty;
    }
    reference_oid(entry).map_or(ReferenceInput::Empty, ReferenceInput::Blob)
}

fn reference_oid(entry: &GitStatusEntry) -> Option<String> {
    if entry.staged && !entry.unstaged {
        return entry.head_oid.clone();
    }
    entry.index_oid.clone().or_else(|| entry.head_oid.clone())
}

fn load_reference_blob(
    state: &SourceStateRef,
    tab: &Rc<EditorTab>,
    process: &GitProcess,
    repo: PathBuf,
    entry: GitStatusEntry,
    oid: &str,
    source: String,
) {
    let cancellable = gio::Cancellable::new();
    track_minimap_cancellable(state, tab, &source, &cancellable);
    let cancellable_for_callback = cancellable.clone();
    let weak_state = Rc::downgrade(state);
    let weak_tab = Rc::downgrade(tab);
    process.cat_blob(
        oid,
        &cancellable,
        Rc::new(move |result| {
            if let Some(state) = weak_state.upgrade() {
                remove_minimap_cancellable(&state, &cancellable_for_callback);
                if state.borrow().repo.as_ref() != Some(&repo) {
                    return;
                }
            } else {
                return;
            }
            let timed_out = matches!(&result, Err(error) if error == &GitProcessError::TimedOut);
            if cancellable_for_callback.is_cancelled() && !timed_out {
                return;
            }
            let (Some(state), Some(tab)) = (weak_state.upgrade(), weak_tab.upgrade()) else {
                return;
            };
            if !entry_matches_snapshot(&state.borrow(), &entry) {
                return;
            }
            if tab.is_dirty() {
                tab.mark_source_control_minimap_pending(source.clone());
                return;
            }
            match result.and_then(decode_minimap_text) {
                Ok(reference_text) => {
                    tab.apply_source_control_minimap_diff(MinimapDiffInput {
                        source: source.clone(),
                        reference_text,
                        all_deleted: false,
                    });
                }
                Err(GitProcessError::Cancelled) => {}
                Err(error) if git_error_skips_minimap(&error) => {
                    tab.clear_source_control_minimap_diff();
                }
                Err(_error) => {
                    tab.apply_source_control_minimap_diff(MinimapDiffInput {
                        source: source.clone(),
                        reference_text: String::new(),
                        all_deleted: false,
                    });
                }
            }
        }),
    );
}

fn entry_blocks_minimap(
    entry: &GitStatusEntry,
    attrs: &GitAttrState,
    content_type: Option<&str>,
) -> bool {
    if entry.path.as_utf8().is_none() {
        return true;
    }
    if matches!(
        entry.status,
        GitFileStatus::Conflicted | GitFileStatus::Unsupported
    ) {
        return true;
    }
    attrs.is_unavailable()
        || attrs.blocks(entry.path.raw())
        || entry.worktree_mode.blocks_actions(entry.status)
        || content_type_blocks_minimap(content_type)
}

fn content_type_blocks_minimap(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return false;
    };
    content_type == "application/octet-stream"
        || content_type.starts_with("image/")
        || content_type.starts_with("audio/")
        || content_type.starts_with("video/")
        || content_type.starts_with("font/")
}

fn entry_matches_snapshot(state: &SourceControlState, entry: &GitStatusEntry) -> bool {
    state.snapshot.entries.iter().any(|current| {
        current.path.raw() == entry.path.raw()
            && current.status == entry.status
            && current.head_oid == entry.head_oid
            && current.index_oid == entry.index_oid
            && current.staged == entry.staged
            && current.unstaged == entry.unstaged
            && current.worktree_mode == entry.worktree_mode
    })
}

fn git_error_skips_minimap(error: &GitProcessError) -> bool {
    matches!(
        error,
        GitProcessError::OutputTooLarge
            | GitProcessError::BinaryContent
            | GitProcessError::ParseFailed
    )
}

fn source_token(repo: &std::path::Path, entry: &GitStatusEntry) -> String {
    format!(
        "{}|{}|{:?}|{:?}|{:?}|{}|{}|{:?}",
        repo.display(),
        entry.path.display(),
        entry.status,
        entry.head_oid,
        entry.index_oid,
        entry.staged,
        entry.unstaged,
        entry.worktree_mode
    )
}
