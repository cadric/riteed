use std::path::{Path, PathBuf};
use std::rc::Rc;

use glib::SList;
use gtk4::{gio, glib, prelude::*};
use sourceview5::prelude::*;

use crate::document_limits::{
    OpenFileSupport, loaded_text_supports_editor_hard_cap, text_supports_editor_line_lengths,
};
use crate::editor_format::{EncodingInfo, LineEndingMode, LoadedTextFormat, SavedTextFormat};
use crate::error::AppError;

#[derive(Clone)]
pub struct LoadedDocument {
    pub path: PathBuf,
    pub display_path: Option<PathBuf>,
    pub text: String,
    pub uri: String,
    pub format: SavedTextFormat,
    pub source_file: sourceview5::File,
    pub disk_size: Option<u64>,
}

#[derive(Clone)]
pub struct SavedDocument {
    pub path: PathBuf,
    pub display_path: Option<PathBuf>,
    pub uri: String,
    pub format: SavedTextFormat,
    pub source_file: sourceview5::File,
}

pub struct LocalPathInfo {
    pub path: PathBuf,
    pub display_path: Option<PathBuf>,
}

struct TextLoadRequest {
    loader: sourceview5::FileLoader,
    scratch_buffer: sourceview5::Buffer,
    source_file: sourceview5::File,
    file: gio::File,
    path: PathBuf,
    display_path: Option<PathBuf>,
    disk_size: Option<u64>,
    cancellable: Option<gio::Cancellable>,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
}

#[derive(Clone)]
struct TextLoadStart {
    loader: sourceview5::FileLoader,
    scratch_buffer: sourceview5::Buffer,
    source_file: sourceview5::File,
    file: gio::File,
    path: PathBuf,
    display_path: Option<PathBuf>,
    cancellable: Option<gio::Cancellable>,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
}

#[derive(Clone, Debug)]
pub enum LoadFailure {
    DecodeFailed(PathBuf),
    TooBig(PathBuf),
    LineTooLong { path: PathBuf, size: u64 },
    Failed(AppError),
}

#[derive(Clone, Debug)]
pub enum SaveFailure {
    InvalidChars,
    ExternallyModified,
    Failed(AppError),
}

// PARSER-BOUNDARY: id=document_file_load
pub fn load_text_file(
    file: &gio::File,
    candidate_encodings: Option<&SList<sourceview5::Encoding>>,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
) {
    load_text_file_internal(file, candidate_encodings, cancellable, None, callback);
}

pub(crate) fn load_text_file_with_open_support(
    file: &gio::File,
    candidate_encodings: Option<&SList<sourceview5::Encoding>>,
    cancellable: Option<&gio::Cancellable>,
    open_support: OpenFileSupport,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
) {
    load_text_file_internal(
        file,
        candidate_encodings,
        cancellable,
        Some(open_support),
        callback,
    );
}

fn load_text_file_internal(
    file: &gio::File,
    candidate_encodings: Option<&SList<sourceview5::Encoding>>,
    cancellable: Option<&gio::Cancellable>,
    open_support: Option<OpenFileSupport>,
    callback: Rc<dyn Fn(Result<LoadedDocument, LoadFailure>)>,
) {
    let path_info = match local_path_info(file) {
        Ok(path_info) => path_info,
        Err(error) => {
            callback(Err(LoadFailure::Failed(error)));
            return;
        }
    };
    let path = path_info.path;
    let display_path = path_info.display_path;
    let source_file = sourceview5::File::builder().location(file).build();
    let scratch_buffer = sourceview5::Buffer::builder()
        .enable_undo(false)
        .implicit_trailing_newline(false)
        .build();
    let loader = sourceview5::FileLoader::new(&scratch_buffer, &source_file);
    if let Some(candidate_encodings) = candidate_encodings {
        loader.set_candidate_encodings(Some(candidate_encodings));
    }

    let cancellable_for_load = cancellable.cloned();
    let start = TextLoadStart {
        loader,
        scratch_buffer,
        source_file,
        file: file.clone(),
        path,
        display_path,
        cancellable: cancellable_for_load,
        callback,
    };
    if let Some(support) = open_support {
        start.start_with_support(support);
        return;
    }
    crate::document_limits::query_file_supports_open(
        file,
        cancellable,
        Rc::new(move |result| {
            let support = match result {
                Ok(support) => support,
                Err(error) => {
                    let callback = Rc::clone(&start.callback);
                    callback(Err(map_load_failure(&start.path, &error)));
                    return;
                }
            };
            start.clone().start_with_support(support);
        }),
    );
}

