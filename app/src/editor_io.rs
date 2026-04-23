use std::path::{Path, PathBuf};
use std::rc::Rc;

use glib::SList;
use gtk4::{gio, glib, prelude::*};
use sourceview5::prelude::*;

use crate::editor_format::{EncodingInfo, LineEndingMode, LoadedTextFormat, SavedTextFormat};
use crate::error::AppError;

#[derive(Clone)]
pub struct LoadedDocument {
    pub path: PathBuf,
    pub text: String,
    pub uri: String,
    pub format: SavedTextFormat,
    pub source_file: sourceview5::File,
}

#[derive(Clone)]
pub struct SavedDocument {
    pub path: PathBuf,
    pub uri: String,
    pub format: SavedTextFormat,
    pub source_file: sourceview5::File,
}

#[derive(Clone, Debug)]
pub enum LoadFailure {
    DecodeFailed(PathBuf),
    TooBig(PathBuf),
    Failed(AppError),
}

#[derive(Clone, Debug)]
pub enum SaveFailure {
    InvalidChars,
    ExternallyModified,
    Failed(AppError),
}

pub fn load_text_file(
    file: &gio::File,
    candidate_encodings: Option<&SList<sourceview5::Encoding>>,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
) {
    let path = match local_path(file) {
        Ok(path) => path,
        Err(error) => {
            callback(Err(LoadFailure::Failed(error)));
            return;
        }
    };

    let source_file = sourceview5::File::builder().location(file).build();
    let scratch_buffer = sourceview5::Buffer::builder()
        .enable_undo(false)
        .implicit_trailing_newline(false)
        .build();
    let loader = sourceview5::FileLoader::new(&scratch_buffer, &source_file);
    if let Some(candidate_encodings) = candidate_encodings {
        loader.set_candidate_encodings(Some(candidate_encodings));
    }
    let callback_loader = loader.clone();

    let callback_path = path.clone();
    let callback_file = file.clone();
    loader.load_async(
        glib::Priority::DEFAULT,
        cancellable,
        move |result| match result {
            Ok(()) => {
                let start = scratch_buffer.start_iter();
                let end = scratch_buffer.end_iter();
                let loaded_format = LoadedTextFormat::from_disk_text(
                    scratch_buffer.text(&start, &end, true).to_string(),
                    LineEndingMode::from_source(callback_loader.newline_type()),
                    EncodingInfo::from_encoding(&callback_loader.encoding()),
                );
                callback(Ok(LoadedDocument {
                    path: callback_path.clone(),
                    text: loaded_format.text,
                    uri: callback_file.uri().to_string(),
                    format: loaded_format.format,
                    source_file: source_file.clone(),
                }));
            }
            Err(error) => callback(Err(map_load_failure(&callback_path, &error))),
        },
    );
}

pub fn save_text_file(
    live_source_file: Option<&sourceview5::File>,
    target_path: &Path,
    buffer: &sourceview5::Buffer,
    format: &SavedTextFormat,
    flags: sourceview5::FileSaverFlags,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<SavedDocument, SaveFailure>)>,
) {
    let path = target_path.to_path_buf();
    let target_file = gio::File::for_path(&path);
    let use_live_source_file = live_source_file
        .and_then(|file| file.location().path())
        .is_some_and(|current_path| current_path == path);

    let source_file = if use_live_source_file {
        live_source_file
            .cloned()
            .unwrap_or_else(|| sourceview5::File::builder().location(&target_file).build())
    } else {
        sourceview5::File::builder().location(&target_file).build()
    };

    buffer.set_implicit_trailing_newline(format.implicit_trailing_newline());

    let saver = sourceview5::FileSaver::builder()
        .buffer(buffer)
        .file(&source_file)
        .flags(flags)
        .build();
    saver.set_newline_type(format.line_ending_mode().into_source());
    let encoding = format.encoding().to_source_encoding();
    saver.set_encoding(encoding.as_ref());
    let saved_format = format.clone();

    saver.save_async(
        glib::Priority::DEFAULT,
        cancellable,
        move |result| match result {
            Ok(()) => callback(Ok(SavedDocument {
                path: path.clone(),
                uri: target_file.uri().to_string(),
                format: saved_format.clone(),
                source_file: source_file.clone(),
            })),
            Err(error) => callback(Err(map_save_failure(&path, &error))),
        },
    );
}

