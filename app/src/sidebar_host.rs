use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;

pub(crate) struct SidebarHost {
    root: adw::ToolbarView,
}

impl SidebarHost {
    #[must_use]
    pub(crate) fn new(
        files: &impl IsA<gtk4::Widget>,
        source_control: &impl IsA<gtk4::Widget>,
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
        git_page.set_icon_name(Some("folder-vcs-symbolic"));

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

        Self { root }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }
}
