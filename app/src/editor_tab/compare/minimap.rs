use gtk4::{pango, prelude::*};
use sourceview5::prelude::*;

use crate::editor_tab::EditorTab;
use crate::editor_zoom::resolve_minimap_font_description;

const COMPARE_MINIMAP_WIDTH: i32 = 72;

pub(super) struct CompareMinimap {
    pub(super) holder: gtk4::Box,
    map: sourceview5::Map,
    scrolled: gtk4::ScrolledWindow,
}

impl CompareMinimap {
    pub(super) fn new(tab: &EditorTab, view: &sourceview5::View) -> Self {
        let minimap_font = resolve_minimap_font_description(&tab.settings.editor_font());
        let map = sourceview5::Map::builder()
            .view(view)
            .font_desc(&minimap_font)
            .build();
        map.set_can_focus(false);
        map.set_cursor_visible(false);
        map.set_editable(false);
        map.set_focusable(false);
        map.set_hexpand(false);
        map.set_monospace(true);
        map.set_vexpand(true);

        let holder = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .visible(tab.settings.show_minimap())
            .build();
        holder.set_hexpand(false);
        holder.set_vexpand(true);
        holder.set_width_request(COMPARE_MINIMAP_WIDTH);
        holder.append(&map);

        Self {
            holder,
            map,
            scrolled: gtk4::ScrolledWindow::new(),
        }
    }

    pub(super) fn with_scrolled(mut self, scrolled: &gtk4::ScrolledWindow) -> Self {
        self.scrolled = scrolled.clone();
        self
    }

    pub(super) fn set_visible(&self, visible: bool) {
        self.holder.set_visible(visible);
        let policy = if visible {
            gtk4::PolicyType::External
        } else {
            gtk4::PolicyType::Automatic
        };
        self.scrolled.set_vscrollbar_policy(policy);
    }

    pub(super) fn set_font_desc(&self, font_desc: Option<&pango::FontDescription>) {
        self.map.set_font_desc(font_desc);
    }

    #[cfg(test)]
    pub(super) fn visible_for_tests(&self) -> bool {
        self.holder.property::<bool>("visible")
    }
}
