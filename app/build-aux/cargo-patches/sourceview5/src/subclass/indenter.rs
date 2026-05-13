// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for implementing the [`Indenter`](crate::Indenter) interface.

use crate::{Indenter, View, prelude::*};
use glib::subclass::prelude::*;
use glib::translate::*;

pub trait IndenterImpl: ObjectImpl + ObjectSubclass<Type: IsA<glib::Object>> {
    fn is_trigger(
        &self,
        view: &View,
        location: &gtk::TextIter,
        state: gdk::ModifierType,
        keyval: gdk::Key,
    ) -> bool {
        self.parent_is_trigger(view, location, state, keyval)
    }

    fn indent(&self, view: &View, iter: &mut gtk::TextIter) {
        self.parent_indent(view, iter)
    }
}

pub trait IndenterImplExt: IndenterImpl {
    fn parent_is_trigger(
        &self,
        view: &View,
        location: &gtk::TextIter,
        state: gdk::ModifierType,
        keyval: gdk::Key,
    ) -> bool {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<Indenter>()
                as *const ffi::GtkSourceIndenterInterface;

            let func = (*parent_iface)
                .is_trigger
                .expect("no parent \"is_trigger\" implementation");

            from_glib(func(
                self.obj().unsafe_cast_ref::<Indenter>().to_glib_none().0,
                view.to_glib_none().0,
                location.to_glib_none().0,
                state.into_glib(),
                keyval.into_glib(),
            ))
        }
    }

    fn parent_indent(&self, view: &View, iter: &mut gtk::TextIter) {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<Indenter>()
                as *const ffi::GtkSourceIndenterInterface;

            let func = (*parent_iface)
                .indent
                .expect("no parent \"indent\" implementation");

            func(
                self.obj().unsafe_cast_ref::<Indenter>().to_glib_none().0,
                view.to_glib_none().0,
                iter.to_glib_none_mut().0,
            )
        }
    }
}
impl<T: IndenterImpl> IndenterImplExt for T {}

unsafe impl<T: IndenterImpl> IsImplementable<T> for Indenter {
    fn interface_init(iface: &mut glib::Interface<Self>) {
        let iface = iface.as_mut();

        iface.is_trigger = Some(indenter_is_trigger::<T>);
        iface.indent = Some(indenter_indent::<T>);
    }
}

unsafe extern "C" fn indenter_is_trigger<T: IndenterImpl>(
    indenter: *mut ffi::GtkSourceIndenter,
    view: *mut ffi::GtkSourceView,
    iter: *const gtk::ffi::GtkTextIter,
    state: gdk::ffi::GdkModifierType,
    keyval: u32,
) -> glib::ffi::gboolean {
    unsafe {
        let instance = &*(indenter as *mut T::Instance);
        let imp = instance.imp();

        imp.is_trigger(
            &from_glib_borrow(view),
            &from_glib_borrow(iter),
            from_glib(state),
            from_glib(keyval),
        )
        .into_glib()
    }
}

unsafe extern "C" fn indenter_indent<T: IndenterImpl>(
    indenter: *mut ffi::GtkSourceIndenter,
    view: *mut ffi::GtkSourceView,
    iterptr: *mut gtk::ffi::GtkTextIter,
) {
    unsafe {
        let instance = &*(indenter as *mut T::Instance);
        let imp = instance.imp();
        let mut iter = from_glib_full(iterptr);
        imp.indent(&from_glib_borrow(view), &mut iter);
        *iterptr = *iter.to_glib_full();
    }
}
