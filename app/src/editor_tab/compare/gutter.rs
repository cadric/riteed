use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{glib, prelude::*};
use sourceview5::prelude::*;

use super::model::{DiffRowKind, DiffRowModel};
use super::presentation::{DiffPresentation, PresentationSide};
use super::unified::{UnifiedLine, UnifiedLineSide, UnifiedPresentation};

const EMPTY_MARKER: char = '\u{00A0}';
const NUMBER_XPAD: i32 = 6;
const MARKER_XPAD: i32 = 3;
const FALLBACK_DIGIT_WIDTH: i32 = 10;
const NUMBER_RENDERER_POSITION: i32 = 0;
const MARKER_RENDERER_POSITION: i32 = 10;
const UNIFIED_REF_RENDERER_POSITION: i32 = 0;
const UNIFIED_MARKER_RENDERER_POSITION: i32 = 10;
const UNIFIED_CUR_RENDERER_POSITION: i32 = 20;

pub(super) struct CompareGutters {
    left: GutterHandle,
    right: GutterHandle,
    state: Rc<GutterState>,
}

pub(super) struct UnifiedGutter {
    reference_renderer: sourceview5::GutterRendererText,
    marker_renderer: sourceview5::GutterRendererText,
    current_renderer: sourceview5::GutterRendererText,
    reference_handler: Option<glib::SignalHandlerId>,
    marker_handler: Option<glib::SignalHandlerId>,
    current_handler: Option<glib::SignalHandlerId>,
}

struct GutterState {
    presentation: Rc<RefCell<DiffPresentation>>,
    row_model: Rc<RefCell<DiffRowModel>>,
}

impl CompareGutters {
    pub(super) fn new(
        left_view: &sourceview5::View,
        right_view: &sourceview5::View,
        presentation: &Rc<RefCell<DiffPresentation>>,
        row_model: &Rc<RefCell<DiffRowModel>>,
    ) -> Self {
        let state = Rc::new(GutterState {
            presentation: Rc::clone(presentation),
            row_model: Rc::clone(row_model),
        });
        Self {
            left: install_line_number_gutter(
                left_view,
                Rc::clone(&state),
                PresentationSide::Reference,
            ),
            right: install_line_number_gutter(
                right_view,
                Rc::clone(&state),
                PresentationSide::Current,
            ),
            state,
        }
    }

    pub(super) fn refresh(&self) {
        let presentation = self.state.presentation.borrow();
        let left_width = self.left.measure_width_request(&presentation);
        let right_width = self.right.measure_width_request(&presentation);
        let width = GutterWidths {
            number: left_width.number.max(right_width.number),
            marker: left_width.marker.max(right_width.marker),
        };
        self.left.refresh(width);
        self.right.refresh(width);
    }

    #[cfg(test)]
    pub(super) fn width_requests(&self) -> (i32, i32) {
        (self.left.width_request(), self.right.width_request())
    }
}

impl UnifiedGutter {
    pub(super) fn new(
        view: &sourceview5::View,
        presentation: &Rc<RefCell<UnifiedPresentation>>,
    ) -> Self {
        view.set_show_line_numbers(false);
        view.set_show_line_marks(false);
        let reference_renderer = unified_number_renderer(1.0);
        let marker_renderer = unified_marker_renderer();
        let current_renderer = unified_number_renderer(0.0);

        let reference_state = Rc::clone(presentation);
        let reference_handler =
            reference_renderer.connect_query_data(move |renderer, _lines, line| {
                let row = usize::try_from(line).map_or(usize::MAX, |value| value);
                let label = reference_state
                    .borrow()
                    .lines
                    .get(row)
                    .and_then(unified_reference_number)
                    .map_or_else(String::new, |line| line.to_string());
                renderer.set_text(&label);
            });
        let marker_state = Rc::clone(presentation);
        let marker_handler = marker_renderer.connect_query_data(move |renderer, _lines, line| {
            let row = usize::try_from(line).map_or(usize::MAX, |value| value);
            let label = marker_state
                .borrow()
                .lines
                .get(row)
                .map_or(EMPTY_MARKER, unified_marker)
                .to_string();
            renderer.set_text(&label);
        });
        let current_state = Rc::clone(presentation);
        let current_handler = current_renderer.connect_query_data(move |renderer, _lines, line| {
            let row = usize::try_from(line).map_or(usize::MAX, |value| value);
            let label = current_state
                .borrow()
                .lines
                .get(row)
                .and_then(unified_current_number)
                .map_or_else(String::new, |line| line.to_string());
            renderer.set_text(&label);
        });

        let gutter = sourceview5::prelude::ViewExt::gutter(view, gtk4::TextWindowType::Left);
        let _inserted = gutter.insert(&reference_renderer, UNIFIED_REF_RENDERER_POSITION);
        let _inserted = gutter.insert(&marker_renderer, UNIFIED_MARKER_RENDERER_POSITION);
        let _inserted = gutter.insert(&current_renderer, UNIFIED_CUR_RENDERER_POSITION);
        let handle = Self {
            reference_renderer,
            marker_renderer,
            current_renderer,
            reference_handler: Some(reference_handler),
            marker_handler: Some(marker_handler),
            current_handler: Some(current_handler),
        };
        handle.refresh();
        handle
    }

