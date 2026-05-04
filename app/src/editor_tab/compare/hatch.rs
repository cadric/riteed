use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{cairo, gdk, glib, prelude::*};
use libadwaita as adw;

use super::presentation::{DiffPresentation, PresentationSide};

const NORMAL_BASE_ALPHA: f32 = 0.025;
const NORMAL_STRIPE_ALPHA: f32 = 0.085;
const HIGH_CONTRAST_ALPHA: f32 = 0.10;
const STRIPE_WIDTH: f64 = 2.0;
const STRIPE_PERIOD: f64 = 8.0;

pub(super) struct CompareHatches {
    left: CompareHatchLayer,
    right: CompareHatchLayer,
}

pub(super) struct CompareHatchLayer {
    view: sourceview5::View,
    area: gtk4::DrawingArea,
    #[cfg(test)]
    presentation: Rc<RefCell<DiffPresentation>>,
    #[cfg(test)]
    side: PresentationSide,
    high_contrast: Rc<Cell<bool>>,
    handlers: HatchAdjustmentHandlers,
    detached: Cell<bool>,
}

struct HatchAdjustmentHandlers {
    vertical: AdjustmentHandlers,
    horizontal: AdjustmentHandlers,
}

struct AdjustmentHandlers {
    adjustment: gtk4::Adjustment,
    value_changed: Option<glib::SignalHandlerId>,
    changed: Option<glib::SignalHandlerId>,
}

#[derive(Clone, Copy, Debug)]
struct HatchRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy, Debug)]
enum HatchRenderMode {
    Normal,
    HighContrast,
}

impl CompareHatches {
    pub(super) fn new(
        left: CompareHatchEndpoint<'_>,
        right: CompareHatchEndpoint<'_>,
        presentation: &Rc<RefCell<DiffPresentation>>,
    ) -> Self {
        Self {
            left: CompareHatchLayer::new(left, presentation, PresentationSide::Reference),
            right: CompareHatchLayer::new(right, presentation, PresentationSide::Current),
        }
    }

    pub(super) fn refresh(&self) {
        self.left.refresh();
        self.right.refresh();
    }

    pub(super) fn refresh_style(&self) {
        self.left.refresh_style();
        self.right.refresh_style();
    }

    #[cfg(test)]
    pub(super) fn overlay_states_for_tests(&self) -> (HatchOverlayState, HatchOverlayState) {
        (
            self.left.overlay_state_for_tests(),
            self.right.overlay_state_for_tests(),
        )
    }

    #[cfg(test)]
    pub(super) fn visible_regions_for_tests(&self) -> (Vec<HatchRegion>, Vec<HatchRegion>) {
        (
            self.left.visible_regions_for_tests(),
            self.right.visible_regions_for_tests(),
        )
    }

    #[cfg(test)]
    pub(super) fn viewports_for_tests(&self) -> (HatchViewport, HatchViewport) {
        (
            self.left.viewport_for_tests(),
            self.right.viewport_for_tests(),
        )
    }

    #[cfg(test)]
    pub(super) fn set_left_horizontal_value_for_tests(&self, value: f64) {
        self.left.set_horizontal_value_for_tests(value);
    }

    pub(super) fn detach(&mut self) {
        self.left.detach();
        self.right.detach();
    }
}

#[derive(Clone, Copy)]
pub(super) struct CompareHatchEndpoint<'a> {
    pub(super) view: &'a sourceview5::View,
    pub(super) vadjustment: &'a gtk4::Adjustment,
    pub(super) hadjustment: &'a gtk4::Adjustment,
}

