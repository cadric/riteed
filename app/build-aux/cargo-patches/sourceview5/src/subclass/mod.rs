// rustdoc-stripper-ignore-next
//! Traits intended for creating custom types.

// rustdoc-stripper-ignore-next
/// Struct to hold a pointer and free it on `Drop::drop`
pub(crate) struct PtrHolder<T, F: Fn(*mut T) + 'static>(*mut T, F);

impl<T, F: Fn(*mut T) + 'static> Drop for PtrHolder<T, F> {
    fn drop(&mut self) {
        (self.1)(self.0)
    }
}

pub mod buffer;
pub mod completion_proposal;
pub mod completion_provider;
pub mod gutter_renderer;
pub mod hover_provider;
pub mod indenter;
pub mod style_scheme_chooser;
pub mod view;

// rustdoc-stripper-ignore-next
/// Traits intended for blanket imports.
pub mod prelude {
    pub use super::buffer::{BufferImpl, BufferImplExt};
    pub use super::completion_proposal::CompletionProposalImpl;
    pub use super::completion_provider::{CompletionProviderImpl, CompletionProviderImplExt};
    pub use super::gutter_renderer::{GutterRendererImpl, GutterRendererImplExt};
    pub use super::hover_provider::{HoverProviderImpl, HoverProviderImplExt};
    pub use super::indenter::{IndenterImpl, IndenterImplExt};
    pub use super::style_scheme_chooser::{StyleSchemeChooserImpl, StyleSchemeChooserImplExt};
    pub use super::view::{ViewImpl, ViewImplExt};
}
