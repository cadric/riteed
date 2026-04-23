use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::dialogs;
use crate::error::AppError;
use crate::workspace::{OpenSource, Workspace};

struct OpenRequest {
    source: OpenSource,
    files: Vec<gio::File>,
    index: usize,
    failures: usize,
    successes: usize,
    selected_uri: Option<String>,
    restored_selected_page: Option<adw::TabPage>,
}

pub(crate) fn request_open_dialog(workspace: &Rc<Workspace>, parent: &adw::ApplicationWindow) {
    workspace.ensure_default_tab();
    let dialog = gtk4::FileDialog::builder()
        .title(pgettext("file dialog title", "Open Text Files"))
        .accept_label(pgettext("file dialog action", "Open"))
        .modal(true)
        .build();
    apply_text_filters(&dialog);

    let weak = Rc::downgrade(workspace);
    dialog.open_multiple(Some(parent), None::<&gio::Cancellable>, move |result| {
        if let Some(workspace) = weak.upgrade() {
            match result {
                Ok(model) => {
                    workspace.request_open_files(files_from_model(&model), OpenSource::Dialog);
                }
                Err(error) => {
                    if !error.matches(gtk4::DialogError::Dismissed) {
                        dialogs::present_error(&workspace.shell, &AppError::from(error));
                    }
                }
            }
        }
    });
}

pub(crate) fn open_files_internal(
    workspace: &Rc<Workspace>,
    files: Vec<gio::File>,
    source: OpenSource,
    selected_uri: Option<String>,
) {
    if files.is_empty() {
        if source == OpenSource::SessionRestore {
            finish_restore(workspace, 0, None);
        }
        return;
    }

    if workspace.tab_view.n_pages() == 0 {
        let _tab = workspace.add_empty_tab(true);
    }

    let request = Rc::new(RefCell::new(OpenRequest {
        source,
        files,
        index: 0,
        failures: 0,
        successes: 0,
        selected_uri,
        restored_selected_page: None,
    }));
    process_open_request(workspace, request);
}

fn process_open_request(workspace: &Rc<Workspace>, request: Rc<RefCell<OpenRequest>>) {
    let next_open = {
        let mut state = request.borrow_mut();
        if state.index >= state.files.len() {
            None
        } else {
            let file = state.files[state.index].clone();
            state.index += 1;
            Some((state.source, file.clone(), file.uri().to_string()))
        }
    };
    let Some((source, file, desired_uri)) = next_open else {
        finish_open_request(workspace, &request);
        return;
    };

    if let Some(existing) = workspace.find_tab_by_uri(&desired_uri) {
        request.borrow_mut().successes += 1;
        if source != OpenSource::SessionRestore {
            workspace.remember_recent_uri(&desired_uri);
        } else if request
            .borrow()
            .selected_uri
            .as_deref()
            .is_some_and(|uri| uri == desired_uri)
        {
            request.borrow_mut().restored_selected_page = existing.page();
        }
        if let Some(page) = existing.page() {
            workspace.tab_view.set_selected_page(&page);
        }
        workspace.refresh_selected_state();
        process_open_request(workspace, request);
        return;
    }

    let (tab, remove_on_failure) = acquire_open_target(workspace);
    if let Some(page) = tab.page() {
        workspace.tab_view.set_selected_page(&page);
    }

    let weak = Rc::downgrade(workspace);
    let opened_file = file.clone();
    let tab_for_result = tab.clone();
    tab.clone().load_file(
        &workspace.shell,
        &file,
        Rc::new(move |result| {
            if let Some(workspace) = weak.upgrade() {
                match result {
                    Ok(uri) => {
                        request.borrow_mut().successes += 1;
                        if source != OpenSource::SessionRestore {
                            workspace.remember_recent_uri(&uri);
                        } else if request
                            .borrow()
                            .selected_uri
                            .as_deref()
                            .is_some_and(|wanted| wanted == uri.as_str())
                        {
                            request.borrow_mut().restored_selected_page = tab_for_result.page();
                        }
                        workspace.persist_session_state_if_needed();
                        workspace.refresh_selected_state();
                    }
                    Err(error) => {
                        request.borrow_mut().failures += 1;
                        if remove_on_failure {
                            workspace.close_tab_if_clean(&tab_for_result);
                        }
                        handle_open_failure(&workspace, source, &opened_file, &error);
                    }
                }
                process_open_request(&workspace, request.clone());
            }
        }),
    );
}

fn finish_open_request(workspace: &Rc<Workspace>, request: &Rc<RefCell<OpenRequest>>) {
    let request = request.borrow();
    match request.source {
        OpenSource::Drop => {
            if request.failures > 0 && request.successes == 0 {
                dialogs::present_message(
                    &workspace.shell,
                    &gettext("Unable to Open Dropped Files"),
                    &gettext("Some files could not be opened."),
                );
            } else if request.failures > 0 {
                workspace.show_toast(&gettext("Some files could not be opened."));
            }
        }
        OpenSource::SessionRestore => {
            finish_restore(
                workspace,
                request.failures,
                request.restored_selected_page.clone(),
            );
        }
        OpenSource::Dialog | OpenSource::AppOpen | OpenSource::Recent => {}
    }
}

fn finish_restore(
    workspace: &Rc<Workspace>,
    failures: usize,
    restored_selected_page: Option<adw::TabPage>,
) {
    if workspace.tab_view.n_pages() > 0 {
        if let Some(page) = restored_selected_page {
            workspace.tab_view.set_selected_page(&page);
        } else if workspace.tab_view.n_pages() > 0 {
            workspace
                .tab_view
                .set_selected_page(&workspace.tab_view.nth_page(0));
        }
    } else {
        let _unused = workspace.add_empty_tab(true);
    }

    if failures > 0 {
        workspace.toast_overlay.add_toast(adw::Toast::new(&gettext(
            "Some files from the last session could not be reopened.",
        )));
    }
    workspace.state.borrow_mut().restoring_session = false;
    workspace.refresh_selected_state();
    workspace.persist_session_state_if_needed();
}

fn acquire_open_target(workspace: &Rc<Workspace>) -> (Rc<crate::editor_tab::EditorTab>, bool) {
    if workspace.tab_view.n_pages() == 1 {
        let existing = workspace.ordered_tabs();
        if let Some(tab) = existing.first()
            && tab.is_clean_untitled()
        {
            return (tab.clone(), false);
        }
    }
    (workspace.add_empty_tab(true), true)
}

fn handle_open_failure(
    workspace: &Workspace,
    source: OpenSource,
    file: &gio::File,
    error: &AppError,
) {
    match source {
        OpenSource::Recent => {
            dialogs::present_error(&workspace.shell, error);
            if file.path().as_ref().is_some_and(|path| !path.exists()) {
                workspace.prune_recent_uri(&file.uri());
            }
        }
        OpenSource::Dialog | OpenSource::AppOpen => dialogs::present_error(&workspace.shell, error),
        OpenSource::SessionRestore | OpenSource::Drop => {}
    }
}

fn apply_text_filters(dialog: &gtk4::FileDialog) {
    let text_filter = gtk4::FileFilter::new();
    text_filter.set_name(Some(&pgettext("file filter", "Plain Text Files")));
    text_filter.add_mime_type("text/plain");
    text_filter.add_suffix("txt");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&any_filter);

    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&text_filter));
}

fn files_from_model(model: &gio::ListModel) -> Vec<gio::File> {
    let mut files = Vec::new();
    for index in 0..model.n_items() {
        if let Some(item) = model.item(index)
            && let Ok(file) = item.downcast::<gio::File>()
        {
            files.push(file);
        }
    }
    files
}
