use std::rc::Rc;

use crate::editor_search::{SearchScope, SearchTarget};
use crate::window::Window;

impl Window {
    pub(super) fn install_document_callbacks(self: &Rc<Self>) {
        crate::window_format_menu::install(
            &self.change_encoding_action,
            &self.line_ending_action,
            &self.workspace,
        );

        let weak = Rc::downgrade(self);
        self.save_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save();
            }
        });

        let weak = Rc::downgrade(self);
        self.recent_files_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.show_recent_files();
            }
        });

        let weak = Rc::downgrade(self);
        self.save_as_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save_as();
            }
        });

        let weak = Rc::downgrade(self);
        self.close_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_close_current_tab();
            }
        });

        let weak = Rc::downgrade(self);
        self.search_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.open_search(false);
            }
        });

        let weak = Rc::downgrade(self);
        self.replace_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.open_search(true);
            }
        });

        let weak = Rc::downgrade(self);
        self.find_in_files_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.show_find_in_files();
            }
        });

        let weak = Rc::downgrade(self);
        self.find_next_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.find_next();
            }
        });

        let weak = Rc::downgrade(self);
        self.find_prev_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.find_previous();
            }
        });
    }

    pub(super) fn open_search_with_scope(self: &Rc<Self>, scope: SearchScope, replace_mode: bool) {
        let selected = self.workspace.selected_tab();
        let target = if scope == SearchScope::Document {
            selected.as_ref().map_or(SearchTarget::Source, |tab| {
                tab.capture_search_target_for_open()
            })
        } else {
            SearchTarget::Source
        };
        let prefill = selected
            .as_ref()
            .and_then(|tab| tab.single_line_search_selection_text(target));
        self.workspace
            .search
            .open_with_scope(selected, target, scope, replace_mode, prefill);
    }

    pub(super) fn open_project_search(self: &Rc<Self>) {
        self.project.set_sidebar_visible(true);
        self.open_search_with_scope(SearchScope::Project, false);
    }
}
