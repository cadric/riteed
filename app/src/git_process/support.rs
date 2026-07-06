#[cfg(all(not(test), not(feature = "stress")))]
const GIT_BIN: &str = "/app/bin/git";
// The feature-gated `riteed-stress` driver and unit tests run outside the
// Flatpak sandbox and use host Git plumbing instead of `/app/bin/git`. This
// path is never compiled into the released Flatpak app binary.
#[cfg(any(test, feature = "stress"))]
const GIT_BIN: &str = "/usr/bin/git";
const FALSE_BIN: &str = "/usr/bin/false";

use super::GitProcessError;

pub(super) fn base_args() -> Vec<String> {
    [
        GIT_BIN,
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub(super) fn git_env() -> [(&'static str, &'static str); 10] {
    [
        ("LC_ALL", "C"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_LITERAL_PATHSPECS", "1"),
        ("GIT_PAGER", "cat"),
        ("PAGER", "cat"),
        ("GIT_ASKPASS", FALSE_BIN),
        ("SSH_ASKPASS", FALSE_BIN),
        ("GIT_EDITOR", FALSE_BIN),
        ("GIT_EXTERNAL_DIFF", FALSE_BIN),
    ]
}

pub(super) fn optional_text(result: Result<String, GitProcessError>) -> String {
    result.unwrap_or_else(|_error| String::new())
}

pub(super) fn identity_part_is_valid(value: &str) -> bool {
    !value.contains('\n') && !value.contains('\r') && !value.contains('\0')
}

pub(super) fn stderr_text(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes)
        .map_or_else(|_| String::from("Git command failed."), ToOwned::to_owned)
}
