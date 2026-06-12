use std::cell::Cell;
use std::rc::{Rc, Weak};

use gettextrs::pgettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::{EditorTab, TabKind};
use crate::editor_io::{self, LoadFailure};
use crate::error::AppError;

const COMPARE_SCROLL_LAYOUT_RETRIES: u8 = 8;

mod change_list;
mod clipboard;
mod controller;
mod diff;
mod display;
mod gutter;
mod hatch;
mod inline;
mod interaction;
mod layout;
mod menu;
mod minimap;
mod minimap_rows;
mod model;
mod navigation;
mod padding;
mod presentation;
mod presentation_display;
mod render;
mod render_unified;
mod reveal;
mod review_session;
mod review_session_reveal;
#[cfg(test)]
mod review_session_tests;
mod scroll;
mod status;
mod target;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod testing_minimap;
#[cfg(test)]
mod testing_render;
mod ui;
mod unified;
pub(in crate::editor_tab) mod viewport;

use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};
use display::CompareDisplayModel;
use layout::sync_reference_language;
use model::DiffRowModel;
use presentation::DiffPresentation;
use render::CompareTags;
use render_unified::UnifiedTags;
use target::{CompareTarget, CompareTargetKind};
use unified::UnifiedPresentation;

pub(in crate::editor_tab) use change_list::present as present_change_list_dialog;
pub(in crate::editor_tab) use minimap_rows::{
    MinimapRow, MinimapRowKind, compute as compute_minimap_rows,
};
pub(in crate::editor_tab) use review_session::ReviewSession;
pub(crate) use review_session::{ReviewFileInput, ReviewScrollTarget};
#[cfg(test)]
pub(crate) use testing::row_count_for_texts_for_tests;

#[cfg(feature = "fuzzing")]
pub(crate) fn fuzz_compute_diff(reference_text: &str, current_text: &str) -> (bool, usize) {
    let computation =
        diff::compute_diff_with_options(reference_text, current_text, diff::DiffOptions::default());
    (
        computation.skip_reason.is_some(),
        computation.model.changed_row_count(),
    )
}

pub(in crate::editor_tab) fn review_toolbar() -> gtk4::Box {
    ui::review_toolbar()
}

pub(crate) struct CompareController {
    target: CompareTarget,
    toolbar: gtk4::Box,
    status_label: gtk4::Label,
    layout_root: adw::BreakpointBin,
    layout_stack: gtk4::Stack,
    paned: gtk4::Paned,
    editable_snapshot: String,
    reference_text: String,
    left_view: sourceview5::View,
    left_buffer: sourceview5::Buffer,
    left_minimap: minimap::CompareMinimap,
    right_view: sourceview5::View,
    right_buffer: sourceview5::Buffer,
    right_minimap: minimap::CompareMinimap,
    unified_view: sourceview5::View,
    unified_buffer: sourceview5::Buffer,
    unified_minimap: minimap::CompareMinimap,
    left_vadjustment: gtk4::Adjustment,
    right_vadjustment: gtk4::Adjustment,
    unified_vadjustment: gtk4::Adjustment,
    scroll_past_end_floor: i32,
    tags: CompareTags,
    unified_tags: UnifiedTags,
    presentation: Rc<std::cell::RefCell<DiffPresentation>>,
    display_model: Rc<std::cell::RefCell<CompareDisplayModel>>,
    row_model: Rc<std::cell::RefCell<DiffRowModel>>,
    unified_presentation: Rc<std::cell::RefCell<UnifiedPresentation>>,
    gutters: gutter::CompareGutters,
    unified_gutter: gutter::UnifiedGutter,
    hatches: hatch::CompareHatches,
    scroll_sync: scroll::CompareScrollSync,
    current_hunk: Option<usize>,
    cancellable: Option<gio::Cancellable>,
    style_manager: adw::StyleManager,
    style_handler: Option<glib::SignalHandlerId>,
    high_contrast_handler: Option<glib::SignalHandlerId>,
    diff_options: diff::DiffOptions,
    review_settings: CompareReviewSettingsSnapshot,
    view_mode: CompareViewMode,
    view_mode_cell: Rc<Cell<CompareViewMode>>,
    minimap_user_visible: Rc<Cell<bool>>,
    minimap_width_suppressed: Rc<Cell<bool>>,
    revealed_rows: std::collections::BTreeSet<usize>,
    hidden_trim_whitespace_differences: bool,
}

