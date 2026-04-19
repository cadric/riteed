use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentState {
    path: Option<PathBuf>,
    last_saved_text: String,
}

impl DocumentState {
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            path: None,
            last_saved_text: String::new(),
        }
    }

    #[must_use]
    pub fn from_loaded(path: PathBuf, text: String) -> Self {
        Self {
            path: Some(path),
            last_saved_text: text,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    pub fn set_saved(&mut self, path: PathBuf, text: String) {
        self.path = Some(path);
        self.last_saved_text = text;
    }

    pub fn replace_with_new(&mut self) {
        *self = Self::new_empty();
    }

    #[must_use]
    pub fn is_dirty(&self, current_text: &str) -> bool {
        self.last_saved_text != current_text
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(path) => path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .map_or_else(|| path.display().to_string(), ToString::to_string),
            None => String::from("Untitled.txt"),
        }
    }

    #[must_use]
    pub fn subtitle(&self) -> String {
        self.path.as_ref().map_or_else(
            || String::from("Plain Text Document"),
            |path| path.display().to_string(),
        )
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
    fn empty_document_is_clean() {
        let document = DocumentState::new_empty();
        assert!(!document.is_dirty(""));
        assert_eq!(document.display_name(), "Untitled.txt");
    }

    #[test]
    fn loaded_document_tracks_dirty_state() {
        let document = DocumentState::from_loaded("notes.txt".into(), String::from("hello"));
        assert!(!document.is_dirty("hello"));
        assert!(document.is_dirty("changed"));
    }

    #[test]
    fn save_path_adds_txt_extension_when_missing() {
        let normalized = DocumentState::normalized_save_path(Path::new("/tmp/notes"));
        assert_eq!(normalized.to_string_lossy(), "/tmp/notes.txt");
    }
}