impl CompareHatchLayer {
    fn new(
        endpoint: CompareHatchEndpoint<'_>,
        presentation: &Rc<RefCell<DiffPresentation>>,
        side: PresentationSide,
    ) -> Self {
        let area = gtk4::DrawingArea::builder()
            .accessible_role(gtk4::AccessibleRole::Presentation)
            .can_target(false)
            .focusable(false)
            .build();
        area.set_can_focus(false);
        area.set_content_width(1);
        area.set_content_height(1);
        area.set_size_request(1, 1);

        let high_contrast = Rc::new(Cell::new(adw::StyleManager::default().is_high_contrast()));
        install_draw_func(&area, endpoint.view, presentation, side, &high_contrast);
        endpoint.view.add_overlay(&area, 0, 0);

        let handlers = HatchAdjustmentHandlers::new(endpoint, &area);
        let layer = Self {
            view: endpoint.view.clone(),
            area,
            #[cfg(test)]
            presentation: Rc::clone(presentation),
            #[cfg(test)]
            side,
            high_contrast,
            handlers,
            detached: Cell::new(false),
        };
        layer.refresh();
        layer
    }

    fn refresh(&self) {
        refresh_overlay(&self.view, &self.area);
    }

    fn refresh_style(&self) {
        self.high_contrast
            .set(adw::StyleManager::default().is_high_contrast());
        self.refresh();
    }

    fn detach(&mut self) {
        if self.detached.replace(true) {
            return;
        }
        self.handlers.disconnect();
        if self.area.parent().is_some() {
            self.area.unparent();
        }
    }

    #[cfg(test)]
    fn set_horizontal_value_for_tests(&self, value: f64) {
        self.handlers.horizontal.adjustment.set_value(value);
    }

    #[cfg(test)]
    fn overlay_state_for_tests(&self) -> HatchOverlayState {
        HatchOverlayState {
            can_target: self.area.can_target(),
            focusable: self.area.is_focusable(),
        }
    }

    #[cfg(test)]
    fn visible_regions_for_tests(&self) -> Vec<HatchRegion> {
        visible_hatch_regions(&self.view, &self.presentation.borrow(), self.side)
    }

    #[cfg(test)]
    fn viewport_for_tests(&self) -> HatchViewport {
        viewport_for_view(&self.view)
    }
}

fn install_draw_func(
    area: &gtk4::DrawingArea,
    view: &sourceview5::View,
    presentation: &Rc<RefCell<DiffPresentation>>,
    side: PresentationSide,
    high_contrast: &Rc<Cell<bool>>,
) {
    let view_weak = view.downgrade();
    let draw_presentation = Rc::clone(presentation);
    let draw_high_contrast = Rc::clone(high_contrast);
    area.set_draw_func(move |_, context, width, height| {
        let Some(view) = view_weak.upgrade() else {
            return;
        };
        let presentation = draw_presentation.borrow();
        draw_visible_hatches(
            context,
            width,
            height,
            &view,
            &presentation,
            side,
            draw_high_contrast.get(),
        );
    });
}

fn refresh_overlay(view: &sourceview5::View, area: &gtk4::DrawingArea) {
    let visible = view.visible_rect();
    let width = visible.width().max(1);
    let height = visible.height().max(1);
    area.set_content_width(width);
    area.set_content_height(height);
    area.set_size_request(width, height);
    view.move_overlay(area, visible.x(), visible.y());
    area.queue_draw();
}

fn draw_visible_hatches(
    context: &cairo::Context,
    width: i32,
    height: i32,
    view: &sourceview5::View,
    presentation: &DiffPresentation,
    side: PresentationSide,
    high_contrast: bool,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let visible = view.visible_rect();
    let mode = if high_contrast {
        HatchRenderMode::HighContrast
    } else {
        HatchRenderMode::Normal
    };
    let color = view.color();
    for region in visible_hatch_regions(view, presentation, side) {
        draw_hatch_rect(
            context,
            HatchRect {
                x: f64::from(region.x),
                y: f64::from(region.y),
                width: f64::from(region.width),
                height: f64::from(region.height),
            },
            f64::from(width),
            f64::from(visible.x()),
            f64::from(visible.y()),
            &color,
            mode,
        );
    }
}

