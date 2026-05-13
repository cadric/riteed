use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, prelude::*};
use libadwaita::prelude::*;
use sourceview5::prelude::*;

use super::diff::{DiffOptions, compute_diff_with_options};
use super::display::{CompareDisplayModel, CompareDisplayOptions, build_display_model};
use super::gutter::{CompareGutters, UnifiedGutter};
use super::hatch::{CompareHatchEndpoint, CompareHatches};
use super::layout::{
    SPLIT_LAYOUT_NAME, UNIFIED_LAYOUT_NAME, build_compare_layout_root, build_compare_paned,
    build_current_pane, build_reference_pane, build_unified_pane, connect_style_handlers,
};
use super::model::DiffRowModel;
use super::navigation::{target_hunk_for_navigation, top_visible_row};
use super::presentation::DiffPresentation;
use super::presentation_display::build_presentation_from_display;
use super::render::{
    CompareTags, apply_display_tags, apply_placeholder_tags, apply_presentation, clear_tags,
};
use super::render_unified::UnifiedTags;
use super::reveal::{RevealScope, reveal_rows};
use super::scroll::{CompareScrollEndpoint, install_scroll_sync};
use super::status::current_hunk_after_recompute;
use super::ui::compare_toolbar;
use super::unified::UnifiedPresentation;
use super::viewport;
use super::{CompareController, CompareTarget};
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{clear_zoom_css_classes, restore_zoom_css_class};
use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};

impl CompareController {
    pub(super) fn new(tab: &Rc<EditorTab>, target: CompareTarget) -> Self {
        let row_model = Rc::new(RefCell::new(DiffRowModel::empty()));
        let presentation = Rc::new(RefCell::new(DiffPresentation::empty()));
        let display_model = Rc::new(RefCell::new(CompareDisplayModel::empty()));
        let unified_presentation = Rc::new(RefCell::new(UnifiedPresentation::default()));
        let left = build_reference_pane(tab, &presentation);
        let right = build_current_pane(tab, &presentation);
        let unified = build_unified_pane(tab);
        let gutters = CompareGutters::new(&left.view, &right.view, &presentation, &row_model);
        let unified_gutter = UnifiedGutter::new(&unified.view, &unified_presentation);
        let toolbar = compare_toolbar(&target.title);
        let status_label = toolbar.status_label.clone();
        let paned = build_compare_paned(&left.root, &right.root);
        let view_mode = tab.settings.compare_view_mode();
        let view_mode_cell = Rc::new(Cell::new(view_mode));
        let layout = build_compare_layout_root(
            &paned,
            &unified.root,
            &left.view,
            &unified.view,
            Rc::clone(&view_mode_cell),
        );
        let scroll_sync = install_scroll_sync(
            CompareScrollEndpoint {
                adjustment: &left.scrolled.vadjustment(),
                buffer: &left.buffer,
                view: &left.view,
            },
            CompareScrollEndpoint {
                adjustment: &right.scrolled.vadjustment(),
                buffer: &right.buffer,
                view: &right.view,
            },
            &row_model,
        );
        let hatches = CompareHatches::new(
            CompareHatchEndpoint {
                view: &left.view,
                vadjustment: &left.scrolled.vadjustment(),
                hadjustment: &left.scrolled.hadjustment(),
            },
            CompareHatchEndpoint {
                view: &right.view,
                vadjustment: &right.scrolled.vadjustment(),
                hadjustment: &right.scrolled.hadjustment(),
            },
            &presentation,
        );
        let style_handlers = connect_style_handlers(tab);

        let mut controller = Self {
            target,
            toolbar: toolbar.root,
            status_label,
            layout_root: layout.root,
            layout_stack: layout.stack,
            paned,
            editable_snapshot: tab.buffer_text(),
            reference_text: String::new(),
            left_view: left.view,
            left_buffer: left.buffer.clone(),
            right_view: right.view,
            right_buffer: right.buffer.clone(),
            unified_view: unified.view,
            unified_buffer: unified.buffer.clone(),
            tags: CompareTags::new(&left.buffer, &right.buffer),
            unified_tags: UnifiedTags::new(&unified.buffer),
            presentation,
            display_model,
            row_model,
            unified_presentation,
            gutters,
            unified_gutter,
            hatches,
            scroll_sync,
            current_hunk: None,
            cancellable: None,
            style_manager: style_handlers.manager,
            style_handler: Some(style_handlers.style_handler),
            high_contrast_handler: Some(style_handlers.high_contrast_handler),
            diff_options: DiffOptions {
                ignore_leading_trailing_whitespace: tab
                    .settings
                    .compare_ignore_leading_trailing_whitespace(),
            },
            review_settings: tab.settings.compare_review_settings_snapshot(),
            view_mode,
            view_mode_cell,
            revealed_rows: BTreeSet::new(),
            hidden_trim_whitespace_differences: false,
        };
        super::clipboard::install_unified_clipboard(
            &controller.unified_view,
            &controller.unified_buffer,
            &controller.unified_presentation,
        );
        controller.sync_layout();
        controller
    }

