use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use super::EditorTab;
use crate::editor_io::{self, LoadFailure};
use crate::error::AppError;

mod diff;
mod target;
mod ui;

use diff::{DiffPlan, compute_diff_plan};
use target::{CompareTarget, CompareTargetKind};
use ui::{
    CompareTags, apply_diff_tags, apply_line_tag, buffer_text, clear_tags, compare_toolbar,
    configure_reference_view, install_scroll_sync, remove_current_tags, scroll_to_line,
};

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
    left_handler: Option<glib::SignalHandlerId>,
    right_handler: Option<glib::SignalHandlerId>,
    style_manager: adw::StyleManager,
    style_handler: Option<glib::SignalHandlerId>,
}

impl EditorTab {
    #[must_use]
    pub fn is_compare_active(&self) -> bool {
        self.compare
            .try_borrow()
            .map_or(true, |compare| compare.is_some())
    }

    #[must_use]
    pub fn has_saved_local_uri(&self) -> bool {
        self.saved_file().is_some()
    }

    #[must_use]
    pub fn compare_reference_is_refreshable(&self) -> bool {
        self.compare.try_borrow().is_ok_and(|compare| {
            compare
                .as_ref()
                .is_some_and(|compare| compare.target.is_refreshable())
        })
    }

    #[must_use]
    pub fn is_compare_with_current_disk(&self) -> bool {
        let Some(uri) = self.uri() else {
            return false;
        };
        self.compare.try_borrow().is_ok_and(|compare| {
            compare.as_ref().is_some_and(|compare| {
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

    pub fn refresh_compare_reference(self: &Rc<Self>, callback: Rc<dyn Fn(Result<(), AppError>)>) {
        let target = self
            .compare
            .try_borrow()
            .ok()
            .and_then(|compare| compare.as_ref().map(|compare| compare.target.clone()));
        if let Some(target) = target {
            self.load_compare_reference(&target, callback);
        } else {
            callback(Err(AppError::Cancelled));
        }
    }

    pub fn exit_compare(&self) {
        self.bump_compare_generation();
        let compare = self.compare.borrow_mut().take();
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
        let Ok(mut compare_state) = self.compare.try_borrow_mut() else {
            return;
        };
        if let Some(compare) = compare_state.as_mut() {
            compare.recompute(&self.text_buffer, &self.text_view, &text);
        }
    }

    pub(super) fn apply_compare_style(&self) {
        let Ok(compare_state) = self.compare.try_borrow() else {
            return;
        };
        if let Some(compare) = compare_state.as_ref() {
            self.settings
                .apply_source_style_scheme(&compare.reference_buffer);
            compare.apply_tag_colors(self.settings.editor_palette_is_dark());
            compare.apply_current_hunk(&self.text_buffer);
        }
    }

    pub(super) fn sync_compare_reference_after_save(&self, saved_uri: &str) {
        let text = self.buffer_text();
        let Ok(mut compare_state) = self.compare.try_borrow_mut() else {
            return;
        };
        if let Some(compare) = compare_state.as_mut()
            && compare.target.kind == CompareTargetKind::Disk
            && compare.target.uri.as_deref() == Some(saved_uri)
        {
            compare.set_reference_text(&text, false);
            compare.recompute(&self.text_buffer, &self.text_view, &text);
        }
    }

    #[cfg(test)]
    pub(crate) fn compare_diff_count_for_tests(&self) -> usize {
        self.compare
            .try_borrow()
            .ok()
            .and_then(|compare| {
                compare
                    .as_ref()
                    .map(|compare| compare.diff_plan.hunks.len())
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn compare_status_for_tests(&self) -> String {
        self.compare
            .try_borrow()
            .ok()
            .and_then(|compare| {
                compare
                    .as_ref()
                    .map(|compare| compare.status_label.text().to_string())
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn compare_current_hunk_for_tests(&self) -> Option<usize> {
        self.compare
            .try_borrow()
            .ok()
            .and_then(|compare| compare.as_ref().and_then(|compare| compare.current_hunk))
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
        compare.apply_tag_colors(self.settings.editor_palette_is_dark());
        self.compare.borrow_mut().replace(compare);
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
                let mut compare_state = self.compare.borrow_mut();
                if let Some(compare) = compare_state.as_mut() {
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
        if let Some(compare) = self.compare.borrow_mut().as_mut() {
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
                if tab.compare_request_generation.get() != generation {
                    return;
                }
                let outcome = match result {
                    Ok(document) => {
                        let editable_text = tab.buffer_text();
                        let applied = {
                            let mut compare_state = tab.compare.borrow_mut();
                            if let Some(compare) = compare_state.as_mut()
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
        if let Some(compare) = self.compare.borrow_mut().as_mut() {
            compare.move_hunk(&self.text_buffer, &self.text_view, direction);
        }
    }

    fn bump_compare_generation(&self) -> u64 {
        let next = self.compare_request_generation.get().saturating_add(1);
        self.compare_request_generation.set(next);
        next
    }
}

impl CompareController {
    fn new(tab: &Rc<EditorTab>, target: CompareTarget) -> Self {
        let reference_buffer = sourceview5::Buffer::builder()
            .enable_undo(false)
            .implicit_trailing_newline(false)
            .build();
        tab.settings.apply_source_style_scheme(&reference_buffer);
        let reference_view = sourceview5::View::with_buffer(&reference_buffer);
        configure_reference_view(tab, &reference_view);
        let reference_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&reference_view)
            .build();
        reference_scrolled.set_hexpand(true);
        reference_scrolled.set_vexpand(true);
        reference_scrolled.set_min_content_width(0);

        let toolbar = compare_toolbar(&target.title);
        let status_label = toolbar
            .last_child()
            .and_downcast::<gtk4::Label>()
            .unwrap_or_else(|| gtk4::Label::new(None));
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);
        paned.set_resize_end_child(true);
        paned.set_shrink_end_child(false);
        paned.set_end_child(Some(&reference_scrolled));
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        let left_adjustment = tab.scrolled.vadjustment();
        let right_adjustment = reference_scrolled.vadjustment();
        let (left_handler, right_handler) = install_scroll_sync(
            &left_adjustment,
            &right_adjustment,
            &tab.text_view,
            &reference_view,
        );
        let style_manager = adw::StyleManager::default();
        let weak = Rc::downgrade(tab);
        let style_handler = style_manager.connect_dark_notify(move |_| {
            if let Some(tab) = weak.upgrade()
                && let Some(compare) = tab.compare.borrow_mut().as_mut()
            {
                compare.apply_tag_colors(tab.settings.editor_palette_is_dark());
                compare.apply_current_hunk(&tab.text_buffer);
            }
        });

        Self {
            target,
            toolbar,
            status_label,
            paned,
            reference_view,
            reference_buffer: reference_buffer.clone(),
            tags: CompareTags::new(&tab.text_buffer, &reference_buffer),
            diff_plan: DiffPlan::empty(),
            current_hunk: None,
            cancellable: None,
            left_adjustment,
            right_adjustment,
            left_handler: Some(left_handler),
            right_handler: Some(right_handler),
            style_manager,
            style_handler: Some(style_handler),
        }
    }

    fn set_loading(&mut self, cancellable: &gio::Cancellable) {
        if let Some(previous) = self.cancellable.take() {
            previous.cancel();
        }
        self.cancellable = Some(cancellable.clone());
        self.status_label
            .set_label(&pgettext("compare status", "Loading Reference..."));
    }

    fn finish_loading(&mut self) {
        self.cancellable = None;
    }

    fn cancel(&mut self) {
        if let Some(cancellable) = self.cancellable.take() {
            cancellable.cancel();
        }
    }

    fn set_reference_text(&self, text: &str, implicit_trailing_newline: bool) {
        self.reference_buffer
            .set_implicit_trailing_newline(implicit_trailing_newline);
        self.reference_buffer.set_text(text);
        self.reference_buffer.set_modified(false);
    }

    fn recompute(
        &mut self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
        editable_text: &str,
    ) {
        clear_tags(editable_buffer, &self.reference_buffer, &self.tags);
        let reference_text = buffer_text(&self.reference_buffer);
        let previous = self.current_hunk;
        self.diff_plan = compute_diff_plan(editable_text, &reference_text);
        self.current_hunk = if self.diff_plan.hunks.is_empty() || self.diff_plan.too_large {
            None
        } else {
            Some(previous.unwrap_or(0).min(self.diff_plan.hunks.len() - 1))
        };
        apply_diff_tags(
            editable_buffer,
            &self.reference_buffer,
            &self.diff_plan,
            &self.tags,
        );
        self.apply_current_hunk(editable_buffer);
        self.update_status();
        if self.current_hunk == Some(0) {
            self.scroll_current_hunk(editable_buffer, editable_view);
        }
    }

    fn move_hunk(
        &mut self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
        direction: i32,
    ) {
        if self.diff_plan.hunks.is_empty() || self.diff_plan.too_large {
            return;
        }
        let len = self.diff_plan.hunks.len();
        let current = self.current_hunk.unwrap_or(0);
        let next = if direction < 0 {
            current.checked_sub(1).unwrap_or(len - 1)
        } else {
            (current + 1) % len
        };
        self.current_hunk = Some(next);
        self.apply_current_hunk(editable_buffer);
        self.update_status();
        self.scroll_current_hunk(editable_buffer, editable_view);
    }

    fn apply_current_hunk(&self, editable_buffer: &sourceview5::Buffer) {
        remove_current_tags(editable_buffer, &self.reference_buffer, &self.tags);
        let Some(index) = self.current_hunk else {
            return;
        };
        let Some(hunk) = self.diff_plan.hunks.get(index) else {
            return;
        };
        for line in &hunk.editable_lines {
            apply_line_tag(editable_buffer, *line, &self.tags.editable_current);
        }
        for line in &hunk.reference_lines {
            apply_line_tag(&self.reference_buffer, *line, &self.tags.reference_current);
        }
    }

    fn scroll_current_hunk(
        &self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
    ) {
        let Some(index) = self.current_hunk else {
            return;
        };
        let Some(hunk) = self.diff_plan.hunks.get(index) else {
            return;
        };
        if let Some(line) = hunk.editable_lines.first() {
            scroll_to_line(editable_buffer, editable_view, *line);
        }
        if let Some(line) = hunk.reference_lines.first() {
            scroll_to_line(&self.reference_buffer, &self.reference_view, *line);
        }
    }

    fn update_status(&self) {
        if self.diff_plan.too_large {
            self.status_label
                .set_label(&gettext("Too large to compare differences."));
            return;
        }
        let hunk_count = self.diff_plan.hunks.len();
        if hunk_count == 0 {
            self.status_label
                .set_label(&gettext("No differences were found."));
            return;
        }
        let changed_lines = self.diff_plan.changed_line_count();
        let plural_count = u32::try_from(changed_lines).map_or(u32::MAX, |value| value);
        let text = ngettext("%d changed line", "%d changed lines", plural_count)
            .replace("%d", &changed_lines.to_string());
        if let Some(current) = self.current_hunk {
            self.status_label
                .set_label(&format!("{} · {}/{}", text, current + 1, hunk_count));
        } else {
            self.status_label.set_label(&text);
        }
    }

    fn apply_tag_colors(&self, dark: bool) {
        self.tags.apply_colors(dark);
    }
}

impl Drop for CompareController {
    fn drop(&mut self) {
        self.cancel();
        if let Some(handler) = self.left_handler.take() {
            self.left_adjustment.disconnect(handler);
        }
        if let Some(handler) = self.right_handler.take() {
            self.right_adjustment.disconnect(handler);
        }
        if let Some(handler) = self.style_handler.take() {
            self.style_manager.disconnect(handler);
        }
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
