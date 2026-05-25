use std::rc::Rc;

use gtk4::gdk;
use gtk4::{glib, prelude::*};
use sourceview5::prelude::*;

use super::{EditorSearch, ProjectSearchRequest, SearchScope, SearchTarget, replace};

impl EditorSearch {
    pub(super) fn install_callbacks(
        self: &Rc<Self>,
        close_button: &gtk4::Button,
        reveal_replace_button: &gtk4::Button,
    ) {
        self.install_scope_callbacks();
        self.install_entry_callbacks();
        self.install_option_and_navigation_callbacks();
        self.install_replace_callbacks(close_button, reveal_replace_button);
        self.install_search_bar_callback();
    }

    fn install_scope_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.scope_bar.connect_scope_changed(Rc::new(move |scope| {
            if let Some(search) = weak.upgrade() {
                search.switch_scope(scope);
            }
        }));
    }

    fn install_entry_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.search_entry.connect_search_changed(move |_| {
            if let Some(search) = weak.upgrade() {
                search.on_search_changed();
            }
        });

        let weak = Rc::downgrade(self);
        self.search_entry.connect_activate(move |_| {
            if let Some(search) = weak.upgrade() {
                search.find_next();
            }
        });

        let controller = gtk4::EventControllerKey::new();
        controller.connect_key_pressed({
            let weak = Rc::downgrade(self);
            move |_, key, _, modifiers| {
                if key == gdk::Key::Return && modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
                    if let Some(search) = weak.upgrade() {
                        search.find_previous();
                    }
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        });
        self.search_entry.add_controller(controller);
    }

    fn install_option_and_navigation_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.match_case_button.connect_toggled(move |_| {
            if let Some(search) = weak.upgrade() {
                search.on_search_changed();
            }
        });

        let weak = Rc::downgrade(self);
        self.previous_button.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.find_previous();
            }
        });

        let weak = Rc::downgrade(self);
        self.next_button.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.find_next();
            }
        });

        let weak = Rc::downgrade(self);
        self.replace_button.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.replace_current();
            }
        });

        let weak = Rc::downgrade(self);
        self.replace_all_button.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.replace_all();
            }
        });
    }

    fn install_replace_callbacks(
        self: &Rc<Self>,
        close_button: &gtk4::Button,
        reveal_replace_button: &gtk4::Button,
    ) {
        close_button.connect_clicked({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(search) = weak.upgrade() {
                    search.close();
                }
            }
        });

        reveal_replace_button.connect_clicked({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(search) = weak.upgrade() {
                    if search.active_target_is_preview() {
                        replace::set_replace_mode_visible(&search.replace_row, false);
                        search.update_result_state();
                        return;
                    }
                    search.last_replace_mode.set(true);
                    replace::set_replace_mode_visible(&search.replace_row, true);
                    search.replace_entry.grab_focus();
                    search.update_result_state();
                }
            }
        });
    }

    fn install_search_bar_callback(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.search_bar
            .connect_search_mode_enabled_notify(move |bar| {
                if let Some(search) = weak.upgrade() {
                    if bar.is_search_mode() {
                        match search.scope_bar.current_scope() {
                            SearchScope::Document => search.enter_document_scope(),
                            SearchScope::Project => search.enter_project_scope(),
                        }
                    } else {
                        search.drop_active_context();
                        search.clear_manual_message();
                        search.result_label.set_label("");
                        search.update_result_state();
                    }
                }
            });
    }

    fn switch_scope(self: &Rc<Self>, scope: SearchScope) {
        match scope {
            SearchScope::Document => self.enter_document_scope(),
            SearchScope::Project => self.enter_project_scope(),
        }
    }

    pub(super) fn enter_project_scope(self: &Rc<Self>) {
        self.drop_active_context();
        replace::set_replace_mode_visible(&self.replace_row, false);
        self.reveal_replace_button.set_visible(false);
        self.scope_bar.force_project();
        self.result_label.set_label("");
        self.set_action_sensitivity(false);
        self.dispatch_project_query();
    }

    pub(super) fn enter_document_scope(self: &Rc<Self>) {
        self.scope_bar.reset_to_document();
        self.sync_document_replace_visibility();
        let active_tab = self.state.borrow().active_tab.clone();
        self.bind_active_context(active_tab);
    }

    pub(super) fn active_target_is_preview(&self) -> bool {
        self.state.borrow().active_target == SearchTarget::Preview
    }

    pub(super) fn sync_document_replace_visibility(&self) {
        let preview_target = self.active_target_is_preview();
        self.reveal_replace_button.set_visible(!preview_target);
        replace::set_replace_mode_visible(
            &self.replace_row,
            !preview_target && self.last_replace_mode.get(),
        );
    }

    fn on_search_changed(self: &Rc<Self>) {
        self.clear_manual_message();
        let query = self.query();
        self.settings
            .set_search_text(if query.is_empty() { None } else { Some(&query) });
        self.settings
            .set_case_sensitive(self.match_case_button.is_active());
        if self.scope_bar.current_scope() == SearchScope::Project {
            self.dispatch_project_query();
            self.result_label.set_label("");
            self.set_action_sensitivity(false);
            return;
        }
        let active_tab = self.state.borrow().active_tab.clone();
        if let Some(tab) = active_tab {
            self.bind_active_context(Some(tab));
        } else {
            self.update_result_state();
        }
    }

    fn dispatch_project_query(&self) {
        let Some(dispatch) = self
            .project_search_dispatch
            .borrow()
            .as_ref()
            .map(Rc::clone)
        else {
            return;
        };
        dispatch(ProjectSearchRequest::Query {
            query: self.query().to_string(),
            match_case: self.match_case_button.is_active(),
        });
    }
}
