use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use sourceview5::prelude::*;

use crate::document::DocumentState;
use crate::document_limits::{
    EDITOR_HARD_LIMIT_BYTES, FileTier, LARGE_FILE_LIMIT_BYTES, OpenDecision,
};
use crate::editor_format::SavedTextFormat;
use crate::editor_io;
use crate::error::AppError;
use crate::large_file::viewer::LargeFileViewer;

use super::{DocumentSurface, EditorTab};

type OpenCallback = Rc<dyn Fn(Result<String, AppError>)>;

impl EditorTab {
    #[must_use]
    pub(crate) fn is_editor_surface(&self) -> bool {
        self.state.borrow().large_file.surface == DocumentSurface::Editor
    }

    #[must_use]
    pub(crate) fn can_save_document(&self) -> bool {
        self.is_document() && !self.is_loading()
    }

    #[must_use]
    pub(crate) fn editor_heavy_features_enabled(&self) -> bool {
        self.state
            .borrow()
            .large_file
            .file_size
            .is_none_or(|size| size < self.settings.large_file_thresholds().full_feature)
    }

    pub(crate) fn open_viewer_for_large_file(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        size: u64,
        decision: OpenDecision,
        callback: OpenCallback,
    ) {
        let OpenDecision::Viewer { tier, edit_allowed } = decision else {
            self.load_file(parent, file, callback);
            return;
        };
        self.show_large_file_viewer(parent, file, size, tier, edit_allowed, &callback);
    }

    pub(crate) fn show_large_file_restore_placeholder(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        size: u64,
        decision: OpenDecision,
        on_remove: Rc<dyn Fn()>,
        callback: OpenCallback,
    ) {
        let OpenDecision::Viewer { tier, edit_allowed } = decision else {
            self.load_file(parent, file, callback);
            return;
        };
        let uri =
            match self.set_large_file_identity(file, size, DocumentSurface::RestorePlaceholder) {
                Ok(uri) => uri,
                Err(error) => {
                    callback(Err(error));
                    return;
                }
            };
        let box_ = build_placeholder_box(
            size,
            tier,
            edit_allowed,
            Rc::new({
                let weak = Rc::downgrade(self);
                let parent = parent.clone();
                let file = file.clone();
                move || {
                    if let Some(tab) = weak.upgrade() {
                        let callback: OpenCallback = Rc::new(|_result| {});
                        tab.show_large_file_viewer(
                            &parent,
                            &file,
                            size,
                            tier,
                            edit_allowed,
                            &callback,
                        );
                    }
                }
            }),
            Rc::new({
                let weak = Rc::downgrade(self);
                let parent = parent.clone();
                let file = file.clone();
                move || {
                    if let Some(tab) = weak.upgrade() {
                        tab.confirm_and_open_editor(&parent, &file, size, edit_allowed);
                    }
                }
            }),
            on_remove,
        );
        self.replace_large_file_widget(&box_.upcast(), None);
        self.sync_presentation();
        callback(Ok(uri));
    }

    fn show_large_file_viewer(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        size: u64,
        tier: FileTier,
        edit_allowed: bool,
        callback: &OpenCallback,
    ) {
        let uri = match self.set_large_file_identity(file, size, DocumentSurface::LargeFileViewer) {
            Ok(uri) => uri,
            Err(error) => {
                callback(Err(error));
                return;
            }
        };
        let edit_warning = edit_warning_for_size(size, tier, edit_allowed);
        let viewer = LargeFileViewer::new(
            file,
            size,
            edit_allowed,
            &edit_warning,
            Rc::new({
                let weak = Rc::downgrade(self);
                let parent = parent.clone();
                let file = file.clone();
                move || {
                    if let Some(tab) = weak.upgrade() {
                        tab.confirm_and_open_editor(&parent, &file, size, edit_allowed);
                    }
                }
            }),
        );
        self.replace_large_file_widget(&viewer.widget(), Some(viewer));
        self.sync_presentation();
        callback(Ok(uri));
    }

