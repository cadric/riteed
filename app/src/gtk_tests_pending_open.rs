use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::error::AppError;
use crate::gtk_tests_document_close::Fixture;
use crate::workspace::OpenSource;

type OpenCallback = Rc<dyn Fn(Result<String, AppError>)>;
type Completions = Rc<RefCell<VecDeque<(bool, Box<dyn FnOnce()>)>>>;

thread_local! {
    static HELD: RefCell<Option<(String, Completions)>> = const { RefCell::new(None) };
}

pub(crate) fn hold_completion(file: &gio::File, callback: OpenCallback) -> OpenCallback {
    let held = HELD.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|(uri, _)| uri == file.uri().as_str())
            .map(|(_, completions)| Rc::clone(completions))
    });
    let Some(held) = held else {
        return callback;
    };
    Rc::new(move |result| {
        let cancelled = matches!(result, Err(AppError::Cancelled));
        let callback = Rc::clone(&callback);
        held.borrow_mut()
            .push_back((cancelled, Box::new(move || callback(result))));
    })
}

struct HoldGuard;

impl Drop for HoldGuard {
    fn drop(&mut self) {
        HELD.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

pub(crate) fn exercise_pending_open_ownership(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let workspace = &fixture.workspace;
    let Some(first) = workspace.selected_tab() else {
        unreachable!("fixture has an initial tab");
    };
    workspace.request_new_tab();
    let file = gio::File::for_path(fixture.directory.join("pending.txt"));
    let first_token = super::register_pending_open(workspace, &file, &first);
    assert!(super::find_tab_by_file(workspace, &file).is_some());
    fixture.select(&first);
    fixture.window.request_close_current_tab();
    assert!(!fixture.attached(&first));
    // A live Rc must not make a detached target selectable again.
    assert!(super::find_tab_by_file(workspace, &file).is_none());

    let second = workspace.add_empty_tab(true);
    let second_token = super::register_pending_open(workspace, &file, &second);
    // Deliver the exact cleanup operation owned by A after B is registered.
    super::clear_pending_open(workspace, &file, &first, first_token);
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 1);
    assert!(
        super::find_tab_by_file(workspace, &file).is_some_and(|tab| Rc::ptr_eq(&tab, &second)),
        "old completion must preserve its successor's attached target"
    );
    let count = workspace.tab_view.n_pages();
    fixture
        .window
        .request_open_files(vec![file.clone()], OpenSource::AppOpen);
    assert_eq!(workspace.tab_view.n_pages(), count);
    assert!(
        workspace
            .selected_tab()
            .is_some_and(|tab| Rc::ptr_eq(&tab, &second))
    );

    // Token ownership also separates successive registrations on the same tab.
    let newer_token = super::register_pending_open(workspace, &file, &second);
    super::clear_pending_open(workspace, &file, &second, second_token);
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 1);
    assert_eq!(
        workspace.state.borrow().pending_open_targets[0].token,
        newer_token
    );
    super::clear_pending_open(workspace, &file, &second, newer_token);
    assert!(workspace.state.borrow().pending_open_targets.is_empty());
    exercise_held_close_reopen(app);
}

fn exercise_held_close_reopen(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let workspace = &fixture.workspace;
    let path = fixture.directory.join("held-open.txt");
    assert!(std::fs::write(&path, b"reopened\n").is_ok());
    let file = gio::File::for_path(path);
    let held: Completions = Rc::new(RefCell::new(VecDeque::new()));
    HELD.with(|slot| {
        assert!(slot.borrow().is_none());
        slot.replace(Some((file.uri().to_string(), Rc::clone(&held))));
    });
    let _guard = HoldGuard;
    fixture
        .window
        .request_open_files(vec![file.clone()], OpenSource::AppOpen);
    let Some(first) = workspace.selected_tab() else {
        unreachable!("first open owns a tab");
    };
    fixture.window.request_close_current_tab();
    assert!(!fixture.attached(&first));
    fixture
        .window
        .request_open_files(vec![file.clone()], OpenSource::AppOpen);
    let Some(second) = workspace.selected_tab() else {
        unreachable!("reopen owns a tab");
    };
    assert!(!Rc::ptr_eq(&first, &second));
    assert!(fixture.attached(&second));
    crate::gtk_tests::spin_until("both real open completions are held", || {
        held.borrow().len() == 2
    });
    let cancelled_index = held.borrow().iter().position(|(cancelled, _)| *cancelled);
    let Some(cancelled_index) = cancelled_index else {
        unreachable!("closed A must cancel");
    };
    let Some((_, resume_first)) = held.borrow_mut().remove(cancelled_index) else {
        unreachable!("A completion exists");
    };
    resume_first();
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 1);
    assert!(super::find_tab_by_file(workspace, &file).is_some_and(|tab| Rc::ptr_eq(&tab, &second)));
    assert!(
        workspace
            .selected_tab()
            .is_some_and(|tab| Rc::ptr_eq(&tab, &second))
    );
    let Some((cancelled, resume_second)) = held.borrow_mut().pop_front() else {
        unreachable!("B completion exists");
    };
    assert!(!cancelled);
    resume_second();
    assert!(workspace.state.borrow().pending_open_targets.is_empty());
    assert_eq!(second.buffer_text(), "reopened");
    assert_eq!(second.document_uri().as_deref(), Some(file.uri().as_str()));
    assert!(fixture.attached(&second));
    second.clear_monitor();
}
