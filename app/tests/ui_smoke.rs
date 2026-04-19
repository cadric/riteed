#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use gtk4 as gtk;
use libadwaita as adw;

#[test]
fn constructs_main_surfaces() {
    riteed::bootstrap_runtime();
    let _gtk = gtk::init();
    let _adw = adw::init();

    let window_builder = gtk::Builder::from_resource("/io/github/cadric/Riteed/ui/window.ui");
    let window: Option<adw::ApplicationWindow> = window_builder.object("window");
    assert!(window.is_some());

    let preferences_builder =
        gtk::Builder::from_resource("/io/github/cadric/Riteed/ui/preferences.ui");
    let preferences: Option<adw::PreferencesDialog> =
        preferences_builder.object("preferences_dialog");
    assert!(preferences.is_some());

    let shortcuts_builder = gtk::Builder::from_resource("/io/github/cadric/Riteed/ui/shortcuts.ui");
    let shortcuts: Option<gtk::ShortcutsWindow> = shortcuts_builder.object("shortcuts_window");
    assert!(shortcuts.is_some());
}