    fn confirm_and_open_editor(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        size: u64,
        edit_allowed: bool,
    ) {
        if !edit_allowed {
            crate::dialogs::present_message(
                parent,
                &gettext("Editing Is Not Available"),
                &gettext("This file is above Riteed's measured safe editing limit."),
            );
            return;
        }
        if self.settings.always_allow_large_file_edit() {
            self.open_large_file_in_editor(parent, file);
            return;
        }
        let body = edit_confirmation_body(size);
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Edit the Large File Anyway?"))
            .body(body)
            .build();
        dialog.add_responses(&[
            ("cancel", &pgettext("alert response", "Cancel")),
            ("edit", &pgettext("alert response", "Edit Anyway")),
        ]);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.set_response_appearance("edit", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        let expected_page = self.page();
        let file = file.clone();
        let parent = parent.clone();
        dialog.choose(
            Some(&parent.clone()),
            None::<&gio::Cancellable>,
            move |response| {
                if response != "edit" {
                    return;
                }
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                if !tab.large_file_prompt_matches(expected_page.as_ref()) {
                    return;
                }
                tab.open_large_file_in_editor(&parent, &file);
            },
        );
    }

    fn large_file_prompt_matches(&self, expected_page: Option<&adw::TabPage>) -> bool {
        let Some(expected_page) = expected_page else {
            return false;
        };
        self.page()
            .as_ref()
            .is_some_and(|page| page == expected_page)
            && !self.is_editor_surface()
    }

    fn open_large_file_in_editor(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
    ) {
        let previous_surface = self.prepare_large_file_editor_load();
        let weak = Rc::downgrade(self);
        self.load_file(
            parent,
            file,
            Rc::new({
                let parent = parent.clone();
                move |result| {
                    if let Err(error) = result {
                        if let Some(tab) = weak.upgrade() {
                            tab.restore_large_file_surface_after_failed_editor_load(
                                previous_surface,
                            );
                        }
                        crate::dialogs::present_error(&parent, &error);
                    }
                }
            }),
        );
    }

    fn prepare_large_file_editor_load(&self) -> DocumentSurface {
        let mut state = self.state.borrow_mut();
        let previous_surface = state.large_file.surface;
        state.large_file.surface = DocumentSurface::Editor;
        previous_surface
    }

    fn restore_large_file_surface_after_failed_editor_load(
        &self,
        previous_surface: DocumentSurface,
    ) {
        {
            let mut state = self.state.borrow_mut();
            if state.large_file.widget.is_none() {
                return;
            }
            state.large_file.surface = previous_surface;
        }
        self.content.set_visible(false);
        self.sync_presentation();
    }

    pub(crate) fn reapply_large_file_feature_gates(self: &Rc<Self>) {
        if !self.is_document() || !self.is_editor_surface() {
            return;
        }
        if self.editor_heavy_features_enabled() {
            if let Some(file) = self.saved_file() {
                self.refresh_language_for_file(&file);
            }
        } else {
            self.state.borrow_mut().document.language_id = None;
            self.text_buffer
                .set_language(Option::<&sourceview5::Language>::None);
            self.clear_source_control_minimap_diff();
        }
        self.apply_minimap_visibility();
        self.sync_markdown_preview_availability();
        self.sync_presentation();
    }

    fn set_large_file_identity(
        &self,
        file: &gio::File,
        size: u64,
        surface: DocumentSurface,
    ) -> Result<String, AppError> {
        let path_info = editor_io::local_path_info(file)?;
        let format = SavedTextFormat::new_document_defaults();
        {
            let mut state = self.state.borrow_mut();
            state.document.document = DocumentState::from_loaded_with_display_path(
                path_info.path,
                path_info.display_path,
                format.clone(),
            );
            state.document.saved_format = format;
            state.document.source_file = None;
            state.document.content_type = None;
            state.document.language_id = None;
            state.large_file.surface = surface;
            state.large_file.file_size = Some(size);
            state.io.loading = false;
            state.autosave.paused_message = None;
        }
        self.text_buffer.set_text("");
        self.text_buffer.set_modified(false);
        self.content.set_visible(false);
        Ok(file.uri().to_string())
    }

    fn replace_large_file_widget(
        &self,
        widget: &gtk4::Widget,
        viewer: Option<Rc<LargeFileViewer>>,
    ) {
        let old_widget = {
            let mut state = self.state.borrow_mut();
            if let Some(viewer) = state.large_file.viewer.take() {
                viewer.cancel();
            }
            let old_widget = state.large_file.widget.take();
            state.large_file.widget = Some(widget.clone());
            state.large_file.viewer = viewer;
            old_widget
        };
        if let Some(old_widget) = old_widget {
            self.root.remove(&old_widget);
        }
        self.root.append(widget);
    }

    pub(crate) fn clear_large_file_surface(&self) {
        let widget = self.state.borrow_mut().large_file.clear_surface();
        if let Some(widget) = widget {
            self.root.remove(&widget);
        }
    }

    #[cfg(test)]
    pub(crate) fn large_file_surface_for_tests(&self) -> &'static str {
        match self.state.borrow().large_file.surface {
            DocumentSurface::Editor => "editor",
            DocumentSurface::LargeFileViewer => "viewer",
            DocumentSurface::RestorePlaceholder => "restore-placeholder",
        }
    }