fn map_load_failure(path: &Path, error: &glib::Error) -> LoadFailure {
    match error.kind::<sourceview5::FileLoaderError>() {
        Some(
            sourceview5::FileLoaderError::EncodingAutoDetectionFailed
            | sourceview5::FileLoaderError::ConversionFallback,
        ) => LoadFailure::DecodeFailed(path.to_path_buf()),
        Some(sourceview5::FileLoaderError::TooBig) => LoadFailure::TooBig(path.to_path_buf()),
        Some(sourceview5::FileLoaderError::__Unknown(_) | _) | None => LoadFailure::Failed(
            AppError::ReadFailed(path.to_path_buf(), error.message().to_string()),
        ),
    }
}

fn map_save_failure(path: &Path, error: &glib::Error) -> SaveFailure {
    let message = error.message().to_string();
    match error.kind::<sourceview5::FileSaverError>() {
        Some(sourceview5::FileSaverError::InvalidChars) => SaveFailure::InvalidChars,
        Some(sourceview5::FileSaverError::ExternallyModified) => SaveFailure::ExternallyModified,
        Some(sourceview5::FileSaverError::__Unknown(_) | _) | None => {
            if message.contains("Invalid byte sequence in conversion input") {
                SaveFailure::InvalidChars
            } else {
                SaveFailure::Failed(AppError::WriteFailed(path.to_path_buf(), message))
            }
        }
    }
}

/// # Errors
///
/// Returns an error when the provided file is not backed by a local path.
pub fn local_path(file: &gio::File) -> Result<PathBuf, AppError> {
    file.path().ok_or(AppError::NonLocalFile)
}

#[cfg(test)]
mod tests {
    use gtk4::{gio, glib};

    use super::{LoadFailure, SaveFailure, local_path, map_load_failure, map_save_failure};
    use crate::error::AppError;

    #[test]
    fn load_failures_map_to_decode_and_size_errors() {
        let path = std::path::Path::new("/tmp/example.txt");

        let decode = glib::Error::new(
            sourceview5::FileLoaderError::EncodingAutoDetectionFailed,
            "decode failed",
        );
        assert!(matches!(
            map_load_failure(path, &decode),
            LoadFailure::DecodeFailed(mapped) if mapped == path
        ));

        let too_big = glib::Error::new(sourceview5::FileLoaderError::TooBig, "too big");
        assert!(matches!(
            map_load_failure(path, &too_big),
            LoadFailure::TooBig(mapped) if mapped == path
        ));
    }

    #[test]
    fn save_failures_map_to_expected_categories() {
        let path = std::path::Path::new("/tmp/example.txt");

        let invalid = glib::Error::new(sourceview5::FileSaverError::InvalidChars, "invalid chars");
        assert!(matches!(
            map_save_failure(path, &invalid),
            SaveFailure::InvalidChars
        ));

        let external = glib::Error::new(
            sourceview5::FileSaverError::ExternallyModified,
            "externally modified",
        );
        assert!(matches!(
            map_save_failure(path, &external),
            SaveFailure::ExternallyModified
        ));
    }

    #[test]
    fn invalid_byte_sequence_fallback_maps_to_invalid_chars() {
        let path = std::path::Path::new("/tmp/example.txt");
        let generic = glib::Error::new(
            gio::IOErrorEnum::Failed,
            "Invalid byte sequence in conversion input",
        );
        assert!(matches!(
            map_save_failure(path, &generic),
            SaveFailure::InvalidChars
        ));
    }

    #[test]
    fn non_local_files_are_rejected() {
        let file = gio::File::for_uri("resource:///io/github/cadric/Riteed/missing.txt");
        assert!(matches!(local_path(&file), Err(AppError::NonLocalFile)));
    }
}
