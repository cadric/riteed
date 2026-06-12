use std::cell::RefCell;

use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::editor_zoom::resolve_editor_font_description;
use crate::error::AppError;

/// Remembered print dialog choices for the lifetime of the window, so the
/// user's paper size, orientation, and printer survive between print runs.
#[derive(Default)]
pub(crate) struct PrintSession {
    print_settings: RefCell<Option<gtk4::PrintSettings>>,
    page_setup: RefCell<Option<gtk4::PageSetup>>,
}

pub(crate) struct PrintJob<'a> {
    pub(crate) parent: &'a adw::ApplicationWindow,
    pub(crate) view: &'a sourceview5::View,
    pub(crate) title: &'a str,
    pub(crate) body_font: &'a str,
    pub(crate) session: &'a PrintSession,
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
    let (operation, _compositor) = build_print_operation(job.view, job.title, job.body_font);
    operation.set_show_progress(true);

    if let Some(settings) = job.session.print_settings.borrow().as_ref() {
        operation.set_print_settings(Some(settings));
    }
    if let Some(page_setup) = job.session.page_setup.borrow().as_ref() {
        operation.set_default_page_setup(Some(page_setup));
    }

    match operation.run(gtk4::PrintOperationAction::PrintDialog, Some(job.parent)) {
        // Apply means "settings should be stored"; Cancel explicitly means
        // they should not. Persist only on Apply.
        Ok(gtk4::PrintOperationResult::Apply) => {
            let print_settings = match operation.print_settings() {
                Some(settings) => settings,
                None => gtk4::PrintSettings::new(),
            };
            job.session.print_settings.replace(Some(print_settings));
            job.session
                .page_setup
                .replace(Some(operation.default_page_setup()));
        }
        Ok(gtk4::PrintOperationResult::Error) | Err(_) => {
            crate::dialogs::present_error(
                job.parent,
                &AppError::Internal(gettext("Printing failed.")),
            );
        }
        Ok(_) => {}
    }
}

pub(crate) fn build_print_operation(
    view: &sourceview5::View,
    title: &str,
    body_font: &str,
) -> (gtk4::PrintOperation, sourceview5::PrintCompositor) {
    let operation = gtk4::PrintOperation::new();
    operation.set_job_name(title);
    operation.set_embed_page_setup(true);
    operation.set_unit(gtk4::Unit::Mm);

    let compositor = sourceview5::PrintCompositor::from_view(view);
    configure_compositor(&compositor, view, title, body_font);

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

    (operation, compositor)
}

fn configure_compositor(
    compositor: &sourceview5::PrintCompositor,
    view: &sourceview5::View,
    title: &str,
    body_font: &str,
) {
    compositor.set_body_font_name(body_font);
    compositor.set_print_header(true);
    compositor.set_header_format(
        true,
        Some(&header_title(title)),
        None,
        Some(&pgettext("print header", "Page %N of %Q")),
    );
    compositor.set_header_font_name(Some(HEADER_FONT));
    compositor.set_print_footer(false);
    compositor.set_highlight_syntax(true);
    compositor.set_wrap_mode(gtk4::WrapMode::WordChar);
    compositor.set_tab_width(view.tab_width());
    compositor.set_print_line_numbers(line_number_interval(view.shows_line_numbers()));
    compositor.set_line_numbers_font_name(Some(&line_numbers_font_name(body_font)));
    set_margins(compositor);
}

const HEADER_FONT: &str = "Sans 9";
const LINE_NUMBERS_FONT_SIZE_PT: i32 = 8;

/// Header format strings expand strftime codes plus %N/%Q, so a literal `%`
/// in a document title must be doubled.
fn header_title(title: &str) -> String {
    title.replace('%', "%%")
}

fn line_numbers_font_name(body_font: &str) -> String {
    let mut desc = gtk4::pango::FontDescription::from_string(body_font);
    desc.set_size(LINE_NUMBERS_FONT_SIZE_PT * gtk4::pango::SCALE);
    desc.to_string()
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
    use super::{header_title, line_number_interval, line_numbers_font_name, print_body_font_name};

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

    #[test]
    fn header_title_escapes_percent_signs() {
        assert_eq!(header_title("100% done.md"), "100%% done.md");
    }

    #[test]
    fn line_numbers_font_uses_body_family_at_eight_points() {
        assert_eq!(
            line_numbers_font_name("JetBrains Mono 12"),
            "JetBrains Mono 8"
        );
        assert_eq!(line_numbers_font_name("Monospace 11"), "Monospace 8");
    }
}
