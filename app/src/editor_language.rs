use std::path::Path;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};
use sourceview5::prelude::*;

#[derive(Clone)]
pub struct LanguageDetection {
    pub content_type: Option<String>,
    pub language_id: Option<String>,
    pub language: Option<sourceview5::Language>,
}

pub fn detect_for_file(file: &gio::File, callback: Rc<dyn Fn(LanguageDetection)>) {
    let file_for_query = file.clone();
    let file_for_guess = file.clone();
    file_for_query.query_info_async(
        "standard::content-type",
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |result| {
            let content_type = result
                .ok()
                .and_then(|info| info.content_type().map(|value| value.to_string()));
            callback(detection_for_path(
                file_for_guess.path().as_deref(),
                content_type,
            ));
        },
    );
}

#[must_use]
pub fn detection_for_path(path: Option<&Path>, content_type: Option<String>) -> LanguageDetection {
    let manager = sourceview5::LanguageManager::default();
    let language = if let Some(path) = path {
        manager.guess_language(Some(path), content_type.as_deref())
    } else {
        manager.guess_language(None::<&Path>, content_type.as_deref())
    };
    let language_id = language.as_ref().map(|item| item.id().to_string());

    LanguageDetection {
        content_type,
        language_id,
        language,
    }
}

pub fn apply_detection(buffer: &sourceview5::Buffer, detection: &LanguageDetection) {
    buffer.set_language(detection.language.as_ref());
    buffer.set_highlight_syntax(detection.language.is_some());
}