impl TextLoadStart {
    fn start_with_support(self, support: OpenFileSupport) {
        if !support.supports_open {
            (self.callback)(Err(LoadFailure::TooBig(self.path.clone())));
            return;
        }
        TextLoadRequest {
            loader: self.loader,
            scratch_buffer: self.scratch_buffer,
            source_file: self.source_file,
            file: self.file,
            path: self.path,
            display_path: self.display_path,
            disk_size: support.size,
            cancellable: self.cancellable,
            callback: self.callback,
        }
        .start();
    }
}

impl TextLoadRequest {
    fn start(self) {
        let Self {
            loader,
            scratch_buffer,
            source_file,
            file,
            path,
            display_path,
            disk_size,
            cancellable,
            callback,
        } = self;
        let callback_loader = loader.clone();
        loader.load_async(
            glib::Priority::DEFAULT,
            cancellable.as_ref(),
            move |result| match result {
                Ok(()) => {
                    let start = scratch_buffer.start_iter();
                    let end = scratch_buffer.end_iter();
                    let loaded_format = LoadedTextFormat::from_disk_text(
                        scratch_buffer.text(&start, &end, true).to_string(),
                        LineEndingMode::from_source(callback_loader.newline_type()),
                        EncodingInfo::from_encoding(&callback_loader.encoding()),
                    );
                    if !loaded_text_supports_editor_hard_cap(loaded_format.text.len()) {
                        callback(Err(LoadFailure::TooBig(path.clone())));
                        return;
                    }
                    if !text_supports_editor_line_lengths(&loaded_format.text) {
                        let size = disk_size.unwrap_or_else(|| {
                            crate::large_file::usize_to_u64(loaded_format.text.len())
                        });
                        callback(Err(LoadFailure::LineTooLong {
                            path: path.clone(),
                            size,
                        }));
                        return;
                    }
                    callback(Ok(LoadedDocument {
                        path: path.clone(),
                        display_path: display_path.clone(),
                        text: loaded_format.text,
                        uri: file.uri().to_string(),
                        format: loaded_format.format,
                        source_file: source_file.clone(),
                        disk_size,
                    }));
                }
                Err(error) => callback(Err(map_load_failure(&path, &error))),
            },
        );
    }
}

/// # Errors
///
/// Returns an error when the file is not local.
pub fn validate_text_file_open(file: &gio::File) -> Result<LocalPathInfo, LoadFailure> {
    local_path_info(file).map_err(LoadFailure::Failed)
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
    let display_path = crate::document::portal_host_display_path(&path);
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
    if let Err(error) = validate_buffer_supports_encoding(target_path, buffer, format) {
        callback(Err(error));
        return;
    }

    saver.save_async(
        glib::Priority::DEFAULT,
        cancellable,
        move |result| match result {
            Ok(()) => callback(Ok(SavedDocument {
                path: path.clone(),
                display_path: display_path.clone(),
                uri: target_file.uri().to_string(),
                format: saved_format.clone(),
                source_file: source_file.clone(),
            })),
            Err(error) => callback(Err(map_save_failure(&path, &error))),
        },
    );
}

fn validate_buffer_supports_encoding(
    path: &Path,
    buffer: &sourceview5::Buffer,
    format: &SavedTextFormat,
) -> Result<(), SaveFailure> {
    if format.encoding().is_utf8() {
        return Ok(());
    }
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    let text = buffer.text(&start, &end, true);
    validate_text_supports_charset(
        path,
        text.as_str(),
        format.encoding().charset(),
        format.encoding().is_utf8(),
    )
}

fn validate_text_supports_charset(
    path: &Path,
    text: &str,
    charset: &str,
    is_utf8: bool,
) -> Result<(), SaveFailure> {
    if is_utf8 {
        return Ok(());
    }
    match glib::convert(text.as_bytes(), charset, "UTF-8") {
        Ok((_converted, _bytes_read)) => Ok(()),
        Err(glib::CvtError::IllegalSequence { .. }) => Err(SaveFailure::InvalidChars),
        Err(glib::CvtError::Convert(error)) => Err(SaveFailure::Failed(AppError::WriteFailed(
            path.to_path_buf(),
            error.message().to_string(),
        ))),
    }
}

