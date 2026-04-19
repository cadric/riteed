pub const MAX_RECENT_FILES: usize = 10;

#[must_use]
pub fn remember_recent(existing: &[String], uri: &str) -> Vec<String> {
    let mut updated = vec![String::from(uri)];
    for candidate in existing {
        if candidate != uri && updated.len() < MAX_RECENT_FILES {
            updated.push(candidate.clone());
        }
    }
    updated
}

#[must_use]
pub fn forget_recent(existing: &[String], uri: &str) -> Vec<String> {
    existing
        .iter()
        .filter(|candidate| candidate.as_str() != uri)
        .cloned()
        .collect()
}

#[must_use]
pub fn session_snapshot(ordered_saved_uris: &[Option<String>]) -> Vec<String> {
    ordered_saved_uris.iter().filter_map(Clone::clone).collect()
}

#[must_use]
pub fn selected_session_value(selected_uri: Option<String>) -> String {
    selected_uri.unwrap_or_default()
}

#[must_use]
pub fn list_changed(current: &[String], next: &[String]) -> bool {
    current != next
}

#[must_use]
pub fn string_changed(current: &str, next: &str) -> bool {
    current != next
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RECENT_FILES, forget_recent, list_changed, remember_recent, selected_session_value,
        session_snapshot, string_changed,
    };

    #[test]
    fn remember_recent_moves_uri_to_front_and_caps_length() {
        let existing = (0..MAX_RECENT_FILES)
            .map(|index| format!("file:///tmp/{index}.txt"))
            .collect::<Vec<_>>();
        let updated = remember_recent(&existing, "file:///tmp/3.txt");
        assert_eq!(
            updated.first().map(String::as_str),
            Some("file:///tmp/3.txt")
        );
        assert_eq!(updated.len(), MAX_RECENT_FILES);
    }

    #[test]
    fn forget_recent_prunes_missing_entry() {
        let existing = vec![
            String::from("file:///tmp/one.txt"),
            String::from("file:///tmp/two.txt"),
        ];
        let updated = forget_recent(&existing, "file:///tmp/two.txt");
        assert_eq!(updated, vec![String::from("file:///tmp/one.txt")]);
    }

    #[test]
    fn session_snapshot_keeps_only_saved_uris_in_order() {
        let snapshot = session_snapshot(&[
            Some(String::from("file:///tmp/one.txt")),
            None,
            Some(String::from("file:///tmp/two.txt")),
        ]);
        assert_eq!(
            snapshot,
            vec![
                String::from("file:///tmp/one.txt"),
                String::from("file:///tmp/two.txt"),
            ]
        );
    }

    #[test]
    fn selected_session_value_uses_empty_string_sentinel() {
        assert!(selected_session_value(None).is_empty());
        assert_eq!(
            selected_session_value(Some(String::from("file:///tmp/one.txt"))),
            "file:///tmp/one.txt"
        );
    }

    #[test]
    fn change_detection_helpers_are_strict() {
        assert!(!list_changed(&[String::from("a")], &[String::from("a")]));
        assert!(list_changed(&[String::from("a")], &[String::from("b")]));
        assert!(!string_changed("same", "same"));
        assert!(string_changed("one", "two"));
    }
}
