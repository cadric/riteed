use std::path::{Component, Path, PathBuf};

use super::GitProcessError;

const CONTEXT_LINE_COUNT: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitRepoContext {
    pub(crate) work_tree: PathBuf,
    pub(crate) git_dir: PathBuf,
    pub(crate) git_common_dir: PathBuf,
    pub(crate) head_path: PathBuf,
    pub(crate) index_path: PathBuf,
    pub(crate) index_lock_path: PathBuf,
    pub(crate) refs_heads_dir: PathBuf,
    pub(crate) packed_refs_path: PathBuf,
}

impl GitRepoContext {
    pub(super) fn parse(
        bytes: &[u8],
        fallback_base: &Path,
        paths_are_absolute: bool,
    ) -> Result<Self, GitProcessError> {
        let text = String::from_utf8(bytes.to_vec()).map_err(|_| GitProcessError::ParseFailed)?;
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        if lines.len() != CONTEXT_LINE_COUNT {
            return Err(GitProcessError::ParseFailed);
        }
        Ok(Self {
            work_tree: output_path(lines[0], fallback_base, paths_are_absolute)?,
            git_dir: output_path(lines[1], fallback_base, paths_are_absolute)?,
            git_common_dir: output_path(lines[2], fallback_base, paths_are_absolute)?,
            head_path: output_path(lines[3], fallback_base, paths_are_absolute)?,
            index_path: output_path(lines[4], fallback_base, paths_are_absolute)?,
            index_lock_path: output_path(lines[5], fallback_base, paths_are_absolute)?,
            refs_heads_dir: output_path(lines[6], fallback_base, paths_are_absolute)?,
            packed_refs_path: output_path(lines[7], fallback_base, paths_are_absolute)?,
        })
    }
}

pub(super) fn parse_single_git_path(
    text: &str,
    fallback_base: &Path,
    paths_are_absolute: bool,
) -> Result<PathBuf, GitProcessError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(GitProcessError::ParseFailed);
    }
    output_path(trimmed, fallback_base, paths_are_absolute)
}

pub(super) fn fallback_base(folder: &Path) -> PathBuf {
    if folder.is_absolute() {
        return normalize_path(folder);
    }
    let Ok(current) = std::env::current_dir() else {
        return normalize_path(folder);
    };
    normalize_path(&current.join(folder))
}

fn output_path(
    value: &str,
    fallback_base: &Path,
    paths_are_absolute: bool,
) -> Result<PathBuf, GitProcessError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GitProcessError::ParseFailed);
    }
    let path = PathBuf::from(trimmed);
    if paths_are_absolute || path.is_absolute() {
        return Ok(normalize_path(&path));
    }
    Ok(normalize_path(&fallback_base.join(path)))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::RootDir))
                {
                    continue;
                }
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{GitRepoContext, fallback_base, parse_single_git_path};

    #[test]
    fn context_parser_reads_absolute_rev_parse_output() {
        let output = b"/repo\0ignored";
        assert!(GitRepoContext::parse(output, Path::new("/repo"), true).is_err());

        let output = b"/repo\n/repo/.git\n/repo/.git\n/repo/.git/HEAD\n/repo/.git/index\n/repo/.git/index.lock\n/repo/.git/refs/heads\n/repo/.git/packed-refs\n";
        let context = GitRepoContext::parse(output, Path::new("/repo"), true);
        assert!(
            context.is_ok_and(|repo| repo.index_lock_path == Path::new("/repo/.git/index.lock"))
        );
    }

    #[test]
    fn context_parser_absolutizes_relative_git_paths() {
        let output = b"/repo\n/repo/.git\n../.git\n../.git/HEAD\n../.git/index\n../.git/index.lock\n../.git/refs/heads\n../.git/packed-refs\n";
        let context = GitRepoContext::parse(output, Path::new("/repo/app"), false);
        assert!(context.is_ok_and(|repo| repo.head_path == Path::new("/repo/.git/HEAD")));
    }

    #[test]
    fn single_git_path_parser_absolutizes_fallback_output() {
        let path = parse_single_git_path("../.git/refs/heads/main", Path::new("/repo/app"), false);
        assert_eq!(path, Ok(PathBuf::from("/repo/.git/refs/heads/main")));
    }

    #[test]
    fn relative_fallback_base_uses_current_directory() {
        let base = fallback_base(Path::new("app"));
        assert!(base.ends_with("app"));
    }
}
