use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events};

pub(crate) fn exercise_v12_power_tools(test_app: &adw::Application) {
    empty_source_buffer_line_count_follows_gtk();
    exercise_print_runner_injection(test_app);
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
    window.set_print_runner_for_tests(Rc::new(move |_parent, view, title| {
        let buffer = view.buffer();
        captured_for_runner.replace(Some((title.to_owned(), buffer.char_count())));
    }));
    assert!(window.activate_print_for_tests());
    drain_events(4);

    assert_eq!(
        captured.borrow().clone(),
        Some((String::from("Untitled"), 8))
    );
}
