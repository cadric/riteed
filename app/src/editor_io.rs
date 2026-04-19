use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::gio;
use gtk4::prelude::*;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct LoadedDocument {
    pub path: PathBuf,
    pub text: String,
    pub uri: String,
}

#[derive(Clone, Debug)]
pub struct SavedDocument {
    pub path: PathBuf,
    pub text: String,
    pub uri: String,
}

pub fn load_utf8_file(file: &gio::File, callback: Rc<dyn Fn(Result<LoadedDocument, AppError>)>) {
    let path = match local_path(file) {
        Ok(path) => path,
        Err(error) => {
            callback(Err(error));
            return;
        }
    };

    let file_clone = file.clone();
    file.load_contents_async(None::<&gio::Cancellable>, move |result| match result {
        Ok((bytes, _etag)) => match String::from_utf8(bytes.as_ref().to_vec()) {
            Ok(text) => callback(Ok(LoadedDocument {
                path: path.clone(),
                text,
                uri: file_clone.uri().to_string(),
            })),
            Err(_) => callback(Err(AppError::InvalidUtf8(path.clone()))),
        },
        Err(error) => callback(Err(AppError::ReadFailed(
            path.clone(),
            error.message().to_string(),
        ))),
    });
}

pub fn save_utf8_file(
    path: &Path,
    text: String,
    callback: Rc<dyn Fn(Result<SavedDocument, AppError>)>,
) {
    let path = path.to_path_buf();
    let file = gio::File::for_path(&path);
    let save_file = file.clone();
    file.replace_contents_async(
        text.clone().into_bytes(),
        None,
        false,
        gio::FileCreateFlags::REPLACE_DESTINATION,
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok((_etag, _)) => callback(Ok(SavedDocument {
                path: path.clone(),
                text: text.clone(),
                uri: save_file.uri().to_string(),
            })),
            Err((_bytes, error)) => callback(Err(AppError::WriteFailed(
                path.clone(),
                error.message().to_string(),
            ))),
        },
    );
}

/// # Errors
///
/// Returns an error when the provided file is not backed by a local path.
pub fn local_path(file: &gio::File) -> Result<PathBuf, AppError> {
    file.path().ok_or(AppError::NonLocalFile)
}