    #[cfg(test)]
    pub(crate) fn large_file_viewer_text_for_tests(&self) -> String {
        self.state
            .borrow()
            .large_file
            .viewer
            .as_ref()
            .map_or_else(String::new, |viewer| viewer.text_for_tests())
    }

    #[cfg(test)]
    pub(crate) fn large_file_viewer_status_for_tests(&self) -> String {
        self.state
            .borrow()
            .large_file
            .viewer
            .as_ref()
            .map_or_else(String::new, |viewer| viewer.status_for_tests())
    }

    #[cfg(test)]
    pub(crate) fn activate_large_file_edit_for_tests(&self) -> bool {
        let viewer = self.state.borrow().large_file.viewer.clone();
        viewer.is_some_and(|viewer| viewer.activate_edit_for_tests())
    }

    #[cfg(test)]
    pub(crate) fn activate_large_file_refresh_for_tests(&self) -> bool {
        let viewer = self.state.borrow().large_file.viewer.clone();
        if let Some(viewer) = viewer {
            viewer.activate_refresh_for_tests();
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(crate) fn activate_large_file_placeholder_remove_for_tests(&self) -> bool {
        if self.state.borrow().large_file.surface != DocumentSurface::RestorePlaceholder {
            return false;
        }
        let widget = self.state.borrow().large_file.widget.clone();
        widget.is_some_and(|widget| activate_button_with_label(&widget, "Remove"))
    }
}

#[cfg(test)]
fn activate_button_with_label(widget: &gtk4::Widget, label: &str) -> bool {
    if let Ok(button) = widget.clone().downcast::<gtk4::Button>()
        && button.label().as_deref() == Some(label)
    {
        button.emit_clicked();
        return true;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if activate_button_with_label(&current, label) {
            return true;
        }
        child = current.next_sibling();
    }
    false
}

fn build_placeholder_box(
    size: u64,
    tier: FileTier,
    edit_allowed: bool,
    on_viewer: Rc<dyn Fn()>,
    on_editor: Rc<dyn Fn()>,
    on_remove: Rc<dyn Fn()>,
) -> gtk4::Box {
    let heading = gtk4::Label::new(Some(&gettext("Large file restored as a placeholder.")));
    heading.add_css_class("title-3");
    heading.set_xalign(0.0);

    let body = gtk4::Label::new(Some(&placeholder_body(size, tier, edit_allowed)));
    body.set_wrap(true);
    body.set_xalign(0.0);

    let viewer_button = gtk4::Button::with_label(&pgettext("placeholder button", "Open in Viewer"));
    let editor_button = gtk4::Button::with_label(&pgettext("placeholder button", "Open in Editor"));
    editor_button.set_sensitive(edit_allowed);
    let remove_button = gtk4::Button::with_label(&pgettext("placeholder button", "Remove"));

    viewer_button.connect_clicked(move |_| on_viewer());
    editor_button.connect_clicked(move |_| on_editor());
    remove_button.connect_clicked(move |_| on_remove());

    let actions = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .build();
    actions.append(&viewer_button);
    actions.append(&editor_button);
    actions.append(&remove_button);

    let box_ = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .margin_start(24)
        .margin_end(24)
        .margin_top(24)
        .margin_bottom(24)
        .build();
    box_.set_hexpand(true);
    box_.set_vexpand(true);
    box_.append(&heading);
    box_.append(&body);
    box_.append(&actions);
    box_
}

fn placeholder_body(size: u64, tier: FileTier, edit_allowed: bool) -> String {
    let base =
        gettext("Riteed did not load this file during startup, so the session stays responsive.");
    let tier_copy = edit_warning_for_size(size, tier, edit_allowed);
    format!("{base}\n\n{tier_copy}")
}

fn edit_warning_for_size(size: u64, tier: FileTier, edit_allowed: bool) -> String {
    if !edit_allowed {
        return gettext(
            "Viewing remains available, but editing is refused above the measured safe editing limit.",
        );
    }
    let disabled = gettext(
        "Syntax highlighting, minimap, Markdown preview, Source Control diff, and autosave remain disabled.",
    );
    if matches!(tier, FileTier::VeryLarge) || size >= LARGE_FILE_LIMIT_BYTES {
        format!(
            "{}\n\n{}",
            gettext(
                "Editing this file may take 30 seconds or more and can significantly slow the app."
            ),
            disabled
        )
    } else {
        format!(
            "{}\n\n{}",
            gettext("Editing this file may take 5-15 seconds."),
            disabled
        )
    }
}

fn edit_confirmation_body(size: u64) -> String {
    let template = gettext(
        "This file is %1$s bytes. Loading it into the editor can freeze the window temporarily and may use several times the file size in memory.",
    );
    let body = template.replace("%1$s", &size.to_string());
    let cap = gettext("The current measured safe editing limit is %1$s bytes.");
    format!(
        "{}\n\n{}\n\n{}",
        body,
        cap.replace("%1$s", &EDITOR_HARD_LIMIT_BYTES.to_string()),
        gettext(
            "Syntax highlighting, minimap, Markdown preview, Source Control diff, and autosave will stay disabled."
        )
    )
}

#[cfg(test)]
mod tests {
    use super::{edit_confirmation_body, edit_warning_for_size, placeholder_body};
    use crate::document_limits::{EDITOR_HARD_LIMIT_BYTES, FileTier, LARGE_FILE_LIMIT_BYTES};

    #[test]
    fn edit_warning_refuses_above_measured_edit_cap() {
        let warning = edit_warning_for_size(
            EDITOR_HARD_LIMIT_BYTES.saturating_add(1),
            FileTier::Large,
            false,
        );

        assert!(warning.contains("editing is refused"));
        assert!(warning.contains("Viewing remains available"));
    }

    #[test]
    fn edit_warning_refusal_takes_precedence_over_size_tier() {
        let warning = edit_warning_for_size(0, FileTier::Small, false);

        assert!(warning.contains("editing is refused"));
        assert!(!warning.contains("5-15 seconds"));
        assert!(!warning.contains("30 seconds"));
    }

    #[test]
    fn edit_warning_uses_stronger_copy_for_very_large_files() {
        let medium_warning = edit_warning_for_size(EDITOR_HARD_LIMIT_BYTES, FileTier::Large, true);
        let strong_warning =
            edit_warning_for_size(LARGE_FILE_LIMIT_BYTES, FileTier::VeryLarge, true);

        assert!(medium_warning.contains("5-15 seconds"));
        assert!(strong_warning.contains("30 seconds or more"));
        assert!(strong_warning.contains("autosave remain disabled"));
    }

    #[test]
    fn placeholder_body_combines_startup_and_tier_copy() {
        let body = placeholder_body(EDITOR_HARD_LIMIT_BYTES, FileTier::Large, true);

        assert!(body.contains("during startup"));
        assert!(body.contains("5-15 seconds"));
    }

    #[test]
    fn placeholder_body_includes_refusal_copy_for_viewer_only_files() {
        let body = placeholder_body(EDITOR_HARD_LIMIT_BYTES + 1, FileTier::ViewerOnly, false);

        assert!(body.contains("during startup"));
        assert!(body.contains("editing is refused"));
    }

    #[test]
    fn edit_confirmation_body_names_size_cap_and_disabled_features() {
        let body = edit_confirmation_body(123);

        assert!(body.contains("123 bytes"));
        assert!(body.contains(&EDITOR_HARD_LIMIT_BYTES.to_string()));
        assert!(body.contains("autosave will stay disabled"));
    }
}
