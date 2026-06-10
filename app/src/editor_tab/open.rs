use std::path::Path;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib::SList};
use libadwaita as adw;

use crate::dialogs::{self, DecodeFailureResponse, ReopenWithEncodingResponse};
use crate::document_limits::{OpenDecision, OpenFileSupport};
use crate::editor_io::{self, LoadFailure, LoadedDocument};
use crate::editor_tab::{EditorTab, ReloadCause, ReloadResult};
use crate::editor_view::ReloadSnapshot;
use crate::error::AppError;

impl EditorTab {
    pub fn load_file(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        callback: Rc<dyn Fn(Result<String, AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::Cancelled));
            return;
        }
        self.load_file_with_candidates(parent, file, None, None, callback);
    }

    pub(crate) fn load_file_with_open_support(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        open_support: OpenFileSupport,
        callback: Rc<dyn Fn(Result<String, AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::Cancelled));
            return;
        }
        self.load_file_with_candidates(parent, file, None, Some(open_support), callback);
    }

    pub fn reload_from_disk(
        self: &Rc<Self>,
        cause: ReloadCause,
        should_apply: Rc<dyn Fn() -> bool>,
        callback: Rc<dyn Fn(Result<ReloadResult, AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::MissingSavePath));
            return;
        }
        let Some(saved_file) = self.saved_file() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        let Some(expected_uri) = self.document_uri() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        if let Err(error) = editor_io::validate_text_file_open(&saved_file) {
            callback(Err(map_load_failure_to_app_error(error)));
            return;
        }
        let snapshot = ReloadSnapshot::capture(&self.text_buffer);
        let already_loading = {
            let mut state = self.state.borrow_mut();
            if state.io.loading || state.io.external_reload_in_progress {
                true
            } else {
                state.io.loading = true;
                state.io.external_reload_in_progress = true;
                false
            }
        };
        if already_loading {
            callback(Ok(ReloadResult::Deferred));
            return;
        }
        if let Some(page) = self.page() {
            page.set_loading(true);
        }

        let (generation, cancellable) = self.start_io_request(None);
        let weak = Rc::downgrade(self);
        self.with_io_candidate_encodings(|encodings| {
            editor_io::load_text_file(
                &saved_file,
                encodings,
                Some(&cancellable),
                Rc::new(move |result| {
                    let Some(tab) = weak.upgrade() else {
                        callback(Err(AppError::Cancelled));
                        return;
                    };
                    if !tab.finish_io_request(generation) {
                        callback(Err(AppError::Cancelled));
                        return;
                    }
                    match result {
                        Ok(document) => {
                            if !tab.can_apply_reload(cause, &expected_uri, &should_apply) {
                                tab.finish_reload(false);
                                callback(Ok(ReloadResult::Deferred));
                                return;
                            }
                            let monitored_file = gio::File::for_path(&document.path);
                            let callback = callback.clone();
                            let cancelled_callback = callback.clone();
                            tab.apply_loaded_document_async(
                                document,
                                Some(snapshot.clone()),
                                Rc::new(move |tab| {
                                    tab.swap_monitor(&monitored_file);
                                    tab.refresh_language_for_file(&monitored_file);
                                    tab.finish_reload(true);
                                    callback(Ok(ReloadResult::Applied));
                                }),
                                Rc::new(move || {
                                    cancelled_callback(Err(AppError::Cancelled));
                                }),
                            );
                        }
                        Err(LoadFailure::DecodeFailed(path)) => {
                            tab.finish_reload(false);
                            callback(Err(AppError::DecodeFailed(path)));
                        }
                        Err(LoadFailure::TooBig(path)) => {
                            tab.finish_reload(false);
                            callback(Err(AppError::FileTooBig(path)));
                        }
                        Err(LoadFailure::LineTooLong { path, .. }) => {
                            tab.finish_reload(false);
                            callback(Err(AppError::LineTooLong(path)));
                        }
                        Err(LoadFailure::Failed(error)) => {
                            tab.finish_reload(false);
                            callback(Err(error));
                        }
                    }
                }),
            );
        });
    }

    pub fn request_reopen_with_encoding(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        if !self.is_document() {
            callback(Err(AppError::MissingSavePath));
            return;
        }
        if self.state.borrow().ui.external_prompt_active {
            callback(Err(AppError::Cancelled));
            return;
        }
        if self.pending_external_state().is_missing() || self.saved_file().is_none() {
            callback(Err(AppError::MissingSavePath));
            return;
        }

        let candidates = sourceview5::Encoding::default_candidates();
        let current = self.current_format().encoding().to_source_encoding();
        let title = gettext("Choose a Text Encoding");
        let body = gettext("Choose the encoding to use when reopening this document from disk.");
        let dialog_parent = parent.clone();
        let action_parent = parent.clone();
        let weak = Rc::downgrade(self);
        dialogs::choose_encoding(
            &dialog_parent,
            &title,
            &body,
            &candidates,
            current.as_ref(),
            &pgettext("dialog button", "Reopen"),
            move |selection| {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                let Some(encoding) = selection else {
                    callback(Err(AppError::Cancelled));
                    return;
                };
                if tab.is_dirty() {
                    let confirm_parent = action_parent.clone();
                    let weak = Rc::downgrade(&tab);
                    dialogs::confirm_reopen_with_encoding(
                        &confirm_parent.clone(),
                        &tab.title(),
                        move |response| {
                            if let Some(tab) = weak.upgrade() {
                                match response {
                                    ReopenWithEncodingResponse::Reopen => {
                                        tab.reopen_with_encoding(
                                            &confirm_parent,
                                            &encoding,
                                            callback.clone(),
                                        );
                                    }
                                    ReopenWithEncodingResponse::Cancel => {
                                        callback(Err(AppError::Cancelled));
                                    }
                                }
                            }
                        },
                    );
                    return;
                }
                tab.reopen_with_encoding(&action_parent, &encoding, callback.clone());
            },
        );
    }

    fn load_file_with_candidates(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        candidate_encodings: Option<SList<sourceview5::Encoding>>,
        open_support: Option<OpenFileSupport>,
        callback: Rc<dyn Fn(Result<String, AppError>)>,
    ) {
        if let Err(error) = editor_io::validate_text_file_open(file) {
            callback(Err(map_load_failure_to_app_error(error)));
            return;
        }
        self.set_loading(true);
        let start_dirty_generation = self.dirty_generation();
        let (generation, cancellable) = self.start_io_request(candidate_encodings);
        let weak = Rc::downgrade(self);
        let opened_file = file.clone();
        let parent = parent.clone();
        self.with_io_candidate_encodings(|encodings| {
            let load_callback = Rc::new(move |result: Result<LoadedDocument, LoadFailure>| {
                let Some(tab) = weak.upgrade() else {
                    callback(Err(AppError::Cancelled));
                    return;
                };
                if !tab.finish_io_request(generation) {
                    callback(Err(AppError::Cancelled));
                    return;
                }
                match result {
                    Ok(document) => {
                        if tab.dirty_generation() != start_dirty_generation {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(AppError::Cancelled));
                            return;
                        }
                        let monitored_file = gio::File::for_path(&document.path);
                        let uri = document.uri.clone();
                        let callback = callback.clone();
                        let cancelled_callback = callback.clone();
                        tab.apply_loaded_document_async(
                            document,
                            None,
                            Rc::new(move |tab| {
                                tab.swap_monitor(&monitored_file);
                                tab.refresh_language_for_file(&monitored_file);
                                tab.set_loading(false);
                                tab.sync_presentation();
                                tab.grab_focus();
                                callback(Ok(uri.clone()));
                            }),
                            Rc::new(move || cancelled_callback(Err(AppError::Cancelled))),
                        );
                    }
                    Err(LoadFailure::DecodeFailed(path)) => {
                        tab.set_loading(false);
                        tab.sync_presentation();
                        tab.offer_open_with_manual_encoding(
                            &parent,
                            &opened_file,
                            &path,
                            callback.clone(),
                        );
                    }
                    Err(LoadFailure::TooBig(path)) => {
                        tab.set_loading(false);
                        tab.sync_presentation();
                        callback(Err(AppError::FileTooBig(path)));
                    }
                    Err(LoadFailure::LineTooLong { path: _, size }) => {
                        tab.set_loading(false);
                        tab.sync_presentation();
                        let thresholds = tab.settings.large_file_thresholds();
                        let tier = crate::document_limits::tier_for_size_with_thresholds(
                            size,
                            &thresholds,
                        );
                        tab.open_viewer_for_large_file(
                            &parent,
                            &opened_file,
                            size,
                            OpenDecision::Viewer {
                                tier,
                                edit_allowed: false,
                            },
                            callback.clone(),
                        );
                    }
                    Err(LoadFailure::Failed(error)) => {
                        tab.set_loading(false);
                        tab.sync_presentation();
                        callback(Err(error));
                    }
                }
            });
            if let Some(open_support) = open_support {
                editor_io::load_text_file_with_open_support(
                    file,
                    encodings,
                    Some(&cancellable),
                    open_support,
                    load_callback,
                );
            } else {
                editor_io::load_text_file(file, encodings, Some(&cancellable), load_callback);
            }
        });
    }

    fn reopen_with_encoding(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        encoding: &sourceview5::Encoding,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let Some(saved_file) = self.saved_file() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        let Some(expected_uri) = self.document_uri() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        if let Err(error) = editor_io::validate_text_file_open(&saved_file) {
            callback(Err(map_load_failure_to_app_error(error)));
            return;
        }

        self.set_loading(true);
        let snapshot = ReloadSnapshot::capture(&self.text_buffer);
        let candidate_encodings = std::iter::once(encoding.clone()).collect::<SList<_>>();
        let (generation, cancellable) = self.start_io_request(Some(candidate_encodings));
        let weak = Rc::downgrade(self);
        let parent = parent.clone();
        self.with_io_candidate_encodings(|encodings| {
            editor_io::load_text_file(
                &saved_file,
                encodings,
                Some(&cancellable),
                Rc::new(move |result| {
                    let Some(tab) = weak.upgrade() else {
                        callback(Err(AppError::Cancelled));
                        return;
                    };
                    if !tab.finish_io_request(generation) {
                        callback(Err(AppError::Cancelled));
                        return;
                    }
                    match result {
                        Ok(document) => {
                            if tab.document_uri().as_deref() != Some(expected_uri.as_str())
                                || !tab.monitor_target_matches_current()
                            {
                                tab.set_loading(false);
                                tab.sync_presentation();
                                callback(Err(AppError::Cancelled));
                                return;
                            }
                            let monitored_file = gio::File::for_path(&document.path);
                            let callback = callback.clone();
                            let cancelled_callback = callback.clone();
                            tab.apply_loaded_document_async(
                                document,
                                Some(snapshot.clone()),
                                Rc::new(move |tab| {
                                    tab.swap_monitor(&monitored_file);
                                    tab.refresh_language_for_file(&monitored_file);
                                    tab.set_loading(false);
                                    tab.sync_presentation();
                                    callback(Ok(()));
                                }),
                                Rc::new(move || {
                                    cancelled_callback(Err(AppError::Cancelled));
                                }),
                            );
                        }
                        Err(LoadFailure::DecodeFailed(path)) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            tab.offer_reopen_with_manual_encoding(&parent, &path, callback.clone());
                        }
                        Err(LoadFailure::TooBig(path)) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(AppError::FileTooBig(path)));
                        }
                        Err(LoadFailure::LineTooLong { path, .. }) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(AppError::LineTooLong(path)));
                        }
                        Err(LoadFailure::Failed(error)) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(error));
                        }
                    }
                }),
            );
        });
    }

    fn offer_open_with_manual_encoding(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        file: &gio::File,
        path: &Path,
        callback: Rc<dyn Fn(Result<String, AppError>)>,
    ) {
        let weak = Rc::downgrade(self);
        let opened_file = file.clone();
        let dialog_parent = parent.clone();
        let action_parent = parent.clone();
        dialogs::confirm_decode_failure(parent, &path.display().to_string(), move |response| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            match response {
                DecodeFailureResponse::ChooseEncoding => {
                    let candidates = sourceview5::Encoding::default_candidates();
                    let title = gettext("Choose a Text Encoding");
                    let body =
                        gettext("Choose the encoding to use when opening this document from disk.");
                    let weak = Rc::downgrade(&tab);
                    let callback = callback.clone();
                    let action_parent = action_parent.clone();
                    let opened_file = opened_file.clone();
                    dialogs::choose_encoding(
                        &dialog_parent,
                        &title,
                        &body,
                        &candidates,
                        None,
                        &pgettext("dialog button", "Open"),
                        move |selection| {
                            if let Some(tab) = weak.upgrade() {
                                let Some(encoding) = selection else {
                                    callback(Err(AppError::Cancelled));
                                    return;
                                };
                                let selected = std::iter::once(encoding).collect::<SList<_>>();
                                tab.load_file_with_candidates(
                                    &action_parent,
                                    &opened_file,
                                    Some(selected),
                                    None,
                                    callback.clone(),
                                );
                            }
                        },
                    );
                }
                DecodeFailureResponse::Cancel => callback(Err(AppError::Cancelled)),
            }
        });
    }

    fn offer_reopen_with_manual_encoding(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        path: &Path,
        callback: Rc<dyn Fn(Result<(), AppError>)>,
    ) {
        let weak = Rc::downgrade(self);
        let dialog_parent = parent.clone();
        let action_parent = parent.clone();
        dialogs::confirm_decode_failure(parent, &path.display().to_string(), move |response| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            match response {
                DecodeFailureResponse::ChooseEncoding => {
                    let candidates = sourceview5::Encoding::default_candidates();
                    let current = tab.current_format().encoding().to_source_encoding();
                    let title = gettext("Choose a Text Encoding");
                    let body = gettext("Choose the encoding to use when reopening this document.");
                    let weak = Rc::downgrade(&tab);
                    let callback = callback.clone();
                    let action_parent = action_parent.clone();
                    dialogs::choose_encoding(
                        &dialog_parent,
                        &title,
                        &body,
                        &candidates,
                        current.as_ref(),
                        &pgettext("dialog button", "Reopen"),
                        move |selection| {
                            if let Some(tab) = weak.upgrade() {
                                let Some(encoding) = selection else {
                                    callback(Err(AppError::Cancelled));
                                    return;
                                };
                                tab.reopen_with_encoding(
                                    &action_parent,
                                    &encoding,
                                    callback.clone(),
                                );
                            }
                        },
                    );
                }
                DecodeFailureResponse::Cancel => callback(Err(AppError::Cancelled)),
            }
        });
    }
}

fn map_load_failure_to_app_error(error: LoadFailure) -> AppError {
    match error {
        LoadFailure::DecodeFailed(path) => AppError::DecodeFailed(path),
        LoadFailure::TooBig(path) => AppError::FileTooBig(path),
        LoadFailure::LineTooLong { path, .. } => AppError::LineTooLong(path),
        LoadFailure::Failed(error) => error,
    }
}