    pub(super) fn refresh(&self) {
        self.reference_renderer.set_width_request(48);
        self.marker_renderer.set_width_request(16);
        self.current_renderer.set_width_request(48);
        self.reference_renderer.queue_draw();
        self.marker_renderer.queue_draw();
        self.current_renderer.queue_draw();
    }
}

#[derive(Clone, Copy)]
struct GutterWidths {
    number: i32,
    marker: i32,
}

struct GutterHandle {
    number_renderer: sourceview5::GutterRendererText,
    marker_renderer: sourceview5::GutterRendererText,
    number_handler: Option<glib::SignalHandlerId>,
    marker_handler: Option<glib::SignalHandlerId>,
}

impl GutterHandle {
    fn refresh(&self, width: GutterWidths) {
        self.number_renderer.set_width_request(width.number);
        self.marker_renderer.set_width_request(width.marker);
        self.number_renderer.queue_resize();
        self.marker_renderer.queue_resize();
        self.number_renderer.queue_draw();
        self.marker_renderer.queue_draw();
    }

    fn measure_width_request(&self, presentation: &DiffPresentation) -> GutterWidths {
        let digits = line_number_digits(shared_max_line_number(presentation));
        let measured_number = number_width_samples(digits)
            .iter()
            .map(|sample| {
                sourceview5::prelude::GutterRendererTextExt::measure(&self.number_renderer, sample)
                    .0
            })
            .max()
            .unwrap_or_default();
        let measured_marker = marker_width_samples()
            .iter()
            .map(|sample| {
                sourceview5::prelude::GutterRendererTextExt::measure(&self.marker_renderer, sample)
                    .0
            })
            .max()
            .unwrap_or_default();
        GutterWidths {
            number: number_width_request_for_digits(measured_number, digits),
            marker: marker_width_request(measured_marker),
        }
    }

    #[cfg(test)]
    fn width_request(&self) -> i32 {
        self.number_renderer
            .width_request()
            .saturating_add(self.marker_renderer.width_request())
    }
}

impl Drop for GutterHandle {
    fn drop(&mut self) {
        if let Some(handler) = self.number_handler.take() {
            self.number_renderer.disconnect(handler);
        }
        if let Some(handler) = self.marker_handler.take() {
            self.marker_renderer.disconnect(handler);
        }
    }
}

impl Drop for UnifiedGutter {
    fn drop(&mut self) {
        if let Some(handler) = self.reference_handler.take() {
            self.reference_renderer.disconnect(handler);
        }
        if let Some(handler) = self.marker_handler.take() {
            self.marker_renderer.disconnect(handler);
        }
        if let Some(handler) = self.current_handler.take() {
            self.current_renderer.disconnect(handler);
        }
    }
}

