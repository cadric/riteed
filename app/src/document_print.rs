use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::editor_zoom::resolve_editor_font_description;
use crate::error::AppError;

pub(crate) struct PrintJob<'a> {
    pub(crate) parent: &'a adw::ApplicationWindow,
    pub(crate) view: &'a sourceview5::View,
    pub(crate) title: &'a str,
    pub(crate) body_font: &'a str,
}

/// Resolves the stored editor font into a point-based Pango font name for
/// printing. Screen zoom and CSS pixel sizes must not leak onto paper.
pub(crate) fn print_body_font_name(stored_font: &str) -> String {
    let mut desc = resolve_editor_font_description(stored_font);
    if desc.is_size_absolute() {
        let points = desc.size() / gtk4::pango::SCALE;
        desc.set_size(points.max(1) * gtk4::pango::SCALE);
    }
    desc.to_string()
}

pub(crate) fn run_print(job: &PrintJob<'_>) {
    let operation = gtk4::PrintOperation::new();
    operation.set_job_name(job.title);
    operation.set_embed_page_setup(true);
    operation.set_show_progress(true);
    operation.set_unit(gtk4::Unit::Mm);

    let compositor = sourceview5::PrintCompositor::from_view(job.view);
    configure_compositor(&compositor, job.view, job.body_font);

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

    match operation.run(gtk4::PrintOperationAction::PrintDialog, Some(job.parent)) {
        Ok(gtk4::PrintOperationResult::Error) | Err(_) => {
            crate::dialogs::present_error(
                job.parent,
                &AppError::Internal(gettext("Printing failed.")),
            );
        }
        Ok(_) => {}
    }
}

fn configure_compositor(
    compositor: &sourceview5::PrintCompositor,
    view: &sourceview5::View,
    body_font: &str,
) {
    compositor.set_body_font_name(body_font);
    compositor.set_print_header(false);
    compositor.set_print_footer(false);
    compositor.set_highlight_syntax(true);
    compositor.set_wrap_mode(gtk4::WrapMode::WordChar);
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
    use super::{line_number_interval, print_body_font_name};

    #[test]
    fn line_numbers_follow_editor_view_state() {
        assert_eq!(line_number_interval(false), 0);
        assert_eq!(line_number_interval(true), 1);
    }

    #[test]
    fn body_font_passes_stored_point_sizes_through() {
        assert_eq!(
            print_body_font_name("JetBrains Mono 12"),
            "JetBrains Mono 12"
        );
    }

    #[test]
    fn body_font_falls_back_to_default_points() {
        assert_eq!(print_body_font_name(""), "Monospace 11");
    }

    #[test]
    fn body_font_never_uses_pixel_sizes() {
        assert!(!print_body_font_name("Monospace 14.5px").ends_with("px"));
    }
}
