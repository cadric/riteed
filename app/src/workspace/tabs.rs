use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::glib::variant::ToVariant;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;

use super::{OpenSource, Workspace};

pub(crate) type TransferWindowHandler = Rc<dyn Fn() -> Option<Rc<Workspace>>>;

pub(crate) struct TabControls {
    move_backward_action: gio::SimpleAction,
    move_forward_action: gio::SimpleAction,
    move_to_new_window_action: gio::SimpleAction,
    close_other_tabs_action: gio::SimpleAction,
    transfer_window_handler: RefCell<Option<TransferWindowHandler>>,
    transferring_page: RefCell<Option<adw::TabPage>>,
    markdown_preview_action: gio::SimpleAction,
}

impl TabControls {
    pub(crate) fn new() -> Self {
        Self {
            move_backward_action: gio::SimpleAction::new("tab-move-backward", None),
            move_forward_action: gio::SimpleAction::new("tab-move-forward", None),
            move_to_new_window_action: gio::SimpleAction::new("tab-move-to-new-window", None),
            close_other_tabs_action: gio::SimpleAction::new("close-other-tabs", None),
            transfer_window_handler: RefCell::new(None),
            transferring_page: RefCell::new(None),
            markdown_preview_action: gio::SimpleAction::new("tab-toggle-markdown-preview", None),
        }
    }

    fn mark_transferring(&self, page: &adw::TabPage) {
        self.transferring_page.replace(Some(page.clone()));
    }

    fn clear_transferring(&self, page: &adw::TabPage) {
        if self
            .transferring_page
            .borrow()
            .as_ref()
            .is_some_and(|candidate| candidate == page)
        {
            self.transferring_page.replace(None);
        }
    }

    fn is_transferring(&self, page: &adw::TabPage) -> bool {
        self.transferring_page
            .borrow()
            .as_ref()
            .is_some_and(|candidate| candidate == page)
    }
}

pub(crate) fn install(workspace: &Rc<Workspace>) {
    workspace
        .shell
        .add_action(&workspace.tab_controls.move_backward_action);
    workspace
        .shell
        .add_action(&workspace.tab_controls.move_forward_action);
    workspace
        .shell
        .add_action(&workspace.tab_controls.move_to_new_window_action);
    workspace
        .shell
        .add_action(&workspace.tab_controls.close_other_tabs_action);
    workspace
        .shell
        .add_action(&workspace.tab_controls.markdown_preview_action);
    workspace.tab_view.set_menu_model(Some(&build_tab_menu()));
    install_action_callbacks(workspace);
    install_state_callbacks(workspace);
    workspace.sync_tab_action_state();
}

fn install_action_callbacks(workspace: &Rc<Workspace>) {
    let weak = Rc::downgrade(workspace);
    workspace
        .tab_controls
        .move_backward_action
        .connect_activate(move |_, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.move_selected_tab_backward();
            }
        });

    let weak = Rc::downgrade(workspace);
    workspace
        .tab_controls
        .move_forward_action
        .connect_activate(move |_, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.move_selected_tab_forward();
            }
        });

    let weak = Rc::downgrade(workspace);
    workspace
        .tab_controls
        .move_to_new_window_action
        .connect_activate(move |_, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.move_selected_tab_to_new_window();
            }
        });

    let weak = Rc::downgrade(workspace);
    workspace
        .tab_controls
        .close_other_tabs_action
        .connect_activate(move |_, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.request_close_other_tabs();
            }
        });

    let weak = Rc::downgrade(workspace);
    workspace
        .tab_controls
        .markdown_preview_action
        .connect_activate(move |_, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.toggle_markdown_preview_for_selected_tab();
            }
        });
}

fn install_state_callbacks(workspace: &Rc<Workspace>) {
    let weak = Rc::downgrade(workspace);
    workspace
        .tab_view
        .connect_setup_menu(move |tab_view, page| {
            if let (Some(workspace), Some(page)) = (weak.upgrade(), page) {
                tab_view.set_selected_page(page);
                workspace.sync_tab_action_state();
                workspace.refresh_selected_state();
            } else if let Some(workspace) = weak.upgrade() {
                workspace.sync_tab_action_state();
                workspace.refresh_selected_state();
            }
        });

    let weak = Rc::downgrade(workspace);
    workspace.tab_view.connect_page_attached(move |_, page, _| {
        if let Some(workspace) = weak.upgrade() {
            workspace.tab_controls.clear_transferring(page);
            workspace.sync_tab_action_state();
        }
    });

    let weak = Rc::downgrade(workspace);
    workspace.tab_view.connect_page_detached(move |_, _, _| {
        if let Some(workspace) = weak.upgrade() {
            workspace.sync_tab_action_state();
        }
    });

    let weak = Rc::downgrade(workspace);
    workspace.tab_view.connect_page_reordered(move |_, _, _| {
        if let Some(workspace) = weak.upgrade() {
            workspace.sync_tab_action_state();
        }
    });

    let weak = Rc::downgrade(workspace);
    workspace.tab_view.connect_selected_page_notify(move |_| {
        if let Some(workspace) = weak.upgrade() {
            workspace.sync_tab_action_state();
        }
    });
}

