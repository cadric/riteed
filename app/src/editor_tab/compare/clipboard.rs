use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;

use super::unified::{UnifiedLineSide, UnifiedPresentation};

pub(super) fn install_unified_clipboard(
    view: &sourceview5::View,
    buffer: &sourceview5::Buffer,
    presentation: &Rc<RefCell<UnifiedPresentation>>,
) {
    view.set_focusable(true);
    view.set_focus_on_click(true);
    let buffer = buffer.clone();
    let presentation = Rc::clone(presentation);
    view.connect_copy_clipboard(move |view| {
        if copy_selection(&buffer, view, &presentation.borrow()) {
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
    presentation: &UnifiedPresentation,
) -> bool {
    let Some(text) = filtered_selection_text(buffer, presentation) else {
        return false;
    };
    view.display().clipboard().set_text(&text);
    true
}

fn filtered_selection_text(
    buffer: &sourceview5::Buffer,
    presentation: &UnifiedPresentation,
) -> Option<String> {
    let (start, end) = buffer.selection_bounds()?;
    let mut output = String::new();
    let mut wrote = false;
    for line in start.line()..=end.line() {
        let row = usize::try_from(line).ok()?;
        let side = presentation.lines.get(row).map(|line| line.side);
        if !matches!(
            side,
            Some(UnifiedLineSide::Context | UnifiedLineSide::Addition)
        ) {
            continue;
        }
        let line_end = line_end_offset(buffer, line)?;
        let start_offset = if line == start.line() {
            start.line_offset().clamp(0, line_end)
        } else {
            0
        };
        let end_offset = if line == end.line() {
            end.line_offset().clamp(0, line_end)
        } else {
            line_end
        };
        if end_offset > start_offset {
            let segment_start = buffer.iter_at_line_offset(line, start_offset)?;
            let segment_end = buffer.iter_at_line_offset(line, end_offset)?;
            append_projected_line(
                &mut output,
                &mut wrote,
                &buffer.text(&segment_start, &segment_end, false),
            );
        } else {
            append_projected_line(&mut output, &mut wrote, "");
        }
    }
    Some(output)
}

fn append_projected_line(output: &mut String, wrote: &mut bool, segment: &str) {
    if *wrote {
        output.push('\n');
    }
    *wrote = true;
    output.push_str(segment);
}

fn line_end_offset(buffer: &sourceview5::Buffer, line: i32) -> Option<i32> {
    let mut iter = buffer.iter_at_line(line)?;
    let _found = iter.forward_to_line_end();
    Some(iter.line_offset())
}

#[cfg(test)]
mod tests {
    use super::append_projected_line;

    #[test]
    fn unified_clipboard_projects_current_content_only() {
        let mut output = String::new();
        let mut wrote = false;

        append_projected_line(&mut output, &mut wrote, "same");
        append_projected_line(&mut output, &mut wrote, "new");

        assert_eq!(output, "same\nnew");
    }
}
