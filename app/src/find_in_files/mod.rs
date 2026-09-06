use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gio, glib, pango, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;
use crate::error::AppError;
use crate::workspace::{OpenSource, Workspace};

mod scanner;

use scanner::{FindMatch, ScanRequest, ScanSink, ScanSummary, start_scan};

const SEARCH_DEBOUNCE_MS: u64 = 300;

pub(crate) struct FindInFilesController {
    parent: adw::ApplicationWindow,
    workspace: Weak<Workspace>,
    root: adw::ToolbarView,
    refresh_button: gtk4::Button,
    spinner: gtk4::Spinner,
    status_label: gtk4::Label,
    store: gio::ListStore,
    root_folder: RefCell<Option<gio::File>>,
    current_query: RefCell<String>,
    current_match_case: Cell<bool>,
    show_hidden: Cell<bool>,
    // The cancellable stops Gio work; the generation drops late results between async stages.
    generation: Cell<u64>,
    active_cancellable: RefCell<Option<gio::Cancellable>>,
}

impl FindInFilesController {
    #[must_use]
    pub(crate) fn new(
        parent: &adw::ApplicationWindow,
        workspace: &Rc<Workspace>,
        show_hidden: bool,
    ) -> Rc<Self> {
        let title = adw::WindowTitle::new(&pgettext("sidebar mode", "Search Results"), "");
        let refresh_label = gettext("Refresh Search Results");
        let refresh_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(&refresh_label)
            .build();
        refresh_button.update_property(&[Property::Label(&refresh_label)]);

        let spinner = gtk4::Spinner::new();
        spinner.set_visible(false);

        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        header.set_title_widget(Some(&title));
        header.pack_start(&spinner);
        header.pack_end(&refresh_button);

        let status_label = gtk4::Label::new(Some(&gettext("Open a folder to search files.")));
        status_label.add_css_class("dim-label");
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);

        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk4::SingleSelection::new(Some(store.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let list_view = gtk4::ListView::new(Some(selection), Some(create_factory()));
        list_view.set_single_click_activate(true);
        list_view.set_vexpand(true);

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .child(&list_view)
            .build();

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 9);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&status_label);
        content.append(&scroller);

        let root = adw::ToolbarView::new();
        root.add_top_bar(&header);
        root.set_content(Some(&content));

