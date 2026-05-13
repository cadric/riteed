// Take a look at the license at the top of the repository in the LICENSE file.

use crate::{MarkAttributes, Snippet, View, prelude::*};
use glib::translate::*;

mod sealed {
    pub trait Sealed {}
    impl<T: super::IsA<super::View>> Sealed for T {}
}

pub trait ViewManualExt: sealed::Sealed + IsA<View> + 'static {
    #[doc(alias = "gtk_source_view_get_mark_attributes")]
    fn mark_attributes(&self, category: &str, priority: i32) -> Option<MarkAttributes> {
        unsafe {
            from_glib_none(ffi::gtk_source_view_get_mark_attributes(
                self.as_ref().to_glib_none().0,
                category.to_glib_none().0,
                priority as *mut _,
            ))
        }
    }
    #[doc(alias = "gtk_source_view_push_snippet")]
    fn push_snippet<P: IsA<Snippet>>(&self, snippet: &P, mut location: Option<&mut gtk::TextIter>) {
        unsafe {
            ffi::gtk_source_view_push_snippet(
                self.as_ref().to_glib_none().0,
                snippet.as_ref().to_glib_none().0,
                location.to_glib_none_mut().0,
            );
        }
    }
}

impl<O: IsA<View>> ViewManualExt for O {}
