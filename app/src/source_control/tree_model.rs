use std::collections::BTreeMap;

use gtk4::{gio, glib, prelude::*};

use crate::git_status::GitStatusEntry;

#[derive(Clone)]
pub(super) enum SourceControlNode {
    Folder {
        display_name: String,
        full_path: String,
        children_store: gio::ListStore,
    },
    File {
        entry: GitStatusEntry,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SourceControlSelection {
    Folder(String),
    File(Vec<u8>),
}

#[derive(Default)]
struct FolderBuilder {
    folders: BTreeMap<String, FolderBuilder>,
    files: Vec<GitStatusEntry>,
}

pub(super) fn build_root_store(entries: &[GitStatusEntry]) -> gio::ListStore {
    let mut root = FolderBuilder::default();
    for entry in entries {
        insert_entry(&mut root, entry.clone());
    }
    build_store(&root, "")
}

pub(super) fn snapshot_expanded_paths(model: &gtk4::TreeListModel) -> Vec<String> {
    let mut paths = Vec::new();
    for position in 0..model.n_items() {
        let Some(row) = model.row(position) else {
            continue;
        };
        if !row.is_expanded() {
            continue;
        }
        if let Some(SourceControlNode::Folder { full_path, .. }) = node_for_row(&row) {
            paths.push(full_path);
        }
    }
    paths
}

pub(super) fn restore_expanded_paths(model: &gtk4::TreeListModel, paths: &[String]) {
    let mut sorted = paths.to_vec();
    sorted.sort_by(|left, right| depth(left).cmp(&depth(right)).then_with(|| left.cmp(right)));
    for path in sorted {
        if let Some((row, _position)) = find_row(model, &SourceControlSelection::Folder(path)) {
            row.set_expanded(true);
        }
    }
}

pub(super) fn snapshot_selected_node(
    selection: &gtk4::SingleSelection,
) -> Option<SourceControlSelection> {
    let position = selection.selected();
    if position == gtk4::INVALID_LIST_POSITION {
        return None;
    }
    let model = selection.model()?;
    let row = model.item(position)?.downcast::<gtk4::TreeListRow>().ok()?;
    selection_for_row(&row)
}

pub(super) fn restore_selected_node(
    model: &gtk4::TreeListModel,
    selection: &gtk4::SingleSelection,
    selected: Option<SourceControlSelection>,
) {
    let Some(selected) = selected else {
        selection.set_selected(gtk4::INVALID_LIST_POSITION);
        return;
    };
    if let Some((_row, position)) = find_row(model, &selected) {
        selection.set_selected(position);
    } else {
        selection.set_selected(gtk4::INVALID_LIST_POSITION);
    }
}

#[cfg(test)]
pub(crate) fn exercise_state_restore_for_tests() {
    let entries = [
        state_restore_entry("src/main.rs"),
        state_restore_entry("src/bin/lib.rs"),
        state_restore_entry("README.md"),
    ];
    let root = build_root_store(&entries);
    let model = test_tree_list_model(&root);
    restore_expanded_paths(&model, &[String::from("src"), String::from("src/bin")]);
    assert_eq!(
        snapshot_expanded_paths(&model),
        [String::from("src"), String::from("src/bin")]
    );

    let selection = gtk4::SingleSelection::new(Some(model.clone()));
    selection.set_selected(gtk4::INVALID_LIST_POSITION);
    let selected = Some(SourceControlSelection::File(b"src/main.rs".to_vec()));
    restore_selected_node(&model, &selection, selected.clone());
    assert_eq!(snapshot_selected_node(&selection), selected);
    restore_selected_node(
        &model,
        &selection,
        Some(SourceControlSelection::Folder(String::from("src"))),
    );
    assert_eq!(
        snapshot_selected_node(&selection),
        Some(SourceControlSelection::Folder(String::from("src")))
    );
}

pub(super) fn node_for_row(row: &gtk4::TreeListRow) -> Option<SourceControlNode> {
    let item = row.item()?;
    let boxed = item.downcast::<glib::BoxedAnyObject>().ok()?;
    let borrowed = boxed.try_borrow::<SourceControlNode>().ok()?;
    Some((*borrowed).clone())
}

pub(super) fn node_for_position(
    model: &gtk4::TreeListModel,
    position: u32,
) -> Option<(gtk4::TreeListRow, SourceControlNode)> {
    let row = model.row(position)?;
    let node = node_for_row(&row)?;
    Some((row, node))
}

pub(super) fn file_basename(entry: &GitStatusEntry) -> String {
    entry
        .path
        .as_utf8()
        .map(|path| path.trim_end_matches('/'))
        .and_then(|path| path.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| entry.path.display())
        .to_string()
}

fn insert_entry(root: &mut FolderBuilder, entry: GitStatusEntry) {
    let Some(path) = entry.path.as_utf8() else {
        root.files.push(entry);
        return;
    };
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let Some((_file_name, folders)) = parts.split_last() else {
        root.files.push(entry);
        return;
    };
    let mut folder = root;
    for part in folders {
        folder = folder.folders.entry((*part).to_string()).or_default();
    }
    folder.files.push(entry);
}

fn build_store(folder: &FolderBuilder, full_path: &str) -> gio::ListStore {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for (name, child) in &folder.folders {
        let child_path = if full_path.is_empty() {
            name.clone()
        } else {
            format!("{full_path}/{name}")
        };
        let children_store = build_store(child, &child_path);
        store.append(&glib::BoxedAnyObject::new(SourceControlNode::Folder {
            display_name: name.clone(),
            full_path: child_path,
            children_store,
        }));
    }

    let mut files = folder
        .files
        .iter()
        .cloned()
        .map(|entry| (file_basename(&entry).to_lowercase(), entry))
        .collect::<Vec<_>>();
    files.sort_by(|(left_key, left), (right_key, right)| {
        left_key
            .cmp(right_key)
            .then_with(|| left.path.raw().cmp(right.path.raw()))
    });
    for (_key, entry) in files {
        store.append(&glib::BoxedAnyObject::new(SourceControlNode::File {
            entry,
        }));
    }
    store
}

fn depth(path: &str) -> usize {
    path.split('/').filter(|part| !part.is_empty()).count()
}

fn find_row(
    model: &gtk4::TreeListModel,
    selection: &SourceControlSelection,
) -> Option<(gtk4::TreeListRow, u32)> {
    for position in 0..model.n_items() {
        let row = model.row(position)?;
        if selection_for_row(&row).as_ref() == Some(selection) {
            return Some((row, position));
        }
    }
    None
}

fn selection_for_row(row: &gtk4::TreeListRow) -> Option<SourceControlSelection> {
    match node_for_row(row)? {
        SourceControlNode::Folder { full_path, .. } => {
            Some(SourceControlSelection::Folder(full_path))
        }
        SourceControlNode::File { entry } => {
            Some(SourceControlSelection::File(entry.path.raw().to_vec()))
        }
    }
}

#[cfg(test)]
fn state_restore_entry(path: &str) -> GitStatusEntry {
    GitStatusEntry::new(
        crate::git_status::GitPath::from_bytes(path.as_bytes()),
        crate::git_status::GitFileStatus::Modified,
        Some(String::from("head")),
        Some(String::from("index")),
        false,
        true,
    )
}

#[cfg(test)]
fn test_tree_list_model(root: &gio::ListStore) -> gtk4::TreeListModel {
    gtk4::TreeListModel::new(root.clone(), false, false, move |item| {
        let Ok(item) = item.clone().downcast::<glib::BoxedAnyObject>() else {
            return None;
        };
        let Ok(borrowed) = item.try_borrow::<SourceControlNode>() else {
            return None;
        };
        match &*borrowed {
            SourceControlNode::Folder { children_store, .. } => {
                Some(children_store.clone().upcast())
            }
            SourceControlNode::File { .. } => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{SourceControlNode, build_root_store, file_basename};
    use crate::git_status::{GitFileStatus, GitPath, GitStatusEntry};
    use gtk4::prelude::{Cast, ListModelExt};
    use gtk4::{gio, glib};

    #[test]
    fn nested_paths_build_folder_nodes() {
        let store = build_root_store(&[entry("src/bin/main.rs"), entry("src/lib.rs")]);
        assert_eq!(store.n_items(), 1);
        let node = node_at(&store, 0);
        assert!(matches!(node, Some(SourceControlNode::Folder { .. })));
        if let Some(SourceControlNode::Folder {
            display_name,
            children_store,
            ..
        }) = node
        {
            assert_eq!(display_name, "src");
            assert_eq!(children_store.n_items(), 2);
        }
    }

    #[test]
    fn nested_child_stores_keep_full_virtual_paths() {
        let store = build_root_store(&[entry("src/bin/main.rs")]);
        let root = node_at(&store, 0);
        assert!(matches!(root, Some(SourceControlNode::Folder { .. })));
        let Some(SourceControlNode::Folder {
            full_path,
            children_store,
            ..
        }) = root
        else {
            return;
        };
        assert_eq!(full_path, "src");
        let child = node_at(&children_store, 0);
        assert!(matches!(child, Some(SourceControlNode::Folder { .. })));
        let Some(SourceControlNode::Folder {
            full_path,
            children_store,
            ..
        }) = child
        else {
            return;
        };
        assert_eq!(full_path, "src/bin");
        assert_eq!(node_name(&children_store, 0).as_deref(), Some("main.rs"));
    }

    #[test]
    fn file_basename_uses_utf8_leaf_or_display_fallback() {
        assert_eq!(file_basename(&entry("src/lib.rs")), "lib.rs");
        assert_eq!(file_basename(&entry("nested-repo/")), "nested-repo");
        let invalid = GitStatusEntry::new(
            GitPath::from_bytes(b"\xff"),
            GitFileStatus::Modified,
            None,
            None,
            false,
            true,
        );
        assert_eq!(file_basename(&invalid), "Invalid path encoding");
    }

    #[test]
    fn root_files_and_nested_files_coexist() {
        let store = build_root_store(&[entry("README.md"), entry("src/lib.rs")]);
        assert_eq!(node_name(&store, 0).as_deref(), Some("src"));
        assert_eq!(node_name(&store, 1).as_deref(), Some("README.md"));
    }

    #[test]
    fn duplicate_basenames_remain_distinct_and_deterministic() {
        let input = [entry("b/main.rs"), entry("a/main.rs"), entry("README.md")];
        let first = visible_names(&build_root_store(&input));
        let second = visible_names(&build_root_store(&input));
        assert_eq!(first, second);
        assert_eq!(first, ["a", "b", "README.md"]);
    }

    #[test]
    fn staged_deleted_and_non_utf8_entries_stay_visible() {
        let deleted = GitStatusEntry::new(
            GitPath::from_bytes(b"deleted.txt"),
            GitFileStatus::Deleted,
            Some(String::from("head")),
            None,
            true,
            false,
        );
        let invalid = GitStatusEntry::new(
            GitPath::from_bytes(b"\xff"),
            GitFileStatus::Modified,
            None,
            None,
            false,
            true,
        );
        let store = build_root_store(&[deleted, invalid]);
        assert_eq!(
            visible_names(&store),
            ["deleted.txt", "Invalid path encoding"]
        );
    }

    fn entry(path: &str) -> GitStatusEntry {
        GitStatusEntry::new(
            GitPath::from_bytes(path.as_bytes()),
            GitFileStatus::Modified,
            Some(String::from("head")),
            Some(String::from("index")),
            false,
            true,
        )
    }

    fn node_at(store: &gio::ListStore, index: u32) -> Option<SourceControlNode> {
        let item = store.item(index)?;
        let boxed = item.downcast::<glib::BoxedAnyObject>().ok()?;
        let borrowed = boxed.try_borrow::<SourceControlNode>().ok()?;
        Some((*borrowed).clone())
    }

    fn node_name(store: &gio::ListStore, index: u32) -> Option<String> {
        match node_at(store, index)? {
            SourceControlNode::Folder { display_name, .. } => Some(display_name),
            SourceControlNode::File { entry } => Some(file_basename(&entry)),
        }
    }

    fn visible_names(store: &gio::ListStore) -> Vec<String> {
        (0..store.n_items())
            .filter_map(|position| node_name(store, position))
            .collect()
    }
}