fn map_load_failure(path: &Path, error: &glib::Error) -> LoadFailure {
    if error.matches(gio::IOErrorEnum::Cancelled) {
        return LoadFailure::Failed(AppError::Cancelled);
    }
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
    if is_invalid_chars_save_error(error) {
        return SaveFailure::InvalidChars;
    }
    match error.kind::<sourceview5::FileSaverError>() {
        Some(sourceview5::FileSaverError::ExternallyModified) => SaveFailure::ExternallyModified,
        Some(sourceview5::FileSaverError::__Unknown(_) | _) | None => SaveFailure::Failed(
            AppError::WriteFailed(path.to_path_buf(), error.message().to_string()),
        ),
    }
}

fn is_invalid_chars_save_error(error: &glib::Error) -> bool {
    error.matches(glib::ConvertError::IllegalSequence)
        || error.matches(sourceview5::FileSaverError::InvalidChars)
}

/// # Errors
///
/// Returns an error when the provided file is not backed by a local path.
pub fn local_path(file: &gio::File) -> Result<PathBuf, AppError> {
    local_path_info(file).map(|info| info.path)
}

/// # Errors
///
/// Returns an error when the provided file is not backed by a local path.
pub fn local_path_info(file: &gio::File) -> Result<LocalPathInfo, AppError> {
    let path = file.path().ok_or(AppError::NonLocalFile)?;
    let display_path = crate::document::portal_host_display_path(&path);
    Ok(LocalPathInfo { path, display_path })
}

#[cfg(test)]
mod tests {
    use gtk4::{gio, glib};

    use super::{
        LoadFailure, SaveFailure, local_path, local_path_info, map_load_failure, map_save_failure,
        validate_text_supports_charset,
    };
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
    fn long_line_failures_carry_path_and_size() {
        let failure = LoadFailure::LineTooLong {
            path: std::path::PathBuf::from("/tmp/example.txt"),
            size: 7,
        };
        assert!(matches!(failure, LoadFailure::LineTooLong { size: 7, .. }));
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
    fn generic_io_failure_message_does_not_drive_invalid_chars_mapping() {
        let path = std::path::Path::new("/tmp/example.txt");
        let generic = glib::Error::new(
            gio::IOErrorEnum::Failed,
            "Invalid byte sequence in conversion input",
        );
        assert!(matches!(
            map_save_failure(path, &generic),
            SaveFailure::Failed(AppError::WriteFailed(mapped, _)) if mapped == path
        ));
    }

    #[test]
    fn glib_conversion_illegal_sequence_maps_to_invalid_chars() {
        let path = std::path::Path::new("/tmp/example.txt");
        let conversion = glib::Error::new(glib::ConvertError::IllegalSequence, "localized");
        assert!(matches!(
            map_save_failure(path, &conversion),
            SaveFailure::InvalidChars
        ));
    }

    #[test]
    fn charset_preflight_maps_conversion_failure_to_invalid_chars() {
        assert!(matches!(
            validate_text_supports_charset(
                std::path::Path::new("/tmp/example.txt"),
                "emoji 😀",
                "ISO-8859-1",
                false
            ),
            Err(SaveFailure::InvalidChars)
        ));
    }

    #[test]
    fn text_load_request_guards_post_decode_editor_cap() {
        let source = include_str!("editor_io.rs");

        assert!(source.contains("loaded_text_supports_editor_hard_cap(loaded_format.text.len())"));
        assert!(source.contains("LoadFailure::TooBig(path.clone())"));
    }

    #[test]
    fn non_local_files_are_rejected() {
        let file = gio::File::for_uri("resource:///io/github/cadric/Riteed/missing.txt");
        assert!(matches!(local_path(&file), Err(AppError::NonLocalFile)));
    }

    #[test]
    fn local_path_info_uses_cached_portal_display_path_only() {
        crate::document_portal::reset_cache_for_tests();
        let file = gio::File::for_path("/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md");
        let info = local_path_info(&file);
        assert!(info.is_ok());
        let Ok(info) = info else {
            return;
        };
        assert_eq!(info.display_path, None);

        crate::document_portal::cache_host_path_for_tests(
            "23ef3b31",
            "/home/cadric/Dokumenter/CoreOS_Server".into(),
        );
        let info = local_path_info(&file);
        assert!(info.is_ok());
        let Ok(info) = info else {
            return;
        };
        assert_eq!(
            info.display_path,
            Some("/home/cadric/Dokumenter/CoreOS_Server/AGENTS.md".into())
        );
    }
}
