use std::path::{Path, PathBuf};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::gio;
use libadwaita as adw;

use crate::dialogs::{self, InvalidCharsSaveResponse, StaleSaveResponse};
use crate::document::DocumentState;
use crate::editor_format::{EncodingInfo, SavedTextFormat};
use crate::editor_io::{self, SaveFailure, SavedDocument};
use crate::editor_tab::{EditorTab, SaveKind, SaveOutcome, SaveResult};
use crate::error::AppError;

#[derive(Clone)]
struct SaveRetryContext {
    parent: adw::ApplicationWindow,
    path: PathBuf,
    flags: sourceview5::FileSaverFlags,
    allow_stale_prompt: bool,
    save_kind: SaveKind,
    callback: Rc<dyn Fn(SaveResult)>,
}

impl EditorTab {
    pub fn request_save(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        force_save_as: bool,
        save_kind: SaveKind,
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        if !self.is_document() {
            callback(SaveResult::CancelledByUser);
            return;
        }
        let current_path = self.state.borrow().document.document.path();
        if !force_save_as && let Some(path) = current_path {
            if self.should_show_stale_save_conflict() {
                if save_kind == SaveKind::Autosave {
                    self.pause_autosave(gettext(
                        "Autosave paused because the file changed on disk.",
                    ));
                    callback(SaveResult::Failed(AppError::WriteFailed(
                        path,
                        gettext("The file changed on disk."),
                    )));
                    return;
                }
                let prompt_parent = parent.clone();
                let save_parent = parent.clone();
                let weak = Rc::downgrade(self);
                dialogs::confirm_stale_save(&prompt_parent, &self.title(), move |choice| {
                    if let Some(tab) = weak.upgrade() {
                        match choice {
                            StaleSaveResponse::SaveAnyway => {
                                tab.save_to_path(
                                    &save_parent,
                                    &path,
                                    sourceview5::FileSaverFlags::IGNORE_MODIFICATION_TIME,
                                    false,
                                    SaveKind::Manual,
                                    callback.clone(),
                                );
                            }
                            StaleSaveResponse::Compare => {
                                tab.start_compare_with_disk(callback_for_compare(callback.clone()));
                            }
                            StaleSaveResponse::Cancel => {
                                callback(SaveResult::CancelledByUser);
                            }
                        }
                    }
                });
                return;
            }
            self.save_to_path(
                parent,
                &path,
                sourceview5::FileSaverFlags::NONE,
                true,
                save_kind,
                callback,
            );
            return;
        }
        if save_kind == SaveKind::Autosave {
            callback(SaveResult::CancelledByUser);
            return;
        }
        self.show_save_dialog(parent, callback);
    }

