use gtk4::{gio, prelude::*};
use libadwaita as adw;
use std::rc::Rc;

use crate::gtk_tests::{TempFileFixture, build_window, spin_until, write_temp_file};
use crate::settings::ThemePreference;
use crate::window::Window;
use crate::workspace::OpenSource;

pub(crate) fn exercise_markdown_preview(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let markdown = markdown_fixture();
    let path = write_temp_file(TempFileFixture::MARKDOWN_PREVIEW, markdown.as_bytes());
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();
    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open markdown file", || {
        window.selected_saved_uri_for_tests() == uri
    });

    assert!(window.activate_markdown_preview_for_tests());
    spin_until("markdown preview active", || {
        window.selected_markdown_preview_active_for_tests()
    });
    spin_until("markdown preview rendered", || {
        window
            .selected_markdown_preview_text_for_tests()
            .contains("Heading")
    });
    let preview = window.selected_markdown_preview_text_for_tests();
    assert!(preview.contains("Markdown Frontmatter Metadata"));
    assert!(preview.contains("Markdown Image Placeholder"));
    assert!(preview.contains("Example link"));
    assert!(preview.contains("Alt text"));
    assert!(preview.contains("images are shown as placeholders"));
    assert!(preview.contains("raw HTML is shown as literal text"));
    assert!(preview.contains("<div>raw</div>"));
    assert!(window.selected_markdown_preview_base_css_class_for_tests());
    exercise_preview_zoom_search_and_replace(&window, &preview);
    exercise_preview_copy_and_render_sync(&window, &preview);

    assert!(window.activate_markdown_preview_for_tests());
    spin_until("markdown preview exits", || {
        !window.selected_markdown_preview_active_for_tests()
    });
    let _removed = std::fs::remove_file(path);
}

fn markdown_fixture() -> String {
    let filler = (0..160)
        .map(|index| format!("Filler paragraph {index} with enough text to wrap in preview."))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "---\ntitle: Test\n---\n# Heading\n\nNeedle first\n\n[Example link](https://example.com/docs)\n\n![Alt text](https://example.com/image.png)\n\n<div>raw</div>\n\n{filler}\n\nNeedle second\n\nCopy Anchor\n"
    )
}

fn exercise_preview_zoom_search_and_replace(window: &Rc<Window>, preview: &str) {
    window.activate_status_zoom_in_for_tests();
    drain_for_markdown();
    assert!(
        !window
            .selected_markdown_preview_zoom_css_classes_for_tests()
            .is_empty()
    );

    select_preview_text(window, preview, "Alt text");
    window.open_search(false);
    spin_until("preview search opens with prefill", || {
        window.search_visible_for_tests() && window.search_query_for_tests() == "Alt text"
    });
    assert!(!window.replace_visible_for_tests());
    spin_until("preview search count becomes known", || {
        window.search_result_for_tests() == "1 match"
    });
    window.find_next();
    window.find_previous();

    select_preview_text(window, preview, "Needle first");
    window.open_search(false);
    spin_until("preview repeated search count becomes known", || {
        window.search_query_for_tests() == "Needle first"
            && window.search_result_for_tests() == "1 match"
    });
    select_preview_text(window, preview, "Needle");
    window.open_search(false);
    spin_until("preview repeated query has matches", || {
        window.search_query_for_tests() == "Needle"
            && window.search_result_for_tests() == "2 matches"
    });
    let before_next_scroll = window.selected_markdown_preview_scroll_value_for_tests();
    window.find_next();
    spin_until("preview next match scrolls onscreen", || {
        window.selected_markdown_preview_scroll_value_for_tests() > before_next_scroll + 1.0
    });

    let source_before_replace = window.selected_text_for_tests();
    window.open_search(true);
    drain_for_markdown();
    assert!(!window.replace_visible_for_tests());
    window.set_replace_text_for_tests("Changed alt");
    window.replace_all_for_tests();
    drain_for_markdown();
    assert_eq!(window.selected_text_for_tests(), source_before_replace);
}

fn exercise_preview_copy_and_render_sync(window: &Rc<Window>, preview: &str) {
    select_preview_text(window, preview, "Example link");
    window.open_search(false);
    spin_until("preview link text is searchable", || {
        window.search_query_for_tests() == "Example link"
            && window.search_result_for_tests() == "1 match"
    });

    window.set_selected_markdown_preview_scroll_value_for_tests(10_000.0);
    drain_for_markdown();
    let before_copy_scroll = window.selected_markdown_preview_scroll_value_for_tests();
    assert!(before_copy_scroll > 0.0);
    select_preview_text(window, preview, "Copy Anchor");
    assert!(window.copy_markdown_preview_selection_for_tests());
    spin_until("preview copy preserves scroll", || {
        window.selected_markdown_preview_scroll_value_for_tests() >= before_copy_scroll - 1.0
    });

    window.set_app_appearance_for_tests(ThemePreference::Dark);
    window.sync_appearance_for_tests();
    drain_for_markdown();
    assert_eq!(window.search_result_for_tests(), "1 match");

    window.set_selected_text_for_tests("---\ntitle: Test\n---\n# Updated\n\n[Example link](https://example.com/docs)\n\n![Alt text](https://example.com/image.png)\n");
    spin_until("markdown preview rerenders under active search", || {
        window
            .selected_markdown_preview_text_for_tests()
            .contains("Updated")
            && window.search_result_for_tests() == "1 match"
    });
}

fn select_preview_text(window: &Rc<Window>, preview: &str, needle: &str) {
    assert!(preview.contains(needle));
    let start = i32::try_from(preview.find(needle).unwrap_or_default()).unwrap_or(0);
    let end = start + i32::try_from(needle.len()).unwrap_or(0);
    window.select_markdown_preview_offsets_for_tests(start, end);
}

fn drain_for_markdown() {
    for _ in 0..12 {
        while gtk4::glib::MainContext::default().iteration(false) {}
    }
}
