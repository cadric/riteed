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
