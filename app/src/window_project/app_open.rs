use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::workspace::OpenSource;

use super::{ProjectState, RootChangeOrigin, begin_root_change};

struct AppOpenRequest {
    index: usize,
    items: Vec<gio::File>,
    directories: Vec<gio::File>,
    regular_files: Vec<gio::File>,
}

pub(super) fn handle_application_open(state: &Rc<RefCell<ProjectState>>, files: Vec<gio::File>) {
    if files.is_empty() {
        return;
    }
    let request = Rc::new(RefCell::new(AppOpenRequest {
        index: 0,
        items: files,
        directories: Vec::new(),
        regular_files: Vec::new(),
    }));
    process_app_open_request(state, request);
}

fn process_app_open_request(
    state: &Rc<RefCell<ProjectState>>,
    request: Rc<RefCell<AppOpenRequest>>,
) {
    let next_item = {
        let mut request = request.borrow_mut();
        if request.index >= request.items.len() {
            None
        } else {
            let item = request.items[request.index].clone();
            request.index += 1;
            Some(item)
        }
    };

    let Some(item) = next_item else {
        finish_app_open_request(state, &request);
        return;
    };

    let state_for_callback = Rc::clone(state);
    let item_for_callback = item.clone();
    item.query_info_async(
        "standard::type",
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        None::<&gio::Cancellable>,
        move |result| {
            match result {
                Ok(info) => {
                    if info.file_type() == gio::FileType::Directory {
                        request
                            .borrow_mut()
                            .directories
                            .push(item_for_callback.clone());
                    } else {
                        request
                            .borrow_mut()
                            .regular_files
                            .push(item_for_callback.clone());
                    }
                }
                Err(_) => {
                    request
                        .borrow_mut()
                        .regular_files
                        .push(item_for_callback.clone());
                }
            }
            process_app_open_request(&state_for_callback, request);
        },
    );
}

fn finish_app_open_request(
    state: &Rc<RefCell<ProjectState>>,
    request: &Rc<RefCell<AppOpenRequest>>,
) {
    let (directory, has_extra_directories, regular_files) = {
        let request = request.borrow();
        (
            request.directories.first().cloned(),
            request.directories.len() > 1,
            request.regular_files.clone(),
        )
    };
    let toast_overlay = if has_extra_directories {
        Some({
            let state = state.borrow();
            state.toast_overlay.clone()
        })
    } else {
        None
    };
    let workspace = if regular_files.is_empty() {
        None
    } else {
        let state = state.borrow();
        state.workspace.upgrade()
    };

    if let Some(directory) = directory {
        begin_root_change(state, &directory, RootChangeOrigin::AppOpen);
        if let Some(toast_overlay) = toast_overlay {
            toast_overlay.add_toast(adw::Toast::new(&gettext(
                "Only one folder can be opened at a time.",
            )));
        }
    }

    if let Some(workspace) = workspace {
        workspace.request_open_files(regular_files, OpenSource::AppOpen);
    }
}
