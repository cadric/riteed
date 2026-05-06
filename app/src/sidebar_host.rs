use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;

pub(crate) const SOURCE_CONTROL_ICON: &str = "io.github.cadric.Riteed-source-control-symbolic";

pub(crate) struct SidebarHost {
    root: adw::ToolbarView,
    stack: adw::ViewStack,
    #[cfg(test)]
    source_control_page: adw::ViewStackPage,
}

impl SidebarHost {
    #[must_use]
    pub(crate) fn new(
        files: &impl IsA<gtk4::Widget>,
        source_control: &impl IsA<gtk4::Widget>,
        find_in_files: &impl IsA<gtk4::Widget>,
    ) -> Self {
        let stack = adw::ViewStack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);

        let files_page = stack.add_titled(files, Some("files"), &pgettext("sidebar mode", "Files"));
        files_page.set_icon_name(Some("folder-symbolic"));

        let git_page = stack.add_titled(
            source_control,
            Some("source-control"),
            &pgettext("sidebar mode", "Source Control"),
        );
        git_page.set_icon_name(Some(SOURCE_CONTROL_ICON));

        let find_page = stack.add_titled(
            find_in_files,
            Some("find-in-files"),
            &pgettext("sidebar mode", "Find in Files"),
        );
        find_page.set_icon_name(Some("edit-find-symbolic"));

        let switcher = adw::ViewSwitcher::new();
        switcher.set_policy(adw::ViewSwitcherPolicy::Narrow);
        switcher.set_stack(Some(&stack));

        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        header.set_title_widget(Some(&switcher));

        let root = adw::ToolbarView::new();
        root.add_top_bar(&header);
        root.set_content(Some(&stack));

        Self {
            root,
            stack,
            #[cfg(test)]
            source_control_page: git_page,
        }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }

    pub(crate) fn select_find_in_files(&self) {
        self.stack.set_visible_child_name("find-in-files");
    }

    #[cfg(test)]
    pub(crate) fn source_control_icon_for_tests(&self) -> Option<String> {
        self.source_control_page
            .icon_name()
            .map(|name| name.to_string())
    }
}
