use std::cell::RefCell;

use gettextrs::{gettext, pgettext};
use gtk4::{pango, prelude::*};
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
const PRINT_MARGIN_MM: f64 = 12.7;
const LINE_NUMBER_SEPARATION_MM: f64 = 5.0;
const MM_PER_INCH: f64 = 25.4;
const POINTS_PER_INCH: f64 = 72.0;
const FALLBACK_GUIDE_COLUMNS: u32 = 80;
const MIN_GUIDE_COLUMNS: u32 = 20;
const MAX_GUIDE_COLUMNS: u32 = 240;

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
    compositor.set_top_margin(PRINT_MARGIN_MM, gtk4::Unit::Mm);
    compositor.set_bottom_margin(PRINT_MARGIN_MM, gtk4::Unit::Mm);
    compositor.set_left_margin(PRINT_MARGIN_MM, gtk4::Unit::Mm);
    compositor.set_right_margin(PRINT_MARGIN_MM, gtk4::Unit::Mm);
}

fn line_number_interval(show_line_numbers: bool) -> u32 {
    u32::from(show_line_numbers)
}

/// Columns of the print body font that fit on one printed line. This lets the
/// editor draw a right-margin guide where paper output wraps.
pub(crate) fn print_right_margin_columns(
    context: &pango::Context,
    stored_font: &str,
    show_line_numbers: bool,
    line_count: i32,
) -> u32 {
    let body_font = print_body_font_name(stored_font);
    let mut body_desc = pango::FontDescription::from_string(&body_font);
    set_absolute_point_size(&mut body_desc);
    let body_metrics = context.metrics(Some(&body_desc), None);
    let body_digit_width_pt = pango_units_to_points(body_metrics.approximate_digit_width());

    let line_number_gutter_pt = if show_line_numbers {
        let mut line_desc =
            pango::FontDescription::from_string(&line_numbers_font_name(&body_font));
        set_absolute_point_size(&mut line_desc);
        let line_metrics = context.metrics(Some(&line_desc), None);
        let line_digit_width_pt = pango_units_to_points(line_metrics.approximate_digit_width());
        line_number_gutter_width_pt(line_digit_width_pt, true, line_count)
    } else {
        0.0
    };

    columns_for_print_width(
        printable_line_width_pt(),
        body_digit_width_pt,
        line_number_gutter_pt,
    )
}

fn set_absolute_point_size(desc: &mut pango::FontDescription) {
    let size = desc.size().max(pango::SCALE);
    desc.set_absolute_size(f64::from(size));
}

fn pango_units_to_points(value: i32) -> f64 {
    f64::from(value) / f64::from(pango::SCALE)
}

fn printable_line_width_pt() -> f64 {
    let margin_pt = PRINT_MARGIN_MM * POINTS_PER_INCH / MM_PER_INCH;
    gtk4::PaperSize::new(None).width(gtk4::Unit::Points) - 2.0 * margin_pt
}

fn columns_for_print_width(
    line_width_pt: f64,
    digit_width_pt: f64,
    line_number_gutter_pt: f64,
) -> u32 {
    if !line_width_pt.is_finite()
        || line_width_pt <= 0.0
        || !digit_width_pt.is_finite()
        || digit_width_pt <= 0.0
    {
        return FALLBACK_GUIDE_COLUMNS;
    }
    let gutter = if line_number_gutter_pt.is_finite() {
        line_number_gutter_pt.max(0.0)
    } else {
        0.0
    };
    let available_width = line_width_pt - gutter;
    if available_width <= 0.0 {
        return MIN_GUIDE_COLUMNS;
    }
    let mut columns = MIN_GUIDE_COLUMNS;
    while columns < MAX_GUIDE_COLUMNS
        && f64::from(columns.saturating_add(1)) * digit_width_pt <= available_width
    {
        columns += 1;
    }
    columns
}

fn line_number_gutter_width_pt(
    digit_width_pt: f64,
    show_line_numbers: bool,
    line_count: i32,
) -> f64 {
    if !show_line_numbers || !digit_width_pt.is_finite() || digit_width_pt <= 0.0 {
        return 0.0;
    }
    let separation_pt = LINE_NUMBER_SEPARATION_MM * POINTS_PER_INCH / MM_PER_INCH;
    f64::from(print_line_number_digits(line_count)) * digit_width_pt + separation_pt
}

fn print_line_number_digits(line_count: i32) -> u32 {
    let mut value = u32::try_from(line_count.max(1)).unwrap_or(1);
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::{
        columns_for_print_width, header_title, line_number_gutter_width_pt, line_number_interval,
        line_numbers_font_name, print_body_font_name, print_line_number_digits,
    };

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

    #[test]
    fn guide_columns_divide_printable_width() {
        assert_eq!(columns_for_print_width(523.3, 6.62, 0.0), 79);
        assert_eq!(columns_for_print_width(523.3, 6.62, 31.0), 74);
    }

    #[test]
    fn guide_columns_clamp_and_guard_degenerate_input() {
        assert_eq!(columns_for_print_width(523.3, 0.0, 0.0), 80);
        assert_eq!(columns_for_print_width(523.3, -1.0, 0.0), 80);
        assert_eq!(columns_for_print_width(523.3, f64::NAN, 0.0), 80);
        assert_eq!(columns_for_print_width(f64::INFINITY, 6.6, 0.0), 80);
        assert_eq!(columns_for_print_width(-100.0, 6.6, 0.0), 80);
        assert_eq!(columns_for_print_width(10.0, 6.6, 0.0), 20);
        assert_eq!(columns_for_print_width(100_000.0, 6.6, 0.0), 240);
    }

    #[test]
    fn line_number_gutter_tracks_digit_count() {
        assert_eq!(print_line_number_digits(1), 1);
        assert_eq!(print_line_number_digits(9), 1);
        assert_eq!(print_line_number_digits(10), 2);
        assert_eq!(print_line_number_digits(200), 3);
        assert_eq!(print_line_number_digits(-1), 1);
        assert!(line_number_gutter_width_pt(4.0, false, 200).abs() < f64::EPSILON);
        assert!(line_number_gutter_width_pt(4.0, true, 200) > 25.0);
    }
}
