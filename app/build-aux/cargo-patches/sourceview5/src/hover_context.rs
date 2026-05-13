use crate::HoverContext;
use glib::translate::*;

impl HoverContext {
    #[doc(alias = "gtk_source_hover_context_get_iter")]
    #[doc(alias = "get_iter")]
    pub fn iter(&self) -> Option<gtk::TextIter> {
        unsafe {
            let mut iter = gtk::TextIter::uninitialized();

            let success = from_glib(ffi::gtk_source_hover_context_get_iter(
                self.to_glib_none().0,
                iter.to_glib_none_mut().0,
            ));

            if success { Some(iter) } else { None }
        }
    }
}
