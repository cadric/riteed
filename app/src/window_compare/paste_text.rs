use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita::prelude::*;

use crate::dialog_shell::build_dialog_shell;

type PasteTextCallback = Rc<dyn Fn(Option<String>)>;

pub(super) fn show_paste_text_dialog(
    parent: &impl IsA<gtk4::Widget>,
    initial: Option<&str>,
    on_text: PasteTextCallback,
) {
    let _dialog = present_paste_text_dialog(parent, initial, on_text);
}

#[cfg(test)]
pub(super) fn show_paste_text_dialog_for_tests(
    parent: &impl IsA<gtk4::Widget>,
) -> libadwaita::Dialog {
    present_paste_text_dialog(parent, None, Rc::new(|_text| {}))
}

fn present_paste_text_dialog(
    parent: &impl IsA<gtk4::Widget>,
    initial: Option<&str>,
    on_text: PasteTextCallback,
) -> libadwaita::Dialog {
    let shell = build_dialog_shell(
        &pgettext("paste dialog title", "Paste Text"),
        540,
        Some(420),
        false,
    );
    let dialog = shell.dialog;

    let text_view = gtk4::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    if let Some(initial) = initial {
        text_view.buffer().set_text(initial);
    }
    let scrolled = gtk4::ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .spacing(12)
        .build();
    let accept_button = gtk4::Button::with_label(&pgettext("dialog button", "Compare"));
    accept_button.add_css_class("suggested-action");
    button_box.append(&accept_button);

    let content = shell.content;
    content.append(&scrolled);
    content.append(&button_box);
    dialog.set_default_widget(Some(&accept_button));

    let state = Rc::new(PasteTextDialogState {
        dialog: dialog.downgrade(),
        text_view,
        handled: Cell::new(false),
        callback: RefCell::new(Some(on_text)),
        #[cfg(test)]
        _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary::new(
            crate::dialogs::lifecycle::DialogLeakKind::PasteText,
        ),
    });

    let weak = Rc::downgrade(&state);
    accept_button.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        state.accept();
    });

    let state_for_closed = Rc::clone(&state);
    dialog.connect_closed(move |_| {
        state_for_closed.cancel_if_unhandled();
    });

    dialog.present(Some(parent));
    dialog
}

struct PasteTextDialogState {
    dialog: gtk4::glib::WeakRef<libadwaita::Dialog>,
    text_view: gtk4::TextView,
    handled: Cell<bool>,
    callback: RefCell<Option<PasteTextCallback>>,
    #[cfg(test)]
    _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary,
}

impl PasteTextDialogState {
    fn accept(&self) {
        self.handled.set(true);
        let buffer = self.text_view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        self.respond(Some(text));
        self.close_dialog();
    }

    fn cancel_if_unhandled(&self) {
        if self.handled.get() {
            return;
        }
        self.handled.set(true);
        self.respond(None);
    }

    fn respond(&self, text: Option<String>) {
        if let Some(callback) = self.callback.borrow_mut().take() {
            callback(text);
        }
    }

    fn close_dialog(&self) {
        if let Some(dialog) = self.dialog.upgrade() {
            let _closed = dialog.close();
        }
    }
}
