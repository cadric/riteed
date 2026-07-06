use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use sourceview5::prelude::*;

use super::minimap::CompareMinimap;
use super::presentation::{DiffPresentation, PresentationSide};
use super::ui::configure_presentation_view;
use super::viewport;
use crate::editor_tab::EditorTab;
use crate::settings::CompareViewMode;

pub(super) const SPLIT_LAYOUT_NAME: &str = "split";
pub(super) const UNIFIED_LAYOUT_NAME: &str = "unified";

pub(super) struct PresentationPane {
    pub(super) root: gtk4::Box,
    pub(super) buffer: sourceview5::Buffer,
    pub(super) view: sourceview5::View,
    pub(super) scrolled: gtk4::ScrolledWindow,
    pub(super) minimap: CompareMinimap,
}

pub(super) struct UnifiedPane {
    pub(super) root: gtk4::Box,
    pub(super) buffer: sourceview5::Buffer,
    pub(super) view: sourceview5::View,
    pub(super) scrolled: gtk4::ScrolledWindow,
    pub(super) minimap: CompareMinimap,
}

pub(super) struct StyleHandlers {
    pub(super) manager: adw::StyleManager,
    pub(super) style_handler: gtk4::glib::SignalHandlerId,
    pub(super) high_contrast_handler: gtk4::glib::SignalHandlerId,
}

pub(super) struct CompareLayoutRoot {
    pub(super) root: adw::BreakpointBin,
    pub(super) stack: gtk4::Stack,
}

pub(super) fn build_reference_pane(
    tab: &EditorTab,
    presentation: &Rc<RefCell<DiffPresentation>>,
) -> PresentationPane {
    build_presentation_pane(
        tab,
        &pgettext("compare pane", "Reference"),
        presentation,
        PresentationSide::Reference,
    )
}

pub(super) fn build_current_pane(
    tab: &EditorTab,
    presentation: &Rc<RefCell<DiffPresentation>>,
) -> PresentationPane {
    build_presentation_pane(
        tab,
        &pgettext("compare pane", "Current"),
        presentation,
        PresentationSide::Current,
    )
}

pub(super) fn build_unified_pane(tab: &EditorTab) -> UnifiedPane {
    build_unified_pane_with_title(tab, &pgettext("compare pane", "Unified"))
}

pub(super) fn build_compare_paned(
    left: &impl IsA<gtk4::Widget>,
    right: &impl IsA<gtk4::Widget>,
) -> gtk4::Paned {
    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_resize_start_child(true);
    paned.set_shrink_start_child(false);
    paned.set_resize_end_child(true);
    paned.set_shrink_end_child(false);
    paned.set_start_child(Some(left));
    paned.set_end_child(Some(right));
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    paned
}

pub(super) fn build_compare_layout_root(
    split: &impl IsA<gtk4::Widget>,
    unified: &impl IsA<gtk4::Widget>,
    split_view: &sourceview5::View,
    unified_view: &sourceview5::View,
    mode: Rc<Cell<CompareViewMode>>,
) -> CompareLayoutRoot {
    let stack = gtk4::Stack::builder()
        .hexpand(true)
        .vexpand(true)
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .build();
    stack.add_named(split, Some(SPLIT_LAYOUT_NAME));
    stack.add_named(unified, Some(UNIFIED_LAYOUT_NAME));
    stack.set_visible_child_name(SPLIT_LAYOUT_NAME);

    let root = adw::BreakpointBin::builder()
        .width_request(1)
        .height_request(1)
        .child(&stack)
        .build();
    let condition = adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        ADAPTIVE_COMPARE_MAX_WIDTH_SP,
        adw::LengthUnit::Sp,
    );
    let breakpoint = adw::Breakpoint::new(condition);
    let stack_for_apply = stack.clone();
    let mode_for_apply = Rc::clone(&mode);
    let split_view_for_apply = split_view.clone();
    let unified_view_for_apply = unified_view.clone();
    breakpoint.connect_apply(move |_| {
        if mode_for_apply.get() == CompareViewMode::Adaptive {
            let state = viewport::capture(&split_view_for_apply);
            stack_for_apply.set_visible_child_name(UNIFIED_LAYOUT_NAME);
            viewport::restore(&state, &unified_view_for_apply);
        }
    });
    let stack_for_unapply = stack.clone();
    let split_view_for_unapply = split_view.clone();
    let unified_view_for_unapply = unified_view.clone();
    breakpoint.connect_unapply(move |_| {
        if mode.get() == CompareViewMode::Adaptive {
            let state = viewport::capture(&unified_view_for_unapply);
            stack_for_unapply.set_visible_child_name(SPLIT_LAYOUT_NAME);
            viewport::restore(&state, &split_view_for_unapply);
        }
    });
    root.add_breakpoint(breakpoint);

    CompareLayoutRoot { root, stack }
}

