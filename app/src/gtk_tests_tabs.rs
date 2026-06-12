use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::app::{AppState, ensure_window_for_tests, install_for_tests};
use crate::dialogs::{self, UnsavedResponse};
use crate::document_limits::MIB;
use crate::gtk_tests::{TempFileFixture, drain_events, spin_until, write_temp_file};
use crate::settings::{AppSettings, LargeFileLimitValues};
use crate::workspace::OpenSource;

pub(crate) fn exercise_tab_context_actions() {
    let test_app = adw::Application::builder()
        .application_id("io.github.cadric.Riteed.TabActions")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _registered = test_app.register(None::<&gio::Cancellable>);
    let settings = AppSettings::new_for_tests();
    settings.set_large_file_limit_values(LargeFileLimitValues {
        full_feature: 1,
        editor: 2,
        strong_warning: 3,
        viewer_only: 4,
    });
    let state = std::rc::Rc::new(std::cell::RefCell::new(AppState {
        settings,
        chrome: None,
        windows: Vec::new(),
        last_focused_window: None,
        session_restore_attempted: false,
    }));
    install_for_tests(&test_app, &state);
    test_app.activate();
    let window = ensure_window_for_tests(&test_app, &state);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    spin_until("tab action window creates default tab", || {
        window.tab_count_for_tests() == 1
    });
    window.set_selected_text_for_tests("first");
    window.request_new();
    spin_until("tab action second tab", || {
        window.tab_count_for_tests() == 2
    });
    window.set_selected_text_for_tests("second");
    assert!(window.activate_tab_move_backward_for_tests());
    drain_events(8);
    assert_eq!(window.selected_text_for_tests(), "second");
    assert!(window.activate_tab_move_forward_for_tests());
    drain_events(8);
    assert_eq!(window.selected_text_for_tests(), "second");

    window.request_new();
    spin_until("tab action third tab", || window.tab_count_for_tests() == 3);
    window.set_selected_text_for_tests("third");
    dialogs::queue_unsaved_responses_for_tests(&[
        UnsavedResponse::Discard,
        UnsavedResponse::Discard,
    ]);
    assert!(window.activate_close_other_tabs_for_tests());
    spin_until("close other tabs keeps selected tab", || {
        window.tab_count_for_tests() == 1 && window.selected_text_for_tests() == "third"
    });

    window.request_new();
    spin_until("tab transfer source has two tabs", || {
        window.tab_count_for_tests() == 2
    });
    window.set_selected_text_for_tests("transferred");
    assert!(window.activate_tab_move_to_new_window_for_tests());
    spin_until("tab transfer creates destination window", || {
        state.borrow().windows.len() == 2
    });
    let destination = state.borrow().windows.last().cloned();
    assert!(destination.is_some());
    let Some(destination) = destination else {
        return;
    };
    spin_until("tab transfer moves selected text", || {
        window.tab_count_for_tests() == 1
            && destination.tab_count_for_tests() == 1
            && destination.selected_text_for_tests() == "transferred"
    });
    spin_until("tab transfer restores source zoom class", || {
        window.selected_zoom_css_classes_for_tests().len() == 1
    });
    spin_until("tab transfer applies destination zoom class", || {
        destination.selected_zoom_css_classes_for_tests().len() == 1
    });
    let source_zoom_classes = window.selected_zoom_css_classes_for_tests();
    let destination_zoom_classes = destination.selected_zoom_css_classes_for_tests();
    assert_eq!(source_zoom_classes.len(), 1);
    assert_eq!(destination_zoom_classes.len(), 1);
    assert_ne!(source_zoom_classes, destination_zoom_classes);

    exercise_large_viewer_tab_transfer(&state, &window);

    window.activate_status_zoom_in_for_tests();
    window.activate_status_zoom_in_for_tests();
    drain_events(8);
    assert_eq!(window.zoom_percent_for_tests(), 120);
    assert_eq!(destination.zoom_percent_for_tests(), 100);
    assert_eq!(
        window.selected_zoom_css_classes_for_tests(),
        source_zoom_classes
    );

    destination.activate_status_zoom_in_for_tests();
    drain_events(8);
    assert_eq!(window.zoom_percent_for_tests(), 120);
    assert_eq!(destination.zoom_percent_for_tests(), 110);
    assert_eq!(
        destination.selected_zoom_css_classes_for_tests(),
        destination_zoom_classes
    );
}

fn exercise_large_viewer_tab_transfer(
    state: &std::rc::Rc<std::cell::RefCell<AppState>>,
    window: &std::rc::Rc<crate::window::Window>,
) {
    let path = write_temp_file(
        TempFileFixture::TABS_LARGE_VIEWER_TRANSFER,
        &repeat_seed(b"viewer-transfer\nviewer-line\n", large_viewer_test_len()),
    );
    let file = gio::File::for_path(&path);
    let window_count = state.borrow().windows.len();
    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("large viewer transfer source opens viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .selected_large_file_viewer_text_for_tests()
                .contains("viewer-transfer")
    });
    assert!(window.activate_tab_move_to_new_window_for_tests());
    spin_until("large viewer transfer creates destination window", || {
        state.borrow().windows.len() == window_count.saturating_add(1)
    });
    let large_destination = state.borrow().windows.last().cloned();
    assert!(large_destination.is_some());
    let Some(large_destination) = large_destination else {
        let _removed = std::fs::remove_file(path);
        return;
    };
    spin_until("large viewer survives tab transfer", || {
        large_destination.selected_large_file_surface_for_tests() == Some("viewer")
            && large_destination
                .selected_large_file_viewer_text_for_tests()
                .contains("viewer-transfer")
    });
    let _removed = std::fs::remove_file(path);
}

fn large_viewer_test_len() -> usize {
    usize::try_from(2 * MIB).unwrap_or(0)
}

fn repeat_seed(seed: &[u8], target_len: usize) -> Vec<u8> {
    if seed.is_empty() {
        return vec![b'x'; target_len];
    }
    let mut contents = Vec::with_capacity(target_len);
    while contents.len().saturating_add(seed.len()) <= target_len {
        contents.extend_from_slice(seed);
    }
    let remaining = target_len.saturating_sub(contents.len());
    contents.extend_from_slice(&seed[..remaining]);
    contents
}
