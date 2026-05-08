use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{glib, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::dialogs;
use crate::editor_tab::EditorTab;

mod callbacks;
mod replace;
mod scope;
mod state;
mod support;
use scope::ScopeBar;
pub(crate) use scope::SearchScope;
use state::{SearchBinding, SearchState};
use support::{
    count_matches, format_match_count, format_replaced_count, select_match, selection_matches_query,
};

pub(crate) type ProjectSearchDispatch = Rc<dyn Fn(ProjectSearchRequest)>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectSearchRequest {
    Query { query: String, match_case: bool },
}

pub struct EditorSearch {
    parent_window: adw::ApplicationWindow,
    search_bar: gtk4::SearchBar,
    search_entry: gtk4::SearchEntry,
    replace_entry: gtk4::Entry,
    match_case_button: gtk4::ToggleButton,
    previous_button: gtk4::Button,
    next_button: gtk4::Button,
    reveal_replace_button: gtk4::Button,
    replace_button: gtk4::Button,
    replace_all_button: gtk4::Button,
    scope_bar: ScopeBar,
    result_label: gtk4::Label,
    replace_row: gtk4::Box,
    settings: sourceview5::SearchSettings,
    project_search_dispatch: RefCell<Option<ProjectSearchDispatch>>,
    last_replace_mode: Cell<bool>,
    state: RefCell<SearchState>,
}

impl EditorSearch {
    #[must_use]
    pub fn new(parent_window: &adw::ApplicationWindow) -> Rc<Self> {
        let search_entry = gtk4::SearchEntry::builder().hexpand(true).build();
        let match_case_button =
            gtk4::ToggleButton::with_label(&pgettext("search option", "Match Case"));
        let previous_button = icon_button(
            "go-up-symbolic",
            &pgettext("search action", "Previous Match"),
        );
        let next_button = icon_button("go-down-symbolic", &pgettext("search action", "Next Match"));
        let replace_controls = replace::build_controls();
        let result_label = gtk4::Label::builder().xalign(0.0).hexpand(true).build();
        result_label.add_css_class("dim-label");

        let close_button = icon_button(
            "window-close-symbolic",
            &pgettext("search action", "Close Find"),
        );
        let reveal_replace_button = icon_button(
            "edit-find-replace-symbolic",
            &pgettext("search action", "Show Replace"),
        );

        let scope_bar = ScopeBar::new();
        let first_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        first_row.append(&search_entry);
        first_row.append(scope_bar.widget());
        first_row.append(&reveal_replace_button);
        first_row.append(&close_button);

        let second_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        second_row.append(&previous_button);
        second_row.append(&next_button);
        second_row.append(&match_case_button);
        second_row.append(&result_label);

        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .margin_bottom(6)
            .margin_end(12)
            .margin_start(12)
            .margin_top(6)
            .build();
        content.append(&first_row);
        content.append(&second_row);
        content.append(&replace_controls.row);

        let search_bar = gtk4::SearchBar::new();
        search_bar.connect_entry(&search_entry);
        search_bar.set_child(Some(&content));

        let settings = sourceview5::SearchSettings::new();
        settings.set_wrap_around(true);

        let search = Rc::new(Self {
            parent_window: parent_window.clone(),
            search_bar,
            search_entry,
            replace_entry: replace_controls.entry,
            match_case_button,
            previous_button,
            next_button,
            reveal_replace_button: reveal_replace_button.clone(),
            replace_button: replace_controls.replace_button,
            replace_all_button: replace_controls.replace_all_button,
            scope_bar,
            result_label,
            replace_row: replace_controls.row,
            settings,
            project_search_dispatch: RefCell::new(None),
            last_replace_mode: Cell::new(false),
            state: RefCell::new(SearchState {
                active_tab: None,
                active_binding: None,
                manual_message: None,
            }),
        });
        search.install_callbacks(&close_button, &reveal_replace_button);
        search.update_result_state();
        search
    }

    #[must_use]
    pub fn widget(&self) -> &gtk4::SearchBar {
        &self.search_bar
    }

    pub fn open(
        self: &Rc<Self>,
        tab: Option<Rc<EditorTab>>,
        replace_mode: bool,
        prefill: Option<String>,
    ) {
        self.open_with_scope(tab, SearchScope::Document, replace_mode, prefill);
    }

    pub(crate) fn open_with_scope(
        self: &Rc<Self>,
        tab: Option<Rc<EditorTab>>,
        scope: SearchScope,
        replace_mode: bool,
        prefill: Option<String>,
    ) {
        self.last_replace_mode.set(replace_mode);
        if let Some(prefill) = prefill {
            self.search_entry.set_text(&prefill);
        }
        self.search_bar.set_search_mode(true);
        self.state.borrow_mut().active_tab = tab;
        match scope {
            SearchScope::Document => self.enter_document_scope(),
            SearchScope::Project => self.enter_project_scope(),
        }
        self.search_entry.grab_focus();
        self.search_entry.select_region(0, -1);
    }

    pub(crate) fn set_project_search_dispatch(&self, dispatch: ProjectSearchDispatch) {
        self.project_search_dispatch.replace(Some(dispatch));
    }

    pub fn close(self: &Rc<Self>) {
        self.search_bar.set_search_mode(false);
    }

    pub fn bind_tab(self: &Rc<Self>, tab: Option<Rc<EditorTab>>) {
        let search_active = self.search_bar.is_search_mode();
        self.drop_active_context();
        self.state.borrow_mut().active_tab.clone_from(&tab);
        if search_active && self.scope_bar.current_scope() == SearchScope::Document {
            self.bind_active_context(tab);
        } else if search_active {
            self.result_label.set_label("");
            self.set_action_sensitivity(false);
        } else {
            self.clear_manual_message();
            self.update_result_state();
        }
    }

    pub fn find_next(self: &Rc<Self>) {
        self.clear_manual_message();
        let Some(tab) = self.state.borrow().active_tab.clone() else {
            self.update_result_state();
            return;
        };
        let Some(context) = self.active_context() else {
            self.update_result_state();
            return;
        };
        let buffer = tab.text_buffer();
        let start = buffer
            .selection_bounds()
            .map_or_else(|| buffer.iter_at_mark(&buffer.get_insert()), |(_, end)| end);
        if let Some((match_start, match_end, _wrapped)) = context.forward(&start) {
            select_match(&tab, &match_start, &match_end);
        }
        self.update_result_state();
    }

    pub fn find_previous(self: &Rc<Self>) {
        self.clear_manual_message();
        let Some(tab) = self.state.borrow().active_tab.clone() else {
            self.update_result_state();
            return;
        };
        let Some(context) = self.active_context() else {
            self.update_result_state();
            return;
        };
        let buffer = tab.text_buffer();
        let start = buffer.selection_bounds().map_or_else(
            || buffer.iter_at_mark(&buffer.get_insert()),
            |(start, _)| start,
        );
        if let Some((match_start, match_end, _wrapped)) = context.backward(&start) {
            select_match(&tab, &match_start, &match_end);
        }
        self.update_result_state();
    }

    pub fn replace_current(self: &Rc<Self>) {
        if self.scope_bar.current_scope() == SearchScope::Project {
            return;
        }
        self.clear_manual_message();
        let Some(tab) = self.state.borrow().active_tab.clone() else {
            self.update_result_state();
            return;
        };
        let Some(context) = self.active_context() else {
            self.update_result_state();
            return;
        };
        let Some((mut match_start, mut match_end)) = self.current_match_bounds(&tab, &context)
        else {
            self.update_result_state();
            return;
        };
        let replacement = self.replace_entry.text();
        if let Err(error) = context.replace(&mut match_start, &mut match_end, &replacement) {
            dialogs::present_error(&self.parent_window, &error.into());
            return;
        }
        select_match(&tab, &match_start, &match_end);
        self.update_result_state();
    }

    pub fn replace_all(self: &Rc<Self>) {
        if self.scope_bar.current_scope() == SearchScope::Project {
            return;
        }
        let Some(tab) = self.state.borrow().active_tab.clone() else {
            self.update_result_state();
            return;
        };
        let Some(context) = self.active_context() else {
            self.update_result_state();
            return;
        };
        let query = self.query();
        if query.is_empty() {
            self.update_result_state();
            return;
        }

        let buffer = tab.text_buffer();
        let replacements = count_matches(&context, &buffer);
        if replacements == 0 {
            self.update_result_state();
            return;
        }

        let replacement = self.replace_entry.text();
        buffer.begin_user_action();
        let result = context.replace_all(&replacement);
        buffer.end_user_action();
        if let Err(error) = result {
            dialogs::present_error(&self.parent_window, &error.into());
            return;
        }

        self.state.borrow_mut().manual_message = Some(format_replaced_count(replacements));
        self.update_result_state();
    }

    fn bind_active_context(self: &Rc<Self>, tab: Option<Rc<EditorTab>>) {
        self.drop_active_context();
        self.state.borrow_mut().active_tab.clone_from(&tab);
        let Some(tab) = tab else {
            self.update_result_state();
            return;
        };

        let query = self.query();
        self.settings
            .set_search_text(if query.is_empty() { None } else { Some(&query) });
        self.settings
            .set_case_sensitive(self.match_case_button.is_active());
        if query.is_empty() {
            self.update_result_state();
            return;
        }
        if !tab.supports_search() {
            self.state.borrow_mut().manual_message =
                Some(gettext("Search is disabled for very large files."));
            self.update_result_state();
            return;
        }

        let context = sourceview5::SearchContext::new(&tab.text_buffer(), Some(&self.settings));
        context.set_highlight(true);
        let weak = Rc::downgrade(self);
        context.connect_occurrences_count_notify(move |_| {
            if let Some(search) = weak.upgrade() {
                search.update_result_state();
            }
        });

        self.state.borrow_mut().active_binding = Some(SearchBinding {
            context: context.clone(),
        });
        if let Some((match_start, match_end, _wrapped)) =
            context.forward(&tab.text_buffer().start_iter())
        {
            select_match(&tab, &match_start, &match_end);
        }
        self.update_result_state();
    }

    fn active_context(&self) -> Option<sourceview5::SearchContext> {
        self.state
            .borrow()
            .active_binding
            .as_ref()
            .map(|binding| binding.context.clone())
    }

    fn drop_active_context(&self) {
        let binding = self.state.borrow_mut().active_binding.take();
        if let Some(binding) = binding {
            binding.context.set_highlight(false);
        }
    }

    fn current_match_bounds(
        &self,
        tab: &EditorTab,
        context: &sourceview5::SearchContext,
    ) -> Option<(gtk4::TextIter, gtk4::TextIter)> {
        let buffer = tab.text_buffer();
        let query = self.query();
        if let Some((start, end)) = buffer.selection_bounds()
            && selection_matches_query(
                &buffer.text(&start, &end, true),
                &query,
                self.match_case_button.is_active(),
            )
        {
            return Some((start, end));
        }

        let start = buffer.iter_at_mark(&buffer.get_insert());
        context
            .forward(&start)
            .or_else(|| context.forward(&buffer.start_iter()))
            .map(|(match_start, match_end, _wrapped)| (match_start, match_end))
    }

    fn query(&self) -> glib::GString {
        self.search_entry.text()
    }

    fn clear_manual_message(&self) {
        self.state.borrow_mut().manual_message = None;
    }

    fn update_result_state(&self) {
        let (manual_message, occurrences) = {
            let state = self.state.borrow();
            let occurrences = state
                .active_binding
                .as_ref()
                .map_or(-1, |binding| binding.context.occurrences_count());
            (state.manual_message.clone(), occurrences)
        };

        if let Some(message) = manual_message {
            self.result_label.set_label(&message);
            self.set_action_sensitivity(occurrences > 0);
            return;
        }

        let query = self.query();
        let has_query = !query.is_empty();
        let label = if !has_query || occurrences < 0 {
            String::new()
        } else if occurrences == 0 {
            gettext("No matches")
        } else {
            format_match_count(occurrences.cast_unsigned())
        };
        self.result_label.set_label(&label);
        self.set_action_sensitivity(has_query && occurrences > 0);
    }

    fn set_action_sensitivity(&self, has_matches: bool) {
        let replace_visible = self.replace_row.is_visible();
        self.previous_button.set_sensitive(has_matches);
        self.next_button.set_sensitive(has_matches);
        self.replace_button
            .set_sensitive(has_matches && replace_visible);
        self.replace_all_button
            .set_sensitive(has_matches && replace_visible);
    }

    #[cfg(test)]
    pub(crate) fn is_search_mode_for_tests(&self) -> bool {
        self.search_bar.is_search_mode()
    }

    #[cfg(test)]
    pub(crate) fn is_replace_visible_for_tests(&self) -> bool {
        self.replace_row.is_visible()
    }

    #[cfg(test)]
    pub(crate) fn query_text_for_tests(&self) -> String {
        self.search_entry.text().to_string()
    }

    #[cfg(test)]
    pub(crate) fn result_text_for_tests(&self) -> String {
        self.result_label.text().to_string()
    }

    #[cfg(test)]
    pub(crate) fn set_replace_text_for_tests(&self, text: &str) {
        self.replace_entry.set_text(text);
    }
}

fn icon_button(icon_name: &str, label: &str) -> gtk4::Button {
    let button = gtk4::Button::builder().icon_name(icon_name).build();
    button.set_tooltip_text(Some(label));
    button.update_property(&[Property::Label(label)]);
    button
}
