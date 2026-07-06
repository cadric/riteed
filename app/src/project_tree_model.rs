use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

use crate::project_tree_monitor::{ProjectDirectoryMonitor, ProjectDirectorySnapshot};

mod markers;

const ENUMERATE_ATTRIBUTES: &str = "standard::name,standard::display-name,standard::type";
const ENUMERATE_BATCH_SIZE: i32 = 200;

#[derive(Clone, Debug)]
pub(crate) struct ProjectTreeEntry {
    pub(crate) file: gio::File,
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) display_name: String,
    pub(crate) file_type: gio::FileType,
    pub(crate) git_badge: Option<String>,
    pub(crate) dirty: bool,
    sort_key: String,
}

#[derive(Clone, Debug)]
pub(crate) enum ProjectTreeItem {
    Entry(ProjectTreeEntry),
    Loading,
    Error(String),
}

struct ModelState {
    generation: u64,
    show_hidden: bool,
    root: Option<gio::File>,
    root_store: gio::ListStore,
    active_cancellables: Vec<gio::Cancellable>,
    directory_monitors: HashMap<String, ProjectDirectoryMonitor>,
    git_statuses: HashMap<String, String>,
    dirty_uris: HashSet<String>,
    on_structural_change: Option<Rc<dyn Fn()>>,
}

#[derive(Clone)]
pub(crate) struct ProjectTreeModel {
    state: Rc<RefCell<ModelState>>,
    tree_model: gtk4::TreeListModel,
}

impl ProjectTreeModel {
    #[must_use]
    pub(crate) fn new() -> Self {
        let root_store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let state = Rc::new(RefCell::new(ModelState {
            generation: 0,
            show_hidden: false,
            root: None,
            root_store: root_store.clone(),
            active_cancellables: Vec::new(),
            directory_monitors: HashMap::new(),
            git_statuses: HashMap::new(),
            dirty_uris: HashSet::new(),
            on_structural_change: None,
        }));

        let state_for_children = Rc::clone(&state);
        let tree_model = gtk4::TreeListModel::new(root_store, false, false, move |item| {
            let Ok(item) = item.clone().downcast::<glib::BoxedAnyObject>() else {
                return None;
            };
            let Ok(borrowed) = item.try_borrow::<ProjectTreeItem>() else {
                return None;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                return None;
            };
            if entry.file_type != gio::FileType::Directory {
                return None;
            }

            let child_store = gio::ListStore::new::<glib::BoxedAnyObject>();
            child_store.append(&glib::BoxedAnyObject::new(ProjectTreeItem::Loading));
            start_directory_load(
                &state_for_children,
                entry.file.clone(),
                child_store.clone(),
                LoadReason::DirectoryExpand,
            );
            Some(child_store.upcast())
        });

        Self { state, tree_model }
    }

    #[must_use]
    pub(crate) fn model(&self) -> &gtk4::TreeListModel {
        &self.tree_model
    }

    pub(crate) fn set_auto_refresh_handler(&self, handler: Rc<dyn Fn()>) {
        self.state.borrow_mut().on_structural_change = Some(handler);
    }

    pub(crate) fn set_git_statuses(&self, statuses: Vec<(String, String)>) {
        let next_statuses = statuses.into_iter().collect();
        {
            let mut state = self.state.borrow_mut();
            if state.git_statuses == next_statuses {
                return;
            }
            state.git_statuses.clone_from(&next_statuses);
        }
        markers::update_visible_git_badges(&self.tree_model, &next_statuses);
    }

    pub(crate) fn set_dirty_uris(&self, uris: Vec<String>) {
        let next_uris = uris.into_iter().collect();
        {
            let mut state = self.state.borrow_mut();
            if state.dirty_uris == next_uris {
                return;
            }
            state.dirty_uris.clone_from(&next_uris);
        }
        markers::update_visible_dirty_markers(&self.tree_model, &next_uris);
    }

    pub(crate) fn set_show_hidden(&self, show_hidden: bool) {
        let mut state = self.state.borrow_mut();
        if state.show_hidden == show_hidden {
            return;
        }
        state.show_hidden = show_hidden;
        drop(state);
        self.refresh();
    }

    pub(crate) fn set_root(&self, root: Option<gio::File>) {
        let show_hidden = self.state.borrow().show_hidden;
        self.set_root_with_show_hidden(root, show_hidden);
    }

    pub(crate) fn set_root_with_show_hidden(&self, root: Option<gio::File>, show_hidden: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.generation += 1;
            cancel_transient_state(&mut state);
            state.show_hidden = show_hidden;
            state.root = root;
            state.root_store.remove_all();
        }