        let controller = Rc::new(Self {
            parent: parent.clone(),
            workspace: Rc::downgrade(workspace),
            root,
            refresh_button,
            spinner,
            status_label,
            store,
            root_folder: RefCell::new(None),
            current_query: RefCell::new(String::new()),
            current_match_case: Cell::new(false),
            show_hidden: Cell::new(show_hidden),
            generation: Cell::new(0),
            active_cancellable: RefCell::new(None),
        });
        controller.install_callbacks(&list_view);
        controller
    }

    #[must_use]
    pub(crate) fn widget(&self) -> adw::ToolbarView {
        self.root.clone()
    }

    pub(crate) fn set_project_root(self: &Rc<Self>, root: Option<gio::File>) {
        self.root_folder.replace(root);
        self.cancel_active_scan();
        self.bump_generation();
        self.restart_if_ready();
    }

    pub(crate) fn set_show_hidden(self: &Rc<Self>, show_hidden: bool) {
        if self.show_hidden.replace(show_hidden) == show_hidden {
            return;
        }
        if self.has_active_query() {
            self.start_search_now();
        }
    }

    pub(crate) fn set_query(self: &Rc<Self>, query: &str, match_case: bool) {
        self.current_query.replace(query.to_string());
        self.current_match_case.set(match_case);
        if query.is_empty() {
            self.clear();
            return;
        }
        self.schedule_search();
    }

    pub(crate) fn clear(&self) {
        self.cancel_active_scan();
        self.bump_generation();
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
        self.current_query.borrow_mut().clear();
        self.store.remove_all();
        self.status_label
            .set_label(&gettext("Type to search the open folder."));
    }

    #[must_use]
    pub(crate) fn has_active_query(&self) -> bool {
        !self.current_query.borrow().is_empty()
    }

    pub(crate) fn show_root_missing(&self) {
        self.cancel_active_scan();
        self.bump_generation();
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
        self.store.remove_all();
        self.status_label
            .set_label(&gettext("Open a folder to search files."));
    }

    pub(crate) fn cancel_for_sidebar_close(&self) {
        self.cancel_active_scan();
        self.bump_generation();
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
    }

    pub(crate) fn root_change_handler(self: &Rc<Self>) -> Rc<dyn Fn(Option<gio::File>)> {
        let weak = Rc::downgrade(self);
        Rc::new(move |root| {
            if let Some(controller) = weak.upgrade() {
                controller.set_project_root(root);
            }
        })
    }

    pub(crate) fn show_hidden_handler(self: &Rc<Self>) -> Rc<dyn Fn(bool)> {
        let weak = Rc::downgrade(self);
        Rc::new(move |show_hidden| {
            if let Some(controller) = weak.upgrade() {
                controller.set_show_hidden(show_hidden);
            }
        })
    }

    pub(crate) fn sidebar_visibility_handler(self: &Rc<Self>) -> Rc<dyn Fn(bool)> {
        let weak = Rc::downgrade(self);
        Rc::new(move |visible| {
            if !visible && let Some(controller) = weak.upgrade() {
                controller.cancel_for_sidebar_close();
            }
        })
    }

    fn install_callbacks(self: &Rc<Self>, list_view: &gtk4::ListView) {
        let weak = Rc::downgrade(self);
        self.refresh_button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.start_search_now();
            }
        });

        let weak = Rc::downgrade(self);
        list_view.connect_activate(move |_, position| {
            if let Some(controller) = weak.upgrade() {
                controller.activate_position(position);
            }
        });
    }

    fn schedule_search(self: &Rc<Self>) {
        self.cancel_active_scan();
        let generation = self.bump_generation();
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(SEARCH_DEBOUNCE_MS), move || {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            if controller.generation.get() == generation {
                controller.start_search_for_generation(generation);
            }
        });
    }

    fn start_search_now(self: &Rc<Self>) {
        self.cancel_active_scan();
        let generation = self.bump_generation();
        self.start_search_for_generation(generation);
    }

    fn restart_if_ready(self: &Rc<Self>) {
        if self.root_folder.borrow().is_none() {
            self.store.remove_all();
            self.status_label
                .set_label(&gettext("Open a folder to search files."));
            return;
        }
        if self.has_active_query() {
            self.start_search_now();
            return;
        }
        self.store.remove_all();
        self.status_label
            .set_label(&gettext("Type to search the open folder."));
    }

    fn start_search_for_generation(self: &Rc<Self>, generation: u64) {
        self.store.remove_all();
        let query = self.current_query.borrow().clone();
        if query.is_empty() {
            self.status_label
                .set_label(&gettext("Type to search the open folder."));
            return;
        }
        let Some(root) = self.root_folder.borrow().clone() else {
            self.status_label
                .set_label(&gettext("Open a folder to search files."));
            return;
        };

        let cancellable = gio::Cancellable::new();
        self.active_cancellable.replace(Some(cancellable.clone()));
        self.spinner.set_visible(true);
        self.spinner.set_spinning(true);
        self.status_label.set_label(&searching_label());

        let weak = Rc::downgrade(self);
        let result = Rc::new(move |generation, found| {
            if let Some(controller) = weak.upgrade() {
                controller.add_result(generation, found);
            }
        });
        let weak = Rc::downgrade(self);
        let finish = Rc::new(move |generation, summary| {
            if let Some(controller) = weak.upgrade() {
                controller.finish_scan(generation, &summary);
            }
        });
        start_scan(
            ScanRequest {
                generation,
                root,
                query,
                match_case: self.current_match_case.get(),
                show_hidden: self.show_hidden.get(),
                cancellable,
            },
            ScanSink { result, finish },
        );
    }

    fn add_result(&self, generation: u64, found: FindMatch) {
        if self.generation.get() != generation {
            return;
        }
        self.store.append(&glib::BoxedAnyObject::new(found));
    }

    fn finish_scan(&self, generation: u64, summary: &ScanSummary) {
        if self.generation.get() != generation {
            return;
        }
        self.active_cancellable.take();
        self.spinner.set_spinning(false);
        self.spinner.set_visible(false);
        self.status_label.set_label(&format_summary(summary));
    }

    fn activate_position(&self, position: u32) {
        let Some(found) = result_at(&self.store, position) else {
            return;
        };
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let parent = self.parent.clone();
        let file = found.file.clone();
        workspace.request_open_file_then(
            &file,
            OpenSource::ProjectTree,
            Rc::new(move |result| match result {
                Ok(tab) => select_result(&tab, &found),
                Err(error) if should_present_open_error(&error) => {
                    crate::dialogs::present_error(&parent, &error);
                }
                Err(_) => {}
            }),
        );
    }

    fn cancel_active_scan(&self) {
        if let Some(cancellable) = self.active_cancellable.borrow_mut().take() {
            cancellable.cancel();
        }
    }

    fn bump_generation(&self) -> u64 {
        let next = self.generation.get().saturating_add(1);
        self.generation.set(next);
        next
    }
}

