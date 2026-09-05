use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::{gio, prelude::*};

use crate::editor_tab::EditorTab;
use crate::gtk_tests::spin_until;
use crate::window::Window;
use crate::workspace::{OpenSource, Workspace};

pub(super) struct DocumentReadFixture(PathBuf);

impl DocumentReadFixture {
    pub(super) fn new(case: &str, bytes: &[u8]) -> Self {
        let path = PathBuf::from("/tmp").join(format!(
            "riteed-document-reads-{}-{case}.txt",
            std::process::id()
        ));
        let created = OpenOptions::new().create_new(true).write(true).open(&path);
        assert!(
            created.is_ok(),
            "create isolated fixture {path:?}: {created:?}"
        );
        let Ok(mut file) = created else {
            unreachable!("fixture creation was asserted above");
        };
        let written = file.write_all(bytes);
        assert!(written.is_ok(), "write isolated fixture: {written:?}");
        Self(path)
    }

    pub(super) fn file(&self) -> gio::File {
        gio::File::for_path(&self.0)
    }

    pub(super) fn uri(&self) -> String {
        self.file().uri().to_string()
    }

    pub(super) fn replace_with_directory(&self) {
        let removed = fs::remove_file(&self.0);
        assert!(
            removed.is_ok(),
            "remove fixture before failed save: {removed:?}"
        );
        let created = fs::create_dir(&self.0);
        assert!(created.is_ok(), "create failed-save target: {created:?}");
    }

    pub(super) fn replace_directory_with_file(&self, bytes: &[u8]) {
        let removed = fs::remove_dir(&self.0);
        assert!(removed.is_ok(), "remove failed-save target: {removed:?}");
        let written = fs::write(&self.0, bytes);
        assert!(
            written.is_ok(),
            "restore fixture after failed save: {written:?}"
        );
    }

    pub(super) fn open(&self, window: &Rc<Window>, workspace: &Workspace) -> Rc<EditorTab> {
        let uri = self.uri();
        window.request_open_files(vec![self.file()], OpenSource::AppOpen);
        spin_until("document read fixture opens", || {
            window.selected_saved_uri_for_tests() == uri
                && !window.selected_loading_for_tests()
                && workspace.state.borrow().pending_open_targets.is_empty()
        });
        let tab = loaded_tab(workspace);
        assert!(!tab.is_dirty());
        tab
    }
}

impl Drop for DocumentReadFixture {
    fn drop(&mut self) {
        let _removed = fs::remove_file(&self.0);
        let _removed = fs::remove_dir(&self.0);
    }
}

pub(super) fn workspace_for(window: &Window) -> Rc<Workspace> {
    let workspace = window.workspace_weak_for_tests().upgrade();
    assert!(workspace.is_some(), "window must retain its workspace");
    let Some(workspace) = workspace else {
        unreachable!("workspace ownership was asserted above");
    };
    workspace
}

pub(super) fn loaded_tab(workspace: &Workspace) -> Rc<EditorTab> {
    let tab = workspace.selected_tab();
    assert!(tab.is_some(), "fixture must have a selected tab");
    let Some(tab) = tab else {
        unreachable!("selected tab was asserted above");
    };
    tab
}

pub(super) fn insert_accepted_edit(tab: &EditorTab, suffix: &str) {
    assert!(tab.text_view().is_editable());
    let buffer = tab.text_buffer();
    let mut cursor = buffer.end_iter();
    assert!(buffer.insert_interactive(&mut cursor, suffix, true));
    assert!(tab.is_dirty());
    assert!(buffer.property::<bool>("can-undo"));
}
