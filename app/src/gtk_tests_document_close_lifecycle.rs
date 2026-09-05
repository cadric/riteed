use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::dialogs::{self, UnsavedResponse};
use crate::gtk_tests::spin_until;
use crate::gtk_tests_document_close::Fixture;

pub(crate) fn exercise_stale_callbacks(app: &adw::Application) {
    let fixture = Fixture::new(app, false);
    let tab = fixture.open("saved.txt");
    fixture.workspace.request_new_tab();
    fixture.select(&tab);
    tab.text_buffer().set_text("snapshot A");
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    fixture.window.request_close_current_tab();
    assert!(tab.is_loading());
    let Some(old) = fixture.workspace.state.borrow().close_flow.clone() else {
        unreachable!("first close coordinator");
    };
    // Invoke the production cancellation boundary while the actual save is pending.
    super::cancel_close_flow(&fixture.workspace, &old);
    tab.text_buffer().set_text("newer B");
    fixture.window.request_close_current_tab();
    let Some(current) = fixture.workspace.state.borrow().close_flow.clone() else {
        unreachable!("replacement close coordinator awaiting its dialog");
    };
    assert!(!Rc::ptr_eq(&old, &current));
    // A late response from the earlier dialog must not discard this revision.
    super::handle_close_response(&fixture.workspace, &old, &tab, UnsavedResponse::Discard);
    spin_until("original snapshot write completes", || !tab.is_loading());
    assert!(fixture.attached(&tab));
    assert_eq!(tab.buffer_text(), "newer B");
    assert!(tab.is_dirty());
    assert!(
        fixture
            .workspace
            .state
            .borrow()
            .close_flow
            .as_ref()
            .is_some_and(|coordinator| Rc::ptr_eq(coordinator, &current))
    );
    assert_eq!(
        std::fs::read(fixture.directory.join("saved.txt")).ok(),
        Some(b"snapshot A\n".to_vec())
    );
    super::handle_close_response(&fixture.workspace, &current, &tab, UnsavedResponse::Cancel);
    fixture.settled();
}

pub(crate) fn exercise_detached_queue(app: &adw::Application) {
    for detach_current in [false, true] {
        let fixture = Fixture::new(app, false);
        let destination = Fixture::new(app, false);
        let current = fixture.open("current.txt");
        let future = fixture.open("future.txt");
        current.text_buffer().set_text("current saved");
        future.text_buffer().set_text("future unsaved");
        fixture.select(&current);
        dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
        let _propagation = fixture.window.close_request_for_tests();
        assert!(current.is_loading());
        let detached = if detach_current { &current } else { &future };
        let Some(page) = detached.page() else {
            unreachable!("attached queue member");
        };
        // Exercise GTK's real transfer/detach boundary, independent of menu sensitivity.
        fixture.workspace.tab_view.transfer_page(
            &page,
            &destination.workspace.tab_view,
            destination.workspace.tab_view.n_pages(),
        );
        assert!(!fixture.attached(detached));
        assert!(
            fixture.workspace.state.borrow().close_flow.is_none(),
            "detaching any queued target must cancel the window close"
        );
        spin_until("save settles after transfer", || !current.is_loading());
        assert!(!fixture.workspace.allow_window_close());
        assert_eq!(future.buffer_text(), "future unsaved");
        assert!(future.is_dirty());
        current.clear_monitor();
        future.clear_monitor();
    }
}
