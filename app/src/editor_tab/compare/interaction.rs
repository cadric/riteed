use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use super::presentation::{DiffPresentation, PresentationSide};

pub(super) fn install_presentation_interaction(
    view: &sourceview5::View,
    buffer: &sourceview5::Buffer,
    presentation: &Rc<RefCell<DiffPresentation>>,
    side: PresentationSide,
) {
    view.set_focusable(true);
    view.set_focus_on_click(true);
    let buffer = buffer.clone();
    let presentation = Rc::clone(presentation);
    view.connect_copy_clipboard(move |view| {
        if copy_selection(&buffer, view, &presentation.borrow(), side) {
            view.stop_signal_emission_by_name("copy-clipboard");
        }
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

fn copy_selection(
    buffer: &sourceview5::Buffer,
    view: &sourceview5::View,
    presentation: &DiffPresentation,
    side: PresentationSide,
) -> bool {
    let Some(text) = filtered_selection_text(buffer, presentation, side) else {
        return false;
    };
    view.display().clipboard().set_text(&text);
    true
}

#[cfg(test)]
pub(super) fn copy_selection_for_tests(
    buffer: &sourceview5::Buffer,
    view: &sourceview5::View,
    presentation: &DiffPresentation,
    side: PresentationSide,
) -> bool {
    copy_selection(buffer, view, presentation, side)
}

fn filtered_selection_text(
    buffer: &sourceview5::Buffer,
    presentation: &DiffPresentation,
    side: PresentationSide,
) -> Option<String> {
    let (start, end) = buffer.selection_bounds()?;
    let start_line = start.line();
    let end_line = end.line();
    let mut output = String::new();
    for line in start_line..=end_line {
        let row = usize::try_from(line).ok()?;
        let line_end = line_end_offset(buffer, line)?;
        let start_offset = if line == start_line {
            start.line_offset().clamp(0, line_end)
        } else {
            0
        };
        let end_offset = if line == end_line {
            end.line_offset().clamp(0, line_end)
        } else {
            line_end
        };
        if end_offset > start_offset && presentation.placeholder_marker(side, row).is_none() {
            let segment_start = buffer.iter_at_line_offset(line, start_offset)?;
            let segment_end = buffer.iter_at_line_offset(line, end_offset)?;
            output.push_str(&buffer.text(&segment_start, &segment_end, false));
        }
        if line < end_line {
            output.push('\n');
        }
    }
    Some(output)
}

fn line_end_offset(buffer: &sourceview5::Buffer, line: i32) -> Option<i32> {
    let mut iter = buffer.iter_at_line(line)?;
    let _found = iter.forward_to_line_end();
    Some(iter.line_offset())
}