fn build_tab_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &tab_move_section());
    menu.append_section(None, &tab_markdown_section());
    menu.append_section(None, &tab_compare_section());
    menu.append_section(None, &tab_close_section());
    menu
}

fn tab_move_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(
        Some(&pgettext("tab menu item", "Move Tab _Backward")),
        Some("win.tab-move-backward"),
    );
    section.append(
        Some(&pgettext("tab menu item", "Move Tab _Forward")),
        Some("win.tab-move-forward"),
    );
    section.append(
        Some(&pgettext("tab menu item", "Move to _New Window")),
        Some("win.tab-move-to-new-window"),
    );
    section
}

fn tab_markdown_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append_item(&hidden_when_disabled_item(
        &pgettext("tab menu item", "Toggle Markdown Preview"),
        "win.tab-toggle-markdown-preview",
    ));
    section
}

fn tab_compare_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append_item(&hidden_when_disabled_item(
        &ellipsis_label(pgettext("tab menu item", "Compare With F_ile")),
        "win.tab-compare-with-file",
    ));
    section.append_item(&hidden_when_disabled_item(
        &pgettext("tab menu item", "Compare With _Saved Version"),
        "win.tab-compare-with-saved-version",
    ));
    section.append_item(&hidden_when_disabled_item(
        &ellipsis_label(pgettext("tab menu item", "Compare With _Pasted Text")),
        "win.tab-compare-with-pasted-text",
    ));
    section
}

fn tab_close_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(
        Some(&pgettext("tab menu item", "Close _Other Tabs")),
        Some("win.close-other-tabs"),
    );
    section.append(
        Some(&pgettext("tab menu item", "_Close")),
        Some("win.close"),
    );
    section
}