    pub(super) fn set_loading(&mut self, cancellable: &gio::Cancellable) {
        if let Some(previous) = self.cancellable.take() {
            previous.cancel();
        }
        self.cancellable = Some(cancellable.clone());
        self.status_label.set_label(&ellipsis_label(pgettext(
            "compare status",
            "Loading Reference",
        )));
    }

    pub(super) fn finish_loading(&mut self) {
        self.cancellable = None;
    }

    pub(super) fn cancel(&mut self) {
        if let Some(cancellable) = self.cancellable.take() {
            cancellable.cancel();
        }
    }

    pub(super) fn set_reference_text(&mut self, text: &str) {
        self.reference_text.clear();
        self.reference_text.push_str(text);
    }

    pub(super) fn recompute(&mut self) -> Option<usize> {
        let previous = self.current_hunk;
        let computation = compute_diff_with_options(
            &self.reference_text,
            &self.editable_snapshot,
            self.diff_options,
        );
        let model = computation.model;
        let display = self.display_for_model(&model);
        let hunk_count = model.hunks.len();
        let too_large = model.too_large;
        self.current_hunk = current_hunk_after_recompute(previous, hunk_count, too_large);
        self.hidden_trim_whitespace_differences = computation.hidden_trim_whitespace_differences;
        self.row_model.borrow_mut().clone_from(&model);
        self.apply_display_model(&display);
        self.update_status(computation.hidden_trim_whitespace_differences);
        if self.current_hunk == Some(0) {
            return self.current_hunk_row();
        }
        None
    }

    pub(super) fn move_hunk(&mut self, direction: i32) {
        let model = self.row_model.borrow();
        if model.hunks.is_empty() || model.too_large {
            return;
        }
        let display = self.display_model.borrow();
        let display_top_row = top_visible_row(&self.left_view, display.visible_row_count());
        let top_row = display
            .logical_row_for_display(display_top_row)
            .unwrap_or(display_top_row);
        let base_row = self
            .current_hunk
            .and_then(|index| model.hunks.get(index))
            .map(|hunk| hunk.first_row)
            .filter(|first_row| *first_row >= top_row)
            .unwrap_or(top_row);
        let Some(next) = target_hunk_for_navigation(&model, base_row, direction) else {
            return;
        };
        drop(model);
        self.current_hunk = Some(next);
        self.update_status(false);
        let _scrolled = self.scroll_current_hunk();
    }

    pub(super) fn reveal_above(&mut self) {
        self.reveal_context(RevealScope::Above);
    }

    pub(super) fn reveal_below(&mut self) {
        self.reveal_context(RevealScope::Below);
    }

    pub(super) fn reveal_all(&mut self) {
        self.reveal_context(RevealScope::All);
    }

    pub(super) fn can_reveal_context(&self) -> bool {
        self.current_collapsed_marker().is_some()
    }

    #[must_use]
    pub(super) fn uses_unified_layout(&self) -> bool {
        self.layout_is_unified()
    }

