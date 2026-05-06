use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::error::AppError;

pub(crate) fn run_print(parent: &adw::ApplicationWindow, view: &sourceview5::View, title: &str) {
    let operation = gtk4::PrintOperation::new();
    operation.set_job_name(title);
    operation.set_embed_page_setup(true);
    operation.set_show_progress(true);
    operation.set_unit(gtk4::Unit::Mm);

    let compositor = sourceview5::PrintCompositor::from_view(view);
    configure_compositor(&compositor, view);

    let compositor_for_paginate = compositor.clone();
    operation.connect_paginate(move |operation, context| {
        let finished = compositor_for_paginate.paginate(context);
        if finished {
            operation.set_n_pages(compositor_for_paginate.n_pages());
        }
        finished
    });

    let compositor_for_draw = compositor.clone();
    operation.connect_draw_page(move |_, context, page| {
        compositor_for_draw.draw_page(context, page);
    });

    match operation.run(gtk4::PrintOperationAction::PrintDialog, Some(parent)) {
        Ok(gtk4::PrintOperationResult::Error) | Err(_) => {
            crate::dialogs::present_error(parent, &AppError::Internal(gettext("Printing failed.")));
        }
        Ok(_) => {}
    }
}

fn configure_compositor(compositor: &sourceview5::PrintCompositor, view: &sourceview5::View) {
    compositor.set_print_header(false);
    compositor.set_print_footer(false);
    compositor.set_highlight_syntax(true);
    compositor.set_wrap_mode(view.wrap_mode());
    compositor.set_tab_width(view.tab_width());
    compositor.set_print_line_numbers(line_number_interval(view.shows_line_numbers()));
    set_margins(compositor);
}

fn set_margins(compositor: &sourceview5::PrintCompositor) {
    compositor.set_top_margin(12.7, gtk4::Unit::Mm);
    compositor.set_bottom_margin(12.7, gtk4::Unit::Mm);
    compositor.set_left_margin(12.7, gtk4::Unit::Mm);
    compositor.set_right_margin(12.7, gtk4::Unit::Mm);
}

fn line_number_interval(show_line_numbers: bool) -> u32 {
    u32::from(show_line_numbers)
}

#[cfg(test)]
mod tests {
    use super::line_number_interval;

    #[test]
    fn line_numbers_follow_editor_view_state() {
        assert_eq!(line_number_interval(false), 0);
        assert_eq!(line_number_interval(true), 1);
    }
}
