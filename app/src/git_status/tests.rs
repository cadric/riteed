use super::{
    GitFileStatus, GitWorktreeMode, index_info_line, parse_attrs, parse_ls_tree_entry,
    parse_status, resolve_capabilities,
};

#[test]
fn status_parser_reads_branch_and_entries() {
    let input =
        b"# branch.oid abc\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +1 -0\0\
1 .M N... 100644 100644 100644 abc def src/lib.rs\0? new.txt\0";
    let snapshot = parse_status(input);
    assert_eq!(snapshot.branch.as_deref(), Some("main"));
    assert_eq!(snapshot.head_oid.as_deref(), Some("abc"));
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.entries[0].path.display(), "src/lib.rs");
    assert!(snapshot.entries[0].unstaged);
    assert_eq!(
        snapshot.entries[0].worktree_mode,
        GitWorktreeMode::Regular("100644")
    );
    assert_eq!(snapshot.entries[1].path.display(), "new.txt");
    assert_eq!(snapshot.entries[1].worktree_mode, GitWorktreeMode::Unknown);
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
2 RM N... 100644 100644 120000 abc def R100 link-new\0link-old\0\
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
