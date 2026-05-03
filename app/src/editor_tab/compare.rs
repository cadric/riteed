use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use super::EditorTab;
use crate::editor_io::{self, LoadFailure};
use crate::error::AppError;

mod controller;
mod diff;
mod gutter;
mod inline;
mod interaction;
mod model;
mod navigation;
mod presentation;
mod render;
mod scroll;
mod target;
#[cfg(test)]
mod testing;
mod ui;

use controller::sync_reference_language;
use model::DiffRowModel;
use presentation::DiffPresentation;
use render::CompareTags;
use target::{CompareTarget, CompareTargetKind};

#[cfg(test)]
pub(crate) use testing::row_count_for_texts_for_tests;

pub(crate) struct CompareController {
    target: CompareTarget,
    toolbar: gtk4::Box,
    status_label: gtk4::Label,
    paned: gtk4::Paned,
    editable_snapshot: String,
    reference_text: String,
    left_view: sourceview5::View,
    left_buffer: sourceview5::Buffer,
    right_view: sourceview5::View,
    right_buffer: sourceview5::Buffer,
    tags: CompareTags,
    presentation: Rc<std::cell::RefCell<DiffPresentation>>,
    row_model: Rc<std::cell::RefCell<DiffRowModel>>,
    gutters: gutter::CompareGutters,
    scroll_marks: scroll::CompareScrollMarks,
    current_hunk: Option<usize>,
    cancellable: Option<gio::Cancellable>,
    left_adjustment: gtk4::Adjustment,
    right_adjustment: gtk4::Adjustment,
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
            self.root.remove(&compare.toolbar);
            self.root.remove(&compare.paned);
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
            compare.apply_current_hunk();
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

    pub(super) fn sync_compare_reference_after_save(&self, saved_uri: &str) {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_mut()
            && compare.target.kind == CompareTargetKind::Disk
            && compare.target.uri.as_deref() == Some(saved_uri)
        {
            let snapshot = compare.editable_snapshot.clone();
            compare.set_reference_text(&snapshot, false);
            compare.recompute();
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
        self.exit_compare();
        self.root.remove(&self.content);
        self.minimap_holder.set_visible(false);
        self.scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        let compare = CompareController::new(self, target.clone());
        self.root.append(&compare.toolbar);
        self.root.append(&compare.paned);
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
            let applied = {
                let mut state = self.state.borrow_mut();
                if let Some(compare) = state.compare.active.as_mut() {
                    compare.set_reference_text(reference_text, target.implicit_trailing_newline);
                    compare.recompute();
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
                                compare.recompute();
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
            compare.move_hunk(direction);
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
