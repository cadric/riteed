use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::dialogs::{self, UnsavedResponse};
use crate::editor_tab::EditorTab;
use crate::settings::AppSettings;
use crate::window::Window;
use crate::workspace::{OpenSource, Workspace};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static SAVE_CHOICES: RefCell<VecDeque<Result<gio::File, glib::Error>>> = const { RefCell::new(VecDeque::new()) };
}

pub(crate) fn take_save_choice() -> Option<Result<gio::File, glib::Error>> {
    SAVE_CHOICES.with(|choices| choices.borrow_mut().pop_front())
}

fn queue_save_choice(file: Option<gio::File>) {
    let result = file
        .ok_or_else(|| glib::Error::new(gtk4::DialogError::Dismissed, "Test chooser dismissed"));
    SAVE_CHOICES.with(|choices| choices.borrow_mut().push_back(result));
}

fn spin_until(label: &str, done: impl Fn() -> bool) {
    let wakeup = glib::timeout_add_local(std::time::Duration::from_millis(10), || {
        glib::ControlFlow::Continue
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !done() && std::time::Instant::now() < deadline {
        let _dispatched = glib::MainContext::default().iteration(true);
    }
    wakeup.remove();
    assert!(done(), "{label}");
}

pub(crate) struct Fixture {
    pub(crate) directory: PathBuf,
    pub(crate) window: Rc<Window>,
    pub(crate) workspace: Rc<Workspace>,
}

impl Fixture {
    pub(crate) fn new(app: &adw::Application, autosave: bool) -> Self {
        assert!(gtk4::gdk::Display::default().is_some());
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = PathBuf::from(format!(
            "/tmp/riteed-document-close-{}-{id}",
            std::process::id()
        ));
        assert!(fs::create_dir(&directory).is_ok());
        let settings = AppSettings::new_for_tests();
        settings.set_autosave_enabled(autosave);
        let window = match Window::new_for_tests(app, &settings, None) {
            Ok(window) => window,
            Err(error) => unreachable!("test window: {error:?}"),
        };
        window.ensure_default_tab();
        let Some(workspace) = window.workspace_weak_for_tests().upgrade() else {
            unreachable!("window owns workspace");
        };
        Self {
            directory,
            window,
            workspace,
        }
    }

    pub(crate) fn open(&self, name: &str) -> Rc<EditorTab> {
        let path = self.directory.join(name);
        assert!(fs::write(&path, b"original\n").is_ok());
        let file = gio::File::for_path(path);
        let uri = file.uri().to_string();
        self.window
            .request_open_files(vec![file], OpenSource::AppOpen);
        spin_until("close fixture loaded", || {
            self.window.selected_saved_uri_for_tests() == uri
                && !self.window.selected_loading_for_tests()
                && self
                    .workspace
                    .state
                    .borrow()
                    .pending_open_targets
                    .is_empty()
        });
        let Some(tab) = self.workspace.selected_tab() else {
            unreachable!("loaded tab selected");
        };
        spin_until("close fixture writability", || {
            tab.writability() == crate::editor_tab::Writability::Writable
        });
        tab
    }

    pub(crate) fn select(&self, tab: &EditorTab) {
        let Some(page) = tab.page() else {
            unreachable!("attached tab");
        };
        self.workspace.tab_view.set_selected_page(&page);
    }

    pub(crate) fn attached(&self, tab: &Rc<EditorTab>) -> bool {
        self.workspace
            .ordered_tabs()
            .iter()
            .any(|item| Rc::ptr_eq(item, tab))
    }

    pub(crate) fn settled(&self) {
        spin_until("document close settled", || {
            self.workspace.state.borrow().close_flow.is_none()
                && self
                    .workspace
                    .ordered_tabs()
                    .iter()
                    .all(|tab| !tab.is_loading())
        });
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        dialogs::queue_unsaved_responses_for_tests(&[]);
        SAVE_CHOICES.with(|choices| choices.borrow_mut().clear());
        for tab in self.workspace.ordered_tabs() {
            tab.cancel_io();
            tab.clear_monitor();
        }
        self.window.widget().destroy();
        let _removed = fs::remove_dir_all(&self.directory);
    }
}

fn insert(tab: &EditorTab, text: &str) {
    assert!(
        tab.text_view().is_editable(),
        "editor must accept new input"
    );
    let buffer = tab.text_buffer();
    buffer.begin_user_action();
    assert!(buffer.insert_interactive(&mut buffer.end_iter(), text, true));
    buffer.end_user_action();
    assert!(tab.is_dirty());
    assert!(buffer.can_undo());
}

fn assert_newer_edit_survives(fixture: &Fixture, tab: &Rc<EditorTab>) {
    assert!(
        fixture.attached(tab),
        "newer unsaved text must keep its tab"
    );
    assert_eq!(tab.buffer_text(), "snapshot A + newer B");
    assert!(tab.is_dirty());
    let buffer = tab.text_buffer();
    buffer.undo();
    assert_eq!(tab.buffer_text(), "snapshot A");
    assert!(buffer.can_redo());
    buffer.redo();
    assert_eq!(tab.buffer_text(), "snapshot A + newer B");
    assert!(!fixture.workspace.allow_window_close());
}

pub(crate) fn exercise_window_save(app: &adw::Application) {
    for count in [1, 2] {
        let fixture = Fixture::new(app, false);
        for index in 0..count {
            let tab = fixture.open(&format!("{index}.txt"));
            tab.text_buffer().set_text("saved before closing");
        }
        dialogs::queue_unsaved_responses_for_tests(&vec![UnsavedResponse::Save; count]);
        assert_eq!(
            fixture.window.close_request_for_tests(),
            glib::Propagation::Stop
        );
        spin_until("window save grants close", || {
            fixture.workspace.allow_window_close()
        });
        assert!(fixture.workspace.state.borrow().close_flow.is_none());
        for index in 0..count {
            assert_eq!(
                fs::read(fixture.directory.join(format!("{index}.txt"))).ok(),
                Some(b"saved before closing\n".to_vec())
            );
        }
    }
}

pub(crate) fn exercise_newer_edits(app: &adw::Application) {
    for autosave in [false, true] {
        for mode in ["tab", "window", "others"] {
            let fixture = Fixture::new(app, autosave);
            let tab = fixture.open("saved.txt");
            fixture.workspace.request_new_tab();
            let Some(keep) = fixture.workspace.selected_tab().and_then(|tab| tab.page()) else {
                unreachable!("keep tab");
            };
            fixture.select(&tab);
            tab.text_buffer().set_text("snapshot A");
            dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
            match mode {
                "tab" => fixture.window.request_close_current_tab(),
                "window" => {
                    let _propagation = fixture.window.close_request_for_tests();
                }
                _ => crate::workspace_close::request_close_other_tabs(&fixture.workspace, &keep),
            }
            assert!(tab.is_loading(), "actual save starts before accepted edit");
            insert(&tab, " + newer B");
            fixture.settled();
            assert_eq!(
                fs::read(fixture.directory.join("saved.txt")).ok(),
                Some(b"snapshot A\n".to_vec())
            );
            assert_newer_edit_survives(&fixture, &tab);
            // A fresh deliberate Save-and-close must work after the rejection.
            dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
            fixture.window.request_close_current_tab();
            spin_until("retry closes saved tab", || !fixture.attached(&tab));
            assert_eq!(
                fs::read(fixture.directory.join("saved.txt")).ok(),
                Some(b"snapshot A + newer B\n".to_vec())
            );
        }
    }
}

pub(crate) fn exercise_window_recheck(app: &adw::Application) {
    for earlier_choice in [
        None,
        Some(UnsavedResponse::Discard),
        Some(UnsavedResponse::Save),
    ] {
        let fixture = Fixture::new(app, false);
        let earlier = fixture.open("earlier.txt");
        let last = fixture.open("last.txt");
        last.text_buffer().set_text("last saved");
        if earlier_choice.is_some() {
            earlier.text_buffer().set_text("earlier change");
            fixture.select(&earlier);
        }
        let mut responses = earlier_choice.into_iter().collect::<Vec<_>>();
        responses.push(UnsavedResponse::Save);
        dialogs::queue_unsaved_responses_for_tests(&responses);
        let _propagation = fixture.window.close_request_for_tests();
        spin_until("last document save pending", || last.is_loading());
        insert(&earlier, " + edit after earlier decision");
        let expected = earlier.buffer_text();
        fixture.settled();
        assert!(
            !fixture.workspace.allow_window_close(),
            "final check must include earlier tabs"
        );
        assert!(fixture.attached(&earlier));
        assert_eq!(earlier.buffer_text(), expected);
        assert!(earlier.is_dirty());
    }
}

pub(crate) fn exercise_cancel_and_failure(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let tab = fixture.open("saved.txt");
    fixture.workspace.request_new_tab();
    fixture.select(&tab);
    tab.text_buffer().set_text("unsaved");
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Cancel]);
    fixture.window.request_close_current_tab();
    fixture.settled();
    assert!(fixture.attached(&tab));
    assert!(tab.is_dirty());
    // Real write failure, no permission assumptions (the test also works as root).
    tab.clear_monitor();
    assert!(fs::remove_file(fixture.directory.join("saved.txt")).is_ok());
    assert!(fs::create_dir(fixture.directory.join("saved.txt")).is_ok());
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    fixture.settled();
    assert!(fixture.attached(&tab));
    assert_eq!(tab.buffer_text(), "unsaved");
    assert!(tab.is_dirty());
    assert!(!fixture.workspace.allow_window_close());
    assert!(fs::remove_dir(fixture.directory.join("saved.txt")).is_ok());
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    spin_until("retry after actual write failure", || {
        !fixture.attached(&tab)
    });
    assert_eq!(
        fs::read(fixture.directory.join("saved.txt")).ok(),
        Some(b"unsaved\n".to_vec())
    );
}

