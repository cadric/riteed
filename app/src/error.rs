use std::path::PathBuf;

use gettextrs::{gettext, pgettext};
use gtk4::glib;

#[derive(Clone, Debug)]
pub enum AppError {
    Cancelled,
    Internal(String),
    MissingSavePath,
    NonLocalFile,
    DecodeFailed(PathBuf),
    FileSizeUnavailable(PathBuf),
    FileTooBig(PathBuf),
    SaveTooBig(PathBuf),
    ReadFailed(PathBuf, String),
    WriteFailed(PathBuf, String),
    HelpLaunchFailed(String),
}

impl AppError {
    #[must_use]
    pub fn title(&self) -> String {
        match self {
            Self::Cancelled => pgettext("error title", "Action Cancelled"),
            Self::Internal(_) => gettext("Unable to Build the Window"),
            Self::MissingSavePath => gettext("No Save Location Is Available"),
            Self::NonLocalFile => gettext("Only Local Files Are Supported"),
            Self::DecodeFailed(_)
            | Self::ReadFailed(_, _)
            | Self::FileSizeUnavailable(_)
            | Self::FileTooBig(_) => gettext("Unable to Open the File"),
            Self::SaveTooBig(_) | Self::WriteFailed(_, _) => gettext("Unable to Save the File"),
            Self::HelpLaunchFailed(_) => pgettext("error title", "Unable to Open Help"),
        }
    }

    #[must_use]
    pub fn body(&self) -> String {
        match self {
            Self::Cancelled => gettext("The Requested Action Was Cancelled."),
            Self::Internal(message) | Self::HelpLaunchFailed(message) => message.clone(),
            Self::MissingSavePath => gettext("Choose a Save Location Before Saving This Document."),
            Self::NonLocalFile => {
                gettext("Riteed Only Supports Local Plain Text Files in This Version.")
            }
            Self::DecodeFailed(path) => {
                gettext(
                    "Automatic text decoding was not reliable for this file. Choose a text encoding manually and try again.",
                ) + "\n\n"
                    + &path.display().to_string()
            }
            Self::FileSizeUnavailable(path) => {
                gettext("Riteed could not determine the file size and did not open the file.")
                    + "\n\n"
                    + &path.display().to_string()
            }
            Self::FileTooBig(path) => {
                gettext("The file is too large to open safely.")
                    + "\n\n"
                    + &path.display().to_string()
            }
            Self::SaveTooBig(path) => {
                gettext("The document is too large to save safely.")
                    + "\n\n"
                    + &path.display().to_string()
            }
            Self::ReadFailed(path, message) | Self::WriteFailed(path, message) => {
                path.display().to_string() + "\n\n" + message
            }
        }
    }
}

impl From<glib::Error> for AppError {
    fn from(error: glib::Error) -> Self {
        Self::Internal(error.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn titles_and_bodies_are_non_empty() {
        let errors = [
            AppError::Cancelled,
            AppError::Internal(String::from("internal")),
            AppError::MissingSavePath,
            AppError::NonLocalFile,
            AppError::DecodeFailed("notes.txt".into()),
            AppError::FileSizeUnavailable("notes.txt".into()),
            AppError::FileTooBig("notes.txt".into()),
            AppError::SaveTooBig("notes.txt".into()),
            AppError::ReadFailed("notes.txt".into(), String::from("read")),
            AppError::WriteFailed("notes.txt".into(), String::from("write")),
            AppError::HelpLaunchFailed(String::from("help")),
        ];

        for error in errors {
            assert!(!error.title().is_empty());
            assert!(!error.body().is_empty());
        }
    }
}
