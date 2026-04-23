use std::path::{Path, PathBuf};

use gtk4::{gio, prelude::*};

use crate::editor_format::{EncodingInfo, LineEndingMode, SavedTextFormat};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentState {
    path: Option<PathBuf>,
    format: SavedTextFormat,
}

impl DocumentState {
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            path: None,
            format: SavedTextFormat::new_document_defaults(),
        }
    }

    #[must_use]
    pub fn from_loaded(path: PathBuf, format: SavedTextFormat) -> Self {
        Self {
            path: Some(path),
            format,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    #[must_use]
    pub fn uri(&self) -> Option<String> {
        self.path
            .as_ref()
            .map(|path| gio::File::for_path(path).uri().to_string())
    }

    #[must_use]
    pub fn file_name(&self) -> Option<String> {
        self.path.as_ref().and_then(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .map(ToString::to_string)
        })
    }

    #[must_use]
    pub fn path_display(&self) -> Option<String> {
        self.path.as_ref().map(|path| path.display().to_string())
    }

    pub fn set_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    #[must_use]
    pub fn format(&self) -> &SavedTextFormat {
        &self.format
    }

    pub fn set_format(&mut self, format: SavedTextFormat) {
        self.format = format;
    }

    pub fn set_line_ending_mode(&mut self, line_ending_mode: LineEndingMode) {
        self.format.set_line_ending_mode(line_ending_mode);
    }

    pub fn set_encoding(&mut self, encoding: EncodingInfo) {
        self.format.set_encoding(encoding);
    }

    pub fn set_implicit_trailing_newline(&mut self, implicit_trailing_newline: bool) {
        self.format
            .set_implicit_trailing_newline(implicit_trailing_newline);
    }

    #[must_use]
    pub fn normalized_save_path(path: &Path) -> PathBuf {
        if path.extension().is_some() {
            path.to_path_buf()
        } else {
            path.with_extension("txt")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DocumentState;
    use crate::editor_format::{EncodingInfo, LineEndingMode, SavedTextFormat};
    use std::path::Path;

    #[test]
    fn empty_document_has_no_saved_identity() {
        let document = DocumentState::new_empty();
        assert_eq!(document.file_name(), None);
        assert_eq!(document.path_display(), None);
        assert_eq!(document.uri(), None);
        assert_eq!(document.format().line_ending_mode(), LineEndingMode::Lf);
        assert!(document.format().encoding().is_utf8());
    }

    #[test]
    fn loaded_document_tracks_saved_identity() {
        let document = DocumentState::from_loaded(
            "notes.txt".into(),
            SavedTextFormat::new(LineEndingMode::CrLf, EncodingInfo::utf8(), false),
        );
        assert_eq!(document.file_name().as_deref(), Some("notes.txt"));
        assert_eq!(document.path_display().as_deref(), Some("notes.txt"));
        assert!(
            document
                .uri()
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/notes.txt"))
        );
        assert_eq!(document.format().line_ending_mode(), LineEndingMode::CrLf);
    }

    #[test]
    fn save_path_adds_txt_extension_when_missing() {
        let normalized = DocumentState::normalized_save_path(Path::new("/tmp/notes"));
        assert_eq!(normalized.to_string_lossy(), "/tmp/notes.txt");
    }
}