pub(crate) fn exercise_rejected_sibling_close(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let tab = fixture.open("saved.txt");
    fixture.workspace.request_new_tab();
    let Some(sibling) = fixture.workspace.selected_tab() else {
        unreachable!("sibling tab");
    };
    let Some(sibling_page) = sibling.page() else {
        unreachable!("sibling page");
    };
    fixture.select(&tab);
    tab.text_buffer().set_text("snapshot A");
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    assert!(tab.is_loading());
    fixture.workspace.tab_view.close_page(&sibling_page);
    assert!(fixture.attached(&sibling));
    insert(&tab, " + newer B");
    fixture.settled();
    assert_newer_edit_survives(&fixture, &tab);
    fixture.workspace.tab_view.close_page(&sibling_page);
    assert!(
        !fixture.attached(&sibling),
        "rejected sibling close can be retried"
    );
}

pub(crate) fn exercise_save_as(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let Some(tab) = fixture.workspace.selected_tab() else {
        unreachable!("initial untitled tab");
    };
    fixture.workspace.request_new_tab();
    fixture.select(&tab);
    tab.text_buffer().set_text("snapshot A");
    // Cancelling the path chooser must reject the pending page close.
    queue_save_choice(None);
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    fixture.settled();
    assert!(fixture.attached(&tab));
    assert!(tab.document_uri().is_none());
    assert!(tab.is_dirty());

    let file = gio::File::for_path(fixture.directory.join("save-as.txt"));
    queue_save_choice(Some(file.clone()));
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    assert!(tab.is_loading());
    insert(&tab, " + newer B");
    fixture.settled();
    assert_newer_edit_survives(&fixture, &tab);
    assert_eq!(tab.document_uri().as_deref(), Some(file.uri().as_str()));
    assert_eq!(
        fs::read(fixture.directory.join("save-as.txt")).ok(),
        Some(b"snapshot A\n".to_vec())
    );
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    spin_until("save-as follow-up closes tab", || !fixture.attached(&tab));
    assert_eq!(
        fs::read(fixture.directory.join("save-as.txt")).ok(),
        Some(b"snapshot A + newer B\n".to_vec())
    );
}