fn install_line_number_gutter(
    view: &sourceview5::View,
    state: Rc<GutterState>,
    side: PresentationSide,
) -> GutterHandle {
    view.set_show_line_numbers(false);
    view.set_show_line_marks(false);
    let number_renderer = sourceview5::GutterRendererText::builder()
        .alignment_mode(sourceview5::GutterRendererAlignmentMode::Cell)
        .can_target(false)
        .focusable(false)
        .xalign(1.0)
        .xpad(NUMBER_XPAD)
        .build();
    let marker_renderer = sourceview5::GutterRendererText::builder()
        .alignment_mode(sourceview5::GutterRendererAlignmentMode::Cell)
        .can_target(false)
        .focusable(false)
        .xalign(0.5)
        .xpad(MARKER_XPAD)
        .build();
    let number_state = Rc::clone(&state);
    let number_handler = number_renderer.connect_query_data(move |renderer, _lines, line| {
        let row = usize::try_from(line).map_or(usize::MAX, |value| value);
        let presentation = number_state.presentation.borrow();
        let label = gutter_number_for_row(&presentation, side, row);
        renderer.set_text(&label);
    });
    let marker_handler = marker_renderer.connect_query_data(move |renderer, _lines, line| {
        let row = usize::try_from(line).map_or(usize::MAX, |value| value);
        let presentation = state.presentation.borrow();
        let row_model = state.row_model.borrow();
        let marker = gutter_marker_for_row(&presentation, &row_model, side, row);
        renderer.set_text(&marker);
    });
    let gutter = sourceview5::prelude::ViewExt::gutter(view, gtk4::TextWindowType::Left);
    let _inserted = gutter.insert(&number_renderer, NUMBER_RENDERER_POSITION);
    let _inserted = gutter.insert(&marker_renderer, MARKER_RENDERER_POSITION);
    GutterHandle {
        number_renderer,
        marker_renderer,
        number_handler: Some(number_handler),
        marker_handler: Some(marker_handler),
    }
}

fn gutter_number_for_row(
    presentation: &DiffPresentation,
    side: PresentationSide,
    row: usize,
) -> String {
    presentation
        .line_number(side, row)
        .map_or_else(String::new, |line_number| line_number.to_string())
}

fn gutter_marker_for_row(
    presentation: &DiffPresentation,
    model: &DiffRowModel,
    side: PresentationSide,
    row: usize,
) -> String {
    if presentation.line_number(side, row).is_none() {
        return String::new();
    }
    model
        .rows
        .get(row)
        .map_or(EMPTY_MARKER, |row| marker_for_kind(side, row.kind))
        .to_string()
}

fn marker_for_kind(side: PresentationSide, kind: DiffRowKind) -> char {
    match (side, kind) {
        (PresentationSide::Reference, DiffRowKind::ReferenceOnly | DiffRowKind::Modify) => '-',
        (PresentationSide::Current, DiffRowKind::CurrentOnly | DiffRowKind::Modify) => '+',
        _ => EMPTY_MARKER,
    }
}

fn unified_number_renderer(xalign: f32) -> sourceview5::GutterRendererText {
    sourceview5::GutterRendererText::builder()
        .alignment_mode(sourceview5::GutterRendererAlignmentMode::Cell)
        .can_target(false)
        .focusable(false)
        .xalign(xalign)
        .xpad(NUMBER_XPAD)
        .build()
}

fn unified_marker_renderer() -> sourceview5::GutterRendererText {
    sourceview5::GutterRendererText::builder()
        .alignment_mode(sourceview5::GutterRendererAlignmentMode::Cell)
        .can_target(false)
        .focusable(false)
        .xalign(0.5)
        .xpad(MARKER_XPAD)
        .build()
}

fn unified_reference_number(line: &UnifiedLine) -> Option<usize> {
    match line.side {
        UnifiedLineSide::Context | UnifiedLineSide::Removal => line.reference_line,
        UnifiedLineSide::Addition | UnifiedLineSide::Collapsed => None,
    }
}

fn unified_current_number(line: &UnifiedLine) -> Option<usize> {
    match line.side {
        UnifiedLineSide::Context | UnifiedLineSide::Addition => line.current_line,
        UnifiedLineSide::Removal | UnifiedLineSide::Collapsed => None,
    }
}

fn unified_marker(line: &UnifiedLine) -> char {
    match line.side {
        UnifiedLineSide::Removal => '-',
        UnifiedLineSide::Addition => '+',
        UnifiedLineSide::Context | UnifiedLineSide::Collapsed => EMPTY_MARKER,
    }
}

