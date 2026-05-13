use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::gtk_tests::{build_window, spin_until, write_temp_file};
use crate::workspace::OpenSource;

pub(crate) fn exercise_markdown_preview(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let path = write_temp_file(
        "riteed-markdown-preview.md",
        b"---\ntitle: Test\n---\n# Heading\n\n![Alt text](https://example.com/image.png)\n\n<div>raw</div>\n",
    );
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
    assert!(preview.contains("images are shown as placeholders"));
    assert!(preview.contains("raw HTML is shown as literal text"));
    assert!(preview.contains("<div>raw</div>"));

    assert!(window.activate_markdown_preview_for_tests());
    spin_until("markdown preview exits", || {
        !window.selected_markdown_preview_active_for_tests()
    });
    let _removed = std::fs::remove_file(path);
}
