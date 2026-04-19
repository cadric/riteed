// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for subclassing [`Buffer`](crate::Buffer).

use crate::{BracketMatchType, Buffer, prelude::*};
use glib::translate::*;
use gtk::subclass::prelude::*;

pub trait BufferImpl: TextBufferImpl + ObjectSubclass<Type: IsA<gtk::TextBuffer>> {
    fn bracket_matched(&self, iter: Option<&gtk::TextIter>, state: BracketMatchType) {
        self.parent_bracket_matched(iter, state)
    }
}

pub trait BufferImplExt: BufferImpl {
    fn parent_bracket_matched(&self, iter: Option<&gtk::TextIter>, state: BracketMatchType) {
        unsafe {
            let data = Self::type_data();
            let parent_class = data.as_ref().parent_class() as *mut ffi::GtkSourceBufferClass;
            if let Some(f) = (*parent_class).bracket_matched {
                f(
                    self.obj().unsafe_cast_ref::<Buffer>().to_glib_none().0,
                    mut_override(iter.to_glib_none().0),
                    state.into_glib(),
                )
            }
        }
    }
}

impl<T: BufferImpl> BufferImplExt for T {}

unsafe impl<T: BufferImpl> IsSubclassable<T> for Buffer {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let klass = class.as_mut();
        klass.bracket_matched = Some(buffer_bracket_matched::<T>);
    }
}

unsafe extern "C" fn buffer_bracket_matched<T: BufferImpl>(
    ptr: *mut ffi::GtkSourceBuffer,
    iterptr: *mut gtk::ffi::GtkTextIter,
    state: i32,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        let iter = Option::<gtk::TextIter>::from_glib_borrow(iterptr);
        imp.bracket_matched(iter.as_ref().as_ref(), from_glib(state));
    }
}
