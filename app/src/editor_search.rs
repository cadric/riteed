use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::accessible::Property;
use gtk4::gdk;
use gtk4::{glib, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::dialogs;
use crate::editor_tab::EditorTab;

struct SearchBinding {
    context: sourceview5::SearchContext,
}

struct SearchState {
    active_tab: Option<Rc<EditorTab>>,
    active_binding: Option<SearchBinding>,
    manual_message: Option<String>,
}

pub struct EditorSearch {
    parent_window: adw::ApplicationWindow,
    search_bar: gtk4::SearchBar,
    search_entry: gtk4::SearchEntry,
    replace_entry: gtk4::Entry,
    match_case_button: gtk4::ToggleButton,
    previous_button: gtk4::Button,
    next_button: gtk4::Button,
    replace_button: gtk4::Button,
    replace_all_button: gtk4::Button,
    result_label: gtk4::Label,
    replace_row: gtk4::Box,
    settings: sourceview5::SearchSettings,
    state: RefCell<SearchState>,
}

impl EditorSearch {
    #[must_use]
    pub fn new(parent_window: &adw::ApplicationWindow) -> Rc<Self> {
        let search_entry = gtk4::SearchEntry::builder().hexpand(true).build();
        let replace_entry = gtk4::Entry::builder().hexpand(true).build();
        replace_entry
            .set_placeholder_text(Some(&pgettext("search field placeholder", "Replace With")));

        let match_case_button =
            gtk4::ToggleButton::with_label(&pgettext("search option", "Match Case"));
        let previous_button = icon_button(
            "go-up-symbolic",
            &pgettext("search action", "Previous Match"),
        );
        let next_button = icon_button("go-down-symbolic", &pgettext("search action", "Next Match"));
        let replace_button = gtk4::Button::with_label(&pgettext("search action", "Replace"));
        let replace_all_button =
            gtk4::Button::with_label(&pgettext("search action", "Replace All"));
        let result_label = gtk4::Label::builder().xalign(0.0).hexpand(true).build();
        result_label.add_css_class("dim-label");

        let close_button = icon_button(
            "window-close-symbolic",
            &pgettext("search action", "Close Search"),
        );

        let first_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        first_row.append(&search_entry);
        first_row.append(&close_button);

        let second_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        second_row.append(&previous_button);
        second_row.append(&next_button);
        second_row.append(&match_case_button);
        second_row.append(&result_label);

        let replace_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .visible(false)
            .build();
        replace_row.append(&replace_entry);
        replace_row.append(&replace_button);
        replace_row.append(&replace_all_button);

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
        content.append(&replace_row);

        let search_bar = gtk4::SearchBar::new();
        search_bar.connect_entry(&search_entry);
        search_bar.set_child(Some(&content));

        let settings = sourceview5::SearchSettings::new();
        settings.set_wrap_around(true);

        let search = Rc::new(Self {
            parent_window: parent_window.clone(),
            search_bar,
            search_entry,
            replace_entry,
            match_case_button,
            previous_button,
            next_button,
            replace_button,
            replace_all_button,
            result_label,
            replace_row,
            settings,
            state: RefCell::new(SearchState {
                active_tab: None,
                active_binding: None,
                manual_message: None,
            }),
        });
        search.install_callbacks(&close_button);
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
        self.replace_row.set_visible(replace_mode);
        if let Some(prefill) = prefill {
            self.search_entry.set_text(&prefill);
        }
        self.search_bar.set_search_mode(true);
        self.bind_tab(tab);
        self.search_entry.grab_focus();
        self.search_entry.select_region(0, -1);
    }

    pub fn close(self: &Rc<Self>) {
        self.search_bar.set_search_mode(false);
    }

    pub fn bind_tab(self: &Rc<Self>, tab: Option<Rc<EditorTab>>) {
        let search_active = self.search_bar.is_search_mode();
        self.drop_active_context();
        self.state.borrow_mut().active_tab.clone_from(&tab);
        if search_active {
            self.bind_active_context(tab);
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
        let replacement = self.replace_entry.text().to_string();
        if let Err(error) = context.replace(&mut match_start, &mut match_end, &replacement) {
            dialogs::present_error(&self.parent_window, &error.into());
            return;
        }
        select_match(&tab, &match_start, &match_end);
        self.update_result_state();
    }

    pub fn replace_all(self: &Rc<Self>) {
        let Some(tab) = self.state.borrow().active_tab.clone() else {
            self.update_result_state();
            return;
        };
        let Some(context) = self.active_context() else {
            self.update_result_state();
            return;
        };
        if self.query().is_empty() {
            self.update_result_state();
            return;
        }

        let buffer = tab.text_buffer();
        let replacements = count_matches(&context, &buffer);
        if replacements == 0 {
            self.update_result_state();
            return;
        }

        let replacement = self.replace_entry.text().to_string();
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

    fn install_callbacks(self: &Rc<Self>, close_button: &gtk4::Button) {
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

        close_button.connect_clicked({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(search) = weak.upgrade() {
                    search.close();
                }
            }
        });

        let weak = Rc::downgrade(self);
        self.search_bar
            .connect_search_mode_enabled_notify(move |bar| {
                if let Some(search) = weak.upgrade() {
                    if bar.is_search_mode() {
                        let active_tab = search.state.borrow().active_tab.clone();
                        search.bind_active_context(active_tab);
                    } else {
                        search.drop_active_context();
                        search.clear_manual_message();
                        search.result_label.set_label("");
                        search.update_result_state();
                    }
                }
            });
    }

    fn on_search_changed(self: &Rc<Self>) {
        self.clear_manual_message();
        let query = self.query();
        self.settings
            .set_search_text(if query.is_empty() { None } else { Some(&query) });
        self.settings
            .set_case_sensitive(self.match_case_button.is_active());
        let active_tab = self.state.borrow().active_tab.clone();
        if let Some(tab) = active_tab {
            self.bind_active_context(Some(tab));
        } else {
            self.update_result_state();
        }
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
        if let Some((start, end)) = buffer.selection_bounds()
            && selection_matches_query(
                &buffer.text(&start, &end, true),
                &self.query(),
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

    fn query(&self) -> String {
        self.search_entry.text().to_string()
    }

    fn clear_manual_message(&self) {
        self.state.borrow_mut().manual_message = None;
    }

    fn update_result_state(&self) {
        if let Some(message) = self.state.borrow().manual_message.clone() {
            self.result_label.set_label(&message);
            let has_matches = self
                .state
                .borrow()
                .active_binding
                .as_ref()
                .is_some_and(|binding| binding.context.occurrences_count() > 0);
            self.set_action_sensitivity(has_matches);
            return;
        }

        let occurrences = self
            .state
            .borrow()
            .active_binding
            .as_ref()
            .map_or(-1, |binding| binding.context.occurrences_count());

        let has_query = !self.query().is_empty();
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
        self.previous_button.set_sensitive(has_matches);
        self.next_button.set_sensitive(has_matches);
        self.replace_button
            .set_sensitive(has_matches && self.replace_row.is_visible());
        self.replace_all_button
            .set_sensitive(has_matches && self.replace_row.is_visible());
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

fn count_matches(context: &sourceview5::SearchContext, buffer: &sourceview5::Buffer) -> u32 {
    let mut count = 0;
    let mut iter = buffer.start_iter();
    while let Some((_start, end, wrapped)) = context.forward(&iter) {
        if wrapped {
            break;
        }
        count += 1;
        iter = end;
    }
    count
}

fn select_match(tab: &EditorTab, start: &gtk4::TextIter, end: &gtk4::TextIter) {
    let buffer = tab.text_buffer();
    buffer.select_range(start, end);
    let mut scroll_iter = *start;
    tab.text_view()
        .scroll_to_iter(&mut scroll_iter, 0.2, false, 0.0, 0.0);
}

fn selection_matches_query(selection: &str, query: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        selection == query
    } else {
        selection.to_lowercase() == query.to_lowercase()
    }
}

#[must_use]
pub fn format_match_count(count: u32) -> String {
    ngettext("%d match", "%d matches", count).replace("%d", &count.to_string())
}

#[must_use]
pub fn format_replaced_count(count: u32) -> String {
    ngettext("Replaced %d match", "Replaced %d matches", count).replace("%d", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::{format_match_count, format_replaced_count, selection_matches_query};

    #[test]
    fn match_count_is_plural_sensitive() {
        assert_eq!(format_match_count(1), "1 match");
        assert_eq!(format_match_count(2), "2 matches");
    }

    #[test]
    fn replaced_count_is_plural_sensitive() {
        assert_eq!(format_replaced_count(1), "Replaced 1 match");
        assert_eq!(format_replaced_count(2), "Replaced 2 matches");
    }

    #[test]
    fn case_insensitive_selection_match_works() {
        assert!(selection_matches_query("Hello", "hello", false));
        assert!(!selection_matches_query("Hello", "hello", true));
    }
}
