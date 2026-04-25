use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs;
use crate::editor_tab::EditorTab;
use crate::error::AppError;

use super::WindowCompareController;

#[derive(Clone)]
pub(super) enum CompareSlot {
    None,
    CurrentDocument(Rc<EditorTab>),
    SavedVersion,
    File(gio::File),
    Text(String),
}

impl CompareSlot {
    fn is_set(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn is_plain(&self) -> bool {
        matches!(self, Self::File(_) | Self::Text(_))
    }
}

pub(super) fn present_compare_dialog(controller: &Rc<WindowCompareController>) {
    let (current_document, left_initial, right_initial) = initial_compare_slots(controller);
    let ui = build_compare_dialog_ui(current_document.is_some());
    let state = Rc::new(CompareDialogState {
        controller: Rc::downgrade(controller),
        dialog: ui.dialog.clone(),
        current_document,
        autosave_enabled: controller.workspace.settings.autosave_enabled(),
        left: RefCell::new(left_initial),
        right: RefCell::new(right_initial),
        left_row: ui.left_row.clone(),
        right_row: ui.right_row.clone(),
        compare_button: ui.compare_button.clone(),
        swap_button: ui.swap_button.clone(),
        left_clear_button: ui.left_clear_button.clone(),
        right_clear_button: ui.right_clear_button.clone(),
        right_saved_button: ui.right_saved_button.clone(),
    });

    sync_compare_dialog(&state);
    wire_compare_dialog(controller, &state, &ui);
    ui.dialog.present(Some(&controller.shell));
}

struct CompareDialogUi {
    dialog: adw::Dialog,
    left_row: adw::ActionRow,
    right_row: adw::ActionRow,
    cancel_button: gtk4::Button,
    compare_button: gtk4::Button,
    swap_button: gtk4::Button,
    left_current_button: gtk4::Button,
    left_choose_file_button: gtk4::Button,
    left_paste_text_button: gtk4::Button,
    left_clear_button: gtk4::Button,
    right_saved_button: gtk4::Button,
    right_choose_file_button: gtk4::Button,
    right_paste_text_button: gtk4::Button,
    right_clear_button: gtk4::Button,
}

fn initial_compare_slots(
    controller: &WindowCompareController,
) -> (Option<Rc<EditorTab>>, CompareSlot, CompareSlot) {
    let current_document = controller.workspace.selected_tab();
    let left_initial = current_document.as_ref().map_or(CompareSlot::None, |tab| {
        CompareSlot::CurrentDocument(tab.clone())
    });
    let right_initial = match &left_initial {
        CompareSlot::CurrentDocument(tab)
            if tab.has_saved_local_uri()
                && tab.is_dirty()
                && !controller.workspace.settings.autosave_enabled() =>
        {
            CompareSlot::SavedVersion
        }
        _ => CompareSlot::None,
    };
    (current_document, left_initial, right_initial)
}

fn build_compare_dialog_ui(show_current_document_button: bool) -> CompareDialogUi {
    let dialog = adw::Dialog::builder()
        .title(pgettext("compare dialog title", "Compare"))
        .content_width(620)
        .follows_content_size(true)
        .can_close(true)
        .build();
    let description = gtk4::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .label(gettext(
            "Select the editable document on the left and the reference on the right.",
        ))
        .build();
    let left_row = adw::ActionRow::builder().activatable(false).build();
    let left_group = adw::PreferencesGroup::builder()
        .title(pgettext("compare dialog side", "Left Side"))
        .build();
    left_group.add(&left_row);
    let left_buttons = compare_source_buttons_row();
    let left_current_button =
        gtk4::Button::with_label(&pgettext("compare source", "Current Document"));
    left_current_button.set_visible(show_current_document_button);
    left_buttons.append(&left_current_button);
    let left_choose_file_button =
        gtk4::Button::with_label(&pgettext("compare dialog action", "Choose File..."));
    let left_paste_text_button =
        gtk4::Button::with_label(&pgettext("compare dialog action", "Paste Text..."));
    let left_clear_button = gtk4::Button::with_label(&pgettext("compare dialog action", "Clear"));
    left_buttons.append(&left_choose_file_button);
    left_buttons.append(&left_paste_text_button);
    left_buttons.append(&left_clear_button);
    let swap_button = gtk4::Button::with_label(&pgettext("compare dialog action", "Swap"));
    swap_button.set_halign(gtk4::Align::Center);
    let right_row = adw::ActionRow::builder().activatable(false).build();
    let right_group = adw::PreferencesGroup::builder()
        .title(pgettext("compare dialog side", "Right Side"))
        .build();
    right_group.add(&right_row);
    let right_buttons = compare_source_buttons_row();
    let right_saved_button = gtk4::Button::with_label(&pgettext("compare source", "Saved Version"));
    right_buttons.append(&right_saved_button);
    let right_choose_file_button =
        gtk4::Button::with_label(&pgettext("compare dialog action", "Choose File..."));
    let right_paste_text_button =
        gtk4::Button::with_label(&pgettext("compare dialog action", "Paste Text..."));
    let right_clear_button = gtk4::Button::with_label(&pgettext("compare dialog action", "Clear"));
    right_buttons.append(&right_choose_file_button);
    right_buttons.append(&right_paste_text_button);
    right_buttons.append(&right_clear_button);
    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .spacing(12)
        .build();
    let cancel_button = gtk4::Button::with_label(&pgettext("dialog button", "Cancel"));
    let compare_button = gtk4::Button::with_label(&pgettext("dialog button", "Compare"));
    compare_button.add_css_class("suggested-action");
    button_box.append(&cancel_button);
    button_box.append(&compare_button);
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&description);
    content.append(&left_group);
    content.append(&left_buttons);
    content.append(&swap_button);
    content.append(&right_group);
    content.append(&right_buttons);
    content.append(&button_box);
    dialog.set_child(Some(&content));
    CompareDialogUi {
        dialog,
        left_row,
        right_row,
        cancel_button,
        compare_button,
        swap_button,
        left_current_button,
        left_choose_file_button,
        left_paste_text_button,
        left_clear_button,
        right_saved_button,
        right_choose_file_button,
        right_paste_text_button,
        right_clear_button,
    }
}

