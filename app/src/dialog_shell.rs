use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

pub(crate) struct DialogShell {
    pub(crate) dialog: adw::Dialog,
    pub(crate) content: gtk4::Box,
}

pub(crate) fn build_dialog_shell(
    title: &str,
    content_width: i32,
    content_height: Option<i32>,
    follows_content_size: bool,
) -> DialogShell {
    let mut builder = adw::Dialog::builder()
        .title(title)
        .content_width(content_width)
        .follows_content_size(follows_content_size)
        .can_close(true);
    if let Some(content_height) = content_height {
        builder = builder.content_height(content_height);
    }
    let dialog = builder.build();

    let title_widget = adw::WindowTitle::new(title, "");
    title_widget.set_hexpand(true);

    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(true);
    header.set_title_widget(Some(&title_widget));

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.set_vexpand(true);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));
    dialog.set_child(Some(&toolbar_view));

    DialogShell { dialog, content }
}
