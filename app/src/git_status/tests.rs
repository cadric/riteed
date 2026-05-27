use super::{
    GitFileStatus, GitParseError, GitPath, GitWorktreeMode, MAX_ATTR_PATHS, MAX_STATUS_ENTRIES,
    index_info_line, parse_attrs, parse_ls_tree_entry, parse_status, resolve_capabilities,
};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

fn bounded_proptest_config() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::SourceParallel(
            ".proptest-regressions",
        ))),
        ..ProptestConfig::default()
    }
}

#[test]
fn status_parser_reads_branch_and_entries() {
    let input =
        b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +1 -0\0\
1 .M N... 100644 100644 100644 abc def src/lib.rs\0? new.txt\0? nested-repo/\0";
    let snapshot = parse_status(input);
    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert_eq!(snapshot.head_oid.as_deref(), Some("abc"));
    assert_eq!(snapshot.entries.len(), 3);
    assert_eq!(snapshot.entries[0].path.display(), "src/lib.rs");
    assert!(snapshot.entries[0].unstaged);
    assert_eq!(
        snapshot.entries[0].worktree_mode,
        GitWorktreeMode::Regular("100644")
    );
    assert_eq!(snapshot.entries[1].path.display(), "new.txt");
    assert_eq!(snapshot.entries[1].worktree_mode, GitWorktreeMode::Unknown);
    assert_eq!(snapshot.entries[2].path.display(), "nested-repo/");
    assert_eq!(
        snapshot.entries[2].worktree_mode,
        GitWorktreeMode::Directory
    );
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
fn status_parser_reads_worktree_modes_from_porcelain_v2() {
    let input = b"1 .M N... 100644 100644 100755 abc def chmod.sh\0\
1 .M N... 100644 100644 120000 abc def link-new\0\
u UU N... 100644 100644 100644 000000 abc def ghi conflict.txt\0\
1 .M N... 100644 100644 160000 abc def module\0\
1 .M N... 100644 100644 100600 abc def odd.txt\0";
    let snapshot = parse_status(input);
    assert_eq!(
        snapshot.entries[0].worktree_mode.stage_mode(),
        Some("100755")
    );
    assert_eq!(snapshot.entries[1].path.display(), "link-new");
    assert_eq!(snapshot.entries[1].worktree_mode, GitWorktreeMode::Symlink);
    assert_eq!(snapshot.entries[2].status, GitFileStatus::Conflicted);
    assert_eq!(snapshot.entries[2].worktree_mode, GitWorktreeMode::Absent);
    assert_eq!(snapshot.entries[3].worktree_mode, GitWorktreeMode::Gitlink);
    assert_eq!(
        snapshot.entries[4].worktree_mode,
        GitWorktreeMode::Unsupported
    );
}

#[test]
fn status_parser_caps_entries_and_marks_snapshot_too_large() {
    let mut input = b"# branch.oid abc\0# branch.head main\0".to_vec();
    for index in 0..=MAX_STATUS_ENTRIES {
        input.extend_from_slice(
            format!("1 .M N... 100644 100644 100644 abc def f{index}\0").as_bytes(),
        );
    }

    let snapshot = parse_status(&input);

    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert_eq!(snapshot.entries.len(), MAX_STATUS_ENTRIES);
    assert!(snapshot.too_large);
}

#[test]
fn git_path_display_escapes_controls_and_preserves_identity_bytes() {
    let path = GitPath::from_bytes(b"dir\tname/file\nname\x7f.rs");

    assert_eq!(path.as_utf8(), Some("dir\tname/file\nname\x7f.rs"));
    assert_eq!(path.raw(), b"dir\tname/file\nname\x7f.rs");
    assert_eq!(path.display(), "dir\\tname/file\\nname\\u{7f}.rs");
    assert_eq!(path.display_basename(), "file\\nname\\u{7f}.rs");
}

#[test]
fn git_path_display_escapes_bidi_controls_and_backslashes() {
    let path = GitPath::from_bytes("safe\\path\u{202e}gnp.txt".as_bytes());

    assert_eq!(path.display(), "safe\\\\path\\u{202e}gnp.txt");
    assert_eq!(path.as_utf8(), Some("safe\\path\u{202e}gnp.txt"));
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
    let attrs = parse_attrs(b"a.bin\0filter\0lfs\0b.txt\0text\0unset\0c.txt\0eol\0unspecified\0");
    assert!(attrs.is_ok());
    let attrs = attrs.ok();
    assert!(attrs.as_ref().is_some_and(|attrs| attrs.blocks(b"a.bin")));
    assert!(attrs.as_ref().is_some_and(|attrs| !attrs.blocks(b"b.txt")));
}

#[test]
fn attr_parser_caps_path_count() {
    let mut input = Vec::new();
    for index in 0..=MAX_ATTR_PATHS {
        input.extend_from_slice(format!("f{index}\0filter\0unset\0").as_bytes());
    }

    assert_eq!(parse_attrs(&input), Err(GitParseError::TooLarge));
}

#[test]
fn attr_parser_caps_unique_paths_not_attr_triplets() {
    let mut input = Vec::new();
    for index in 0..MAX_ATTR_PATHS {
        for attr in ["filter", "working-tree-encoding", "text", "eol"] {
            input.extend_from_slice(format!("f{index}\0{attr}\0unset\0").as_bytes());
        }
    }

    assert!(parse_attrs(&input).is_ok());
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

proptest! {
    #![proptest_config(bounded_proptest_config())]

    #[test]
    fn proptest_porcelain_v2_robust(bytes in prop::collection::vec(any::<u8>(), 0..16_384)) {
        let snapshot = parse_status(&bytes);
        let record_count = bytes
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .count();
        let attrs = parse_attrs(&bytes);

        prop_assert!(snapshot.entries.len() <= record_count);
        prop_assert!(
            attrs.is_ok()
                || attrs == Err(super::GitParseError::Malformed)
                || attrs == Err(super::GitParseError::TooLarge)
        );
    }
}
