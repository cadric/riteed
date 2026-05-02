use libadwaita as adw;
use libadwaita::prelude::*;

use crate::APP_ID;

pub(crate) fn configure(window: &adw::ApplicationWindow) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let icon_theme = gtk4::IconTheme::for_display(&display);
        icon_theme.add_resource_path("/io/github/cadric/Riteed/icons");
        if let Ok(path) = std::env::var("RITEED_DEV_ICON_DIR") {
            let icon_dir = std::path::PathBuf::from(path);
            if icon_dir.is_dir() {
                let mut search_paths = icon_theme.search_path();
                search_paths.retain(|existing| existing != &icon_dir);
                search_paths.insert(0, icon_dir);
                let refs = search_paths
                    .iter()
                    .map(std::path::PathBuf::as_path)
                    .collect::<Vec<_>>();
                icon_theme.set_search_path(&refs);
            }
        }
    }
    gtk4::Window::set_default_icon_name(APP_ID);
    window.set_icon_name(Some(APP_ID));
}
