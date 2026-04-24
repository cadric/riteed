use std::rc::Rc;

use gettextrs::gettext;
use gtk4::accessible::Property;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::project_tree::{ProjectTree, ProjectTreeActivation};

pub(crate) struct ProjectBrowser {
    root: adw::ToolbarView,
    title: adw::WindowTitle,
    tree: ProjectTree,
}

impl ProjectBrowser {
    #[must_use]
    pub(crate) fn new(on_activate: Rc<dyn Fn(ProjectTreeActivation)>) -> Self {
        let tree = ProjectTree::new(on_activate);

        let title = adw::WindowTitle::new(&gettext("Project"), "");
        title.set_hexpand(true);

        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        header.set_title_widget(Some(&title));

        let show_hidden_tooltip = gettext("Show Hidden Files");
        let show_hidden = gtk4::ToggleButton::builder()
            .icon_name("view-reveal-symbolic")
            .tooltip_text(&show_hidden_tooltip)
            .build();
        show_hidden.update_property(&[Property::Label(&show_hidden_tooltip)]);
        show_hidden.set_action_name(Some("win.project-show-hidden"));
        header.pack_end(&show_hidden);

        let refresh_tooltip = gettext("Refresh Project Tree");
        let refresh = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(&refresh_tooltip)
            .build();
        refresh.update_property(&[Property::Label(&refresh_tooltip)]);
        refresh.set_action_name(Some("win.refresh-project-tree"));
        header.pack_end(&refresh);

        let close_tooltip = gettext("Close Folder");
        let close_button = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(&close_tooltip)
            .build();
        close_button.update_property(&[Property::Label(&close_tooltip)]);
        close_button.set_action_name(Some("win.close-folder"));
        header.pack_end(&close_button);

        let root = adw::ToolbarView::new();
        root.add_top_bar(&header);
        root.set_content(Some(tree.widget()));

        Self { root, title, tree }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }

    #[must_use]
    pub(crate) fn tree(&self) -> &ProjectTree {
        &self.tree
    }

    pub(crate) fn set_title(&self, title: &str) {
        self.title.set_title(title);
        self.title.set_subtitle("");
    }
}