fn wire_compare_dialog(
    controller: &Rc<WindowCompareController>,
    state: &Rc<CompareDialogState>,
    ui: &CompareDialogUi,
) {
    wire_cancel_action(&ui.dialog, &ui.cancel_button);
    wire_left_actions(controller, state, ui);
    wire_right_actions(controller, state, ui);
    wire_swap_action(state, &ui.swap_button);
    wire_compare_action(state, &ui.compare_button);
}

fn wire_cancel_action(dialog: &adw::Dialog, button: &gtk4::Button) {
    let dialog = dialog.clone();
    button.connect_clicked(move |_| {
        let _closed = dialog.close();
    });
}

fn wire_left_actions(
    controller: &Rc<WindowCompareController>,
    state: &Rc<CompareDialogState>,
    ui: &CompareDialogUi,
) {
    let state_for_current = state.clone();
    ui.left_current_button.connect_clicked(move |_| {
        if let Some(tab) = state_for_current.current_document.as_ref() {
            *state_for_current.left.borrow_mut() = CompareSlot::CurrentDocument(tab.clone());
            if matches!(&*state_for_current.right.borrow(), CompareSlot::None)
                && tab.has_saved_local_uri()
                && tab.is_dirty()
                && !state_for_current.autosave_enabled
            {
                *state_for_current.right.borrow_mut() = CompareSlot::SavedVersion;
            }
            sync_compare_dialog(&state_for_current);
        }
    });

    let state_for_file = state.clone();
    let parent = controller.shell.clone();
    ui.left_choose_file_button.connect_clicked(move |_| {
        let state = state_for_file.clone();
        choose_file(
            &parent,
            &pgettext("file dialog title", "Choose a File"),
            Rc::new(move |file| {
                if let Some(file) = file {
                    *state.left.borrow_mut() = CompareSlot::File(file);
                    sync_compare_dialog(&state);
                }
            }),
        );
    });

    let state_for_paste = state.clone();
    ui.left_paste_text_button.connect_clicked(move |_| {
        let dialog = state_for_paste.dialog.clone();
        let state = state_for_paste.clone();
        show_paste_text_dialog(
            &dialog,
            None,
            Rc::new(move |text| {
                if let Some(text) = text {
                    *state.left.borrow_mut() = CompareSlot::Text(text);
                    sync_compare_dialog(&state);
                }
            }),
        );
    });

    let state_for_clear = state.clone();
    ui.left_clear_button.connect_clicked(move |_| {
        *state_for_clear.left.borrow_mut() = CompareSlot::None;
        sync_compare_dialog(&state_for_clear);
    });
}

