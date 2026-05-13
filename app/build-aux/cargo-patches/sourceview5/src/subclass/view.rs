// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for subclassing [`View`](crate::View).

use crate::{Snippet, View, prelude::*};
use glib::translate::*;
use gtk::subclass::prelude::*;

pub trait ViewImpl: TextViewImpl + ObjectSubclass<Type: IsA<gtk::TextView>> {
    fn line_mark_activated(
        &self,
        iter: &gtk::TextIter,
        button: u32,
        state: gdk::ModifierType,
        n_presses: i32,
    ) {
        self.parent_line_mark_activated(iter, button, state, n_presses)
    }

    fn show_completion(&self) {
        self.parent_show_completion()
    }

    fn move_lines(&self, down: bool) {
        self.parent_move_lines(down)
    }

    fn move_words(&self, steps: i32) {
        self.parent_move_words(steps)
    }

    fn push_snippet(&self, snippet: &Snippet, location: &gtk::TextIter) {
        self.parent_push_snippet(snippet, location)
    }
}
pub trait ViewImplExt: ViewImpl {
    fn parent_line_mark_activated(
        &self,
        iter: &gtk::TextIter,
        button: u32,
        state: gdk::ModifierType,
        n_presses: i32,
    ) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceViewClass;
            if let Some(f) = (*parent_class).line_mark_activated {
                f(
                    self.obj().unsafe_cast_ref::<View>().to_glib_none().0,
                    iter.to_glib_none().0,
                    button,
                    state.into_glib(),
                    n_presses,
                )
            }
        }
    }

    fn parent_show_completion(&self) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceViewClass;
            if let Some(f) = (*parent_class).show_completion {
                f(self.obj().unsafe_cast_ref::<View>().to_glib_none().0)
            }
        }
    }

    fn parent_move_lines(&self, down: bool) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceViewClass;
            if let Some(f) = (*parent_class).move_lines {
                f(
                    self.obj().unsafe_cast_ref::<View>().to_glib_none().0,
                    down.into_glib(),
                )
            }
        }
    }

    fn parent_move_words(&self, steps: i32) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceViewClass;
            if let Some(f) = (*parent_class).move_words {
                f(self.obj().unsafe_cast_ref::<View>().to_glib_none().0, steps)
            }
        }
    }

    fn parent_push_snippet(&self, snippet: &Snippet, location: &gtk::TextIter) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceViewClass;
            if let Some(f) = (*parent_class).push_snippet {
                f(
                    self.obj().unsafe_cast_ref::<View>().to_glib_none().0,
                    snippet.to_glib_none().0,
                    mut_override(location.to_glib_none().0),
                )
            }
        }
    }
}

impl<T: ViewImpl> ViewImplExt for T {}

unsafe impl<T: ViewImpl> IsSubclassable<T> for View {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let klass = class.as_mut();
        klass.line_mark_activated = Some(view_line_mark_activated::<T>);
        klass.show_completion = Some(view_show_completion::<T>);
        klass.move_lines = Some(view_move_lines::<T>);
        klass.move_words = Some(view_move_words::<T>);
        klass.push_snippet = Some(view_push_snippet::<T>);
    }
}

unsafe extern "C" fn view_line_mark_activated<T: ViewImpl>(
    ptr: *mut ffi::GtkSourceView,
    iter: *const gtk::ffi::GtkTextIter,
    button: u32,
    state: gdk::ffi::GdkModifierType,
    n_presses: i32,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.line_mark_activated(&from_glib_borrow(iter), button, from_glib(state), n_presses);
    }
}

unsafe extern "C" fn view_show_completion<T: ViewImpl>(ptr: *mut ffi::GtkSourceView) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.show_completion();
    }
}

unsafe extern "C" fn view_move_lines<T: ViewImpl>(
    ptr: *mut ffi::GtkSourceView,
    down: glib::ffi::gboolean,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.move_lines(from_glib(down));
    }
}

unsafe extern "C" fn view_move_words<T: ViewImpl>(ptr: *mut ffi::GtkSourceView, steps: i32) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.move_words(steps);
    }
}

unsafe extern "C" fn view_push_snippet<T: ViewImpl>(
    ptr: *mut ffi::GtkSourceView,
    snippet: *mut ffi::GtkSourceSnippet,
    iter: *mut gtk::ffi::GtkTextIter,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        imp.push_snippet(&from_glib_borrow(snippet), &from_glib_borrow(iter));
    }
}