        if self.root().is_some() {
            let store = self.state.borrow().root_store.clone();
            store.append(&glib::BoxedAnyObject::new(ProjectTreeItem::Loading));
            if let Some(root) = self.root() {
                start_directory_load(&self.state, root, store, LoadReason::RootRefresh);
            }
        }
    }

    pub(crate) fn refresh(&self) {
        let root = self.root();
        self.set_root(root);
    }

    #[must_use]
    pub(crate) fn root(&self) -> Option<gio::File> {
        self.state.borrow().root.clone()
    }

    #[must_use]
    pub(crate) fn snapshot_expanded_uris(&self) -> Vec<String> {
        let mut uris = Vec::new();
        for position in 0..self.tree_model.n_items() {
            let Some(item) = self.tree_model.item(position) else {
                continue;
            };
            let Ok(row) = item.downcast::<gtk4::TreeListRow>() else {
                continue;
            };
            if !row.is_expanded() {
                continue;
            }
            let Some(item) = row.item() else {
                continue;
            };
            let Ok(boxed) = item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            if entry.file_type == gio::FileType::Directory {
                uris.push(entry.uri.clone());
            }
        }
        uris
    }

    pub(crate) fn restore_expanded_uris(&self, uris: &[String]) {
        if uris.is_empty() {
            return;
        }

        for position in 0..self.tree_model.n_items() {
            let Some(item) = self.tree_model.item(position) else {
                continue;
            };
            let Ok(row) = item.downcast::<gtk4::TreeListRow>() else {
                continue;
            };
            let Some(item) = row.item() else {
                continue;
            };
            let Ok(boxed) = item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            if entry.file_type != gio::FileType::Directory {
                continue;
            }
            if uris.iter().any(|candidate| candidate == &entry.uri) {
                row.set_expanded(true);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn visible_entry_names_for_tests(&self) -> Vec<String> {
        let mut names = Vec::new();
        for position in 0..self.tree_model.n_items() {
            let Some(item) = self.tree_model.item(position) else {
                continue;
            };
            let Ok(row) = item.downcast::<gtk4::TreeListRow>() else {
                continue;
            };
            let Some(item) = row.item() else {
                continue;
            };
            let Ok(boxed) = item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            if let ProjectTreeItem::Entry(entry) = &*borrowed {
                names.push(entry.name.clone());
            }
        }
        names
    }

    #[cfg(test)]
    pub(crate) fn expand_entry_for_tests(&self, name: &str) -> bool {
        for position in 0..self.tree_model.n_items() {
            let Some(row) = self.tree_model.row(position) else {
                continue;
            };
            let Some(item) = row.item() else {
                continue;
            };
            let Ok(boxed) = item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            if entry.file_type == gio::FileType::Directory && entry.name == name {
                row.set_expanded(true);
                return true;
            }
        }
        false
    }

    #[cfg(test)]
    pub(crate) fn monitor_count_for_tests(&self) -> usize {
        self.state.borrow().directory_monitors.len()
    }

    #[cfg(test)]
    pub(crate) fn generation_for_tests(&self) -> u64 {
        self.state.borrow().generation
    }

    #[cfg(test)]
    pub(crate) fn dirty_marker_for_tests(&self, name: &str) -> bool {
        for position in 0..self.tree_model.n_items() {
            let Some(row) = self.tree_model.row(position) else {
                continue;
            };
            let Some(item) = row.item() else {
                continue;
            };
            let Ok(boxed) = item.downcast::<glib::BoxedAnyObject>() else {
                continue;
            };
            let Ok(borrowed) = boxed.try_borrow::<ProjectTreeItem>() else {
                continue;
            };
            let ProjectTreeItem::Entry(entry) = &*borrowed else {
                continue;
            };
            if entry.name == name {
                return entry.dirty;
            }
        }
        false
    }
}

#[derive(Clone, Copy, Debug)]
enum LoadReason {
    RootRefresh,
    DirectoryExpand,
}

#[derive(Clone)]
struct DirectoryLoad {
    state: Rc<RefCell<ModelState>>,
    directory: gio::File,
    store: gio::ListStore,
    enumerator: gio::FileEnumerator,
    cancellable: gio::Cancellable,
    generation: u64,
    show_hidden: bool,
}

fn start_directory_load(
    state: &Rc<RefCell<ModelState>>,
    directory: gio::File,
    store: gio::ListStore,
    _reason: LoadReason,
) {
    let (generation, show_hidden, cancellable) = {
        let mut state = state.borrow_mut();
        let generation = state.generation;
        let show_hidden = state.show_hidden;
        let cancellable = gio::Cancellable::new();
        state.active_cancellables.push(cancellable.clone());
        (generation, show_hidden, cancellable)
    };

    let state = Rc::clone(state);
    glib::idle_add_local_once(move || {
        if state.borrow().generation != generation {
            remove_active_cancellable(&state, &cancellable);
            return;
        }

        let directory_for_callback = directory.clone();
        let store_for_callback = store.clone();
        let state_for_callback = Rc::clone(&state);
        let cancellable_for_callback = cancellable.clone();
        directory.enumerate_children_async(
            ENUMERATE_ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::default(),
            Some(&cancellable),
            move |result| match result {
                Ok(enumerator) => collect_enumerator_batch(
                    &DirectoryLoad {
                        state: state_for_callback.clone(),
                        directory: directory_for_callback.clone(),
                        store: store_for_callback.clone(),
                        enumerator,
                        cancellable: cancellable_for_callback.clone(),
                        generation,
                        show_hidden,
                    },
                    Vec::new(),
                ),
                Err(error) => {
                    remove_active_cancellable(&state_for_callback, &cancellable_for_callback);
                    handle_directory_error(
                        &state_for_callback,
                        &store_for_callback,
                        generation,
                        &error,
                    );
                }
            },
        );
    });
}

fn collect_enumerator_batch(load: &DirectoryLoad, mut collected: Vec<gio::FileInfo>) {
    let load_for_callback = load.clone();
    load.enumerator.next_files_async(
        ENUMERATE_BATCH_SIZE,
        glib::Priority::default(),
        Some(&load.cancellable),
        move |result| {
            if load_for_callback.state.borrow().generation != load_for_callback.generation {
                return;
            }
            match result {
                Ok(batch) => {
                    if batch.is_empty() {
                        remove_active_cancellable(
                            &load_for_callback.state,
                            &load_for_callback.cancellable,
                        );
                        finish_directory_load(&load_for_callback, &collected);
                        return;
                    }
                    collected.extend(batch);
                    collect_enumerator_batch(&load_for_callback, collected);
                }
                Err(error) => {
                    remove_active_cancellable(
                        &load_for_callback.state,
                        &load_for_callback.cancellable,
                    );
                    handle_directory_error(
                        &load_for_callback.state,
                        &load_for_callback.store,
                        load_for_callback.generation,
                        &error,
                    );
                }
            }
        },
    );
}

fn finish_directory_load(load: &DirectoryLoad, infos: &[gio::FileInfo]) {
    let mut entries = Vec::new();
    let (git_statuses, dirty_uris) = {
        let state = load.state.borrow();
        (state.git_statuses.clone(), state.dirty_uris.clone())
    };
    for info in infos {
        let file_type = info.file_type();
        if !matches!(
            file_type,
            gio::FileType::Directory | gio::FileType::Regular | gio::FileType::SymbolicLink
        ) {
            continue;
        }

        let name = info.name().to_string_lossy().to_string();
        if !load.show_hidden && name.starts_with('.') {
            continue;
        }

        let file = load.directory.child(info.name());
        let uri = file.uri().to_string();
        let display_name = info.display_name().to_string();
        let git_badge = git_statuses.get(&uri).cloned();
        let dirty = dirty_uris.contains(&uri);
        entries.push(ProjectTreeEntry {
            file,
            uri: uri.clone(),
            name: name.clone(),
            display_name,
            file_type,
            git_badge,
            dirty,
            sort_key: name.to_lowercase(),
        });
    }

    entries.sort_by(compare_entry);
    load.store.remove_all();
    for entry in entries {
        load.store
            .append(&glib::BoxedAnyObject::new(ProjectTreeItem::Entry(entry)));
    }
    install_directory_monitor(
        load,
        &ProjectDirectorySnapshot::from_infos(infos, load.show_hidden),
    );
}

fn compare_entry(left: &ProjectTreeEntry, right: &ProjectTreeEntry) -> Ordering {
    match (
        left.file_type == gio::FileType::Directory,
        right.file_type == gio::FileType::Directory,
    ) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left
            .sort_key
            .cmp(&right.sort_key)
            .then_with(|| left.name.cmp(&right.name)),
    }
}

fn handle_directory_error(
    state: &Rc<RefCell<ModelState>>,
    store: &gio::ListStore,
    generation: u64,
    error: &glib::Error,
) {
    if state.borrow().generation != generation {
        return;
    }
    if error.matches(gio::IOErrorEnum::Cancelled) {
        return;
    }
    store.remove_all();
    store.append(&glib::BoxedAnyObject::new(ProjectTreeItem::Error(
        error.message().to_string(),
    )));
}

fn cancel_transient_state(state: &mut ModelState) {
    for cancellable in state.active_cancellables.drain(..) {
        cancellable.cancel();
    }
    for (_uri, monitor) in state.directory_monitors.drain() {
        monitor.cancel();
    }
}

fn remove_active_cancellable(state: &Rc<RefCell<ModelState>>, cancellable: &gio::Cancellable) {
    state
        .borrow_mut()
        .active_cancellables
        .retain(|active| active != cancellable);
}

fn install_directory_monitor(load: &DirectoryLoad, initial_snapshot: &ProjectDirectorySnapshot) {
    let uri = load.directory.uri().to_string();
    let Some(handler) = load.state.borrow().on_structural_change.clone() else {
        return;
    };
    if load.state.borrow().generation != load.generation
        || load.state.borrow().directory_monitors.contains_key(&uri)
    {
        return;
    }

    let weak_state = Rc::downgrade(&load.state);
    let generation = load.generation;
    let callback = Rc::new(move || {
        let Some(state) = weak_state.upgrade() else {
            return;
        };
        if state.borrow().generation == generation {
            handler();
        }
    });
    let Ok(monitor) = ProjectDirectoryMonitor::new(
        &load.directory,
        initial_snapshot.clone(),
        load.show_hidden,
        callback,
    ) else {
        return;
    };

    let mut state = load.state.borrow_mut();
    if state.generation != generation || state.directory_monitors.contains_key(&uri) {
        monitor.cancel();
        return;
    }
    state.directory_monitors.insert(uri, monitor);
}
