use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::gtk_tests::{build_window, drain_events, spin_until};

pub(crate) fn exercise_v12_power_tools(test_app: &adw::Application) {
    empty_source_buffer_line_count_follows_gtk();
    exercise_print_runner_injection(test_app);
    exercise_print_preview_engine(test_app);
    exercise_print_preview_action_injection(test_app);
    print_operation_keeps_point_units();
    exercise_print_pagination_matches_paper(test_app);
    exercise_print_preview_margins_blank(test_app);
    exercise_editor_hides_print_margin_guide();
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

fn print_operation_keeps_point_units() {
    let view = sourceview5::View::new();
    let (operation, _compositor) =
        crate::document_print::build_print_operation(&view, "units.md", "Monospace 11");
    assert_eq!(
        operation.unit(),
        gtk4::Unit::None,
        "GtkSourcePrintCompositor lays out in points; GtkPrintOperation must keep GTK's default unit"
    );
}

fn exercise_print_pagination_matches_paper(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let text = "Kort linje med indhold\n".repeat(200);
    window.set_selected_text_for_tests(&text);
    drain_events(4);

    let Some(engine) = window.start_print_preview_engine_for_tests() else {
        return;
    };
    spin_until("print pagination completes", || engine.is_ready());
    let pages = engine.n_pages();
    engine.finish();
    assert!(
        (4..=8).contains(&pages),
        "200 short lines at Monospace 11 should fill 4-8 A4 pages, got {pages}"
    );
}

fn exercise_print_preview_margins_blank(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let text = "Margenmaaling 0123456789\n".repeat(60);
    window.set_selected_text_for_tests(&text);
    drain_events(4);

    let Some(engine) = window.start_print_preview_engine_for_tests() else {
        return;
    };
    spin_until("print preview paginates", || engine.is_ready());
    let texture = engine.render_page(0);
    assert!(texture.is_some(), "preview page 0 did not render");
    if let Some(texture) = texture {
        assert_margin_bands_blank(&texture);
    }
    engine.finish();
}

fn assert_margin_bands_blank(texture: &gtk4::gdk::Texture) {
    let width = usize::try_from(texture.width()).unwrap_or_default();
    let height = usize::try_from(texture.height()).unwrap_or_default();
    assert!(
        width > 100 && height > 100,
        "unexpected preview texture size"
    );
    let stride = width * 4;
    let mut data = vec![0_u8; stride * height];
    texture.download(&mut data, stride);
    let band_bytes = 43 * 4;
    for row in 0..height {
        let line = &data[row * stride..(row + 1) * stride];
        let left_blank = line[..band_bytes].iter().all(|byte| *byte == 255);
        let right_blank = line[stride - band_bytes..].iter().all(|byte| *byte == 255);
        assert!(
            left_blank && right_blank,
            "print margins must stay blank, found ink in row {row}"
        );
    }
}

fn exercise_editor_hides_print_margin_guide() {
    let settings = crate::settings::AppSettings::new_for_tests();
    let view = crate::editor_view::EditorView::new(&settings);
    assert!(
        !view.text_view.shows_right_margin(),
        "editor must not show a print-width guide"
    );
}