fn visible_hatch_regions(
    view: &sourceview5::View,
    presentation: &DiffPresentation,
    side: PresentationSide,
) -> Vec<HatchRegion> {
    let row_count = presentation
        .reference_line_numbers
        .len()
        .min(presentation.current_line_numbers.len());
    if row_count == 0 {
        return Vec::new();
    }
    let visible = view.visible_rect();
    let bottom = visible.y().saturating_add(visible.height());
    let (iter, _line_top) = view.line_at_y(visible.y());
    let mut row = usize::try_from(iter.line())
        .map_or(0, |line| line)
        .min(row_count.saturating_sub(1));
    let mut regions = Vec::new();
    while row < row_count {
        let Some(iter) = view
            .buffer()
            .iter_at_line(i32::try_from(row).map_or(0, |line| line))
        else {
            break;
        };
        let (line_y, line_height) = view.line_yrange(&iter);
        if line_y > bottom {
            break;
        }
        if line_height > 0
            && line_y.saturating_add(line_height) >= visible.y()
            && presentation.hatch_side_for_row(row) == Some(side)
        {
            regions.push(HatchRegion {
                row,
                x: 0,
                y: line_y.saturating_sub(visible.y()),
                width: visible.width().max(0),
                height: line_height,
            });
        }
        row += 1;
    }
    regions
}

fn draw_hatch_rect(
    context: &cairo::Context,
    rect: HatchRect,
    viewport_width: f64,
    visible_x: f64,
    visible_y: f64,
    color: &gdk::RGBA,
    mode: HatchRenderMode,
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let base_alpha = match mode {
        HatchRenderMode::Normal => NORMAL_BASE_ALPHA,
        HatchRenderMode::HighContrast => HIGH_CONTRAST_ALPHA,
    };
    set_source_alpha(context, color, base_alpha);
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    if context.fill().is_err() || matches!(mode, HatchRenderMode::HighContrast) {
        return;
    }
    if context.save().is_err() {
        return;
    }
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    set_source_alpha(context, color, NORMAL_STRIPE_ALPHA);
    context.set_line_width(STRIPE_WIDTH);
    let shift = visible_x - visible_y;
    let min_intercept = rect.y - (rect.x + rect.width) - STRIPE_PERIOD;
    let max_intercept = (rect.y + rect.height) - rect.x + STRIPE_PERIOD;
    let first = ((min_intercept - shift) / STRIPE_PERIOD).floor() * STRIPE_PERIOD;
    let mut intercept = first;
    while intercept + shift <= max_intercept {
        let local_intercept = intercept + shift;
        context.move_to(0.0, local_intercept);
        context.line_to(viewport_width, viewport_width + local_intercept);
        intercept += STRIPE_PERIOD;
    }
    let _stroke = context.stroke();
    let _restore = context.restore();
}

fn set_source_alpha(context: &cairo::Context, color: &gdk::RGBA, alpha: f32) {
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(alpha),
    );
}

impl HatchAdjustmentHandlers {
    fn new(endpoint: CompareHatchEndpoint<'_>, area: &gtk4::DrawingArea) -> Self {
        Self {
            vertical: AdjustmentHandlers::new(endpoint.vadjustment, endpoint.view, area),
            horizontal: AdjustmentHandlers::new(endpoint.hadjustment, endpoint.view, area),
        }
    }

    fn disconnect(&mut self) {
        self.vertical.disconnect();
        self.horizontal.disconnect();
    }
}

impl AdjustmentHandlers {
    fn new(
        adjustment: &gtk4::Adjustment,
        view: &sourceview5::View,
        area: &gtk4::DrawingArea,
    ) -> Self {
        let view_weak = view.downgrade();
        let area_weak = area.downgrade();
        let value_changed = adjustment.connect_value_changed(move |_| {
            refresh_overlay_from_weak(&view_weak, &area_weak);
        });

        let view_weak = view.downgrade();
        let area_weak = area.downgrade();
        let changed = adjustment.connect_changed(move |_| {
            refresh_overlay_from_weak(&view_weak, &area_weak);
        });

        Self {
            adjustment: adjustment.clone(),
            value_changed: Some(value_changed),
            changed: Some(changed),
        }
    }
}

