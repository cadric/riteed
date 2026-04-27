use crate::git_status::GitFileStatus;

pub(super) const STATUS_CLASSES: &[&str] = &[
    "riteed-git-status-modified",
    "riteed-git-status-added",
    "riteed-git-status-deleted",
    "riteed-git-status-untracked",
    "riteed-git-status-conflicted",
    "riteed-git-status-unsupported",
    "dim-label",
];

pub(super) fn status_class_for(status: GitFileStatus) -> &'static str {
    match status {
        GitFileStatus::Modified => "riteed-git-status-modified",
        GitFileStatus::Added => "riteed-git-status-added",
        GitFileStatus::Deleted => "riteed-git-status-deleted",
        GitFileStatus::Untracked => "riteed-git-status-untracked",
        GitFileStatus::Conflicted => "riteed-git-status-conflicted",
        GitFileStatus::Unsupported => "riteed-git-status-unsupported",
    }
}

pub(super) fn status_is_dim(status: GitFileStatus) -> bool {
    matches!(
        status,
        GitFileStatus::Untracked | GitFileStatus::Unsupported
    )
}