fn wire_right_actions(
    controller: &Rc<WindowCompareController>,
    state: &Rc<CompareDialogState>,
    ui: &CompareDialogUi,
) {
    let state_for_saved = state.clone();
    ui.right_saved_button.connect_clicked(move |_| {
        *state_for_saved.right.borrow_mut() = CompareSlot::SavedVersion;
        sync_compare_dialog(&state_for_saved);
    });

    let state_for_file = state.clone();
    let parent = controller.shell.clone();
    ui.right_choose_file_button.connect_clicked(move |_| {
        let state = state_for_file.clone();
        choose_file(
            &parent,
            &pgettext("file dialog title", "Choose a File"),
            Rc::new(move |file| {
                if let Some(file) = file {
                    *state.right.borrow_mut() = CompareSlot::File(file);
                    sync_compare_dialog(&state);
                }
            }),
        );
    });

    let state_for_paste = state.clone();
    ui.right_paste_text_button.connect_clicked(move |_| {
        let dialog = state_for_paste.dialog.clone();
        let state = state_for_paste.clone();
        show_paste_text_dialog(
            &dialog,
            None,
            Rc::new(move |text| {
                if let Some(text) = text {
                    *state.right.borrow_mut() = CompareSlot::Text(text);
                    sync_compare_dialog(&state);
                }
            }),
        );
    });

    let state_for_clear = state.clone();
    ui.right_clear_button.connect_clicked(move |_| {
        *state_for_clear.right.borrow_mut() = CompareSlot::None;
        sync_compare_dialog(&state_for_clear);
    });
}

fn wire_swap_action(state: &Rc<CompareDialogState>, button: &gtk4::Button) {
    let state = state.clone();
    button.connect_clicked(move |_| {
        if !state.swap_button.is_sensitive() {
            return;
        }
        let left = state.left.replace(CompareSlot::None);
        let right = state.right.replace(CompareSlot::None);
        *state.left.borrow_mut() = right;
        *state.right.borrow_mut() = left;
        sync_compare_dialog(&state);
    });
}

fn wire_compare_action(state: &Rc<CompareDialogState>, button: &gtk4::Button) {
    let state = state.clone();
    button.connect_clicked(move |_| {
        if !state.compare_button.is_sensitive() {
            return;
        }
        let left = state.left.borrow().clone();
        let right = state.right.borrow().clone();
        if let Some(controller) = state.controller.upgrade() {
            state.compare_button.set_sensitive(false);
            controller.start_compare_from_dialog(
                left,
                right,
                Rc::new({
                    let state = state.clone();
                    move |result| {
                        sync_compare_dialog(&state);
                        if result.is_ok() {
                            let _closed = state.dialog.close();
                        }
                    }
                }),
            );
        }
    });
}

struct CompareDialogState {
    controller: std::rc::Weak<WindowCompareController>,
    dialog: adw::Dialog,
    current_document: Option<Rc<EditorTab>>,
    autosave_enabled: bool,
    left: RefCell<CompareSlot>,
    right: RefCell<CompareSlot>,
    left_row: adw::ActionRow,
    right_row: adw::ActionRow,
    compare_button: gtk4::Button,
    swap_button: gtk4::Button,
    left_clear_button: gtk4::Button,
    right_clear_button: gtk4::Button,
    right_saved_button: gtk4::Button,
}

fn sync_compare_dialog(state: &Rc<CompareDialogState>) {
    let left_snapshot = state.left.borrow().clone();
    let right_snapshot = state.right.borrow().clone();
    let saved_version_available = match &left_snapshot {
        CompareSlot::CurrentDocument(tab) => {
            tab.has_saved_local_uri() && tab.is_dirty() && !state.autosave_enabled
        }
        _ => false,
    };
    if matches!(right_snapshot, CompareSlot::SavedVersion) && !saved_version_available {
        *state.right.borrow_mut() = CompareSlot::None;
    }

    apply_slot_row(&state.left_row, &left_snapshot, false);
    apply_slot_row(
        &state.right_row,
        &state.right.borrow(),
        saved_version_available,
    );

    let right = state.right.borrow().clone();
    let can_compare = left_snapshot.is_set() && right.is_set();
    state.compare_button.set_sensitive(can_compare);
    state
        .left_clear_button
        .set_sensitive(left_snapshot.is_set());
    state.right_clear_button.set_sensitive(right.is_set());
    state
        .right_saved_button
        .set_sensitive(saved_version_available);
    state
        .swap_button
        .set_sensitive(left_snapshot.is_plain() && right.is_plain());
}

