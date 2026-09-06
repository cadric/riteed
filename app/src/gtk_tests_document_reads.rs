use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::dialogs::encoding::{
    DecodeFailureResponse, queue_decode_failure_responses_for_tests,
    queue_encoding_choices_for_tests,
};
use crate::dialogs::{self, ExternalReloadResponse};
use crate::editor_format::LineEndingMode;
use crate::editor_monitor::{ExternalFileEvent, PendingExternalState};
use crate::editor_tab::{BannerActionKind, ReloadCause, ReloadResult, SaveKind, SaveResult};
use crate::error::AppError;
use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::workspace::OpenSource;

mod fixture;

use fixture::{DocumentReadFixture, insert_accepted_edit, loaded_tab, workspace_for};

fn exercise_initial_metadata_edit(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("metadata-edit", b"disk document\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    let workspace = workspace_for(&window);
    let tab = loaded_tab(&workspace);
    assert!(tab.is_clean_untitled());

    window.request_open_files(vec![fixture.file()], OpenSource::AppOpen);
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 1);
    insert_accepted_edit(&tab, "typed during metadata");
    let format = tab.current_format();

    spin_until("metadata read with intervening edit settles", || {
        workspace.state.borrow().pending_open_targets.is_empty()
            && workspace
                .ordered_tabs()
                .iter()
                .all(|item| !item.is_loading())
    });
    assert_eq!(tab.buffer_text(), "typed during metadata");
    assert!(tab.is_dirty());
    assert!(tab.text_buffer().property::<bool>("can-undo"));
    assert_eq!(tab.current_format(), format);
    assert!(tab.document_uri().is_none());
    window.widget().destroy();
}

