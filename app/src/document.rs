use std::path::{Component, Path, PathBuf};

use gtk4::{gio, prelude::*};

use crate::editor_format::{EncodingInfo, LineEndingMode, SavedTextFormat};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentState {
    path: Option<PathBuf>,
    display_path: Option<PathBuf>,
    format: SavedTextFormat,
}

impl DocumentState {
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            path: None,
            display_path: None,
            format: SavedTextFormat::new_document_defaults(),
        }
    }

    #[must_use]
    pub fn from_loaded(path: PathBuf, format: SavedTextFormat) -> Self {
        Self::from_loaded_with_display_path(path, None, format)
    }

    #[must_use]
    pub fn from_loaded_with_display_path(
        path: PathBuf,
        display_path: Option<PathBuf>,
        format: SavedTextFormat,
    ) -> Self {
        Self {
            path: Some(path),
            display_path,
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
        self.display_path
            .as_ref()
            .or(self.path.as_ref())
            .and_then(|path| {
                path.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .map(ToString::to_string)
            })
    }

    #[must_use]
    pub fn path_display(&self) -> Option<String> {
        self.display_path
            .as_ref()
            .or(self.path.as_ref())
            .map(|path| display_path(path))
    }

    pub fn set_saved(&mut self, path: PathBuf) {
        self.set_saved_with_display_path(path, None);
    }

    pub fn set_saved_with_display_path(&mut self, path: PathBuf, display_path: Option<PathBuf>) {
        self.path = Some(path);
        self.display_path = display_path;
    }

    pub fn set_display_path_for_access_path(
        &mut self,
        access_path: &Path,
        display_path: Option<PathBuf>,
    ) -> bool {
        if self.path.as_deref() != Some(access_path) || self.display_path == display_path {
            return false;
        }
        self.display_path = display_path;
        true
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
        path.to_path_buf()
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    display_path_with_home(path, home.as_deref())
}

pub(crate) fn display_path_for_file(file: &gio::File) -> String {
    file.path().map_or_else(
        || file.uri().to_string(),
        |path| {
            let display_source = portal_host_display_path(&path).unwrap_or(path);
            display_path(&display_source)
        },
    )
}

pub(crate) fn portal_host_display_path(path: &Path) -> Option<PathBuf> {
    crate::document_portal::cached_display_path(path)
}

fn display_path_with_home(path: &Path, home: Option<&Path>) -> String {
    if let Some(relative) = home_relative_path(path, home) {
        return tilde_path(relative);
    }

    if let Some(relative) = portal_home_relative_path(path) {
        return tilde_path(&relative);
    }

    path.display().to_string()
}

fn home_relative_path<'a>(path: &'a Path, home: Option<&Path>) -> Option<&'a Path> {
    let home = home.filter(|home| !home.as_os_str().is_empty())?;
    let relative = path.strip_prefix(home).ok()?;
    if relative.as_os_str().is_empty() {
        None
    } else {
        Some(relative)
    }
}

fn portal_home_relative_path(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    require_root_component(components.next())?;
    require_normal_component(components.next(), "run")?;
    require_normal_component(components.next(), "user")?;
    require_any_normal_component(components.next())?;
    require_normal_component(components.next(), "doc")?;
    require_any_normal_component(components.next())?;

    let mut relative = PathBuf::new();
    for component in components {
        let Component::Normal(part) = component else {
            return None;
        };
        relative.push(part);
    }

    let first = relative.components().next().and_then(|component| {
        let Component::Normal(part) = component else {
            return None;
        };
        part.to_str()
    })?;

    if known_home_dir_name(first) {
        Some(relative)
    } else {
        None
    }
}

fn require_root_component(component: Option<Component<'_>>) -> Option<()> {
    match component {
        Some(Component::RootDir) => Some(()),
        _ => None,
    }
}

fn require_normal_component(component: Option<Component<'_>>, expected: &str) -> Option<()> {
    match component {
        Some(Component::Normal(part)) if part == expected => Some(()),
        _ => None,
    }
}

