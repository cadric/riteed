use std::path::{Path, PathBuf};

use gtk4::{gio, prelude::*};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentState {
    path: Option<PathBuf>,
}

impl DocumentState {
    #[must_use]
    pub fn new_empty() -> Self {
        Self { path: None }
    }

    #[must_use]
    pub fn from_loaded(path: PathBuf) -> Self {
        Self { path: Some(path) }
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
    use std::path::Path;

    #[test]
    fn empty_document_has_no_saved_identity() {
        let document = DocumentState::new_empty();
        assert_eq!(document.file_name(), None);
        assert_eq!(document.path_display(), None);
        assert_eq!(document.uri(), None);
    }

    #[test]
    fn loaded_document_tracks_saved_identity() {
        let document = DocumentState::from_loaded("notes.txt".into());
        assert_eq!(document.file_name().as_deref(), Some("notes.txt"));
        assert_eq!(document.path_display().as_deref(), Some("notes.txt"));
        assert!(
            document
                .uri()
                .as_deref()
                .is_some_and(|uri| uri.ends_with("/notes.txt"))
        );
    }

    #[test]
    fn save_path_adds_txt_extension_when_missing() {
        let normalized = DocumentState::normalized_save_path(Path::new("/tmp/notes"));
        assert_eq!(normalized.to_string_lossy(), "/tmp/notes.txt");
    }
}