impl EditorTab {
    #[must_use]
    pub fn is_compare_active(&self) -> bool {
        self.state
            .try_borrow()
            .map_or(true, |state| state.compare.active.is_some())
    }

    #[must_use]
    pub fn has_saved_local_uri(&self) -> bool {
        self.saved_file().is_some()
    }

    #[must_use]
    pub fn compare_reference_is_refreshable(&self) -> bool {
        self.state.try_borrow().is_ok_and(|state| {
            state
                .compare
                .active
                .as_ref()
                .is_some_and(|compare| compare.target.is_refreshable())
        })
    }

    #[must_use]
    pub fn is_compare_with_current_disk(&self) -> bool {
        let Some(uri) = self.document_uri() else {
            return false;
        };
        self.state.try_borrow().is_ok_and(|state| {
            state.compare.active.as_ref().is_some_and(|compare| {
                compare.target.kind == CompareTargetKind::Disk
                    && compare.target.uri.as_deref() == Some(uri.as_str())
            })
        })
    }

    pub fn start_compare_with_disk(self: &Rc<Self>, callback: Rc<dyn Fn(Result<(), AppError>)>) {
        if !self.is_document() {
            callback(Err(AppError::MissingSavePath));
            return;
        }
        let Some(file) = self.saved_file() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        let target = CompareTarget::disk(file);
        self.start_compare_with_target(&target, callback);
    }

