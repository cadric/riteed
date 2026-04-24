use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::error::AppError;
use crate::workspace::{OpenSource, Workspace};

use super::ProjectState;

const SYMLINK_QUERY_ATTRIBUTES: &str = "standard::type,standard::target-uri";

pub(super) fn handle_symlink_activation(state: &Rc<RefCell<ProjectState>>, file: &gio::File) {
    let (generation, cancellable) = {
        let mut state_mut = state.borrow_mut();
        if let Some(cancellable) = state_mut.symlink_cancellable.take() {
            cancellable.cancel();
        }
        state_mut.symlink_generation += 1;
        let generation = state_mut.symlink_generation;
        let cancellable = gio::Cancellable::new();
        state_mut.symlink_cancellable = Some(cancellable.clone());
        (generation, cancellable)
    };

    let state_for_callback = Rc::clone(state);
    let file_for_callback = file.clone();
    file.query_info_async(
        SYMLINK_QUERY_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        Some(&cancellable),
        move |result| {
            if state_for_callback.borrow().symlink_generation != generation {
                return;
            }

            let mut state_mut = state_for_callback.borrow_mut();
            state_mut.symlink_cancellable = None;
            let workspace = match result {
                Ok(info) => handle_symlink_result(&mut state_mut, &file_for_callback, &info),
                Err(error) => {
                    if error.matches(gio::IOErrorEnum::Cancelled) {
                        return;
                    }
                    let key = format!("symlink-error:{}", file_for_callback.uri());
                    if state_mut.toast_keys.insert(key) {
                        state_mut
                            .toast_overlay
                            .add_toast(adw::Toast::new(&AppError::from(error).body()));
                    }
                    None
                }
            };
            drop(state_mut);
            if let Some(workspace) = workspace {
                workspace
                    .request_open_files(vec![file_for_callback.clone()], OpenSource::ProjectTree);
            }
        },
    );
}

fn handle_symlink_result(
    state: &mut ProjectState,
    file: &gio::File,
    info: &gio::FileInfo,
) -> Option<Rc<Workspace>> {
    let target_uri = info
        .attribute_as_string("standard::target-uri")
        .map_or_else(|| file.uri().to_string(), |value| value.to_string());

    match info.file_type() {
        gio::FileType::Directory => {
            let key = format!("symlink-dir:{target_uri}");
            if state.toast_keys.insert(key) {
                state.toast_overlay.add_toast(adw::Toast::new(&gettext(
                    "Opening folder links is not supported yet.",
                )));
            }
            None
        }
        gio::FileType::Regular => state.workspace.upgrade(),
        _ => {
            let key = format!("symlink-unsupported:{target_uri}");
            if state.toast_keys.insert(key) {
                state
                    .toast_overlay
                    .add_toast(adw::Toast::new(&gettext("That link could not be opened.")));
            }
            None
        }
    }
}