impl AdjustmentHandlers {
    fn disconnect(&mut self) {
        if let Some(handler) = self.value_changed.take() {
            self.adjustment.disconnect(handler);
        }
        if let Some(handler) = self.changed.take() {
            self.adjustment.disconnect(handler);
        }
    }
}

fn refresh_overlay_from_weak(
    view: &glib::WeakRef<sourceview5::View>,
    area: &glib::WeakRef<gtk4::DrawingArea>,
) {
    let (Some(view), Some(area)) = (view.upgrade(), area.upgrade()) else {
        return;
    };
    refresh_overlay(&view, &area);
}

impl Drop for AdjustmentHandlers {
    fn drop(&mut self) {
        self.disconnect();
    }
}

impl Drop for CompareHatchLayer {
    fn drop(&mut self) {
        self.detach();
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HatchOverlayState {
    pub(super) can_target: bool,
    pub(super) focusable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HatchRegion {
    pub(super) row: usize,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HatchViewport {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[cfg(test)]
fn viewport_for_view(view: &sourceview5::View) -> HatchViewport {
    let visible = view.visible_rect();
    HatchViewport {
        x: visible.x(),
        y: visible.y(),
        width: visible.width(),
        height: visible.height(),
    }
}

#[cfg(test)]
mod tests {
    use super::{HatchRect, HatchRenderMode, draw_hatch_rect};
    use gtk4::{cairo, gdk};

    #[test]
    fn normal_hatch_produces_diagonal_alpha_variation() {
        let Some((mut surface, context)) = test_surface() else {
            return;
        };
        draw_hatch_rect(
            &context,
            test_rect(),
            64.0,
            0.0,
            0.0,
            &gdk::RGBA::new(0.0, 0.0, 0.0, 1.0),
            HatchRenderMode::Normal,
        );
        drop(context);
        let (min_alpha, max_alpha) = alpha_range(&mut surface);
        assert!(
            max_alpha > min_alpha + 8,
            "expected stripe alpha variation, got {min_alpha}..{max_alpha}"
        );
    }

    #[test]
    fn high_contrast_hatch_uses_solid_fallback() {
        let Some((mut surface, context)) = test_surface() else {
            return;
        };
        draw_hatch_rect(
            &context,
            test_rect(),
            64.0,
            0.0,
            0.0,
            &gdk::RGBA::new(0.0, 0.0, 0.0, 1.0),
            HatchRenderMode::HighContrast,
        );
        drop(context);
        let (min_alpha, max_alpha) = alpha_range(&mut surface);
        assert_eq!(
            min_alpha, max_alpha,
            "high contrast hatch should be solid, got {min_alpha}..{max_alpha}"
        );
    }

    fn test_surface() -> Option<(cairo::ImageSurface, cairo::Context)> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 32);
        assert!(surface.is_ok(), "failed to create cairo surface");
        let Ok(surface) = surface else {
            return None;
        };
        let context = cairo::Context::new(&surface);
        assert!(context.is_ok(), "failed to create cairo context");
        let Ok(context) = context else {
            return None;
        };
        Some((surface, context))
    }

    fn test_rect() -> HatchRect {
        HatchRect {
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 32.0,
        }
    }

    fn alpha_range(surface: &mut cairo::ImageSurface) -> (u8, u8) {
        surface.flush();
        let stride = usize::try_from(surface.stride()).map_or(0, |value| value);
        let data = surface.data();
        assert!(data.is_ok(), "failed to read cairo surface data");
        let Ok(data) = data else {
            return (0, 0);
        };
        let mut min_alpha = u8::MAX;
        let mut max_alpha = 0;
        for y in (4_usize..28).step_by(4) {
            for x in (4_usize..60).step_by(4) {
                let index = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
                let Some(alpha) = data.get(index + 3) else {
                    continue;
                };
                min_alpha = min_alpha.min(*alpha);
                max_alpha = max_alpha.max(*alpha);
            }
        }
        (min_alpha, max_alpha)
    }
}
