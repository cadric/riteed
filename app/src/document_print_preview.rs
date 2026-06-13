use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{cairo, gdk, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use sourceview5::prelude::*;

const PREVIEW_DPI: f64 = 96.0;
const POINTS_PER_INCH: f64 = 72.0;
// GtkSourcePrintCompositor draws in point space. The preview surface is
// rasterized at 96 dpi, so scale cairo by 96/72 while reporting 72 dpi to
// GtkPrintContext to keep paper geometry and font metrics aligned.
const PREVIEW_SCALE: f64 = PREVIEW_DPI / POINTS_PER_INCH;

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
                context.set_cairo_context(&cr, POINTS_PER_INCH, POINTS_PER_INCH);
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
                cr.scale(PREVIEW_SCALE, PREVIEW_SCALE);
                context.set_cairo_context(&cr, POINTS_PER_INCH, POINTS_PER_INCH);
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
                    &crate::error::AppError::Internal(gettext("Print preview failed.")),
                );
                None
            }
            Ok(_) => Some(engine),
        }
    }

    pub(crate) fn set_on_ready(&self, callback: Box<dyn Fn()>) {
        if self.ready.get() {
            callback();
        }
        self.on_ready.replace(Some(callback));
    }

    #[cfg(test)]
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

struct PreviewDialogState {
    engine: RefCell<Option<Rc<PreviewEngine>>>,
    current_page: Cell<i32>,
    parent: adw::ApplicationWindow,
    view: sourceview5::View,
    title: String,
    font_family: String,
    picture: gtk4::Picture,
    page_label: gtk4::Label,
    previous_button: gtk4::Button,
    next_button: gtk4::Button,
}

pub(crate) fn present_preview(
    parent: &adw::ApplicationWindow,
    view: &sourceview5::View,
    title: &str,
    body_font: &str,
    on_print: Rc<dyn Fn(&str)>,
) {
    let desc = gtk4::pango::FontDescription::from_string(body_font);
    let font_family = desc
        .family()
        .map_or_else(|| String::from("Monospace"), |family| family.to_string());
    let initial_size = (desc.size() / gtk4::pango::SCALE).max(6);

    let picture = gtk4::Picture::builder()
        .can_shrink(true)
        .content_fit(gtk4::ContentFit::Contain)
        .build();
    picture.add_css_class("card");

    let page_label = gtk4::Label::new(None);
    page_label.add_css_class("dim-label");

    let previous_button = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text(pgettext("print preview", "Previous Page"))
        .build();
    previous_button.set_sensitive(false);
    let next_button = gtk4::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text(pgettext("print preview", "Next Page"))
        .build();
    next_button.set_sensitive(false);

    let size_adjustment = gtk4::Adjustment::new(f64::from(initial_size), 6.0, 32.0, 1.0, 4.0, 0.0);
    let size_spin = gtk4::SpinButton::new(Some(&size_adjustment), 1.0, 0);
    size_spin.set_tooltip_text(Some(&pgettext("print preview", "Text Size")));

    let mut print_label = pgettext("print preview", "Print");
    print_label.push('…');
    let print_button = gtk4::Button::with_label(&print_label);
    print_button.add_css_class("suggested-action");

    let header = adw::HeaderBar::new();
    header.pack_start(&previous_button);
    header.pack_start(&page_label);
    header.pack_start(&next_button);
    header.pack_end(&print_button);
    header.pack_end(&size_spin);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&picture)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&scrolled));

    let dialog = adw::Dialog::builder()
        .title(gettext("Print Preview"))
        .content_width(760)
        .content_height(980)
        .child(&toolbar_view)
        .build();

    let Some(engine) = PreviewEngine::start(parent, view, title, body_font) else {
        return;
    };
    let state = Rc::new(PreviewDialogState {
        engine: RefCell::new(Some(engine)),
        current_page: Cell::new(0),
        parent: parent.clone(),
        view: view.clone(),
        title: String::from(title),
        font_family,
        picture,
        page_label,
        previous_button,
        next_button,
    });

    wire_ready(&state);

    let nav_state = Rc::clone(&state);
    state.previous_button.connect_clicked(move |_| {
        nav_state.show_page(nav_state.current_page.get() - 1);
    });
    let nav_state = Rc::clone(&state);
    state.next_button.connect_clicked(move |_| {
        nav_state.show_page(nav_state.current_page.get() + 1);
    });

    let size_state = Rc::clone(&state);
    size_spin.connect_value_changed(move |spin| {
        size_state.restart_with_size(spin.value_as_int());
    });

    let print_state = Rc::clone(&state);
    let print_dialog = dialog.clone();
    let print_size_spin = size_spin.clone();
    print_button.connect_clicked(move |_| {
        if let Some(engine) = print_state.engine.borrow().as_ref() {
            engine.finish();
        }
        print_dialog.close();
        let body_font = format!(
            "{} {}",
            print_state.font_family,
            print_size_spin.value_as_int()
        );
        on_print(&body_font);
    });

    let close_state = Rc::clone(&state);
    dialog.connect_closed(move |_| {
        if let Some(engine) = close_state.engine.borrow().as_ref() {
            engine.finish();
        }
    });

    dialog.present(Some(parent));
}

impl PreviewDialogState {
    fn show_page(self: &Rc<Self>, page: i32) {
        let Some(engine) = self.engine.borrow().clone() else {
            return;
        };
        let total = engine.n_pages().max(1);
        let page = page.clamp(0, total - 1);
        self.current_page.set(page);
        if let Some(texture) = engine.render_page(page) {
            self.picture.set_paintable(Some(&texture));
        }
        let template = pgettext("print preview", "Page %1$d of %2$d");
        self.page_label.set_label(
            &template
                .replace("%1$d", &(page + 1).to_string())
                .replace("%2$d", &total.to_string()),
        );
        self.previous_button.set_sensitive(page > 0);
        self.next_button.set_sensitive(page + 1 < total);
    }

    fn restart_with_size(self: &Rc<Self>, size: i32) {
        if let Some(engine) = self.engine.borrow().as_ref() {
            engine.finish();
        }
        let body_font = format!("{} {}", self.font_family, size.max(6));
        let engine = PreviewEngine::start(&self.parent, &self.view, &self.title, &body_font);
        self.engine.replace(engine);
        wire_ready(self);
    }
}

fn wire_ready(state: &Rc<PreviewDialogState>) {
    let Some(engine) = state.engine.borrow().clone() else {
        return;
    };
    let ready_state = Rc::clone(state);
    engine.set_on_ready(Box::new(move || {
        ready_state.show_page(ready_state.current_page.get());
    }));
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
