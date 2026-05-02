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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitPath {
    raw: Vec<u8>,
    display: String,
    utf8: Option<String>,
}

impl GitPath {
    #[must_use]
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        match String::from_utf8(bytes.to_vec()) {
            Ok(value) => Self {
                raw: bytes.to_vec(),
                display: value.clone(),
                utf8: Some(value),
            },
            Err(_) => Self {
                raw: bytes.to_vec(),
                display: pgettext("git path fallback", "Invalid path encoding"),
                utf8: None,
            },
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
        let disabled = pgettext("git action disabled", "Refresh required");
        Self {
            path,
            status,
            head_oid,
            index_oid,
            staged,
            unstaged,
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
    let mut records = bytes.split(|byte| *byte == 0);
    while let Some(record) = records.next() {
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
            Some(b'2') => {
                if let Some(entry) = parse_rename_entry(record, records.next()) {
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
    let parts: Vec<&[u8]> = bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .collect();
    if !parts.len().is_multiple_of(3) {
        return Err(GitParseError::Malformed);
    }
    let mut blocked = BTreeSet::new();
    for triple in parts.chunks(3) {
        let Some(path) = triple.first() else {
            return Err(GitParseError::Malformed);
        };
        let Some(value) = triple.get(2) else {
            return Err(GitParseError::Malformed);
        };
        if attr_blocks(value) {
            blocked.insert((*path).to_vec());
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
    let head_oid = fields.get(6).and_then(oid_field);
    let index_oid = fields.get(7).and_then(oid_field);
    let path = fields.get(8).copied()?;
    let (status, staged, unstaged) = status_from_xy(xy);
    let status = if is_submodule(submodule) {
        GitFileStatus::Unsupported
    } else {
        status
    };
    Some(GitStatusEntry::new(
        GitPath::from_bytes(path),
        status,
        head_oid,
        index_oid,
        staged,
        unstaged,
    ))
}

fn parse_rename_entry(record: &[u8], next_record: Option<&[u8]>) -> Option<GitStatusEntry> {
    let fields = split_fields(record, 10);
    let xy = fields.get(1).copied()?;
    let submodule = fields.get(2).copied()?;
    let head_oid = fields.get(6).and_then(oid_field);
    let index_oid = fields.get(7).and_then(oid_field);
    let path = fields.get(9).copied().or(next_record)?;
    let (status, staged, unstaged) = status_from_xy(xy);
    let status = if is_submodule(submodule) {
        GitFileStatus::Unsupported
    } else {
        status
    };
    Some(GitStatusEntry::new(
        GitPath::from_bytes(path),
        status,
        head_oid,
        index_oid,
        staged,
        unstaged,
    ))
}

fn parse_unmerged_entry(record: &[u8]) -> Option<GitStatusEntry> {
    let fields = split_fields(record, 11);
    let path = fields.get(10).copied()?;
    Some(GitStatusEntry::new(
        GitPath::from_bytes(path),
        GitFileStatus::Conflicted,
        None,
        None,
        true,
        true,
    ))
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
    String::from_utf8(bytes.to_vec()).ok()
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
mod tests {
    use super::{
        GitFileStatus, index_info_line, parse_attrs, parse_ls_tree_entry, parse_status,
        resolve_capabilities,
    };

    #[test]
    fn status_parser_reads_branch_and_entries() {
        let input = b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +1 -0\0\
1 .M N... 100644 100644 100644 abc def src/lib.rs\0? new.txt\0";
        let snapshot = parse_status(input);
        assert_eq!(snapshot.branch.as_deref(), Some("main"));
        assert_eq!(snapshot.head_oid.as_deref(), Some("abc"));
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].path.display(), "src/lib.rs");
        assert!(snapshot.entries[0].unstaged);
        assert_eq!(snapshot.entries[1].path.display(), "new.txt");
    }

    #[test]
    fn status_parser_keeps_unborn_history_without_head_oid() {
        let snapshot = parse_status(b"# branch.oid (initial)\0# branch.head main\0");
        assert!(snapshot.unborn);
        assert_eq!(snapshot.head_oid, None);
    }

    #[test]
    fn status_parser_marks_submodules_and_unmerged_unsupported() {
        let input = b"1 .M S... 160000 160000 160000 abc def module\0u UU N... 100644 100644 100644 100644 abc def ghi jkl conflict.txt\0";
        let snapshot = parse_status(input);
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].status.badge(), "x");
        assert_eq!(snapshot.entries[1].status.badge(), "!");
    }

    #[test]
    fn status_labels_and_badges_cover_all_states() {
        let states = [
            (GitFileStatus::Added, "A", "Added"),
            (GitFileStatus::Modified, "M", "Modified"),
            (GitFileStatus::Deleted, "D", "Deleted"),
            (GitFileStatus::Untracked, "U", "Untracked"),
            (GitFileStatus::Conflicted, "!", "Conflict"),
            (GitFileStatus::Unsupported, "x", "Unsupported"),
        ];
        for (status, badge, label) in states {
            assert_eq!(status.badge(), badge);
            assert_eq!(status.label(), label);
        }
    }

    #[test]
    fn attr_parser_blocks_content_conversion() {
        let attrs =
            parse_attrs(b"a.bin\0filter\0lfs\0b.txt\0text\0unset\0c.txt\0eol\0unspecified\0");
        assert!(attrs.is_ok());
        let attrs = attrs.ok();
        assert!(attrs.as_ref().is_some_and(|attrs| attrs.blocks(b"a.bin")));
        assert!(attrs.as_ref().is_some_and(|attrs| !attrs.blocks(b"b.txt")));
    }

    #[test]
    fn index_info_includes_stage_zero() {
        assert_eq!(
            index_info_line("100644", "abc", b"a,b.txt"),
            b"100644 abc 0\ta,b.txt\0"
        );
    }

    #[test]
    fn ls_tree_is_reshaped_without_type() {
        assert_eq!(
            parse_ls_tree_entry(b"100755 blob abc123\ttool.sh"),
            Some((String::from("100755"), String::from("abc123")))
        );
    }

    #[test]
    fn capability_guard_allows_only_sha1_without_eol_conversion() {
        assert!(resolve_capabilities("sha1\n", "false\n", "").object_format_supported);
        assert!(resolve_capabilities("sha1\n", "false\n", "").eol_supported);
        assert!(!resolve_capabilities("sha256\n", "false\n", "").object_format_supported);
        assert!(!resolve_capabilities("sha1\n", "true\n", "").eol_supported);
        assert!(!resolve_capabilities("sha1\n", "false\n", "lf\n").eol_supported);
    }

    #[test]
    fn parsers_do_not_panic_on_pseudo_random_bytes() {
        let mut data = Vec::new();
        let mut seed = 0x1234_5678_u32;
        for _ in 0..512 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            data.push((seed >> 24) as u8);
        }
        let _snapshot = parse_status(&data);
        let _attrs = parse_attrs(&data);
    }
}