fn apply_slot_row(row: &adw::ActionRow, slot: &CompareSlot, saved_version_available: bool) {
    match slot {
        CompareSlot::None => {
            row.set_title(&pgettext("compare source", "Not Set"));
            row.set_subtitle(&gettext("Choose File... or Paste Text..."));
        }
        CompareSlot::CurrentDocument(tab) => {
            row.set_title(&pgettext("compare source", "Current Document"));
            row.set_subtitle(&tab.title());
        }
        CompareSlot::SavedVersion => {
            row.set_title(&pgettext("compare source", "Saved Version"));
            if saved_version_available {
                row.set_subtitle("");
            } else {
                row.set_subtitle(&gettext("Unavailable"));
            }
        }
        CompareSlot::File(file) => {
            row.set_title(&pgettext("compare source", "File"));
            row.set_subtitle(&file_detail(file));
        }
        CompareSlot::Text(_) => {
            row.set_title(&pgettext("compare source", "Pasted Text"));
            row.set_subtitle("");
        }
    }
}

fn compare_source_buttons_row() -> gtk4::Box {
    gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .build()
}

fn file_detail(file: &gio::File) -> String {
    file.path()
        .map_or_else(|| file.uri().to_string(), |path| path.display().to_string())
}

fn choose_file(
    parent: &adw::ApplicationWindow,
    title: &str,
    on_file: Rc<dyn Fn(Option<gio::File>)>,
) {
    let dialog = gtk4::FileDialog::builder()
        .title(title)
        .accept_label(pgettext("file dialog action", "Choose"))
        .modal(true)
        .build();
    apply_text_filters(&dialog);
    let parent_for_open = parent.clone();
    let parent_for_error = parent.clone();
    dialog.open(
        Some(&parent_for_open),
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(file) => on_file(Some(file)),
            Err(error) if error.matches(gtk4::DialogError::Dismissed) => on_file(None),
            Err(error) => {
                dialogs::present_error(&parent_for_error, &AppError::from(error));
                on_file(None);
            }
        },
    );
}

fn show_paste_text_dialog(
    parent: &adw::Dialog,
    initial: Option<&str>,
    on_text: Rc<dyn Fn(Option<String>)>,
) {
    let dialog = adw::Dialog::builder()
        .title(pgettext("paste dialog title", "Paste Text"))
        .content_width(540)
        .content_height(420)
        .follows_content_size(false)
        .can_close(true)
        .build();

    let text_view = gtk4::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    if let Some(initial) = initial {
        text_view.buffer().set_text(initial);
    }
    let scrolled = gtk4::ScrolledWindow::builder()
        .child(&text_view)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_height(240)
        .build();

    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .spacing(12)
        .build();
    let cancel_button = gtk4::Button::with_label(&pgettext("dialog button", "Cancel"));
    let accept_button = gtk4::Button::with_label(&pgettext("dialog button", "Use Text"));
    accept_button.add_css_class("suggested-action");
    button_box.append(&cancel_button);
    button_box.append(&accept_button);

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&scrolled);
    content.append(&button_box);
    dialog.set_child(Some(&content));

    let handled = Rc::new(Cell::new(false));
    let callback = Rc::new(RefCell::new(Some(on_text)));

    {
        let dialog = dialog.clone();
        let handled = handled.clone();
        let callback = callback.clone();
        cancel_button.connect_clicked(move |_| {
            handled.set(true);
            if let Some(callback) = callback.borrow_mut().take() {
                callback(None);
            }
            let _closed = dialog.close();
        });
    }

    {
        let dialog = dialog.clone();
        let handled = handled.clone();
        let callback = callback.clone();
        accept_button.connect_clicked(move |_| {
            handled.set(true);
            let buffer = text_view.buffer();
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string();
            if let Some(callback) = callback.borrow_mut().take() {
                callback(Some(text));
            }
            let _closed = dialog.close();
        });
    }

    {
        let handled = handled.clone();
        let callback = callback.clone();
        dialog.connect_closed(move |_| {
            if handled.get() {
                return;
            }
            handled.set(true);
            if let Some(callback) = callback.borrow_mut().take() {
                callback(None);
            }
        });
    }

    dialog.present(Some(parent));
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
