use super::preference_identity;

#[test]
fn preference_identity_requires_two_nonempty_valid_parts() {
    assert!(preference_identity(&(String::new(), String::new())).is_none());
    assert!(preference_identity(&(String::from("Ada"), String::new())).is_none());
    assert!(
        preference_identity(&(
            String::from("Ada\nLovelace"),
            String::from("ada@example.test")
        ))
        .is_none()
    );
    assert!(
        preference_identity(&(
            String::from("Ada Lovelace"),
            String::from("ada@example.test")
        ))
        .is_some()
    );
}
