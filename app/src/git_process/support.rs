#[cfg(not(test))]
const GIT_BIN: &str = "/app/bin/git";
#[cfg(test)]
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

pub(super) fn git_env() -> [(&'static str, &'static str); 9] {
    [
        ("LC_ALL", "C"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
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
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| String::from("Git command failed."))
}

pub(super) fn redact_git_argv(argv: &[String]) -> Vec<String> {
    argv.iter()
        .map(|arg| match arg.as_str() {
            value if value.starts_with("user.name=") => String::from("user.name=<redacted>"),
            value if value.starts_with("user.email=") => String::from("user.email=<redacted>"),
            _ => arg.clone(),
        })
        .collect()
}