    pub fn start_compare_with_file(
        self: &Rc<Self>,
        file: &gio::File,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::Cancelled));
            return;
        }
        let target = CompareTarget::file(file.clone());
        self.start_compare_with_target(&target, callback);
    }

    pub fn start_compare_with_text(
        self: &Rc<Self>,
        text: &str,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::Cancelled));
            return;
        }
        let target = CompareTarget::text(pgettext("compare source", "Pasted Text"), text.into());
        self.start_compare_with_target(&target, callback);
    }

    pub(crate) fn start_compare_with_reference_text(
        self: &Rc<Self>,
        title: String,
        text: String,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let target = CompareTarget::text(title, text);
        self.start_compare_with_target(&target, callback);
    }

    pub fn refresh_compare_reference(self: &Rc<Self>, callback: Rc<dyn Fn(Result<(), AppError>)>) {
        let target = self.state.try_borrow().ok().and_then(|state| {
            state
                .compare
                .active
                .as_ref()
                .map(|compare| compare.target.clone())
        });
        if let Some(target) = target {
            self.load_compare_reference(&target, callback);
        } else {
            callback(Err(AppError::Cancelled));
        }
    }

    pub fn exit_compare(&self) {
        self.bump_compare_generation();
        let compare = self.state.borrow_mut().compare.active.take();
        if let Some(mut compare) = compare {
            compare.cancel();
            compare.detach_visual_layers();
            self.root.remove(&compare.toolbar);
            self.root.remove(&compare.layout_root);
            compare.paned.set_start_child(Option::<&gtk4::Widget>::None);
            compare.paned.set_end_child(Option::<&gtk4::Widget>::None);
            drop(compare);
            self.root.append(&self.content);
            self.apply_minimap_visibility();
            self.apply_word_wrap();
            self.sync_presentation();
        }
    }

    pub fn compare_next_diff(&self) {
        self.move_compare_hunk(1);
    }

    pub fn compare_previous_diff(&self) {
        self.move_compare_hunk(-1);
    }

    pub fn compare_reveal_above(&self) {
        if let Some(compare) = self.state.borrow_mut().compare.active.as_mut() {
            compare.reveal_above();
        }
    }

    pub fn compare_reveal_below(&self) {
        if let Some(compare) = self.state.borrow_mut().compare.active.as_mut() {
            compare.reveal_below();
        }
    }

    pub fn compare_reveal_all(&self) {
        if let Some(compare) = self.state.borrow_mut().compare.active.as_mut() {
            compare.reveal_all();
        }
    }

    #[must_use]
    pub fn compare_can_reveal_context(&self) -> bool {
        self.state
            .borrow()
            .compare
            .active
            .as_ref()
            .is_some_and(CompareController::can_reveal_context)
    }

    #[must_use]
    pub fn compare_uses_unified_layout(&self) -> bool {
        self.state
            .borrow()
            .compare
            .active
            .as_ref()
            .is_some_and(CompareController::uses_unified_layout)
    }

    pub(crate) fn refresh_compare_settings(self: &Rc<Self>) {
        let snapshot = self.settings.compare_review_settings_snapshot();
        let view_mode = self.settings.compare_view_mode();
        if let Some(row) = {
            let mut state = self.state.borrow_mut();
            state
                .compare
                .active
                .as_mut()
                .and_then(|compare| compare.apply_settings(snapshot, view_mode))
        } {
            self.queue_compare_scroll_to_row(row, self.compare_generation());
        }

        let session = self.state.borrow().review.session.clone();
        if let Some(session) = session {
            {
                let mut session = session.borrow_mut();
                session.rebuild_displays(snapshot);
                session.render_into_buffer(&self.text_buffer);
            }
            self.text_buffer.set_modified(false);
            let wrap_mode = if snapshot.word_wrap {
                gtk4::WrapMode::WordChar
            } else {
                gtk4::WrapMode::None
            };
            self.text_view.set_wrap_mode(wrap_mode);
            self.sync_presentation();
        } else if self.kind() == TabKind::GitReview {
            let wrap_mode = if snapshot.word_wrap {
                gtk4::WrapMode::WordChar
            } else {
                gtk4::WrapMode::None
            };
            self.text_view.set_wrap_mode(wrap_mode);
        }
    }

    pub(crate) fn apply_compare_style(&self) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_ref() {
            self.settings
                .apply_source_style_scheme(&compare.left_buffer);
            self.settings
                .apply_source_style_scheme(&compare.right_buffer);
            sync_reference_language(&self.text_buffer, &compare.left_buffer);
            sync_reference_language(&self.text_buffer, &compare.right_buffer);
            compare.apply_tag_colors();
            compare.gutters.refresh();
        }
    }

    pub(crate) fn clear_compare_zoom_style(&self) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_ref() {
            compare.clear_zoom_style();
        }
    }

    pub(crate) fn restore_compare_zoom_style(&self, css_class: &str) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_ref() {
            compare.restore_zoom_style(css_class);
        }
    }

    pub(super) fn sync_compare_reference_after_save(self: &Rc<Self>, saved_uri: &str) {
        let generation = self.compare_generation();
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        let mut scroll_row = None;
        if let Some(compare) = state.compare.active.as_mut()
            && compare.target.kind == CompareTargetKind::Disk
            && compare.target.uri.as_deref() == Some(saved_uri)
        {
            let snapshot = compare.editable_snapshot.clone();
            compare.set_reference_text(&snapshot);
            scroll_row = compare.recompute();
        }
        drop(state);
        if let Some(row) = scroll_row {
            self.queue_compare_scroll_to_row(row, generation);
        }
    }

    fn start_compare_with_target(
        self: &Rc<Self>,
        target: &CompareTarget,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if self.is_compare_with_current_disk() && target.kind == CompareTargetKind::Disk {
            callback(Ok(()));
            return;
        }
        self.enter_compare_layout(target);
        self.load_compare_reference(target, callback);
    }

    fn enter_compare_layout(self: &Rc<Self>, target: &CompareTarget) {
        self.exit_markdown_preview();
        self.exit_compare();
        self.root.remove(&self.content);
        self.minimap_holder.set_visible(false);
        self.scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        let compare = CompareController::new(self, target.clone());
        self.root.append(&compare.toolbar);
        self.root.append(&compare.layout_root);
        compare.apply_tag_colors();
        self.state.borrow_mut().compare.active = Some(compare);
        self.text_view.set_wrap_mode(gtk4::WrapMode::None);
        self.sync_presentation();
    }

    fn load_compare_reference(
        self: &Rc<Self>,
        target: &CompareTarget,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if target.kind == CompareTargetKind::Text {
            let Some(reference_text) = target.text.as_deref() else {
                callback(Err(AppError::Cancelled));
                return;
            };
            let (applied, scroll_row) = {
                let mut state = self.state.borrow_mut();
                if let Some(compare) = state.compare.active.as_mut() {
                    compare.set_reference_text(reference_text);
                    (true, compare.recompute())
                } else {
                    (false, None)
                }
            };
            self.sync_presentation();
            if let Some(row) = scroll_row {
                self.queue_compare_scroll_to_row(row, self.compare_generation());
            }
            callback(if applied {
                Ok(())
            } else {
                Err(AppError::Cancelled)
            });
            return;
        }

        let Some(file) = target.file.as_ref() else {
            callback(Err(AppError::Cancelled));
            return;
        };
        let Some(target_uri) = target.uri.clone() else {
            callback(Err(AppError::Cancelled));
            return;
        };

        let generation = self.bump_compare_generation();
        let cancellable = gio::Cancellable::new();
        if let Some(compare) = self.state.borrow_mut().compare.active.as_mut() {
            compare.set_loading(&cancellable);
        }
        let weak = Rc::downgrade(self);
        editor_io::load_text_file(
            file,
            None,
            Some(&cancellable),
            Rc::new(move |result| {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                if tab.compare_generation() != generation {
                    return;
                }
                let outcome = match result {
                    Ok(document) => {
                        let (applied, scroll_row) = {
                            let mut state = tab.state.borrow_mut();
                            if let Some(compare) = state.compare.active.as_mut()
                                && compare.target.uri.as_deref() == Some(target_uri.as_str())
                            {
                                compare.finish_loading();
                                compare.set_reference_text(&document.text);
                                (true, compare.recompute())
                            } else {
                                (false, None)
                            }
                        };
                        if let Some(row) = scroll_row {
                            tab.queue_compare_scroll_to_row(row, generation);
                        }
                        if applied {
                            Ok(())
                        } else {
                            Err(AppError::Cancelled)
                        }
                    }
                    Err(error) => Err(map_reference_load_error(error)),
                };
                tab.sync_presentation();
                callback(outcome);
            }),
        );
    }

    fn move_compare_hunk(&self, direction: i32) {
        if let Some(compare) = self.state.borrow_mut().compare.active.as_mut() {
            compare.move_hunk(direction);
        }
    }

    fn bump_compare_generation(&self) -> u64 {
        self.state.borrow_mut().compare.next_generation()
    }

    fn compare_generation(&self) -> u64 {
        self.state.borrow().compare.request_generation
    }

    fn queue_compare_scroll_to_row(self: &Rc<Self>, row: usize, generation: u64) {
        queue_compare_scroll_attempt(Rc::downgrade(self), row, generation, 0);
    }
}

fn queue_compare_scroll_attempt(weak: Weak<EditorTab>, row: usize, generation: u64, attempt: u8) {
    let _source = glib::idle_add_local_full(glib::Priority::LOW, move || {
        let Some(tab) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if tab.compare_generation() != generation {
            return glib::ControlFlow::Break;
        }
        let scrolled = match tab.state.try_borrow() {
            Ok(state) => {
                let Some(compare) = state.compare.active.as_ref() else {
                    return glib::ControlFlow::Break;
                };
                compare.scroll_to_row(row)
            }
            Err(_error) => false,
        };
        if scrolled || attempt >= COMPARE_SCROLL_LAYOUT_RETRIES {
            glib::ControlFlow::Break
        } else {
            queue_compare_scroll_attempt(Rc::downgrade(&tab), row, generation, attempt + 1);
            glib::ControlFlow::Break
        }
    });
}

fn map_reference_load_error(error: LoadFailure) -> AppError {
    match error {
        LoadFailure::DecodeFailed(path) => AppError::DecodeFailed(path),
        LoadFailure::TooBig(path) => AppError::FileTooBig(path),
        LoadFailure::LineTooLong { path, .. } => AppError::LineTooLong(path),
        LoadFailure::Failed(error) => error,
    }
}
