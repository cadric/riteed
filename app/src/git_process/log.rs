use std::rc::Rc;

use gtk4::gio;

use super::{GitCallback, GitProcess, GitProcessError};

const LOG_CAP: usize = 256 * 1024;
const MAX_LOG_LIMIT: usize = 25;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitCommitSummary {
    pub(crate) short_hash: String,
    pub(crate) full_hash: String,
    pub(crate) author: String,
    pub(crate) date: String,
    pub(crate) subject: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitLogState {
    Commits(Vec<GitCommitSummary>),
    NoHistory,
}

impl GitProcess {
    pub(crate) fn recent_commits(
        &self,
        limit: usize,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitLogState>,
    ) {
        let limit = limit.clamp(1, MAX_LOG_LIMIT).to_string();
        self.run(
            [
                "log",
                "-z",
                "-n",
                limit.as_str(),
                "--date=short",
                "--pretty=format:%h%x00%H%x00%an%x00%ad%x00%s",
            ],
            None,
            LOG_CAP,
            false,
            cancellable,
            Rc::new(move |result| callback(parse_log_result(result))),
        );
    }
}

fn parse_log_result(
    result: Result<super::GitRunOutput, GitProcessError>,
) -> Result<GitLogState, GitProcessError> {
    match result {
        Ok(output) => parse_recent_commits(&output.stdout).map(GitLogState::Commits),
        Err(GitProcessError::CommandFailed(stderr)) if no_history_error(&stderr) => {
            Ok(GitLogState::NoHistory)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn parse_recent_commits(bytes: &[u8]) -> Result<Vec<GitCommitSummary>, GitProcessError> {
    let mut fields: Vec<&[u8]> = bytes.split(|byte| *byte == 0).collect();
    while fields.last().is_some_and(|field| field.is_empty()) {
        let _removed = fields.pop();
    }
    if fields.is_empty() {
        return Ok(Vec::new());
    }
    if !fields.len().is_multiple_of(5) {
        return Err(GitProcessError::ParseFailed);
    }
    fields
        .chunks(5)
        .map(|chunk| {
            Ok(GitCommitSummary {
                short_hash: field_text(chunk[0])?,
                full_hash: field_text(chunk[1])?,
                author: field_text(chunk[2])?,
                date: field_text(chunk[3])?,
                subject: field_text(chunk[4])?,
            })
        })
        .collect()
}

fn field_text(bytes: &[u8]) -> Result<String, GitProcessError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| GitProcessError::ParseFailed)
}

fn no_history_error(stderr: &str) -> bool {
    stderr.contains("does not have any commits yet")
        || stderr.contains("your current branch")
        || stderr.contains("bad default revision")
}

#[cfg(test)]
mod tests {
    use super::{GitCommitSummary, GitLogState, parse_log_result, parse_recent_commits};
    use crate::git_process::{GitProcessError, GitRunOutput};

    #[test]
    fn parser_reads_five_nul_separated_fields() {
        let bytes = b"abc\x00abcdef\x00Ada\x002026-04-26\x00Initial commit\x00def\x00defghi\x00Bea\x002026-04-25\x00Follow-up";
        let commits = parse_recent_commits(bytes);
        assert_eq!(
            commits,
            Ok(vec![
                GitCommitSummary {
                    short_hash: String::from("abc"),
                    full_hash: String::from("abcdef"),
                    author: String::from("Ada"),
                    date: String::from("2026-04-26"),
                    subject: String::from("Initial commit"),
                },
                GitCommitSummary {
                    short_hash: String::from("def"),
                    full_hash: String::from("defghi"),
                    author: String::from("Bea"),
                    date: String::from("2026-04-25"),
                    subject: String::from("Follow-up"),
                },
            ])
        );
    }

    #[test]
    fn parser_tolerates_trailing_record_separator() {
        let commits =
            parse_recent_commits(b"abc\x00abcdef\x00Ada\x002026-04-26\x00Initial commit\x00");
        assert!(commits.is_ok_and(|items| items.len() == 1));
    }

    #[test]
    fn parser_rejects_partial_records() {
        assert!(matches!(
            parse_recent_commits(b"abc\0abcdef"),
            Err(GitProcessError::ParseFailed)
        ));
    }

    #[test]
    fn no_history_errors_are_distinct_from_transient_failures() {
        assert_eq!(
            parse_log_result(Err(GitProcessError::CommandFailed(String::from(
                "fatal: your current branch 'main' does not have any commits yet"
            )))),
            Ok(GitLogState::NoHistory)
        );
        assert!(matches!(
            parse_log_result(Err(GitProcessError::CommandFailed(String::from(
                "fatal: not a git repository"
            )))),
            Err(GitProcessError::CommandFailed(_))
        ));
        assert_eq!(
            parse_log_result(Ok(GitRunOutput {
                status: 0,
                stdout: Vec::new()
            })),
            Ok(GitLogState::Commits(Vec::new()))
        );
    }
}