pub(super) fn connect_style_handlers(tab: &Rc<EditorTab>) -> StyleHandlers {
    let manager = adw::StyleManager::default();
    let weak = Rc::downgrade(tab);
    let style_handler = manager.connect_dark_notify(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.apply_compare_style();
        }
    });
    let weak = Rc::downgrade(tab);
    let high_contrast_handler = manager.connect_high_contrast_notify(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.apply_compare_style();
        }
    });
    StyleHandlers {
        manager,
        style_handler,
        high_contrast_handler,
    }
}

pub(super) fn sync_reference_language(
    editable_buffer: &sourceview5::Buffer,
    reference_buffer: &sourceview5::Buffer,
) {
    let language = editable_buffer.language();
    reference_buffer.set_language(language.as_ref());
    reference_buffer.set_highlight_syntax(language.is_some());
}

fn build_presentation_pane(
    tab: &EditorTab,
    title: &str,
    presentation: &Rc<RefCell<DiffPresentation>>,
    side: PresentationSide,
) -> PresentationPane {
    let buffer = sourceview5::Buffer::builder()
        .enable_undo(false)
        .implicit_trailing_newline(false)
        .build();
    tab.settings.apply_source_style_scheme(&buffer);
    sync_reference_language(&tab.text_buffer, &buffer);
    let view = sourceview5::View::with_buffer(&buffer);
    configure_presentation_view(tab, &view);
    super::interaction::install_presentation_interaction(&view, &buffer, presentation, side);
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&view)
        .build();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_min_content_width(0);
    let minimap = CompareMinimap::new(tab, &view, &scrolled);
    let root = titled_pane_root(title, &scrolled, &minimap.holder);
    PresentationPane {
        root,
        buffer,
        view,
        scrolled,
        minimap,
    }
}

fn build_unified_pane_with_title(tab: &EditorTab, title: &str) -> UnifiedPane {
    let buffer = sourceview5::Buffer::builder()
        .enable_undo(false)
        .implicit_trailing_newline(false)
        .build();
    tab.settings.apply_source_style_scheme(&buffer);
    sync_reference_language(&tab.text_buffer, &buffer);
    let view = sourceview5::View::with_buffer(&buffer);
    configure_presentation_view(tab, &view);
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&view)
        .build();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_min_content_width(0);
    let minimap = CompareMinimap::new(tab, &view, &scrolled);
    let root = titled_pane_root(title, &scrolled, &minimap.holder);
    UnifiedPane {
        root,
        buffer,
        view,
        scrolled,
        minimap,
    }
}

fn titled_pane_root(
    title: &str,
    scrolled: &gtk4::ScrolledWindow,
    minimap_holder: &gtk4::Box,
) -> gtk4::Box {
    let label = gtk4::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(4)
        .build();
    label.add_css_class("dim-label");
    let root = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .build();
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.append(scrolled);
    content.append(minimap_holder);
    root.append(&label);
    root.append(&content);
    root
}

const ADAPTIVE_COMPARE_MAX_WIDTH_SP: f64 = 1_000.0;