fn require_any_normal_component(component: Option<Component<'_>>) -> Option<()> {
    match component {
        Some(Component::Normal(part)) if !part.is_empty() => Some(()),
        _ => None,
    }
}

fn known_home_dir_name(name: &str) -> bool {
    matches!(
        name,
        "Desktop"
            | "Documents"
            | "Downloads"
            | "Music"
            | "Pictures"
            | "Videos"
            | "Billeder"
            | "Dokumenter"
            | "Hentede filer"
            | "Musik"
            | "Skrivebord"
            | "Videoer"
    )
}

fn tilde_path(relative: &Path) -> String {
    format!("~/{}", relative.display())
}

#[cfg(test)]
mod tests {
    use super::{DocumentState, display_path_with_home};
    use crate::editor_format::{EncodingInfo, LineEndingMode, SavedTextFormat};
    use std::path::{Path, PathBuf};

    fn test_home_path(relative: &str) -> PathBuf {
        std::env::var_os("HOME").map_or_else(
            || PathBuf::from("/home/cadric").join(relative),
            |home| PathBuf::from(home).join(relative),
        )
    }

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
    fn loaded_document_can_display_host_path_for_portal_access_path() {
        let document = DocumentState::from_loaded_with_display_path(
            "/run/user/1000/doc/bafc6e7f/docs.policy.json".into(),
            Some(test_home_path(
                "Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json",
            )),
            SavedTextFormat::new_document_defaults(),
        );
        assert_eq!(document.file_name().as_deref(), Some("docs.policy.json"));
        assert_eq!(
            document.path_display().as_deref(),
            Some(
                "~/Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json"
            )
        );
        assert!(
            document
                .uri()
                .as_deref()
                .is_some_and(|uri| uri.contains("/run/user/1000/doc/bafc6e7f/docs.policy.json"))
        );
    }

    #[test]
    fn stale_display_path_update_is_rejected() {
        let mut document = DocumentState::from_loaded(
            "/run/user/1000/doc/bafc6e7f/docs.policy.json".into(),
            SavedTextFormat::new_document_defaults(),
        );
        assert!(!document.set_display_path_for_access_path(
            Path::new("/run/user/1000/doc/other/docs.policy.json"),
            Some(test_home_path("docs.policy.json")),
        ));
        assert_eq!(
            document.path_display().as_deref(),
            Some("/run/user/1000/doc/bafc6e7f/docs.policy.json")
        );

        assert!(document.set_display_path_for_access_path(
            Path::new("/run/user/1000/doc/bafc6e7f/docs.policy.json"),
            Some(test_home_path("docs.policy.json")),
        ));
        assert_eq!(
            document.path_display().as_deref(),
            Some("~/docs.policy.json")
        );
    }

    #[test]
    fn path_display_compacts_home_paths() {
        let display = display_path_with_home(
            Path::new("/home/cadric/Dokumenter/2.txt"),
            Some(Path::new("/home/cadric")),
        );
        assert_eq!(display, "~/Dokumenter/2.txt");
    }

    #[test]
    fn path_display_compacts_known_portal_home_dirs() {
        let display = display_path_with_home(
            Path::new("/run/user/1000/doc/c600d4b0/Dokumenter/2.txt"),
            None,
        );
        assert_eq!(display, "~/Dokumenter/2.txt");
    }

    #[test]
    fn path_display_keeps_unknown_portal_paths_explicit() {
        let path = Path::new("/run/user/1000/doc/308bde05/riteed/AGENTS.md");
        let display = display_path_with_home(path, None);
        assert_eq!(display, path.display().to_string());
    }

    #[test]
    fn save_path_preserves_extensionless_names() {
        for path in [
            "/tmp/notes",
            "/tmp/Makefile",
            "/tmp/LICENSE",
            "/tmp/.gitignore",
        ] {
            let normalized = DocumentState::normalized_save_path(Path::new(path));
            assert_eq!(normalized, Path::new(path));
        }
    }
}
