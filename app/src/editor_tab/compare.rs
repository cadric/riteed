use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::EditorTab;
use crate::editor_io::{self, LoadFailure};
use crate::error::AppError;

mod controller;
mod diff;
mod target;
mod ui;

use controller::sync_reference_language;
use diff::DiffPlan;
use target::{CompareTarget, CompareTargetKind};
use ui::{CompareTags, clear_tags};

pub(crate) struct CompareController {
    target: CompareTarget,
    toolbar: gtk4::Box,
    status_label: gtk4::Label,
    paned: gtk4::Paned,
    reference_view: sourceview5::View,
    reference_buffer: sourceview5::Buffer,
    tags: CompareTags,
    diff_plan: DiffPlan,
    current_hunk: Option<usize>,
    cancellable: Option<gio::Cancellable>,
    left_adjustment: gtk4::Adjustment,
    right_adjustment: gtk4::Adjustment,
    scroll_anchors: Rc<std::cell::RefCell<Vec<diff::CompareLineAnchor>>>,
    left_handler: Option<glib::SignalHandlerId>,
    right_handler: Option<glib::SignalHandlerId>,
    style_manager: adw::StyleManager,
    style_handler: Option<glib::SignalHandlerId>,
    high_contrast_handler: Option<glib::SignalHandlerId>,
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
        let Some(uri) = self.uri() else {
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
        let target = CompareTarget::file(file.clone());
        self.start_compare_with_target(&target, callback);
    }

    pub fn start_compare_with_text(
        self: &Rc<Self>,
        text: &str,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
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
            clear_tags(&self.text_buffer, &compare.reference_buffer, &compare.tags);
            self.root.remove(&compare.toolbar);
            self.root.remove(&compare.paned);
            compare.paned.set_start_child(Option::<&gtk4::Widget>::None);
            compare.paned.set_end_child(Option::<&gtk4::Widget>::None);
            drop(compare);
            self.root.append(&self.content);
            self.apply_minimap_visibility();
            self.sync_presentation();
        }
    }

    pub fn compare_next_diff(&self) {
        self.move_compare_hunk(1);
    }

    pub fn compare_previous_diff(&self) {
        self.move_compare_hunk(-1);
    }

    pub(super) fn recompute_compare_from_editable(&self) {
        let text = self.buffer_text();
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_mut() {
            compare.recompute(&self.text_buffer, &self.text_view, &text);
        }
    }

    pub(crate) fn apply_compare_style(&self) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_ref() {
            self.settings
                .apply_source_style_scheme(&compare.reference_buffer);
            sync_reference_language(&self.text_buffer, &compare.reference_buffer);
            compare.apply_tag_colors(
                self.settings.editor_palette_is_dark(),
                compare.style_manager.is_high_contrast(),
            );
            compare.apply_current_hunk(&self.text_buffer);
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

    pub(super) fn sync_compare_reference_after_save(&self, saved_uri: &str) {
        let text = self.buffer_text();
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_mut()
            && compare.target.kind == CompareTargetKind::Disk
            && compare.target.uri.as_deref() == Some(saved_uri)
        {
            compare.set_reference_text(&text, false);
            compare.recompute(&self.text_buffer, &self.text_view, &text);
        }
    }

    #[cfg(test)]
    pub(crate) fn compare_diff_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.diff_plan.hunks.len())
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn compare_status_for_tests(&self) -> String {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.status_label.text().to_string())
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn compare_current_hunk_for_tests(&self) -> Option<usize> {
        self.state.try_borrow().ok().and_then(|state| {
            state
                .compare
                .active
                .as_ref()
                .and_then(|compare| compare.current_hunk)
        })
    }

    #[cfg(test)]
    pub(crate) fn compare_editable_highlight_count_for_tests(&self) -> usize {
        compare_highlight_count(&self.text_buffer)
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
        self.exit_compare();
        self.root.remove(&self.content);
        self.minimap_holder.set_visible(false);
        self.scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        let compare = CompareController::new(self, target.clone());
        compare.paned.set_start_child(Some(&self.content));
        self.root.append(&compare.toolbar);
        self.root.append(&compare.paned);
        compare.apply_tag_colors(
            self.settings.editor_palette_is_dark(),
            compare.style_manager.is_high_contrast(),
        );
        self.state.borrow_mut().compare.active = Some(compare);
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
            let editable_text = self.buffer_text();
            let applied = {
                let mut state = self.state.borrow_mut();
                if let Some(compare) = state.compare.active.as_mut() {
                    compare.set_reference_text(reference_text, target.implicit_trailing_newline);
                    compare.recompute(&self.text_buffer, &self.text_view, &editable_text);
                    true
                } else {
                    false
                }
            };
            self.sync_presentation();
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
                        let editable_text = tab.buffer_text();
                        let applied = {
                            let mut state = tab.state.borrow_mut();
                            if let Some(compare) = state.compare.active.as_mut()
                                && compare.target.uri.as_deref() == Some(target_uri.as_str())
                            {
                                compare.finish_loading();
                                compare.set_reference_text(
                                    &document.text,
                                    document.format.implicit_trailing_newline(),
                                );
                                compare.recompute(&tab.text_buffer, &tab.text_view, &editable_text);
                                true
                            } else {
                                false
                            }
                        };
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
            compare.move_hunk(&self.text_buffer, &self.text_view, direction);
        }
    }

    fn bump_compare_generation(&self) -> u64 {
        self.state.borrow_mut().compare.next_generation()
    }

    fn compare_generation(&self) -> u64 {
        self.state.borrow().compare.request_generation
    }
}

fn map_reference_load_error(error: LoadFailure) -> AppError {
    match error {
        LoadFailure::DecodeFailed(path) => AppError::DecodeFailed(path),
        LoadFailure::TooBig(path) => AppError::FileTooBig(path),
        LoadFailure::Failed(error) => error,
    }
}

#[cfg(test)]
fn compare_highlight_count(buffer: &sourceview5::Buffer) -> usize {
    let mut iter = buffer.start_iter();
    let end = buffer.end_iter();
    let mut count = 0;
    while iter < end {
        count += iter
            .tags()
            .iter()
            .filter(|tag| tag.background_rgba().is_some())
            .count();
        if !iter.forward_char() {
            break;
        }
    }
    count
}
