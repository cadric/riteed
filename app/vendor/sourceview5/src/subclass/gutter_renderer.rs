// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for subclassing [`GutterRenderer`](crate::GutterRenderer).

use crate::{Buffer, GutterLines, GutterRenderer, View, prelude::*};
use glib::translate::*;
use gtk::subclass::prelude::*;

pub trait GutterRendererImpl: WidgetImpl + ObjectSubclass<Type: IsA<gtk::Widget>> {
    fn query_data(&self, lines: &GutterLines, line: u32) {
        self.parent_query_data(lines, line)
    }

    fn begin(&self, lines: &GutterLines) {
        self.parent_begin(lines)
    }

    fn end(&self) {
        self.parent_end()
    }

    fn change_buffer(&self, old_buffer: Option<&Buffer>) {
        self.parent_change_buffer(old_buffer)
    }

    fn change_view(&self, old_view: Option<&View>) {
        self.parent_change_view(old_view)
    }

    fn query_activatable(&self, iter: &gtk::TextIter, area: &gdk::Rectangle) -> bool {
        self.parent_query_activatable(iter, area)
    }

    fn activate(
        &self,
        iter: &gtk::TextIter,
        area: &gdk::Rectangle,
        button: u32,
        state: gdk::ModifierType,
        n_presses: i32,
    ) {
        self.parent_activate(iter, area, button, state, n_presses)
    }

    fn snapshot_line(&self, snapshot: &gtk::Snapshot, lines: &GutterLines, line: u32) {
        self.parent_snapshot_line(snapshot, lines, line)
    }
}

pub trait GutterRendererImplExt: GutterRendererImpl {
    fn parent_query_data(&self, lines: &GutterLines, line: u32) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).query_data {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    lines.to_glib_none().0,
                    line,
                )
            }
        }
    }

    fn parent_begin(&self, lines: &GutterLines) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).begin {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    lines.to_glib_none().0,
                )
            }
        }
    }

    fn parent_end(&self) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).end {
                f(self
                    .obj()
                    .unsafe_cast_ref::<GutterRenderer>()
                    .to_glib_none()
                    .0)
            }
        }
    }

    fn parent_change_buffer(&self, old_buffer: Option<&Buffer>) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).change_buffer {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    old_buffer.to_glib_none().0,
                )
            }
        }
    }

    fn parent_change_view(&self, old_view: Option<&View>) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).change_view {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    old_view.to_glib_none().0,
                )
            }
        }
    }

    fn parent_query_activatable(&self, iter: &gtk::TextIter, area: &gdk::Rectangle) -> bool {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).query_activatable {
                from_glib(f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    mut_override(iter.to_glib_none().0),
                    mut_override(area.to_glib_none().0),
                ))
            } else {
                false
            }
        }
    }

    fn parent_activate(
        &self,
        iter: &gtk::TextIter,
        area: &gdk::Rectangle,
        button: u32,
        state: gdk::ModifierType,
        n_presses: i32,
    ) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).activate {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    mut_override(iter.to_glib_none().0),
                    mut_override(area.to_glib_none().0),
                    button,
                    state.into_glib(),
                    n_presses,
                )
            }
        }
    }

    fn parent_snapshot_line(&self, snapshot: &gtk::Snapshot, lines: &GutterLines, line: u32) {
        unsafe {
            let data = Self::type_data();
            let parent_class =
                data.as_ref().parent_class() as *mut ffi::GtkSourceGutterRendererClass;
            if let Some(f) = (*parent_class).snapshot_line {
                f(
                    self.obj()
                        .unsafe_cast_ref::<GutterRenderer>()
                        .to_glib_none()
                        .0,
                    snapshot.to_glib_none().0,
                    lines.to_glib_none().0,
                    line,
                )
            }
        }
    }
}

impl<T: GutterRendererImpl> GutterRendererImplExt for T {}

unsafe impl<T: GutterRendererImpl> IsSubclassable<T> for GutterRenderer {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let klass = class.as_mut();
        klass.query_data = Some(gutter_renderer_query_data::<T>);
        klass.begin = Some(gutter_renderer_begin::<T>);
        klass.end = Some(gutter_renderer_end::<T>);
        klass.change_buffer = Some(gutter_renderer_change_buffer::<T>);
        klass.change_view = Some(gutter_renderer_change_view::<T>);
        klass.query_activatable = Some(gutter_renderer_query_activatable::<T>);
        klass.activate = Some(gutter_renderer_activate::<T>);
        klass.snapshot_line = Some(gutter_renderer_snapshot_line::<T>);
    }
}

unsafe extern "C" fn gutter_renderer_query_data<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    lines: *mut ffi::GtkSourceGutterLines,
    line: u32,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.query_data(&from_glib_borrow(lines), line);
    }
}

unsafe extern "C" fn gutter_renderer_begin<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    lines: *mut ffi::GtkSourceGutterLines,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.begin(&from_glib_borrow(lines));
    }
}

unsafe extern "C" fn gutter_renderer_end<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.end();
    }
}

unsafe extern "C" fn gutter_renderer_change_buffer<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    buffer: *mut ffi::GtkSourceBuffer,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        let buffer: Borrowed<Option<Buffer>> = from_glib_borrow(buffer);
        imp.change_buffer(buffer.as_ref().as_ref());
    }
}

unsafe extern "C" fn gutter_renderer_change_view<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    view: *mut ffi::GtkSourceView,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        let view: Borrowed<Option<View>> = from_glib_borrow(view);
        imp.change_view(view.as_ref().as_ref());
    }
}

unsafe extern "C" fn gutter_renderer_query_activatable<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    iter: *mut gtk::ffi::GtkTextIter,
    rect: *mut gdk::ffi::GdkRectangle,
) -> glib::ffi::gboolean {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.query_activatable(&from_glib_borrow(iter), &from_glib_borrow(rect))
            .into_glib()
    }
}

unsafe extern "C" fn gutter_renderer_activate<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    iter: *mut gtk::ffi::GtkTextIter,
    rect: *mut gdk::ffi::GdkRectangle,
    button: u32,
    state: gdk::ffi::GdkModifierType,
    n_presses: i32,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.activate(
            &from_glib_borrow(iter),
            &from_glib_borrow(rect),
            button,
            from_glib(state),
            n_presses,
        )
    }
}

unsafe extern "C" fn gutter_renderer_snapshot_line<T: GutterRendererImpl>(
    ptr: *mut ffi::GtkSourceGutterRenderer,
    snapshot: *mut gtk::ffi::GtkSnapshot,
    lines: *mut ffi::GtkSourceGutterLines,
    line: u32,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.snapshot_line(&from_glib_borrow(snapshot), &from_glib_borrow(lines), line)
    }
}
