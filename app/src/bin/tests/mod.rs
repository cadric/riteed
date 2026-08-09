use gtk4::prelude::*;

use super::*;

#[test]
fn source_control_lock_wait_state_is_visible() {
    assert!(gtk4::init().is_ok());
    let root = gtk4::Label::new(Some("Waiting for another Git operation to finish"))
        .upcast::<gtk4::Widget>();

    assert_eq!(
        source_control_state_label(&root),
        Some("Waiting for another Git operation to finish")
    );
}
