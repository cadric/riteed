use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::dialogs;
use crate::document_limits::{
    self, OpenDecision, OpenFilePlan, OpenFileSupport, OpenPlanQueryResult,
};
use crate::editor_tab::{DocumentReadGuard, EditorTab};
use crate::error::AppError;
use crate::large_file::file_path_for_error;
use crate::workspace::{OpenSource, Workspace};

mod pending;

#[cfg(test)]
pub(crate) use pending::tests::exercise_pending_open_ownership;

use pending::{acquire_open_target, clear_pending_open, find_tab_by_file, register_pending_open};

struct OpenRequest {
    source: OpenSource,
    files: Vec<gio::File>,
    index: usize,
    failures: usize,
    successes: usize,
    selected_file: Option<gio::File>,
    restored_selected_page: Option<adw::TabPage>,
}

pub(crate) fn request_open_dialog(workspace: &Rc<Workspace>, parent: &adw::ApplicationWindow) {
    workspace.ensure_default_tab();
    let dialog = gtk4::FileDialog::builder()
        .title(pgettext("file dialog title", "Open Files"))
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
        selected_file: selected_uri.map(|uri| gio::File::for_uri(&uri)),
        restored_selected_page: None,
    }));
    process_open_request(workspace, request);
}

pub(crate) fn request_open_file_then(
    workspace: &Rc<Workspace>,
    file: &gio::File,
    source: OpenSource,
    callback: Rc<dyn Fn(Result<Rc<crate::editor_tab::EditorTab>, AppError>)>,
) {
    if workspace.tab_view.n_pages() == 0 {
        let _tab = workspace.add_empty_tab(true);
    }
    if let Some(existing) = find_tab_by_file(workspace, file) {
        if source != OpenSource::SessionRestore {
            workspace.remember_recent_uri(
                &existing
                    .session_uri()
                    .unwrap_or_else(|| file.uri().to_string()),
            );
        }
        if let Some(page) = existing.page() {
            workspace.tab_view.set_selected_page(&page);
        }
        workspace.refresh_selected_state();
        callback(Ok(existing));
        return;
    }
    let (tab, remove_on_failure) = acquire_open_target(workspace);
    let pending_token = register_pending_open(workspace, file, &tab);
    if let Some(page) = tab.page() {
        workspace.tab_view.set_selected_page(&page);
    }
    let weak = Rc::downgrade(workspace);
    let opened_file = file.clone();
    let tab_for_result = tab.clone();
    let expected_page = tab.page();
    open_file_in_tab(
        workspace,
        &tab,
        file,
        source,
        Rc::new(move |result| {
            if let Some(workspace) = weak.upgrade() {
                clear_pending_open(&workspace, &opened_file, &tab_for_result, pending_token);
                if !open_target_is_current(&workspace, &tab_for_result, expected_page.as_ref()) {
                    callback(Err(AppError::Cancelled));
                    return;
                }
                match result {
                    Ok(uri) => {
                        if source != OpenSource::SessionRestore {
                            workspace.remember_recent_uri(&uri);
                        }
                        workspace.persist_session_state_if_needed();
                        workspace.refresh_selected_state();
                        callback(Ok(tab_for_result.clone()));
                    }
                    Err(error) => {
                        if remove_on_failure
                            && !matches!(error, AppError::DocumentChangedDuringRead)
                        {
                            workspace.close_tab_if_clean(&tab_for_result);
                        }
                        if source != OpenSource::SourceControl
                            || matches!(error, AppError::DocumentChangedDuringRead)
                        {
                            handle_open_failure(&workspace, source, &opened_file, &error);
                        }
                        callback(Err(error));
                    }
                }
            }
        }),
    );
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

    if source == OpenSource::SessionRestore
        && !crate::document_limits::file_supports_session_restore(&file)
    {
        request.borrow_mut().failures += 1;
        process_open_request(workspace, request);
        return;
    }

    if let Some(existing) = find_tab_by_file(workspace, &file) {
        request.borrow_mut().successes += 1;
        let existing_uri = existing
            .session_uri()
            .unwrap_or_else(|| desired_uri.clone());
        if source != OpenSource::SessionRestore {
            workspace.remember_recent_uri(&existing_uri);
        } else if request
            .borrow()
            .selected_file
            .as_ref()
            .is_some_and(|wanted| wanted.equal(&file))
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
    let pending_token = register_pending_open(workspace, &file, &tab);
    if let Some(page) = tab.page() {
        workspace.tab_view.set_selected_page(&page);
    }

    let weak = Rc::downgrade(workspace);
    let opened_file = file.clone();
    let tab_for_result = tab.clone();
    let expected_page = tab.page();
    open_file_in_tab(
        workspace,
        &tab,
        &file,
        source,
        Rc::new(move |result| {
            if let Some(workspace) = weak.upgrade() {
                clear_pending_open(&workspace, &opened_file, &tab_for_result, pending_token);
                if !open_target_is_current(&workspace, &tab_for_result, expected_page.as_ref()) {
                    request.borrow_mut().failures += 1;
                    process_open_request(&workspace, request.clone());
                    return;
                }
                match result {
                    Ok(uri) => {
                        request.borrow_mut().successes += 1;
                        if source != OpenSource::SessionRestore {
                            workspace.remember_recent_uri(&uri);
                        } else if request
                            .borrow()
                            .selected_file
                            .as_ref()
                            .is_some_and(|wanted| wanted.equal(&gio::File::for_uri(&uri)))
                        {
                            request.borrow_mut().restored_selected_page = tab_for_result.page();
                        }
                        workspace.persist_session_state_if_needed();
                        workspace.refresh_selected_state();
                    }
                    Err(error) => {
                        if !matches!(error, AppError::DocumentChangedDuringRead) {
                            request.borrow_mut().failures += 1;
                        }
                        if remove_on_failure
                            && !matches!(error, AppError::DocumentChangedDuringRead)
                        {
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

fn open_file_in_tab(
    workspace: &Rc<Workspace>,
    tab: &Rc<EditorTab>,
    file: &gio::File,
    source: OpenSource,
    callback: Rc<dyn Fn(Result<String, AppError>)>,
) {
    let read_guard = tab.capture_document_read_guard();
    let weak = Rc::downgrade(workspace);
    let tab = tab.clone();
    let file = file.clone();
    let file_for_query = file.clone();
    document_limits::query_file_open_plan(
        &file_for_query,
        None::<&gio::Cancellable>,
        Rc::new(move |result| {
            let Some(workspace) = weak.upgrade() else {
                return;
            };
            if let Err(error) = verify_open_read_guard(&workspace, &tab, &read_guard) {
                callback(Err(error));
                return;
            }
            let size = match result {
                Ok(OpenPlanQueryResult::KnownSize(size)) => size,
                Ok(OpenPlanQueryResult::NonRegular) => {
                    callback(Err(non_regular_file_error(&file)));
                    return;
                }
                Ok(OpenPlanQueryResult::SizeUnavailable) => {
                    callback(Err(file_size_unavailable_error(&file)));
                    return;
                }
                Err(error) => {
                    callback(Err(AppError::from(error)));
                    return;
                }
            };
            let plan = OpenFilePlan {
                size,
                decision: document_limits::open_decision_for_size_with_thresholds(
                    size,
                    &workspace.settings.large_file_thresholds(),
                ),
            };
            route_open_plan(
                &workspace,
                &tab,
                &file,
                source,
                plan,
                read_guard.clone(),
                callback.clone(),
            );
        }),
    );
}

fn route_open_plan(
    workspace: &Rc<Workspace>,
    tab: &Rc<EditorTab>,
    file: &gio::File,
    source: OpenSource,
    plan: OpenFilePlan,
    read_guard: DocumentReadGuard,
    callback: Rc<dyn Fn(Result<String, AppError>)>,
) {
    if let Err(error) = verify_open_read_guard(workspace, tab, &read_guard) {
        callback(Err(error));
        return;
    }
    match plan.decision {
        OpenDecision::Editor { .. } => tab.clone().load_file_with_open_support_guarded(
            &workspace.shell,
            file,
            OpenFileSupport {
                supports_open: true,
                size: Some(plan.size),
            },
            read_guard,
            callback,
        ),
        OpenDecision::Viewer { .. } if source == OpenSource::SessionRestore => {
            let weak = Rc::downgrade(workspace);
            let weak_tab = Rc::downgrade(tab);
            tab.show_large_file_restore_placeholder(
                &workspace.shell,
                file,
                plan.size,
                plan.decision,
                Rc::new(move || {
                    let Some(workspace) = weak.upgrade() else {
                        return;
                    };
                    if let Some(tab) = weak_tab.upgrade()
                        && let Some(page) = tab.page()
                    {
                        workspace.tab_view.close_page(&page);
                    }
                }),
                callback,
            );
        }
        OpenDecision::Viewer { .. } => {
            tab.open_viewer_for_large_file(
                &workspace.shell,
                file,
                plan.size,
                plan.decision,
                callback,
            );
        }
    }
}

fn non_regular_file_error(file: &gio::File) -> AppError {
    AppError::ReadFailed(
        file_path_for_error(file),
        gettext("The selected item is not a regular file."),
    )
}

fn file_size_unavailable_error(file: &gio::File) -> AppError {
    AppError::FileSizeUnavailable(file_path_for_error(file))
}

fn open_target_is_current(
    workspace: &Workspace,
    tab: &Rc<EditorTab>,
    expected_page: Option<&adw::TabPage>,
) -> bool {
    let Some(expected_page) = expected_page else {
        return false;
    };
    tab.page()
        .as_ref()
        .is_some_and(|page| page == expected_page)
        && workspace
            .find_tab_by_page(expected_page)
            .is_some_and(|current| Rc::ptr_eq(&current, tab))
}

fn verify_open_read_guard(
    workspace: &Workspace,
    tab: &Rc<EditorTab>,
    read_guard: &DocumentReadGuard,
) -> Result<(), AppError> {
    if !open_target_is_current(workspace, tab, read_guard.page()) {
        return Err(AppError::Cancelled);
    }
    tab.verify_document_read_guard(read_guard)
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
        OpenSource::Dialog
        | OpenSource::AppOpen
        | OpenSource::Recent
        | OpenSource::ProjectTree
        | OpenSource::SourceControl => {}
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
    workspace.finish_session_restore_without_startup_write();
    workspace.refresh_selected_state();
}

fn handle_open_failure(
    workspace: &Rc<Workspace>,
    source: OpenSource,
    file: &gio::File,
    error: &AppError,
) {
    if matches!(error, AppError::Cancelled) {
        return;
    }
    if matches!(error, AppError::DocumentChangedDuringRead) {
        workspace.show_toast(&error.body());
        return;
    }
    match source {
        OpenSource::Recent => {
            dialogs::present_error(&workspace.shell, error);
            prune_recent_if_missing(workspace, file);
        }
        OpenSource::Dialog | OpenSource::AppOpen | OpenSource::ProjectTree => {
            dialogs::present_error(&workspace.shell, error);
        }
        OpenSource::SourceControl | OpenSource::SessionRestore | OpenSource::Drop => {}
    }
}

fn prune_recent_if_missing(workspace: &Rc<Workspace>, file: &gio::File) {
    let weak = Rc::downgrade(workspace);
    let uri = file.uri().to_string();
    file.query_info_async(
        "standard::type",
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        None::<&gio::Cancellable>,
        move |result| {
            let Err(error) = result else {
                return;
            };
            if !error.matches(gio::IOErrorEnum::NotFound) {
                return;
            }
            if let Some(workspace) = weak.upgrade() {
                workspace.prune_recent_uri(&uri);
            }
        },
    );
}

fn apply_text_filters(dialog: &gtk4::FileDialog) {
    let text_filter = gtk4::FileFilter::new();
    text_filter.set_name(Some(&pgettext("file filter", "Plain Text Files")));
    text_filter.add_mime_type("text/plain");
    text_filter.add_suffix("txt");

    let markdown_filter = gtk4::FileFilter::new();
    markdown_filter.set_name(Some(&pgettext("file filter", "Markdown Source Files")));
    markdown_filter.add_mime_type("text/markdown");
    markdown_filter.add_suffix("md");
    markdown_filter.add_suffix("markdown");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&markdown_filter);
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
