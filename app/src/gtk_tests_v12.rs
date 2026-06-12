use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events, spin_until};

pub(crate) fn exercise_v12_power_tools(test_app: &adw::Application) {
    empty_source_buffer_line_count_follows_gtk();
    exercise_print_runner_injection(test_app);
    exercise_print_preview_engine(test_app);
    exercise_print_preview_action_injection(test_app);
}

fn empty_source_buffer_line_count_follows_gtk() {
    let buffer = sourceview5::Buffer::new(None);
    assert_eq!(buffer.line_count(), 1);
}

fn exercise_print_runner_injection(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("print me");
    drain_events(4);
    assert_eq!(
        window.document_tool_actions_enabled_for_tests(),
        (true, true)
    );

    let captured = Rc::new(RefCell::new(None));
    let captured_for_runner = Rc::clone(&captured);
    window.set_print_runner_for_tests(Rc::new(move |job: &crate::document_print::PrintJob<'_>| {
        let buffer = job.view.buffer();
        captured_for_runner.replace(Some((
            job.title.to_owned(),
            buffer.char_count(),
            job.body_font.to_owned(),
        )));
    }));
    assert!(window.activate_print_for_tests());
    drain_events(4);

    assert_eq!(
        captured.borrow().clone(),
        Some((String::from("Untitled"), 8, String::from("Monospace 11")))
    );
}

fn exercise_print_preview_engine(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let text = "preview me\n".repeat(40);
    window.set_selected_text_for_tests(&text);
    drain_events(4);

    let Some(engine) = window.start_print_preview_engine_for_tests() else {
        return;
    };
    spin_until("print preview paginates", || engine.is_ready());

    assert!(engine.n_pages() >= 1);
    let texture = engine.render_page(0);
    assert!(texture.is_some(), "preview page 0 did not render");
    if let Some(texture) = texture {
        assert!(texture.width() > 300);
        assert!(texture.height() > texture.width() / 2);
    }
    engine.finish();
}

fn exercise_print_preview_action_injection(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("preview action");
    drain_events(4);

    let captured = Rc::new(RefCell::new(None));
    let captured_for_runner = Rc::clone(&captured);
    window.set_print_preview_runner_for_tests(Rc::new(move |_parent, _view, title, body_font| {
        captured_for_runner.replace(Some((title.to_owned(), body_font.to_owned())));
    }));
    assert!(window.activate_print_preview_for_tests());
    drain_events(4);

    assert_eq!(
        captured.borrow().clone(),
        Some((String::from("Untitled"), String::from("Monospace 11")))
    );
}
