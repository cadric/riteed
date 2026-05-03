use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::prelude::*;
use sourceview5::prelude::*;

use crate::editor_tab::EditorTab;
use crate::editor_zoom::{
    EDITOR_VIEW_CSS_CLASS, copy_zoom_css_classes, resolve_scroll_past_end_padding,
};

pub(super) struct CompareToolbar {
    pub(super) root: gtk4::Box,
    pub(super) status_label: gtk4::Label,
}

pub(super) fn configure_presentation_view(tab: &EditorTab, view: &sourceview5::View) {
    let padding = resolve_scroll_past_end_padding(&tab.settings.editor_font());
    view.set_accepts_tab(false);
    view.set_bottom_margin(padding);
    view.set_cursor_visible(false);
    view.set_editable(false);
    view.set_hexpand(true);
    view.set_highlight_current_line(false);
    view.set_left_margin(12);
    view.set_monospace(true);
    view.set_right_margin(12);
    view.set_show_line_marks(false);
    view.set_show_line_numbers(false);
    view.set_top_margin(12);
    view.set_vexpand(true);
    view.set_wrap_mode(gtk4::WrapMode::None);
    view.add_css_class(EDITOR_VIEW_CSS_CLASS);
    copy_zoom_css_classes(&tab.text_view, view);
    tab.settings.apply_indentation(view);
}

pub(super) fn compare_toolbar(reference_title: &str) -> CompareToolbar {
    let toolbar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .margin_start(6)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    toolbar.set_hexpand(true);
    toolbar.append(
        &gtk4::Label::builder()
            .label(pgettext("compare toolbar", "Compare"))
            .build(),
    );
    let reference = gtk4::Label::builder()
        .label(reference_title)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .xalign(0.0)
        .build();
    reference.set_hexpand(true);
    toolbar.append(&reference);
    let read_only = gtk4::Label::builder()
        .label(pgettext(
            "compare toolbar",
            "Read-only - Exit Compare to edit",
        ))
        .xalign(0.0)
        .build();
    read_only.add_css_class("dim-label");
    read_only.update_property(&[Property::Label(&pgettext(
        "compare toolbar",
        "Read-only - Exit Compare to edit",
    ))]);
    toolbar.append(&read_only);
    toolbar.append(&toolbar_button(
        "go-up-symbolic",
        &pgettext("compare action", "Previous Difference"),
        "win.diff-prev",
    ));
    toolbar.append(&toolbar_button(
        "go-down-symbolic",
        &pgettext("compare action", "Next Difference"),
        "win.diff-next",
    ));
    toolbar.append(&toolbar_button(
        "view-refresh-symbolic",
        &pgettext("compare action", "Refresh Reference"),
        "win.compare-refresh-reference",
    ));
    toolbar.append(&toolbar_button(
        "window-close-symbolic",
        &pgettext("compare action", "Exit Compare"),
        "win.compare-exit",
    ));
    let status = gtk4::Label::builder()
        .label(ellipsis_label(pgettext(
            "compare status",
            "Loading Reference",
        )))
        .xalign(1.0)
        .build();
    status.add_css_class("dim-label");
    toolbar.append(&status);
    CompareToolbar {
        root: toolbar,
        status_label: status,
    }
}

fn toolbar_button(icon_name: &str, tooltip: &str, action_name: &str) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .action_name(action_name)
        .build();
    button.update_property(&[Property::Label(tooltip)]);
    button
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}
