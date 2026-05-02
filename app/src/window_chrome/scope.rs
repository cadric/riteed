use gtk4::prelude::*;

pub(crate) const TAB_BAR_CLASS: &str = "riteed-tab-bar";
pub(crate) const TAB_VIEW_CLASS: &str = "riteed-tab-view";
pub(crate) const SIDEBAR_HEADER_CLASS: &str = "riteed-sidebar-header";
pub(crate) const SIDEBAR_SWITCHER_CLASS: &str = "riteed-sidebar-switcher";
pub(crate) const SIDEBAR_STACK_CLASS: &str = "riteed-sidebar-stack";
pub(crate) const SIDEBAR_CONTENT_CLASS: &str = "riteed-sidebar-content";

pub(super) fn add_class_once(widget: &impl IsA<gtk4::Widget>, css_class: &str) {
    let widget = widget.as_ref();
    if !widget.has_css_class(css_class) {
        widget.add_css_class(css_class);
    }
}

pub(super) fn remove_class(widget: &impl IsA<gtk4::Widget>, css_class: &str) {
    let widget = widget.as_ref();
    if widget.has_css_class(css_class) {
        widget.remove_css_class(css_class);
    }
}
