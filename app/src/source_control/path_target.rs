use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use gtk4::gio;
use gtk4::prelude::*;

#[must_use]
pub(crate) fn raw_path_for_uri(repo: &Path, uri: &str) -> Option<Vec<u8>> {
    let file = gio::File::for_uri(uri);
    let path = file.path()?;
    let relative = path.strip_prefix(repo).ok()?;
    Some(relative.as_os_str().as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    use gtk4::gio;
    use gtk4::prelude::*;

    use super::raw_path_for_uri;

    #[test]
    fn resolves_in_repo_uri_to_raw_relative_path() {
        let repo = PathBuf::from("/tmp/riteed-repo");
        let file = gio::File::for_path(repo.join("src/main.rs"));

        assert_eq!(
            raw_path_for_uri(&repo, file.uri().as_str()),
            Some(b"src/main.rs".to_vec())
        );
    }

    #[test]
    fn rejects_uri_outside_repo() {
        let repo = PathBuf::from("/tmp/riteed-repo");
        let file = gio::File::for_path("/tmp/other/file.rs");

        assert_eq!(raw_path_for_uri(&repo, file.uri().as_str()), None);
    }

    #[test]
    fn preserves_non_utf8_path_bytes() {
        let repo = PathBuf::from("/tmp/riteed-repo");
        let mut raw = b"bad-".to_vec();
        raw.push(0xFF);
        raw.extend_from_slice(b".txt");
        let relative = PathBuf::from(OsString::from_vec(raw.clone()));
        let file = gio::File::for_path(repo.join(relative));

        assert_eq!(raw_path_for_uri(&repo, file.uri().as_str()), Some(raw));
    }

    #[test]
    fn rejects_non_file_uri() {
        let repo = PathBuf::from("/tmp/riteed-repo");

        assert_eq!(raw_path_for_uri(&repo, "resource:///app/file.rs"), None);
    }
}
