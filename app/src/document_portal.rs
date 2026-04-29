use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use gtk4::{gio, glib, prelude::*};

const DOCUMENTS_PORTAL_NAME: &str = "org.freedesktop.portal.Documents";
const DOCUMENTS_PORTAL_PATH: &str = "/org/freedesktop/portal/documents";
const DOCUMENTS_PORTAL_TIMEOUT_MS: i32 = 500;

type PortalDisplayCallback = Box<dyn FnOnce(Option<PathBuf>)>;

thread_local! {
    static PORTAL_HOST_PATHS: RefCell<PortalHostPathCache> =
        RefCell::new(PortalHostPathCache::default());
}

#[derive(Default)]
struct PortalHostPathCache {
    resolved: HashMap<String, PathBuf>,
    in_flight: HashMap<String, Vec<PendingLookup>>,
}

struct PendingLookup {
    portal_path: PortalPath,
    callback: PortalDisplayCallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PortalPath {
    document_id: String,
    relative_path: PathBuf,
}

#[must_use]
pub(crate) fn cached_display_path(path: &Path) -> Option<PathBuf> {
    let portal_path = PortalPath::parse(path)?;
    PORTAL_HOST_PATHS.with(|cache| {
        cache
            .borrow()
            .resolved
            .get(&portal_path.document_id)
            .map(|host_path| portal_display_path(host_path, &portal_path.relative_path))
    })
}

pub(crate) fn resolve_display_path_async(
    path: &Path,
    callback: impl FnOnce(Option<PathBuf>) + 'static,
) {
    let Some(portal_path) = PortalPath::parse(path) else {
        callback(None);
        return;
    };
    if let Some(display_path) = cached_display_path(path) {
        callback(Some(display_path));
        return;
    }

    let document_id = portal_path.document_id.clone();
    let should_start = enqueue_lookup(portal_path, Box::new(callback));
    if should_start {
        request_host_path(document_id);
    }
}

fn enqueue_lookup(portal_path: PortalPath, callback: PortalDisplayCallback) -> bool {
    PORTAL_HOST_PATHS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let callbacks = cache
            .in_flight
            .entry(portal_path.document_id.clone())
            .or_default();
        callbacks.push(PendingLookup {
            portal_path,
            callback,
        });
        callbacks.len() == 1
    })
}

fn request_host_path(document_id: String) {
    let flags =
        gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES | gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS;
    gio::DBusProxy::for_bus(
        gio::BusType::Session,
        flags,
        None::<&gio::DBusInterfaceInfo>,
        DOCUMENTS_PORTAL_NAME,
        DOCUMENTS_PORTAL_PATH,
        DOCUMENTS_PORTAL_NAME,
        None::<&gio::Cancellable>,
        move |proxy| {
            let Ok(proxy) = proxy else {
                finish_host_lookup(&document_id, None);
                return;
            };
            let parameters = (vec![document_id.clone()],).to_variant();
            proxy.call(
                "GetHostPaths",
                Some(&parameters),
                gio::DBusCallFlags::NONE,
                DOCUMENTS_PORTAL_TIMEOUT_MS,
                None::<&gio::Cancellable>,
                move |result| {
                    let host_path = host_path_from_result(&document_id, result);
                    finish_host_lookup(&document_id, host_path.as_deref());
                },
            );
        },
    );
}

fn host_path_from_result(
    document_id: &str,
    result: Result<glib::Variant, glib::Error>,
) -> Option<PathBuf> {
    let result = result.ok()?;
    let (paths,): (HashMap<String, Vec<u8>>,) = result.get()?;
    let path_bytes = paths.get(document_id)?;
    let path_text = std::str::from_utf8(path_bytes).ok()?;
    Some(PathBuf::from(path_text))
}

fn finish_host_lookup(document_id: &str, host_path: Option<&Path>) {
    let pending = PORTAL_HOST_PATHS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(host_path) = host_path {
            cache
                .resolved
                .insert(document_id.to_string(), host_path.to_path_buf());
        }
        cache.in_flight.remove(document_id).unwrap_or_default()
    });

    for pending in pending {
        let display_path = host_path
            .map(|host_path| portal_display_path(host_path, &pending.portal_path.relative_path));
        (pending.callback)(display_path);
    }
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