    fn show_save_dialog(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Save the Document"))
            .accept_label(pgettext("file dialog action", "Save"))
            .modal(true)
            .build();
        dialog.set_initial_name(Some(&self.save_name_suggestion()));
        if let Some(path) = self.state.borrow().document.document.path() {
            dialog.set_initial_file(Some(&gio::File::for_path(path)));
        }
        super::apply_text_filters(&dialog);

        let weak = Rc::downgrade(self);
        let dialog_parent = parent.clone();
        let save_parent = parent.clone();
        dialog.save(
            Some(&dialog_parent),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(tab) = weak.upgrade() {
                    match result {
                        Ok(file) => match editor_io::local_path(&file) {
                            Ok(path) => {
                                let normalized = DocumentState::normalized_save_path(&path);
                                tab.save_to_path(
                                    &save_parent,
                                    &normalized,
                                    sourceview5::FileSaverFlags::NONE,
                                    true,
                                    SaveKind::Manual,
                                    callback.clone(),
                                );
                            }
                            Err(error) => callback(SaveResult::Failed(error)),
                        },
                        Err(error) if error.matches(gtk4::DialogError::Dismissed) => {
                            callback(SaveResult::CancelledByUser);
                        }
                        Err(error) => callback(SaveResult::Failed(AppError::from(error))),
                    }
                }
            },
        );
    }

    fn save_to_path(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        path: &Path,
        flags: sourceview5::FileSaverFlags,
        allow_stale_prompt: bool,
        save_kind: SaveKind,
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        let old_uri = self.document_uri();
        let previous_file = self.saved_file();
        self.clear_monitor();
        self.set_loading(true);
        let save_format = self.current_format();
        let dialog_format = save_format.clone();
        let live_source_file = self.source_file();
        let parent_window = parent.clone();
        let (generation, cancellable) = self.start_io_request(None);
        let weak = Rc::downgrade(self);
        let save_path = path.to_path_buf();
        let retry_context = SaveRetryContext {
            parent: parent_window.clone(),
            path: save_path.clone(),
            flags,
            allow_stale_prompt,
            save_kind,
            callback: callback.clone(),
        };
        editor_io::save_text_file(
            live_source_file.as_ref(),
            path,
            &self.text_buffer,
            &save_format,
            flags,
            Some(&cancellable),
            Rc::new(move |result| {
                if let Some(tab) = weak.upgrade() {
                    if !tab.finish_io_request(generation) {
                        return;
                    }
                    match result {
                        Ok(saved) => {
                            tab.finish_successful_save(saved, old_uri.clone(), &callback);
                        }
                        Err(SaveFailure::InvalidChars) => {
                            if retry_context.save_kind == SaveKind::Autosave {
                                tab.pause_autosave(gettext(
                                    "Autosave paused because the document contains characters that cannot be saved with the selected encoding.",
                                ));
                                tab.restore_after_failed_save(previous_file.as_ref());
                                callback(SaveResult::Failed(AppError::WriteFailed(
                                    save_path.clone(),
                                    gettext(
                                        "The document contains characters that cannot be saved with the selected encoding.",
                                    ),
                                )));
                            } else {
                                tab.handle_invalid_chars_save_failure(
                                    previous_file.as_ref(),
                                    &dialog_format,
                                    &retry_context,
                                );
                            }
                        }
                        Err(SaveFailure::ExternallyModified)
                            if retry_context.save_kind == SaveKind::Autosave =>
                        {
                            tab.pause_autosave(gettext(
                                "Autosave paused because the file changed on disk.",
                            ));
                            tab.restore_after_failed_save(previous_file.as_ref());
                            callback(SaveResult::Failed(AppError::WriteFailed(
                                save_path.clone(),
                                gettext("The file changed on disk."),
                            )));
                        }
                        Err(SaveFailure::ExternallyModified) if allow_stale_prompt => {
                            tab.handle_stale_save_failure(previous_file.as_ref(), &retry_context);
                        }
                        Err(SaveFailure::ExternallyModified) => {
                            tab.restore_after_failed_save(previous_file.as_ref());
                            callback(SaveResult::Failed(AppError::WriteFailed(
                                save_path.clone(),
                                gettext(
                                    "The file changed again on disk and could not be overwritten.",
                                ),
                            )));
                        }
                        Err(SaveFailure::Failed(error)) => {
                            if retry_context.save_kind == SaveKind::Autosave {
                                tab.pause_autosave(gettext(
                                    "Autosave paused because the file could not be written.",
                                ));
                            }
                            tab.restore_after_failed_save(previous_file.as_ref());
                            callback(SaveResult::Failed(error));
                        }
                    }
                }
            }),
        );
    }

    fn restore_after_failed_save(self: &Rc<Self>, previous_file: Option<&gio::File>) {
        if let Some(previous_file) = previous_file {
            self.swap_monitor(previous_file);
        }
        self.set_loading(false);
        self.sync_presentation();
    }

    fn finish_successful_save(
        self: &Rc<Self>,
        saved: SavedDocument,
        old_uri: Option<String>,
        callback: &Rc<dyn Fn(SaveResult)>,
    ) {
        let monitored_file = gio::File::for_path(&saved.path);
        let new_uri = saved.uri.clone();
        self.apply_saved_document(saved);
        self.resolve_pending_external();
        self.swap_monitor(&monitored_file);
        self.refresh_language_for_file(&monitored_file);
        self.set_loading(false);
        self.sync_presentation();
        self.grab_focus();
        callback(SaveResult::Saved(SaveOutcome { old_uri, new_uri }));
    }

    fn handle_invalid_chars_save_failure(
        self: &Rc<Self>,
        previous_file: Option<&gio::File>,
        dialog_format: &SavedTextFormat,
        retry_context: &SaveRetryContext,
    ) {
        self.restore_after_failed_save(previous_file);
        let weak = Rc::downgrade(self);
        let retry_context = retry_context.clone();
        let prompt_parent = retry_context.parent.clone();
        let encoding_name = dialog_format.encoding().display_name().to_string();
        dialogs::confirm_invalid_chars_save(&prompt_parent, &encoding_name, move |response| {
            if let Some(tab) = weak.upgrade() {
                match response {
                    InvalidCharsSaveResponse::ChooseEncoding => {
                        tab.choose_save_encoding(&retry_context);
                    }
                    InvalidCharsSaveResponse::Cancel => {
                        (retry_context.callback)(SaveResult::CancelledByUser);
                    }
                }
            }
        });
    }

    fn choose_save_encoding(self: &Rc<Self>, retry_context: &SaveRetryContext) {
        let candidates = sourceview5::Encoding::default_candidates();
        let current = self.current_format().encoding().to_source_encoding();
        let title = gettext("Choose a Text Encoding");
        let body = gettext("Choose the encoding to use when saving this document.");
        let weak = Rc::downgrade(self);
        let retry_context = retry_context.clone();
        let choose_parent = retry_context.parent.clone();
        dialogs::choose_encoding(
            &choose_parent,
            &title,
            &body,
            &candidates,
            current.as_ref(),
            &pgettext("dialog button", "Save"),
            move |selection| {
                if let Some(tab) = weak.upgrade() {
                    let Some(encoding) = selection else {
                        (retry_context.callback)(SaveResult::CancelledByUser);
                        return;
                    };
                    tab.set_current_encoding(EncodingInfo::from_encoding(&encoding));
                    tab.save_to_path(
                        &retry_context.parent,
                        &retry_context.path,
                        retry_context.flags,
                        retry_context.allow_stale_prompt,
                        retry_context.save_kind,
                        retry_context.callback.clone(),
                    );
                }
            },
        );
    }

    fn handle_stale_save_failure(
        self: &Rc<Self>,
        previous_file: Option<&gio::File>,
        retry_context: &SaveRetryContext,
    ) {
        self.restore_after_failed_save(previous_file);
        let weak = Rc::downgrade(self);
        let retry_context = retry_context.clone();
        let prompt_parent = retry_context.parent.clone();
        dialogs::confirm_stale_save(&prompt_parent, &self.title(), move |choice| {
            if let Some(tab) = weak.upgrade() {
                match choice {
                    StaleSaveResponse::SaveAnyway => tab.save_to_path(
                        &retry_context.parent,
                        &retry_context.path,
                        sourceview5::FileSaverFlags::IGNORE_MODIFICATION_TIME,
                        false,
                        retry_context.save_kind,
                        retry_context.callback.clone(),
                    ),
                    StaleSaveResponse::Compare => {
                        tab.start_compare_with_disk(callback_for_compare(
                            retry_context.callback.clone(),
                        ));
                    }
                    StaleSaveResponse::Cancel => {
                        (retry_context.callback)(SaveResult::CancelledByUser);
                    }
                }
            }
        });
    }
}

fn callback_for_compare(callback: Rc<dyn Fn(SaveResult)>) -> Rc<dyn Fn(Result<(), AppError>)> {
    Rc::new(move |result| match result {
        Ok(()) => callback(SaveResult::CancelledByUser),
        Err(error) => callback(SaveResult::Failed(error)),
    })
}
