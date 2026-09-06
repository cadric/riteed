use std::rc::Rc;

use gtk4::gio;
use libadwaita as adw;

use crate::gtk_tests_document_close::Fixture;
use crate::workspace::OpenSource;

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
}