    pub(super) fn apply_settings(
        &mut self,
        snapshot: CompareReviewSettingsSnapshot,
        view_mode: CompareViewMode,
    ) -> Option<usize> {
        let viewport = viewport::capture(self.active_view());
        self.review_settings = snapshot;
        self.diff_options.ignore_leading_trailing_whitespace =
            snapshot.ignore_leading_trailing_whitespace;
        self.view_mode = view_mode;
        self.sync_layout();
        self.apply_wrap_override();
        let row = self.recompute();
        viewport::restore(&viewport, self.active_view());
        row
    }

    fn apply_display_model(&mut self, display: &CompareDisplayModel) {
        clear_tags(&self.left_buffer, &self.right_buffer, &self.tags);
        let presentation = build_presentation_from_display(display);
        apply_presentation(&self.left_buffer, &self.right_buffer, &presentation);
        self.apply_unified_display(display, self.hidden_trim_whitespace_differences);
        self.presentation.borrow_mut().clone_from(&presentation);
        self.display_model.borrow_mut().clone_from(display);
        self.gutters.refresh();
        self.unified_gutter.refresh();
        self.hatches.refresh();
        apply_display_tags(&self.left_buffer, &self.right_buffer, display, &self.tags);
        apply_placeholder_tags(
            &self.left_buffer,
            &self.right_buffer,
            &presentation,
            &self.tags,
        );
    }

    fn display_for_model(&self, model: &DiffRowModel) -> CompareDisplayModel {
        let reference_lines = line_slices(&self.reference_text);
        let current_lines = line_slices(&self.editable_snapshot);
        let options = CompareDisplayOptions {
            collapse_unchanged: self.review_settings.collapse_unchanged,
            context_lines: usize::try_from(self.review_settings.context_lines)
                .map_or(3, |value| value)
                .clamp(1, 10),
            revealed_rows: self.revealed_rows.clone(),
        };
        build_display_model(None, model, &reference_lines, &current_lines, &options)
    }

    pub(super) fn apply_tag_colors(&self) {
        self.tags.apply_colors(&self.left_view);
        self.unified_tags.apply_colors(&self.unified_view);
        self.hatches.refresh_style();
    }

    pub(crate) fn apply_wrap_override(&self) {
        self.left_view.set_wrap_mode(gtk4::WrapMode::None);
        self.right_view.set_wrap_mode(gtk4::WrapMode::None);
        let unified_wrap = if self.review_settings.word_wrap {
            gtk4::WrapMode::WordChar
        } else {
            gtk4::WrapMode::None
        };
        self.unified_view.set_wrap_mode(unified_wrap);
        self.left_view.set_show_line_numbers(false);
        self.right_view.set_show_line_numbers(false);
        self.unified_view.set_show_line_numbers(false);
        self.left_view.set_show_line_marks(false);
        self.right_view.set_show_line_marks(false);
        self.unified_view.set_show_line_marks(false);
        self.hatches.refresh();
    }

    pub(super) fn clear_zoom_style(&self) {
        clear_zoom_css_classes(&self.left_view);
        clear_zoom_css_classes(&self.right_view);
        clear_zoom_css_classes(&self.unified_view);
        self.gutters.refresh();
        self.unified_gutter.refresh();
        self.hatches.refresh();
    }

    pub(super) fn restore_zoom_style(&self, css_class: &str) {
        restore_zoom_css_class(&self.left_view, css_class);
        restore_zoom_css_class(&self.right_view, css_class);
        restore_zoom_css_class(&self.unified_view, css_class);
        self.gutters.refresh();
        self.unified_gutter.refresh();
        self.hatches.refresh();
    }

    pub(super) fn detach_visual_layers(&mut self) {
        self.hatches.detach();
    }

    pub(super) fn scroll_to_row(&self, row: usize) -> bool {
        self.scroll_sync.scroll_to_row(row)
    }

    fn scroll_current_hunk(&self) -> bool {
        self.current_hunk_row()
            .is_some_and(|row| self.scroll_to_row(row))
    }

    fn current_hunk_row(&self) -> Option<usize> {
        let index = self.current_hunk?;
        let model = self.row_model.borrow();
        let hunk = model.hunks.get(index)?;
        self.display_model
            .borrow()
            .row_for_logical(hunk.first_row)
            .or(Some(hunk.first_row))
    }

