use glib::translate::*;

glib::wrapper! {
    #[doc(alias = "GtkSourceRegionIter")]
    pub struct RegionIter(BoxedInline<ffi::GtkSourceRegionIter>);
}

impl RegionIter {
    #[doc(alias = "gtk_source_region_iter_get_subregion")]
    #[doc(alias = "get_subregion")]
    pub fn subregion(&mut self, start: &mut gtk::TextIter, end: &mut gtk::TextIter) -> bool {
        unsafe {
            from_glib(ffi::gtk_source_region_iter_get_subregion(
                self.to_glib_none_mut().0,
                start.to_glib_none_mut().0,
                end.to_glib_none_mut().0,
            ))
        }
    }

    #[doc(alias = "gtk_source_region_iter_is_end")]
    pub fn is_end(&self) -> bool {
        unsafe {
            from_glib(ffi::gtk_source_region_iter_is_end(mut_override(
                self.to_glib_none().0,
            )))
        }
    }

    #[allow(clippy::should_implement_trait)]
    #[doc(alias = "gtk_source_region_iter_next")]
    pub fn next(&mut self) -> bool {
        unsafe { from_glib(ffi::gtk_source_region_iter_next(self.to_glib_none_mut().0)) }
    }
}
