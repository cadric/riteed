use std::collections::HashMap;
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
    let portal_path = PortalPath::parse(path)?;
    let host_path = document_portal_host_path(&portal_path.document_id)?;
    Some(portal_display_path(&host_path, &portal_path.relative_path))
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

struct PortalPath {
    document_id: String,
    relative_path: PathBuf,
}

impl PortalPath {
    fn parse(path: &Path) -> Option<Self> {
        let mut components = path.components();
        require_root_component(components.next())?;
        require_normal_component(components.next(), "run")?;
        match normal_component_text(components.next())? {
            "user" => {
                require_any_normal_component(components.next())?;
                require_normal_component(components.next(), "doc")?;
            }
            "flatpak" => require_normal_component(components.next(), "doc")?,
            _ => return None,
        }
        let document_id = normal_component_text(components.next())?.to_string();
        let mut relative_path = PathBuf::new();
        for component in components {
            let Component::Normal(part) = component else {
                return None;
            };
            relative_path.push(part);
        }
        Some(Self {
            document_id,
            relative_path,
        })
    }
}

fn document_portal_host_path(document_id: &str) -> Option<PathBuf> {
    let proxy = gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES | gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None::<&gio::DBusInterfaceInfo>,
        "org.freedesktop.portal.Documents",
        "/org/freedesktop/portal/documents",
        "org.freedesktop.portal.Documents",
        None::<&gio::Cancellable>,
    )
    .ok()?;
    let parameters = (vec![document_id.to_string()],).to_variant();
    let result = proxy
        .call_sync(
            "GetHostPaths",
            Some(&parameters),
            gio::DBusCallFlags::NONE,
            500,
            None::<&gio::Cancellable>,
        )
        .ok()?;
    let (paths,): (HashMap<String, Vec<u8>>,) = result.get()?;
    let path_bytes = paths.get(document_id)?;
    let path_text = std::str::from_utf8(path_bytes).ok()?;
    Some(PathBuf::from(path_text))
}

fn portal_display_path(host_path: &Path, portal_relative_path: &Path) -> PathBuf {
    let Some(first_relative_name) =
        portal_relative_path
            .components()
            .next()
            .and_then(|component| match component {
                Component::Normal(part) => Some(part),
                _ => None,
            })
    else {
        return host_path.to_path_buf();
    };
    let relative_without_export_name = host_path
        .file_name()
        .filter(|name| *name == first_relative_name)
        .and_then(|_| portal_relative_path.strip_prefix(first_relative_name).ok());
    match relative_without_export_name {
        Some(relative) if relative.as_os_str().is_empty() => host_path.to_path_buf(),
        Some(relative) => host_path.join(relative),
        None => host_path.join(portal_relative_path),
    }
}

fn normal_component_text(component: Option<Component<'_>>) -> Option<&str> {
    match component {
        Some(Component::Normal(part)) => part.to_str(),
        _ => None,
    }
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
    use super::{DocumentState, PortalPath, display_path_with_home, portal_display_path};
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
    fn loaded_document_can_display_host_path_for_portal_access_path() {
        let document = DocumentState::from_loaded_with_display_path(
            "/run/user/1000/doc/bafc6e7f/docs.policy.json".into(),
            Some(
                "/home/cadric/Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json"
                    .into(),
            ),
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
    fn portal_path_parser_accepts_document_mount_variants() {
        let user_path = PortalPath::parse(Path::new(
            "/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md",
        ));
        assert!(
            user_path
                .as_ref()
                .is_some_and(|path| path.document_id == "23ef3b31")
        );
        assert_eq!(
            user_path.as_ref().map(|path| path.relative_path.as_path()),
            Some(Path::new("CoreOS_Server/AGENTS.md"))
        );

        let flatpak_path =
            PortalPath::parse(Path::new("/run/flatpak/doc/bafc6e7f/docs.policy.json"));
        assert!(
            flatpak_path
                .as_ref()
                .is_some_and(|path| path.document_id == "bafc6e7f")
        );
        assert_eq!(
            flatpak_path
                .as_ref()
                .map(|path| path.relative_path.as_path()),
            Some(Path::new("docs.policy.json"))
        );
    }

    #[test]
    fn portal_display_path_uses_host_export_path() {
        let exported_folder =
            Path::new("/home/cadric/Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server");
        assert_eq!(
            portal_display_path(
                exported_folder,
                Path::new("CoreOS_Server/policy/docs.policy.json")
            ),
            Path::new(
                "/home/cadric/Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json"
            )
        );

        let exported_file = Path::new(
            "/home/cadric/Drives/Samsung970/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json",
        );
        assert_eq!(
            portal_display_path(exported_file, Path::new("docs.policy.json")),
            exported_file
        );
    }

    #[test]
    fn save_path_adds_txt_extension_when_missing() {
        let normalized = DocumentState::normalized_save_path(Path::new("/tmp/notes"));
        assert_eq!(normalized.to_string_lossy(), "/tmp/notes.txt");
    }
}
