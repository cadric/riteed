use std::collections::BTreeSet;

use gettextrs::pgettext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitActionState {
    Enabled,
    Disabled(String),
}

impl GitActionState {
    #[must_use]
    pub(crate) fn enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Untracked,
    Conflicted,
    Unsupported,
}

impl GitFileStatus {
    #[must_use]
    pub(crate) fn label(self) -> String {
        match self {
            Self::Added => pgettext("git status", "Added"),
            Self::Modified => pgettext("git status", "Modified"),
            Self::Deleted => pgettext("git status", "Deleted"),
            Self::Untracked => pgettext("git status", "Untracked"),
            Self::Conflicted => pgettext("git status", "Conflict"),
            Self::Unsupported => pgettext("git status", "Unsupported"),
        }
    }

    #[must_use]
    pub(crate) const fn badge(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Untracked => "U",
            Self::Conflicted => "!",
            Self::Unsupported => "x",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitWorktreeMode {
    Regular(&'static str),
    Symlink,
    Gitlink,
    Absent,
    Unsupported,
    Unknown,
}

impl GitWorktreeMode {
    #[must_use]
    pub(crate) const fn stage_mode(self) -> Option<&'static str> {
        match self {
            Self::Regular(mode) => Some(mode),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) const fn blocks_actions(self, status: GitFileStatus) -> bool {
        match self {
            Self::Symlink | Self::Gitlink | Self::Unsupported => true,
            Self::Absent => !matches!(status, GitFileStatus::Deleted),
            Self::Regular(_) | Self::Unknown => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitPath {
    raw: Vec<u8>,
    display: String,
    utf8: Option<String>,
}

impl GitPath {
    #[must_use]
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        let utf8 = std::str::from_utf8(bytes).map(ToOwned::to_owned).ok();
        let display = utf8
            .clone()
            .unwrap_or_else(|| pgettext("git path fallback", "Invalid path encoding"));
        Self {
            raw: bytes.to_vec(),
            display,
            utf8,
        }
    }

    #[must_use]
    pub(crate) fn as_utf8(&self) -> Option<&str> {
        self.utf8.as_deref()
    }

    #[must_use]
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    #[must_use]
    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitStatusEntry {
    pub(crate) path: GitPath,
    pub(crate) status: GitFileStatus,
    pub(crate) head_oid: Option<String>,
    pub(crate) index_oid: Option<String>,
    pub(crate) staged: bool,
    pub(crate) unstaged: bool,
    /// Git porcelain v2 mW at status time; disk mode changes are picked up on refresh.
    pub(crate) worktree_mode: GitWorktreeMode,
    pub(crate) stage_action: GitActionState,
    pub(crate) unstage_action: GitActionState,
    pub(crate) discard_action: GitActionState,
    pub(crate) diff_action: GitActionState,
}

impl GitStatusEntry {
    #[must_use]
    pub(crate) fn new(
        path: GitPath,
        status: GitFileStatus,
        head_oid: Option<String>,
        index_oid: Option<String>,
        staged: bool,
        unstaged: bool,
    ) -> Self {
        Self::with_worktree_mode(
            path,
            status,
            head_oid,
            index_oid,
            staged,
            unstaged,
            GitWorktreeMode::Unknown,
        )
    }

    #[must_use]
    pub(crate) fn with_worktree_mode(
        path: GitPath,
        status: GitFileStatus,
        head_oid: Option<String>,
        index_oid: Option<String>,
        staged: bool,
        unstaged: bool,
        worktree_mode: GitWorktreeMode,
    ) -> Self {
        let disabled = pgettext("git action disabled", "Refresh required");
        Self {
            path,
            status,
            head_oid,
            index_oid,
            staged,
            unstaged,
            worktree_mode,
            stage_action: GitActionState::Disabled(disabled.clone()),
            unstage_action: GitActionState::Disabled(disabled.clone()),
            discard_action: GitActionState::Disabled(disabled.clone()),
            diff_action: GitActionState::Disabled(disabled),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GitStatusSnapshot {
    pub(crate) branch: Option<String>,
    pub(crate) head_oid: Option<String>,
    pub(crate) detached: bool,
    pub(crate) unborn: bool,
    pub(crate) entries: Vec<GitStatusEntry>,
    pub(crate) too_large: bool,
}

impl GitStatusSnapshot {
    #[must_use]
    pub(crate) fn changed_paths(&self) -> Vec<GitPath> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GitAttrs {
    blocked: BTreeSet<Vec<u8>>,
}

impl GitAttrs {
    #[must_use]
    pub(crate) fn blocks(&self, path: &[u8]) -> bool {
        self.blocked.contains(path)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitAttrState {
    Known(GitAttrs),
    Unavailable,
}

impl GitAttrState {
    #[must_use]
    pub(crate) fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }

    #[must_use]
    pub(crate) fn blocks(&self, path: &[u8]) -> bool {
        match self {
            Self::Known(attrs) => attrs.blocks(path),
            Self::Unavailable => false,
        }
    }
}

impl Default for GitAttrState {
    fn default() -> Self {
        Self::Known(GitAttrs::default())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GitCapabilities {
    pub(crate) object_format_supported: bool,
    pub(crate) eol_supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitParseError {
    Malformed,
}

#[must_use]
pub(crate) fn parse_status(bytes: &[u8]) -> GitStatusSnapshot {
    let mut snapshot = GitStatusSnapshot::default();
    for record in bytes.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        match record.first().copied() {
            Some(b'#') => parse_branch_line(record, &mut snapshot),
            Some(b'1') => {
                if let Some(entry) = parse_ordinary_entry(record) {
                    snapshot.entries.push(entry);
                }
            }
            Some(b'u') => {
                if let Some(entry) = parse_unmerged_entry(record) {
                    snapshot.entries.push(entry);
                }
            }
            Some(b'?') => snapshot.entries.push(GitStatusEntry::new(
                GitPath::from_bytes(trim_status_prefix(record)),
                GitFileStatus::Untracked,
                None,
                None,
                false,
                true,
            )),
            _ => {}
        }
    }
    snapshot
}

pub(crate) fn parse_attrs(bytes: &[u8]) -> Result<GitAttrs, GitParseError> {
    let mut parts = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty());
    let mut blocked = BTreeSet::new();
    while let Some(path) = parts.next() {
        if parts.next().is_none() {
            return Err(GitParseError::Malformed);
        }
        let Some(value) = parts.next() else {
            return Err(GitParseError::Malformed);
        };
        if attr_blocks(value) {
            blocked.insert(path.to_vec());
        }
    }
    Ok(GitAttrs { blocked })
}

#[must_use]
pub(crate) fn resolve_capabilities(
    object_format: &str,
    autocrlf: &str,
    eol: &str,
) -> GitCapabilities {
    GitCapabilities {
        object_format_supported: object_format.trim() == "sha1",
        eol_supported: eol_supported(autocrlf.trim(), eol.trim()),
    }
}

#[must_use]
pub(crate) fn index_info_line(mode: &str, oid: &str, path: &[u8]) -> Vec<u8> {
    let mut line = Vec::new();
    line.extend_from_slice(mode.as_bytes());
    line.extend_from_slice(b" ");
    line.extend_from_slice(oid.as_bytes());
    line.extend_from_slice(b" 0\t");
    line.extend_from_slice(path);
    line.push(0);
    line
}

#[must_use]
pub(crate) fn parse_ls_tree_entry(bytes: &[u8]) -> Option<(String, String)> {
    let (meta, _path) = split_once(bytes, b'\t')?;
    let mut parts = meta.split(|byte| *byte == b' ');
    let mode = bytes_to_string(parts.next()?)?;
    let _kind = parts.next()?;
    let oid = bytes_to_string(parts.next()?)?;
    Some((mode, oid))
}

fn parse_branch_line(record: &[u8], snapshot: &mut GitStatusSnapshot) {
    if let Some(value) = record.strip_prefix(b"# branch.head ") {
        let head = bytes_to_string(value);
        snapshot.detached = head.as_deref() == Some("(detached)");
        snapshot.branch = head;
    } else if let Some(value) = record.strip_prefix(b"# branch.oid ") {
        snapshot.unborn = value == b"(initial)";
        snapshot.head_oid = if snapshot.unborn {
            None
        } else {
            bytes_to_string(value)
        };
    }
}

fn parse_ordinary_entry(record: &[u8]) -> Option<GitStatusEntry> {
    let fields = split_fields(record, 9);
    let xy = fields.get(1).copied()?;
    let submodule = fields.get(2).copied()?;
    let worktree_mode = parse_worktree_mode(fields.get(5).copied());
    let head_oid = fields.get(6).and_then(oid_field);
    let index_oid = fields.get(7).and_then(oid_field);
    let path = fields.get(8).copied()?;
    let (status, staged, unstaged) = status_from_xy(xy);
    let status = if is_submodule(submodule) {
        GitFileStatus::Unsupported
    } else {
        status
    };
    Some(GitStatusEntry::with_worktree_mode(
        GitPath::from_bytes(path),
        status,
        head_oid,
        index_oid,
        staged,
        unstaged,
        worktree_mode,
    ))
}

fn parse_unmerged_entry(record: &[u8]) -> Option<GitStatusEntry> {
    let fields = split_fields(record, 11);
    let worktree_mode = parse_worktree_mode(fields.get(6).copied());
    let path = fields.get(10).copied()?;
    Some(GitStatusEntry::with_worktree_mode(
        GitPath::from_bytes(path),
        GitFileStatus::Conflicted,
        None,
        None,
        true,
        true,
        worktree_mode,
    ))
}

fn parse_worktree_mode(mode: Option<&[u8]>) -> GitWorktreeMode {
    match mode {
        Some(b"100644") => GitWorktreeMode::Regular("100644"),
        Some(b"100755") => GitWorktreeMode::Regular("100755"),
        Some(b"120000") => GitWorktreeMode::Symlink,
        Some(b"160000") => GitWorktreeMode::Gitlink,
        Some(b"000000") => GitWorktreeMode::Absent,
        Some(_) => GitWorktreeMode::Unsupported,
        None => GitWorktreeMode::Unknown,
    }
}

fn status_from_xy(xy: &[u8]) -> (GitFileStatus, bool, bool) {
    let x = xy.first().copied().unwrap_or(b'.');
    let y = xy.get(1).copied().unwrap_or(b'.');
    let staged = x != b'.';
    let unstaged = y != b'.';
    let status = if x == b'D' || y == b'D' {
        GitFileStatus::Deleted
    } else if x == b'A' {
        GitFileStatus::Added
    } else {
        GitFileStatus::Modified
    };
    (status, staged, unstaged)
}

fn split_fields(record: &[u8], limit: usize) -> Vec<&[u8]> {
    record.splitn(limit, |byte| *byte == b' ').collect()
}

fn trim_status_prefix(record: &[u8]) -> &[u8] {
    if record.len() > 2 { &record[2..] } else { &[] }
}

fn is_submodule(field: &[u8]) -> bool {
    field.first().is_some_and(|byte| *byte != b'N')
}

fn attr_blocks(value: &[u8]) -> bool {
    !value.is_empty() && value != b"unset" && value != b"unspecified"
}

fn eol_supported(autocrlf: &str, eol: &str) -> bool {
    let autocrlf_ok = autocrlf.is_empty() || autocrlf == "false";
    let eol_ok = eol.is_empty() || eol == "native";
    autocrlf_ok && eol_ok
}

fn bytes_to_string(bytes: &[u8]) -> Option<String> {
    std::str::from_utf8(bytes).map(ToOwned::to_owned).ok()
}

fn oid_field(bytes: &&[u8]) -> Option<String> {
    if bytes.iter().all(|byte| *byte == b'.') {
        None
    } else {
        bytes_to_string(bytes)
    }
}

fn split_once(bytes: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == needle)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

#[cfg(test)]
mod tests;
