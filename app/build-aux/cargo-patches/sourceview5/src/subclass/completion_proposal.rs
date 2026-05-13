// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for implementing the [`CompletionProposal`](crate::CompletionProposal) interface.

use crate::{CompletionProposal, prelude::*};
use glib::subclass::prelude::*;

pub trait CompletionProposalImpl: ObjectImpl + ObjectSubclass<Type: IsA<glib::Object>> {}

unsafe impl<T: CompletionProposalImpl> IsImplementable<T> for CompletionProposal {}