#[cfg(test)]
pub(crate) fn reset_cache_for_tests() {
    PORTAL_HOST_PATHS.with(|cache| {
        *cache.borrow_mut() = PortalHostPathCache::default();
    });
}

#[cfg(test)]
pub(crate) fn cache_host_path_for_tests(document_id: &str, host_path: PathBuf) {
    PORTAL_HOST_PATHS.with(|cache| {
        cache
            .borrow_mut()
            .resolved
            .insert(document_id.to_string(), host_path);
    });
}

#[cfg(test)]
fn enqueue_lookup_for_tests(path: &Path, callback: impl FnOnce(Option<PathBuf>) + 'static) -> bool {
    let Some(portal_path) = PortalPath::parse(path) else {
        callback(None);
        return false;
    };
    enqueue_lookup(portal_path, Box::new(callback))
}

#[cfg(test)]
fn finish_lookup_for_tests(document_id: &str, host_path: Option<&Path>) {
    finish_host_lookup(document_id, host_path);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::{
        PortalPath, cache_host_path_for_tests, cached_display_path, enqueue_lookup_for_tests,
        finish_lookup_for_tests, portal_display_path, reset_cache_for_tests,
    };

    #[test]
    fn cache_miss_does_not_resolve_portal_display_path() {
        reset_cache_for_tests();
        assert_eq!(
            cached_display_path(Path::new(
                "/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md"
            )),
            None
        );
    }

    #[test]
    fn cached_document_id_resolves_portal_display_path() {
        reset_cache_for_tests();
        cache_host_path_for_tests(
            "23ef3b31",
            PathBuf::from("/home/cadric/Dokumenter/CoreOS_Server"),
        );
        assert_eq!(
            cached_display_path(Path::new(
                "/run/user/1000/doc/23ef3b31/CoreOS_Server/policy/docs.policy.json"
            )),
            Some(PathBuf::from(
                "/home/cadric/Dokumenter/CoreOS_Server/policy/docs.policy.json"
            ))
        );
    }

    #[test]
    fn in_flight_lookups_are_coalesced_per_document_id() {
        reset_cache_for_tests();
        let results = Rc::new(RefCell::new(Vec::new()));
        let first_results = Rc::clone(&results);
        let first_started = enqueue_lookup_for_tests(
            Path::new("/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md"),
            move |path| first_results.borrow_mut().push(path),
        );
        let second_results = Rc::clone(&results);
        let second_started = enqueue_lookup_for_tests(
            Path::new("/run/user/1000/doc/23ef3b31/CoreOS_Server/README.md"),
            move |path| second_results.borrow_mut().push(path),
        );

        assert!(first_started);
        assert!(!second_started);
        finish_lookup_for_tests(
            "23ef3b31",
            Some(Path::new("/home/cadric/Dokumenter/CoreOS_Server")),
        );
        assert_eq!(
            *results.borrow(),
            vec![
                Some(PathBuf::from(
                    "/home/cadric/Dokumenter/CoreOS_Server/AGENTS.md"
                )),
                Some(PathBuf::from(
                    "/home/cadric/Dokumenter/CoreOS_Server/README.md"
                )),
            ]
        );
    }

    #[test]
    fn failed_lookup_completes_callbacks_without_cache_entry() {
        reset_cache_for_tests();
        let results = Rc::new(RefCell::new(Vec::new()));
        let callback_results = Rc::clone(&results);
        assert!(enqueue_lookup_for_tests(
            Path::new("/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md"),
            move |path| callback_results.borrow_mut().push(path),
        ));
        finish_lookup_for_tests("23ef3b31", None);

        assert_eq!(*results.borrow(), vec![None]);
        assert_eq!(
            cached_display_path(Path::new(
                "/run/user/1000/doc/23ef3b31/CoreOS_Server/AGENTS.md"
            )),
            None
        );
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
}
