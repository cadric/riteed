use gtk4::prelude::*;

pub(super) fn install_presentation_interaction(
    view: &sourceview5::View,
    buffer: &sourceview5::Buffer,
) {
    view.set_focusable(true);
    view.set_focus_on_click(true);
    let buffer = buffer.clone();
    view.connect_copy_clipboard(move |view| {
        copy_selection(&buffer, view);
        view.stop_signal_emission_by_name("copy-clipboard");
    });
    install_copy_shortcuts(view);
}

fn install_copy_shortcuts(view: &sourceview5::View) {
    let controller = gtk4::ShortcutController::new();
    controller.set_scope(gtk4::ShortcutScope::Local);
    add_copy_shortcut(&controller, "<Control>c");
    add_copy_shortcut(&controller, "<Control>Insert");
    view.add_controller(controller);
}

fn add_copy_shortcut(controller: &gtk4::ShortcutController, trigger: &str) {
    let Some(trigger) = gtk4::ShortcutTrigger::parse_string(trigger) else {
        return;
    };
    controller.add_shortcut(gtk4::Shortcut::new(
        Some(trigger),
        Some(gtk4::SignalAction::new("copy-clipboard")),
    ));
}

fn copy_selection(buffer: &sourceview5::Buffer, view: &sourceview5::View) {
    if buffer.selection_bounds().is_none() {
        return;
    }
    buffer.copy_clipboard(&view.display().clipboard());
}

#[cfg(test)]
pub(super) fn copy_selection_for_tests(
    buffer: &sourceview5::Buffer,
    view: &sourceview5::View,
) -> bool {
    let has_selection = buffer.selection_bounds().is_some();
    copy_selection(buffer, view);
    has_selection
}