fn should_present_open_error(error: &AppError) -> bool {
    !matches!(error, AppError::DocumentChangedDuringRead)
}

fn create_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(setup_row);
    factory.connect_bind(bind_row);
    factory
}

fn setup_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let row_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    row_box.add_css_class("riteed-sidebar-row");
    row_box.set_margin_top(5);
    row_box.set_margin_bottom(5);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);

    let title = gtk4::Label::new(None);
    title.set_xalign(0.0);
    title.set_ellipsize(pango::EllipsizeMode::End);
    title.add_css_class("heading");

    let snippet = gtk4::Label::new(None);
    snippet.set_xalign(0.0);
    snippet.set_ellipsize(pango::EllipsizeMode::End);
    snippet.add_css_class("caption");
    snippet.add_css_class("dim-label");

    row_box.append(&title);
    row_box.append(&snippet);
    list_item.set_child(Some(&row_box));
}

fn bind_row(_: &gtk4::SignalListItemFactory, object: &glib::Object) {
    let Ok(list_item) = object.clone().downcast::<gtk4::ListItem>() else {
        return;
    };
    let Some(found) = list_item.item().and_then(|item| result_from_item(&item)) else {
        return;
    };
    let Some((title, snippet)) = row_widgets(&list_item) else {
        return;
    };
    title.set_label(&format!("{}:{}", found.path, found.line_number));
    snippet.set_label(found.line_text.trim());
}

fn row_widgets(list_item: &gtk4::ListItem) -> Option<(gtk4::Label, gtk4::Label)> {
    let row_box = list_item.child()?.downcast::<gtk4::Box>().ok()?;
    let title = row_box.first_child()?.downcast::<gtk4::Label>().ok()?;
    let snippet = title.next_sibling()?.downcast::<gtk4::Label>().ok()?;
    Some((title, snippet))
}

fn result_at(store: &gio::ListStore, position: u32) -> Option<FindMatch> {
    let item = store.item(position)?;
    result_from_item(&item)
}

fn result_from_item(item: &glib::Object) -> Option<FindMatch> {
    let boxed = item.clone().downcast::<glib::BoxedAnyObject>().ok()?;
    let borrowed = boxed.try_borrow::<FindMatch>().ok()?;
    Some((*borrowed).clone())
}

fn select_result(tab: &EditorTab, found: &FindMatch) {
    tab.select_offsets(found.start_offset, found.end_offset);
    tab.grab_focus();
}

fn searching_label() -> String {
    let mut label = gettext("Searching");
    label.push('…');
    label
}

fn format_summary(summary: &ScanSummary) -> String {
    let mut label = if summary.matches == 0 {
        gettext("No matches")
    } else {
        let results = ngettext("%d result", "%d results", summary.matches)
            .replace("%d", &summary.matches.to_string());
        let files = ngettext("in %d file", "in %d files", summary.files_searched)
            .replace("%d", &summary.files_searched.to_string());
        format!("{results} {files}")
    };
    if summary.limited {
        label = format!("{label} {}", gettext("Results limited; refine the search."));
    }
    if summary.skipped > 0 {
        let skipped = ngettext("%d file skipped.", "%d files skipped.", summary.skipped)
            .replace("%d", &summary.skipped.to_string());
        label = format!("{label} {skipped}");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::{ScanSummary, format_summary, should_present_open_error};
    use crate::error::AppError;

    #[test]
    fn document_read_notice_suppresses_a_second_open_error() {
        assert!(!should_present_open_error(
            &AppError::DocumentChangedDuringRead
        ));
        assert!(should_present_open_error(&AppError::Cancelled));
    }

    #[test]
    fn summary_is_plural_sensitive() {
        assert_eq!(
            format_summary(&ScanSummary {
                files_searched: 1,
                matches: 1,
                ..ScanSummary::default()
            }),
            "1 result in 1 file"
        );
        assert_eq!(
            format_summary(&ScanSummary {
                files_searched: 2,
                matches: 3,
                ..ScanSummary::default()
            }),
            "3 results in 2 files"
        );
    }

    #[test]
    fn summary_reports_limits_and_skips() {
        assert_eq!(
            format_summary(&ScanSummary {
                files_searched: 1,
                skipped: 1,
                limited: true,
                ..ScanSummary::default()
            }),
            "No matches Results limited; refine the search. 1 file skipped."
        );
    }
}
