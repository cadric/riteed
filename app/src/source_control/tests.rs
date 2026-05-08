use super::{GitProcessError, git_error_text};

#[test]
fn git_errors_map_to_user_copy() {
    assert_eq!(
        git_error_text(&GitProcessError::InvalidIdentity),
        "The Git identity is not valid."
    );
    assert_eq!(
        git_error_text(&GitProcessError::OutputTooLarge),
        "Git output was too large to process safely."
    );
    assert_eq!(
        git_error_text(&GitProcessError::BinaryContent),
        "Binary files cannot be compared."
    );
    assert_eq!(
        git_error_text(&GitProcessError::ParseFailed),
        "The Git operation failed."
    );
}

#[test]
fn source_control_legacy_list_patterns_stay_removed() {
    let controller = include_str!("../source_control.rs");
    let actions = include_str!("actions.rs");
    let css = include_str!("../../data/ui/appearance.css");
    let patterns = [
        concat!("gtk4::List", "Box"),
        concat!("rebuild", "_rows"),
        concat!("row", "-activated"),
        concat!("row", "_at_index"),
    ];
    for source in [controller, actions, css] {
        for pattern in patterns {
            assert!(!source.contains(pattern));
        }
    }
}

#[test]
fn source_control_row_actions_do_not_overlay_file_names() {
    let list = include_str!("list_view.rs");
    let tree = include_str!("tree_view.rs");
    for source in [list, tree] {
        assert!(!source.contains("add_overlay"));
        assert!(!source.contains("set_measure_overlay"));
        assert!(!source.contains("riteed-git-row-actions"));
        assert!(!source.contains("Stage File"));
        assert!(!source.contains("Unstage File"));
        assert!(!source.contains("Discard Changes"));
    }
    let popover = include_str!("row_popover.rs");
    assert!(popover.contains("popup_for_row"));
    assert!(popover.contains("compute_bounds"));
}

#[test]
fn source_control_row_context_actions_keep_keyboard_and_pointer_access() {
    let list = include_str!("list_view.rs");
    let tree = include_str!("tree_view.rs");
    for source in [list, tree] {
        assert!(source.contains("GestureClick::new"));
        assert!(source.contains("set_button(3)"));
        assert!(source.contains("\"Menu\""));
        assert!(source.contains("\"<Shift>F10\""));
        assert!(source.contains("ShortcutTrigger::parse_string"));
        assert!(source.contains("riteed-sidebar-row"));
    }

    let popover = include_str!("row_popover.rs");
    assert!(popover.contains("let Some(bounds) = row_widget.compute_bounds(list_view) else"));
    assert!(popover.contains("return;"));
    assert!(!popover.contains("unwrap_or_default"));
}

#[test]
fn contextual_git_actions_have_header_buttons_and_a11y_state_binding() {
    let window_ui = include_str!("../../data/ui/window.ui");
    assert!(window_ui.contains("git_actions_group"));
    assert!(window_ui.contains("win.scm-diff-active"));
    assert!(window_ui.contains("win.scm-stage-active"));
    assert!(window_ui.contains("win.scm-unstage-active"));
    assert!(window_ui.contains("win.scm-discard-active"));

    let action_widgets = include_str!("action_widgets.rs");
    assert!(action_widgets.contains("set_tooltip_text(Some(reason))"));
    assert!(action_widgets.contains("Property::Description(reason)"));
}

#[test]
fn project_search_uses_find_bar_instead_of_sidebar_inputs() {
    let find_in_files = include_str!("../find_in_files/mod.rs");
    assert!(!find_in_files.contains("SearchEntry"));
    assert!(!find_in_files.contains("query_entry"));
    assert!(!find_in_files.contains("match_case_button"));
    assert!(find_in_files.contains("current_query"));
    assert!(find_in_files.contains("current_match_case"));

    let actions = include_str!("../window/actions.rs");
    let window = include_str!("../window.rs");
    let app = include_str!("../app.rs");
    assert!(actions.contains("open_search_with_scope(SearchScope::Project, false)"));
    assert!(window.contains("SearchScope::Document"));
    assert!(app.contains("\"win.find-in-files\", &[\"<Ctrl><Shift>f\"]"));

    let sidebar_host = include_str!("../sidebar_host.rs");
    assert!(sidebar_host.contains("set_search_results_visible"));
    assert!(!sidebar_host.contains(".clear()"));
}