    fn sync_layout(&mut self) {
        let viewport = viewport::capture(self.active_view());
        self.view_mode_cell.set(self.view_mode);
        match self.view_mode {
            CompareViewMode::Unified => self
                .layout_stack
                .set_visible_child_name(UNIFIED_LAYOUT_NAME),
            CompareViewMode::Adaptive if self.layout_root.current_breakpoint().is_some() => self
                .layout_stack
                .set_visible_child_name(UNIFIED_LAYOUT_NAME),
            CompareViewMode::Split | CompareViewMode::Adaptive => {
                self.layout_stack.set_visible_child_name(SPLIT_LAYOUT_NAME);
            }
        }
        viewport::restore(&viewport, self.active_view());
    }

    fn layout_is_unified(&self) -> bool {
        self.layout_stack
            .visible_child_name()
            .is_some_and(|name| name.as_str() == UNIFIED_LAYOUT_NAME)
    }

    fn active_view(&self) -> &sourceview5::View {
        if self.layout_is_unified() {
            &self.unified_view
        } else {
            &self.left_view
        }
    }

    fn current_unified_line(&self) -> Option<usize> {
        if !self.layout_is_unified() {
            return None;
        }
        let iter = self
            .unified_buffer
            .iter_at_mark(&self.unified_buffer.get_insert());
        usize::try_from(iter.line()).ok()
    }

    fn current_collapsed_marker(&self) -> Option<CollapsedMarker> {
        let cursor = self.current_unified_line()?;
        let presentation = self.unified_presentation.borrow();
        let mut line_index = cursor.min(presentation.lines.len().saturating_sub(1));
        loop {
            let line = presentation.lines.get(line_index)?;
            if let super::display::DisplayRowIdKind::Collapsed {
                hidden_start,
                hidden_end,
            } = line.display_row_id.kind
            {
                return Some(CollapsedMarker {
                    hidden_start,
                    hidden_end,
                });
            }
            if line_index == 0 {
                return None;
            }
            line_index = line_index.saturating_sub(1);
        }
    }

    fn reveal_context(&mut self, scope: RevealScope) {
        let Some(marker) = self.current_collapsed_marker() else {
            return;
        };
        let rows = reveal_rows(
            marker.hidden_start,
            marker.hidden_end,
            self.review_settings.context_lines,
            scope,
        );
        if rows.is_empty() {
            return;
        }
        let before = self.revealed_rows.len();
        self.revealed_rows.extend(rows);
        if self.revealed_rows.len() == before {
            return;
        }
        let viewport = viewport::capture(&self.unified_view);
        let display = {
            let model = self.row_model.borrow();
            self.display_for_model(&model)
        };
        self.apply_display_model(&display);
        let cursor = self.remaining_marker_line(marker.hidden_start, marker.hidden_end);
        viewport::restore_with_cursor_line(&viewport, &self.unified_view, cursor);
    }

    fn remaining_marker_line(&self, old_start: usize, old_end: usize) -> Option<usize> {
        self.unified_presentation
            .borrow()
            .lines
            .iter()
            .enumerate()
            .find_map(|(line_index, line)| {
                let super::display::DisplayRowIdKind::Collapsed {
                    hidden_start,
                    hidden_end,
                } = line.display_row_id.kind
                else {
                    return None;
                };
                (hidden_start >= old_start && hidden_end <= old_end).then_some(line_index)
            })
    }
}

#[derive(Clone, Copy)]
struct CollapsedMarker {
    hidden_start: usize,
    hidden_end: usize,
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

fn line_slices(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split_inclusive('\n').collect()
    }
}

impl Drop for CompareController {
    fn drop(&mut self) {
        self.detach_visual_layers();
        self.cancel();
        self.scroll_sync.disconnect();
        if let Some(handler) = self.style_handler.take() {
            self.style_manager.disconnect(handler);
        }
        if let Some(handler) = self.high_contrast_handler.take() {
            self.style_manager.disconnect(handler);
        }
    }
}