fn hidden_when_disabled_item(label: &str, action: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    item.set_attribute_value("hidden-when", Some(&"action-disabled".to_variant()));
    item
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

impl Workspace {
    pub(crate) fn set_transfer_window_handler(&self, handler: TransferWindowHandler) {
        self.tab_controls
            .transfer_window_handler
            .replace(Some(handler));
        self.sync_tab_action_state();
    }

    pub(crate) fn sync_tab_action_state(&self) {
        let selected = self.tab_view.selected_page();
        let has_flow = self.state.borrow().close_flow.is_some();
        let n_pages = self.tab_view.n_pages();
        let position = selected
            .as_ref()
            .map_or(-1, |page| self.tab_view.page_position(page));
        let can_create_window = self.tab_controls.transfer_window_handler.borrow().is_some();
        let selected_tab = selected
            .as_ref()
            .and_then(|page| self.find_tab_by_page(page));

        self.tab_controls
            .move_backward_action
            .set_enabled(!has_flow && position > 0);
        self.tab_controls
            .move_forward_action
            .set_enabled(!has_flow && position >= 0 && position + 1 < n_pages);
        self.tab_controls
            .move_to_new_window_action
            .set_enabled(!has_flow && can_create_window && n_pages > 1 && selected.is_some());
        self.tab_controls
            .close_other_tabs_action
            .set_enabled(!has_flow && n_pages > 1 && selected.is_some());
        self.tab_controls.markdown_preview_action.set_enabled(
            !has_flow
                && selected_tab
                    .as_ref()
                    .is_some_and(|tab| tab.can_toggle_markdown_preview()),
        );
    }

    fn toggle_markdown_preview_for_selected_tab(&self) {
        if let Some(tab) = self.selected_tab()
            && tab.can_toggle_markdown_preview()
        {
            tab.toggle_markdown_preview();
            self.sync_tab_action_state();
            self.refresh_selected_state();
        }
    }

    pub(crate) fn add_empty_tab(self: &Rc<Self>, select: bool) -> Rc<EditorTab> {
        let tab = EditorTab::new(&self.settings);
        self.add_tab(tab, select)
    }

    pub(crate) fn add_tab(self: &Rc<Self>, tab: Rc<EditorTab>, select: bool) -> Rc<EditorTab> {
        self.bind_tab_to_workspace(&tab);
        let page = tab.attach(&self.tab_view);
        self.state.borrow_mut().tabs.push(tab.clone());
        if select {
            self.tab_view.set_selected_page(&page);
        }
        Self::apply_tab_workspace_preferences(&tab);
        tab
    }

    pub(crate) fn close_tab_if_clean(&self, tab: &EditorTab) {
        if tab.is_clean_untitled()
            && let Some(page) = tab.page()
        {
            self.tab_view.close_page(&page);
        }
    }

    pub(crate) fn close_review_tabs_for_other_repo(&self, repo: Option<&Path>) {
        let tabs = self.ordered_tabs();
        for tab in tabs {
            if tab.kind() != crate::editor_tab::TabKind::GitReview {
                continue;
            }
            let same_repo = tab
                .review_repo_root()
                .as_deref()
                .is_some_and(|tab_repo| repo == Some(tab_repo));
            if !same_repo && let Some(page) = tab.page() {
                self.tab_view.close_page(&page);
            }
        }
    }

    pub(crate) fn selected_tab(&self) -> Option<Rc<EditorTab>> {
        self.tab_view
            .selected_page()
            .and_then(|page| self.find_tab_by_page(&page))
    }

    pub(crate) fn ordered_tabs(&self) -> Vec<Rc<EditorTab>> {
        (0..self.tab_view.n_pages())
            .filter_map(|position| self.find_tab_by_page(&self.tab_view.nth_page(position)))
            .collect()
    }

    pub(crate) fn find_tab_by_page(&self, page: &adw::TabPage) -> Option<Rc<EditorTab>> {
        self.state
            .borrow()
            .tabs
            .iter()
            .find(|tab| tab.page().as_ref().is_some_and(|item| item == page))
            .cloned()
    }

    pub(crate) fn request_close_selected_tab(&self) {
        if let Some(page) = self.tab_view.selected_page() {
            self.tab_view.close_page(&page);
        } else {
            self.shell.close();
        }
    }

    pub(crate) fn is_transferring_page(&self, page: &adw::TabPage) -> bool {
        self.tab_view.is_transferring_page() || self.tab_controls.is_transferring(page)
    }

    pub(crate) fn clear_transfer_guard(&self, page: &adw::TabPage) {
        self.tab_controls.clear_transferring(page);
    }

    pub(crate) fn request_close_other_tabs(self: &Rc<Self>) {
        if let Some(page) = self.tab_view.selected_page() {
            crate::workspace_close::request_close_other_tabs(self, &page);
        }
    }

    fn move_selected_tab_backward(&self) {
        if self.state.borrow().close_flow.is_some() {
            return;
        }
        if let Some(page) = self.tab_view.selected_page() {
            let _reordered = self.tab_view.reorder_backward(&page);
        }
    }

    fn move_selected_tab_forward(&self) {
        if self.state.borrow().close_flow.is_some() {
            return;
        }
        if let Some(page) = self.tab_view.selected_page() {
            let _reordered = self.tab_view.reorder_forward(&page);
        }
    }

    fn move_selected_tab_to_new_window(self: &Rc<Self>) {
        if self.state.borrow().close_flow.is_some() || self.tab_view.n_pages() <= 1 {
            return;
        }
        let Some(page) = self.tab_view.selected_page() else {
            return;
        };
        let Some(tab) = self.find_tab_by_page(&page) else {
            return;
        };
        let handler = self.tab_controls.transfer_window_handler.borrow().clone();
        let Some(destination) = handler.and_then(|create| create()) else {
            return;
        };

        destination.adopt_tab_for_transfer(&tab, &page);
        self.tab_controls.mark_transferring(&page);
        self.tab_view
            .transfer_page(&page, &destination.tab_view, destination.tab_view.n_pages());
        destination.tab_view.set_selected_page(&page);
        destination.shell.present();
    }

    fn adopt_tab_for_transfer(self: &Rc<Self>, tab: &Rc<EditorTab>, page: &adw::TabPage) {
        if !self
            .state
            .borrow()
            .tabs
            .iter()
            .any(|candidate| Rc::ptr_eq(candidate, tab))
        {
            self.state.borrow_mut().tabs.push(tab.clone());
        }
        self.bind_tab_to_workspace(tab);
        Self::apply_tab_workspace_preferences(tab);
        self.tab_controls.mark_transferring(page);
    }

    fn bind_tab_to_workspace(self: &Rc<Self>, tab: &Rc<EditorTab>) {
        let weak = Rc::downgrade(self);
        tab.set_visual_change_handler(Rc::new(move || {
            if let Some(workspace) = weak.upgrade() {
                workspace.queue_refresh_selected_state();
            }
        }));
        let weak = Rc::downgrade(self);
        tab.set_markdown_preview_change_handler(Rc::new(move || {
            if let Some(workspace) = weak.upgrade() {
                workspace.search.bind_tab(workspace.selected_tab());
            }
        }));
        let weak = Rc::downgrade(self);
        tab.set_file_drop_handler(Rc::new(move |files| {
            if let Some(workspace) = weak.upgrade() {
                workspace.request_open_files(files, OpenSource::Drop);
            }
        }));
        crate::workspace_monitor::install_tab_hooks(self, tab);
        super::autosave::install_tab_autosave(self, tab);
    }

    fn apply_tab_workspace_preferences(tab: &EditorTab) {
        tab.apply_word_wrap();
        tab.apply_line_numbers();
        tab.apply_minimap_visibility();
        tab.apply_current_line_highlight();
        tab.apply_indentation();
        tab.apply_source_style_scheme();
    }

    #[cfg(test)]
    pub(crate) fn find_tab_by_uri(&self, uri: &str) -> Option<Rc<EditorTab>> {
        self.state
            .borrow()
            .tabs
            .iter()
            .find(|tab| {
                tab.document_uri()
                    .as_deref()
                    .is_some_and(|item| item == uri)
            })
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use gtk4::gio;
    use gtk4::prelude::Cast;
    use gtk4::prelude::MenuModelExt;

    use super::build_tab_menu;

    #[test]
    fn tab_menu_contains_standard_tab_actions() {
        let menu = build_tab_menu();
        assert_eq!(menu.n_items(), 4);

        assert_menu_section(
            &section(&menu, 0),
            &[
                "Move Tab _Backward",
                "Move Tab _Forward",
                "Move to _New Window",
            ],
            &[
                "win.tab-move-backward",
                "win.tab-move-forward",
                "win.tab-move-to-new-window",
            ],
        );
        let markdown = section(&menu, 1);
        assert_menu_section(
            &markdown,
            &["Toggle Markdown Preview"],
            &["win.tab-toggle-markdown-preview"],
        );
        assert_eq!(
            item_string(&markdown, 0, "hidden-when").as_deref(),
            Some("action-disabled")
        );

        let compare = section(&menu, 2);
        assert_menu_section(
            &compare,
            &[
                "Compare With F_ile…",
                "Compare With _Saved Version",
                "Compare With _Pasted Text…",
            ],
            &[
                "win.tab-compare-with-file",
                "win.tab-compare-with-saved-version",
                "win.tab-compare-with-pasted-text",
            ],
        );
        for index in 0..compare.n_items() {
            assert_eq!(
                item_string(&compare, index, "hidden-when").as_deref(),
                Some("action-disabled")
            );
        }
        assert_menu_section(
            &section(&menu, 3),
            &["Close _Other Tabs", "_Close"],
            &["win.close-other-tabs", "win.close"],
        );
    }

    fn section(menu: &gio::Menu, index: i32) -> gio::MenuModel {
        let link = menu.item_link(index, "section");
        assert!(link.is_some(), "tab menu section must exist");
        link.unwrap_or_else(|| gio::Menu::new().upcast::<gio::MenuModel>())
    }

    fn assert_menu_section(menu: &gio::MenuModel, labels: &[&str], actions: &[&str]) {
        assert_eq!(usize::try_from(menu.n_items()), Ok(labels.len()));
        for (index, (label, action)) in (0_i32..).zip(labels.iter().zip(actions.iter())) {
            assert_eq!(item_string(menu, index, "label").as_deref(), Some(*label));
            assert_eq!(item_string(menu, index, "action").as_deref(), Some(*action));
        }
    }

    fn item_string(menu: &gio::MenuModel, index: i32, attribute: &str) -> Option<String> {
        menu.item_attribute_value(index, attribute, None)
            .and_then(|value| value.get::<String>())
    }
}