fn exercise_allocated_metadata_undo_keeps_redo(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("metadata-undo", b"disk document\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("occupied tab");
    let workspace = workspace_for(&window);
    window.request_open_files(vec![fixture.file()], OpenSource::AppOpen);
    assert_eq!(workspace.tab_view.n_pages(), 2);
    let target = loaded_tab(&workspace);
    insert_accepted_edit(&target, "temporary metadata edit");
    target.text_buffer().undo();
    assert_eq!(target.buffer_text(), "");
    assert!(!target.is_dirty());
    assert!(target.text_buffer().property::<bool>("can-redo"));

    spin_until("allocated metadata undo conflict settles", || {
        workspace.state.borrow().pending_open_targets.is_empty()
            && workspace.ordered_tabs().iter().all(|tab| !tab.is_loading())
    });
    assert_eq!(workspace.tab_view.n_pages(), 2);
    assert!(
        target
            .page()
            .and_then(|page| workspace.find_tab_by_page(&page))
            .is_some_and(|attached| Rc::ptr_eq(&attached, &target))
    );
    assert!(target.text_buffer().property::<bool>("can-redo"));
    target.text_buffer().redo();
    assert_eq!(target.buffer_text(), "temporary metadata edit");
    assert!(target.is_dirty());
    assert!(target.document_uri().is_none());
    window.widget().destroy();
}

fn exercise_manual_reload_edit(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("manual-reload", b"original\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    let original_uri = tab.document_uri();
    let original_format = tab.current_format();
    window.force_selected_external_banner_for_tests();
    assert_eq!(tab.banner_action_kind(), Some(BannerActionKind::Reload));
    window.trigger_selected_external_action_for_tests();
    assert!(tab.is_loading());
    insert_accepted_edit(&tab, " + edit after Reload");

    spin_until("manual reload conflict settles", || !tab.is_loading());
    assert_eq!(tab.buffer_text(), "original + edit after Reload");
    assert!(tab.is_dirty());
    assert!(tab.text_buffer().property::<bool>("can-undo"));
    assert_eq!(tab.current_format(), original_format);
    assert_eq!(tab.document_uri(), original_uri);
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_direct_reload_reports_one_conflict(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("direct-reload", b"original\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    let callbacks = Rc::new(Cell::new(0_u32));
    let result = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let callbacks = Rc::clone(&callbacks);
            let result = Rc::clone(&result);
            move |value| {
                callbacks.set(callbacks.get().saturating_add(1));
                result.replace(Some(value));
            }
        }),
    );
    insert_accepted_edit(&tab, " + conflict");
    spin_until("direct reload conflict callback", || {
        result.borrow().is_some()
    });
    assert_eq!(callbacks.get(), 1);
    let result = result.borrow();
    assert!(result.as_ref().is_some_and(Result::is_err));
    let Some(Err(error)) = result.as_ref() else {
        return;
    };
    assert_eq!(
        error.body(),
        "The load was cancelled because the document changed. Your changes have been kept."
    );
    assert_eq!(tab.buffer_text(), "original + conflict");
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_automatic_reload_conflict_does_not_retry(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("automatic-reload", b"original\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    tab.clear_monitor();
    workspace.request_new_tab();
    let Some(page) = tab.page() else {
        return;
    };
    let starts = Rc::new(Cell::new(0_u32));
    let handler = page.connect_notify_local(Some("loading"), {
        let starts = Rc::clone(&starts);
        move |page, _| {
            if page.is_loading() {
                starts.set(starts.get().saturating_add(1));
            }
        }
    });
    tab.handle_external_event(ExternalFileEvent::ContentPossiblyChanged);
    assert!(tab.is_loading());
    insert_accepted_edit(&tab, " + automatic conflict");
    assert!(!tab.should_present_dirty_reload_prompt());
    spin_until("automatic reload conflict settles", || !tab.is_loading());
    drain_events(24);
    page.disconnect(handler);
    assert_eq!(starts.get(), 1);
    assert_eq!(tab.buffer_text(), "original + automatic conflict");
    assert!(tab.is_dirty());
    assert!(matches!(
        tab.pending_external_state(),
        PendingExternalState::ContentPossiblyChanged {
            acknowledged: false
        }
    ));
    workspace.tab_view.set_selected_page(&page);
    tab.sync_external_banner(true, true);
    assert!(tab.banner_visible_for_tests());
    assert_eq!(tab.banner_action_kind(), Some(BannerActionKind::Reload));
    assert!(!tab.should_present_dirty_reload_prompt());
    assert!(!tab.should_auto_reload(false, false));
    dialogs::queue_external_reload_responses_for_tests(&[ExternalReloadResponse::KeepCurrent]);
    tab.trigger_external_action_for_tests();
    assert!(tab.pending_external_state().is_acknowledged());
    assert_eq!(tab.buffer_text(), "original + automatic conflict");
    dialogs::queue_external_reload_responses_for_tests(&[]);
    window.widget().destroy();
}

fn exercise_encoding_reopen_edit(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("encoding-reopen", b"original\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    let original_uri = tab.document_uri();
    let original_format = tab.current_format();
    queue_encoding_choices_for_tests(&[Some("ISO-8859-1")]);
    window.request_selected_encoding_from_format_menu_for_tests();
    assert!(tab.is_loading());
    insert_accepted_edit(&tab, " + edit after encoding choice");

    spin_until("encoding reopen conflict settles", || !tab.is_loading());
    assert_eq!(tab.buffer_text(), "original + edit after encoding choice");
    assert!(tab.is_dirty());
    assert!(tab.text_buffer().property::<bool>("can-undo"));
    assert_eq!(tab.current_format(), original_format);
    assert_eq!(tab.document_uri(), original_uri);
    queue_encoding_choices_for_tests(&[]);
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_undo_during_reload(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("undo-reload", b"original\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    let result = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let result = Rc::clone(&result);
            move |value| {
                result.replace(Some(value));
            }
        }),
    );
    insert_accepted_edit(&tab, " + edit to undo");
    tab.text_buffer().undo();
    let text_after_undo = tab.buffer_text();
    let dirty_after_undo = tab.is_dirty();
    spin_until("reload after undo settles", || result.borrow().is_some());
    let result = result.borrow();
    assert!(result.as_ref().is_some_and(Result::is_err));
    let Some(Err(error)) = result.as_ref() else {
        return;
    };
    assert_eq!(
        error.body(),
        "The load was cancelled because the document changed. Your changes have been kept."
    );
    assert_eq!(tab.buffer_text(), text_after_undo);
    assert_eq!(tab.is_dirty(), dirty_after_undo);
    assert!(tab.text_buffer().property::<bool>("can-redo"));
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_decode_retry_success(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("decode-retry", b"h\xe9j\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    queue_decode_failure_responses_for_tests(&[DecodeFailureResponse::ChooseEncoding]);
    queue_encoding_choices_for_tests(&[Some("ISO-8859-1")]);
    window.request_open_files(vec![fixture.file()], OpenSource::AppOpen);
    spin_until("manual decoding retry opens", || {
        window.selected_saved_uri_for_tests() == fixture.uri()
            && window.selected_text_for_tests() == "héj"
            && !window.selected_loading_for_tests()
    });
    let tab = loaded_tab(&workspace);
    assert!(!tab.is_dirty());
    assert_eq!(tab.current_format().encoding().charset(), "ISO-8859-1");
    queue_decode_failure_responses_for_tests(&[]);
    queue_encoding_choices_for_tests(&[]);
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_pending_different_files(test_app: &adw::Application) {
    let first = DocumentReadFixture::new("pending-first", b"first\n");
    let second = DocumentReadFixture::new("pending-second", b"second\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    let workspace = workspace_for(&window);
    window.request_open_files(vec![first.file()], OpenSource::AppOpen);
    window.request_open_files(vec![second.file()], OpenSource::AppOpen);
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 2);
    assert_eq!(workspace.tab_view.n_pages(), 2);
    spin_until("different pending files settle", || {
        workspace.state.borrow().pending_open_targets.is_empty()
            && workspace.ordered_tabs().iter().all(|tab| !tab.is_loading())
    });
    let mut documents = workspace
        .ordered_tabs()
        .iter()
        .map(|tab| (tab.document_uri(), tab.buffer_text()))
        .collect::<Vec<_>>();
    documents.sort();
    let mut expected = vec![
        (Some(first.uri()), String::from("first")),
        (Some(second.uri()), String::from("second")),
    ];
    expected.sort();
    assert_eq!(documents, expected);
    window.widget().destroy();
}

fn exercise_pending_close_reopen(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("pending-close-reopen", b"reopened\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    let workspace = workspace_for(&window);
    window.request_open_files(vec![fixture.file()], OpenSource::AppOpen);
    assert_eq!(workspace.state.borrow().pending_open_targets.len(), 1);
    window.request_close_current_tab();
    drain_events(1);
    window.request_open_files(vec![fixture.file()], OpenSource::AppOpen);
    spin_until("closed pending target reopens in attached tab", || {
        workspace.state.borrow().pending_open_targets.is_empty()
            && workspace
                .ordered_tabs()
                .iter()
                .any(|tab| tab.document_uri().as_deref() == Some(fixture.uri().as_str()))
    });
    assert_eq!(workspace.tab_view.n_pages(), 1);
    let tab = loaded_tab(&workspace);
    assert_eq!(tab.buffer_text(), "reopened");
    assert_eq!(tab.document_uri().as_deref(), Some(fixture.uri().as_str()));
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_reload_transfer_settles(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("reload-transfer", b"transferred\n");
    let source = build_window(test_app);
    let destination = build_window(test_app);
    assert!(source.is_some() && destination.is_some());
    let (Some(source), Some(destination)) = (source, destination) else {
        return;
    };
    let source_workspace = workspace_for(&source);
    let destination_workspace = workspace_for(&destination);
    let tab = fixture.open(&source, &source_workspace);
    source_workspace.request_new_tab();
    let Some(page) = tab.page() else {
        return;
    };
    source_workspace.tab_view.set_selected_page(&page);
    source.set_tab_transfer_window_handler(Rc::new({
        let destination_workspace = Rc::clone(&destination_workspace);
        move || Some(Rc::clone(&destination_workspace))
    }));

    let callbacks = Rc::new(Cell::new(0_u32));
    let result = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let callbacks = Rc::clone(&callbacks);
            let result = Rc::clone(&result);
            move |value| {
                callbacks.set(callbacks.get().saturating_add(1));
                result.replace(Some(value));
            }
        }),
    );
    assert!(tab.is_loading());
    assert!(source.activate_tab_move_to_new_window_for_tests());
    spin_until("transferred reload settles", || result.borrow().is_some());
    assert_eq!(callbacks.get(), 1);
    assert!(matches!(
        result.borrow().as_ref(),
        Some(Err(AppError::Cancelled))
    ));
    assert!(!tab.is_loading());
    assert_eq!(tab.buffer_text(), "transferred");
    assert!(
        destination_workspace
            .find_tab_by_page(&page)
            .is_some_and(|current| Rc::ptr_eq(&current, &tab))
    );
    tab.clear_monitor();
    source.widget().destroy();
    destination.widget().destroy();
}

fn exercise_successful_reload(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("reload-success", b"before\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    let result = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let result = Rc::clone(&result);
            move |value| {
                result.replace(Some(value));
            }
        }),
    );
    spin_until("uncontested reload succeeds", || result.borrow().is_some());
    assert!(matches!(
        result.borrow().as_ref(),
        Some(Ok(ReloadResult::Applied))
    ));
    assert_eq!(tab.buffer_text(), "before");
    assert!(!tab.is_dirty());
    assert_eq!(tab.current_format().line_ending_mode(), LineEndingMode::Lf);
    tab.clear_monitor();
    window.widget().destroy();
}

fn exercise_failed_save_releases_superseded_reload(test_app: &adw::Application) {
    let fixture = DocumentReadFixture::new("failed-save-after-reload", b"before\n");
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    let workspace = workspace_for(&window);
    let tab = fixture.open(&window, &workspace);
    tab.clear_monitor();

    let first_reload = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let first_reload = Rc::clone(&first_reload);
            move |result| {
                first_reload.replace(Some(result));
            }
        }),
    );
    assert!(tab.is_loading());
    insert_accepted_edit(&tab, "local edit");
    fixture.replace_with_directory();

    let save_result = Rc::new(RefCell::new(None));
    tab.request_save(
        window.widget(),
        false,
        SaveKind::Manual,
        Rc::new({
            let save_result = Rc::clone(&save_result);
            move |result| {
                save_result.replace(Some(result));
            }
        }),
    );
    spin_until("superseding save fails", || save_result.borrow().is_some());
    assert!(matches!(
        save_result.borrow().as_ref(),
        Some(SaveResult::Failed(_))
    ));
    spin_until("superseded reload callback settles", || {
        first_reload.borrow().is_some()
    });
    assert!(matches!(
        first_reload.borrow().as_ref(),
        Some(Err(AppError::Cancelled))
    ));
    assert!(!tab.is_loading());

    fixture.replace_directory_with_file(b"after\n");
    let retry = Rc::new(RefCell::new(None));
    tab.reload_from_disk(
        ReloadCause::UserRequested,
        Rc::new(|| true),
        Rc::new({
            let retry = Rc::clone(&retry);
            move |result| {
                retry.replace(Some(result));
            }
        }),
    );
    spin_until("reload after failed save settles", || {
        retry.borrow().is_some()
    });
    assert!(matches!(
        retry.borrow().as_ref(),
        Some(Ok(ReloadResult::Applied))
    ));
    assert_eq!(tab.buffer_text(), "after");
    assert!(!tab.is_loading());
    window.widget().destroy();
}

pub(crate) fn exercise_document_read_integrity(test_app: &adw::Application) {
    exercise_initial_metadata_edit(test_app);
    exercise_allocated_metadata_undo_keeps_redo(test_app);
    exercise_manual_reload_edit(test_app);
    exercise_direct_reload_reports_one_conflict(test_app);
    exercise_automatic_reload_conflict_does_not_retry(test_app);
    exercise_encoding_reopen_edit(test_app);
    exercise_undo_during_reload(test_app);
    exercise_decode_retry_success(test_app);
    exercise_pending_different_files(test_app);
    exercise_pending_close_reopen(test_app);
    exercise_reload_transfer_settles(test_app);
    exercise_successful_reload(test_app);
    exercise_failed_save_releases_superseded_reload(test_app);
}