fn shared_max_line_number(presentation: &DiffPresentation) -> usize {
    presentation
        .max_line_number(PresentationSide::Reference)
        .max(presentation.max_line_number(PresentationSide::Current))
}

fn line_number_digits(value: usize) -> usize {
    value.max(1).ilog10() as usize + 1
}

fn number_width_samples(digits: usize) -> [String; 1] {
    ["8".repeat(digits.max(1))]
}

fn marker_width_samples() -> [String; 3] {
    ["+".to_string(), "-".to_string(), EMPTY_MARKER.to_string()]
}

fn number_width_request_for_digits(measured_width: i32, digits: usize) -> i32 {
    let fallback = i32::try_from(digits).map_or(i32::MAX, |columns| {
        columns.saturating_mul(FALLBACK_DIGIT_WIDTH)
    });
    measured_width.max(fallback).saturating_add(NUMBER_XPAD * 2)
}

fn marker_width_request(measured_width: i32) -> i32 {
    measured_width
        .max(FALLBACK_DIGIT_WIDTH)
        .saturating_add(MARKER_XPAD * 2)
}

#[cfg(test)]
mod tests {
    use super::{
        EMPTY_MARKER, gutter_marker_for_row, gutter_number_for_row, line_number_digits,
        marker_width_request, marker_width_samples, number_width_request_for_digits,
        number_width_samples, shared_max_line_number,
    };
    use crate::editor_tab::compare::diff::compute_diff;
    use crate::editor_tab::compare::presentation::PresentationSide;

    #[test]
    fn line_number_digits_track_max_line() {
        assert_eq!(line_number_digits(9), 1);
        assert_eq!(line_number_digits(99), 2);
        assert_eq!(line_number_digits(100), 3);
        assert_eq!(line_number_digits(1_000), 4);
    }

    #[test]
    fn width_request_adds_padding_and_fallback() {
        assert_eq!(number_width_request_for_digits(0, 3), 42);
        assert_eq!(number_width_request_for_digits(48, 3), 60);
        assert_eq!(marker_width_request(0), 16);
        assert_eq!(marker_width_request(18), 24);
    }

    #[test]
    fn width_samples_measure_number_and_marker_columns_separately() {
        assert_eq!(number_width_samples(3), ["888".to_string()]);
        assert_eq!(
            marker_width_samples(),
            ["+".to_string(), "-".to_string(), EMPTY_MARKER.to_string()]
        );
    }

    #[test]
    fn shared_width_uses_largest_line_number_on_either_side() {
        let current = numbered_lines(120);
        let presentation = compute_diff("a\n", &current).presentation;

        assert_eq!(shared_max_line_number(&presentation), 120);
        assert_eq!(line_number_digits(shared_max_line_number(&presentation)), 3);
    }

    #[test]
    fn gutter_cells_keep_numbers_and_markers_separate() {
        let computation = compute_diff("same\nold\n", "same\nnew\ncurrent\n");
        let model = computation.model;
        let presentation = computation.presentation;

        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Reference, 0),
            "1"
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Reference, 0),
            EMPTY_MARKER.to_string()
        );
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Reference, 1),
            "2"
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Reference, 1),
            "-"
        );
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Reference, 2),
            ""
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Reference, 2),
            ""
        );
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Current, 1),
            "2"
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Current, 1),
            "+"
        );
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Current, 2),
            "3"
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Current, 2),
            "+"
        );

        let computation = compute_diff("same\nremoved\n", "same\n");
        let model = computation.model;
        let presentation = computation.presentation;
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Reference, 1),
            "2"
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Reference, 1),
            "-"
        );
        assert_eq!(
            gutter_number_for_row(&presentation, PresentationSide::Current, 1),
            ""
        );
        assert_eq!(
            gutter_marker_for_row(&presentation, &model, PresentationSide::Current, 1),
            ""
        );
    }

    fn numbered_lines(count: usize) -> String {
        let mut text = String::new();
        for line in 1..=count {
            push_numbered_line(&mut text, line);
        }
        text
    }

    fn push_numbered_line(text: &mut String, line: usize) {
        text.push_str("line ");
        if line < 10 {
            text.push_str("00");
        } else if line < 100 {
            text.push('0');
        }
        text.push_str(&line.to_string());
        text.push('\n');
    }
}
