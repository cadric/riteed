use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{cairo, gdk, glib, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

const PREVIEW_DPI: f64 = 96.0;
const POINTS_PER_INCH: f64 = 72.0;

/// Drives one `GtkPrintOperation` in custom-preview mode and renders pages to
/// textures. Rendering is local cairo work, so it stays inside the Flatpak
/// sandbox instead of relying on GTK's external previewer.
pub(crate) struct PreviewEngine {
    /// Keeps the in-flight operation alive for the whole preview session.
    operation: RefCell<Option<gtk4::PrintOperation>>,
    preview: RefCell<Option<gtk4::PrintOperationPreview>>,
    surface: RefCell<Option<cairo::ImageSurface>>,
    n_pages: Cell<i32>,
    ready: Cell<bool>,
    on_ready: RefCell<Option<Box<dyn Fn()>>>,
}

impl PreviewEngine {
    /// Returns `None` after presenting an error when the preview backend fails
    /// to start.
    pub(crate) fn start(
        parent: &adw::ApplicationWindow,
        view: &sourceview5::View,
        title: &str,
        body_font: &str,
    ) -> Option<Rc<Self>> {
        let (operation, compositor) =
            crate::document_print::build_print_operation(view, title, body_font);
        operation.set_show_progress(false);
        operation.set_allow_async(true);

        let engine = Rc::new(Self {
            operation: RefCell::new(Some(operation.clone())),
            preview: RefCell::new(None),
            surface: RefCell::new(None),
            n_pages: Cell::new(-1),
            ready: Cell::new(false),
            on_ready: RefCell::new(None),
        });

        let engine_for_preview = Rc::clone(&engine);
        operation.connect_preview(move |_, preview, context, _| {
            engine_for_preview.preview.replace(Some(preview.clone()));

            if let Ok(placeholder) = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)
                && let Ok(cr) = cairo::Context::new(&placeholder)
            {
                context.set_cairo_context(&cr, PREVIEW_DPI, PREVIEW_DPI);
            }

            let engine_for_size = Rc::clone(&engine_for_preview);
            preview.connect_got_page_size(move |_, context, page_setup| {
                let width = pixels_from_points(page_setup.paper_width(gtk4::Unit::Points));
                let height = pixels_from_points(page_setup.paper_height(gtk4::Unit::Points));
                let Ok(surface) = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
                else {
                    return;
                };
                let Ok(cr) = cairo::Context::new(&surface) else {
                    return;
                };
                cr.set_source_rgb(1.0, 1.0, 1.0);
                let _ = cr.paint();
                context.set_cairo_context(&cr, PREVIEW_DPI, PREVIEW_DPI);
                engine_for_size.surface.replace(Some(surface));
            });

            let engine_for_ready = Rc::clone(&engine_for_preview);
            let compositor_for_ready = compositor.clone();
            preview.connect_ready(move |_, _| {
                engine_for_ready.n_pages.set(compositor_for_ready.n_pages());
                engine_for_ready.ready.set(true);
                if let Some(callback) = engine_for_ready.on_ready.borrow().as_ref() {
                    callback();
                }
            });

            true
        });

        match operation.run(gtk4::PrintOperationAction::Preview, Some(parent)) {
            Ok(gtk4::PrintOperationResult::Error) | Err(_) => {
                engine.finish();
                crate::dialogs::present_error(
                    parent,
                    &crate::error::AppError::Internal(gettextrs::gettext("Print preview failed.")),
                );
                None
            }
            Ok(_) => Some(engine),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.get()
    }

    pub(crate) fn n_pages(&self) -> i32 {
        self.n_pages.get()
    }

    pub(crate) fn render_page(&self, page: i32) -> Option<gdk::Texture> {
        if !self.ready.get() || page < 0 || page >= self.n_pages.get() {
            return None;
        }
        let preview = self.preview.borrow().clone()?;
        preview.render_page(page);
        let surface = self.surface.borrow().clone()?;
        texture_from_surface(&surface)
    }

    pub(crate) fn finish(&self) {
        if let Some(preview) = self.preview.borrow_mut().take() {
            preview.end_preview();
        }
        self.operation.borrow_mut().take();
        self.ready.set(false);
    }
}

/// Copies the rendered page into an exclusively owned surface and wraps it as a
/// GPU-uploadable texture.
fn texture_from_surface(surface: &cairo::ImageSurface) -> Option<gdk::Texture> {
    surface.flush();
    let width = surface.width();
    let height = surface.height();
    let copy = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).ok()?;
    {
        let cr = cairo::Context::new(&copy).ok()?;
        cr.set_source_surface(surface, 0.0, 0.0).ok()?;
        cr.paint().ok()?;
    }
    let stride = usize::try_from(copy.stride()).ok()?;
    let mut copy = copy;
    let data = copy.data().ok()?;
    let bytes = glib::Bytes::from(&data[..]);

    Some(
        gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        )
        .upcast(),
    )
}

fn pixels_from_points(points: f64) -> i32 {
    let scaled = (points * PREVIEW_DPI / POINTS_PER_INCH).ceil();
    if !scaled.is_finite() {
        return 1;
    }
    let bounded = scaled.min(f64::from(i32::MAX));
    let pixels = format!("{bounded:.0}");
    match pixels.parse::<i32>() {
        Ok(value) => value.max(1),
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::pixels_from_points;

    #[test]
    fn a4_width_in_points_maps_to_96_dpi_pixels() {
        assert_eq!(pixels_from_points(595.28), 794);
    }

    #[test]
    fn degenerate_sizes_clamp_to_one_pixel() {
        assert_eq!(pixels_from_points(0.0), 1);
        assert_eq!(pixels_from_points(-10.0), 1);
    }
}
