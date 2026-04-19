// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for implementing the [`ContentProvider`](crate::ContentProvider) interface.

use crate::{
    CompletionCell, CompletionContext, CompletionProposal, CompletionProvider, prelude::*,
};
use glib::subclass::prelude::*;
use glib::translate::*;
use std::{future::Future, pin::Pin};

pub trait CompletionProviderImpl: ObjectImpl + ObjectSubclass<Type: IsA<glib::Object>> {
    fn activate(&self, context: &CompletionContext, proposal: &CompletionProposal) {
        self.parent_activate(context, proposal)
    }

    fn display(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
        cell: &CompletionCell,
    ) {
        self.parent_display(context, proposal, cell)
    }

    fn title(&self) -> Option<glib::GString> {
        self.parent_title()
    }

    fn priority(&self, context: &CompletionContext) -> i32 {
        self.parent_priority(context)
    }

    fn is_trigger(&self, iter: &gtk::TextIter, c: char) -> bool {
        self.parent_is_trigger(iter, c)
    }

    fn key_activates(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
        keyval: gdk::Key,
        state: gdk::ModifierType,
    ) -> bool {
        self.parent_key_activates(context, proposal, keyval, state)
    }

    fn refilter(&self, context: &CompletionContext, model: &gio::ListModel) {
        self.parent_refilter(context, model)
    }

    fn list_alternates(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
    ) -> Vec<CompletionProposal> {
        self.parent_list_alternates(context, proposal)
    }

    fn populate(&self, context: &CompletionContext) -> Result<gio::ListModel, glib::Error> {
        self.parent_populate(context)
    }

    fn populate_future(
        &self,
        context: &CompletionContext,
    ) -> Pin<Box<dyn Future<Output = Result<gio::ListModel, glib::Error>>>> {
        self.parent_populate_future(context)
    }
}

pub trait CompletionProviderImplExt: CompletionProviderImpl {
    fn parent_activate(&self, context: &CompletionContext, proposal: &CompletionProposal) {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).activate {
                func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                    proposal.to_glib_none().0,
                )
            }
        }
    }

    fn parent_title(&self) -> Option<glib::GString> {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).get_title {
                from_glib_full(func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                ))
            } else {
                None
            }
        }
    }

    fn parent_display(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
        cell: &CompletionCell,
    ) {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).display {
                func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                    proposal.to_glib_none().0,
                    cell.to_glib_none().0,
                )
            }
        }
    }

    fn parent_priority(&self, context: &CompletionContext) -> i32 {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).get_priority {
                func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                )
            } else {
                0
            }
        }
    }

    fn parent_is_trigger(&self, iter: &gtk::TextIter, c: char) -> bool {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).is_trigger {
                from_glib(func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    iter.to_glib_none().0,
                    c.into_glib(),
                ))
            } else {
                false
            }
        }
    }

    fn parent_list_alternates(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
    ) -> Vec<CompletionProposal> {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).list_alternates {
                let output = func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                    proposal.to_glib_none().0,
                );
                FromGlibPtrArrayContainerAsVec::from_glib_full_as_vec(output)
            } else {
                vec![]
            }
        }
    }

    fn parent_refilter(&self, context: &CompletionContext, model: &gio::ListModel) {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).refilter {
                func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                    model.to_glib_none().0,
                )
            }
        }
    }

    fn parent_key_activates(
        &self,
        context: &CompletionContext,
        proposal: &CompletionProposal,
        keyval: gdk::Key,
        state: gdk::ModifierType,
    ) -> bool {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            if let Some(func) = (*parent_iface).key_activates {
                from_glib(func(
                    self.obj()
                        .unsafe_cast_ref::<CompletionProvider>()
                        .to_glib_none()
                        .0,
                    context.to_glib_none().0,
                    proposal.to_glib_none().0,
                    keyval.into_glib(),
                    state.into_glib(),
                ))
            } else {
                false
            }
        }
    }

    fn parent_populate(&self, context: &CompletionContext) -> Result<gio::ListModel, glib::Error> {
        unsafe {
            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            let func = (*parent_iface)
                .populate
                .expect("no parent \"populate\" implementation");
            let mut error = std::ptr::null_mut();
            let result = func(
                self.obj()
                    .unsafe_cast_ref::<CompletionProvider>()
                    .to_glib_none()
                    .0,
                context.to_glib_none().0,
                &mut error,
            );
            if error.is_null() {
                Ok(from_glib_full(result))
            } else {
                Err(from_glib_full(error))
            }
        }
    }

    fn parent_populate_future(
        &self,
        context: &CompletionContext,
    ) -> Pin<Box<dyn Future<Output = Result<gio::ListModel, glib::Error>>>> {
        let context = context.clone();
        Box::pin(gio::GioFuture::new(
            &self.ref_counted(),
            move |imp, cancellable, send| {
                imp.parent_pouplate_async(&context, Some(cancellable), move |res| {
                    send.resolve(res);
                });
            },
        ))
    }

    #[allow(clippy::type_complexity)]
    fn parent_pouplate_async<
        Q: IsA<gio::Cancellable>,
        C: FnOnce(Result<gio::ListModel, glib::Error>) + 'static,
    >(
        &self,
        context: &CompletionContext,
        cancellable: Option<&Q>,
        callback: C,
    ) {
        unsafe {
            let main_context = glib::MainContext::ref_thread_default();
            let is_main_context_owner = main_context.is_owner();
            let has_acquired_main_context = (!is_main_context_owner)
                .then(|| main_context.acquire().ok())
                .flatten();
            assert!(
                is_main_context_owner || has_acquired_main_context.is_some(),
                "Async operations only allowed if the thread is owning the MainContext"
            );

            let type_data = Self::type_data();
            let parent_iface = type_data.as_ref().parent_interface::<CompletionProvider>()
                as *const ffi::GtkSourceCompletionProviderInterface;

            let f = (*parent_iface)
                .populate_async
                .expect("no parent \"populate_async\" implementation");
            let finish = (*parent_iface)
                .populate_finish
                .expect("no parent \"populate_finish\" implementation");

            let user_data: Box<(glib::thread_guard::ThreadGuard<C>, _)> =
                Box::new((glib::thread_guard::ThreadGuard::new(callback), finish));

            unsafe extern "C" fn parent_populate_async_trampoline<
                C: FnOnce(Result<gio::ListModel, glib::Error>) + 'static,
            >(
                source_object_ptr: *mut glib::gobject_ffi::GObject,
                res: *mut gio::ffi::GAsyncResult,
                user_data: glib::ffi::gpointer,
            ) {
                unsafe {
                    let mut error = std::ptr::null_mut();
                    let cb: Box<(
                        glib::thread_guard::ThreadGuard<C>,
                        fn(
                            *mut ffi::GtkSourceCompletionProvider,
                            *mut gio::ffi::GAsyncResult,
                            *mut *mut glib::ffi::GError,
                        ) -> *mut gio::ffi::GListModel,
                    )> = Box::from_raw(user_data as *mut _);
                    let model = cb.1(source_object_ptr as _, res, &mut error);
                    let result = if error.is_null() {
                        Ok(from_glib_full(model))
                    } else {
                        Err(from_glib_full(error))
                    };
                    let cb = cb.0.into_inner();
                    cb(result);
                }
            }

            let cancellable = cancellable.map(|p| p.as_ref());
            let callback = parent_populate_async_trampoline::<C>;
            f(
                self.obj()
                    .unsafe_cast_ref::<CompletionProvider>()
                    .to_glib_none()
                    .0,
                context.to_glib_none().0,
                cancellable.to_glib_none().0,
                Some(callback),
                Box::into_raw(user_data) as *mut _,
            );
        }
    }
}

impl<T: CompletionProviderImpl> CompletionProviderImplExt for T {}

unsafe impl<T: CompletionProviderImpl> IsImplementable<T> for CompletionProvider {
    fn interface_init(iface: &mut glib::Interface<Self>) {
        let iface = iface.as_mut();

        iface.activate = Some(completion_provider_activate::<T>);
        iface.display = Some(completion_provider_display::<T>);
        iface.get_title = Some(completion_provider_get_title::<T>);
        iface.get_priority = Some(completion_provider_get_priority::<T>);
        iface.is_trigger = Some(completion_provider_is_trigger::<T>);
        iface.refilter = Some(completion_provider_refilter::<T>);
        iface.key_activates = Some(completion_provider_key_activates::<T>);
        iface.list_alternates = Some(completion_provider_list_alternates::<T>);
        iface.populate = Some(completion_provider_populate::<T>);
        iface.populate_async = Some(completion_provider_populate_async::<T>);
        iface.populate_finish = Some(completion_provider_populate_finish::<T>);
    }
}

unsafe extern "C" fn completion_provider_activate<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    proposal: *mut ffi::GtkSourceCompletionProposal,
) {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.activate(&from_glib_borrow(context), &from_glib_borrow(proposal))
    }
}

unsafe extern "C" fn completion_provider_display<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    proposal: *mut ffi::GtkSourceCompletionProposal,
    cell: *mut ffi::GtkSourceCompletionCell,
) {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.display(
            &from_glib_borrow(context),
            &from_glib_borrow(proposal),
            &from_glib_borrow(cell),
        )
    }
}

unsafe extern "C" fn completion_provider_get_title<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
) -> *mut libc::c_char {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.title().to_glib_full()
    }
}

unsafe extern "C" fn completion_provider_get_priority<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
) -> i32 {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.priority(&from_glib_borrow(context))
    }
}

unsafe extern "C" fn completion_provider_is_trigger<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    iter: *const gtk::ffi::GtkTextIter,
    c: u32,
) -> glib::ffi::gboolean {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.is_trigger(
            &from_glib_borrow(iter),
            TryFrom::try_from(c).expect("Invalid character"),
        )
        .into_glib()
    }
}

unsafe extern "C" fn completion_provider_refilter<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    model: *mut gio::ffi::GListModel,
) {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.refilter(&from_glib_borrow(context), &from_glib_borrow(model))
    }
}

unsafe extern "C" fn completion_provider_key_activates<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    proposal: *mut ffi::GtkSourceCompletionProposal,
    keyval: u32,
    state: gdk::ffi::GdkModifierType,
) -> glib::ffi::gboolean {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.key_activates(
            &from_glib_borrow(context),
            &from_glib_borrow(proposal),
            from_glib(keyval),
            from_glib(state),
        )
        .into_glib()
    }
}

unsafe extern "C" fn completion_provider_list_alternates<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    proposal: *mut ffi::GtkSourceCompletionProposal,
) -> *mut glib::ffi::GPtrArray {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        imp.list_alternates(&from_glib_borrow(context), &from_glib_borrow(proposal))
            .to_glib_full()
    }
}

unsafe extern "C" fn completion_provider_populate<T: CompletionProviderImpl>(
    provider: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    error: *mut *mut glib::ffi::GError,
) -> *mut gio::ffi::GListModel {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();

        let res = imp.populate(&from_glib_borrow(context));
        match res {
            Ok(model) => {
                *error = std::ptr::null_mut();
                model.to_glib_full()
            }
            Err(err) => {
                *error = err.into_glib_ptr();
                std::ptr::null_mut()
            }
        }
    }
}

unsafe extern "C" fn completion_provider_populate_async<T: CompletionProviderImpl>(
    ptr: *mut ffi::GtkSourceCompletionProvider,
    context: *mut ffi::GtkSourceCompletionContext,
    cancellable_ptr: *mut gio::ffi::GCancellable,
    callback: gio::ffi::GAsyncReadyCallback,
    user_data: glib::ffi::gpointer,
) {
    unsafe {
        let instance = &*(ptr as *mut T::Instance);
        let imp = instance.imp();
        let wrap: CompletionProvider = from_glib_none(ptr);

        let context: CompletionContext = from_glib_none(context);
        let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable_ptr);

        let closure = move |result: gio::LocalTask<gio::ListModel>,
                            source_object: Option<&CompletionProvider>| {
            let result: *mut gio::ffi::GAsyncResult = result
                .unsafe_cast_ref::<gio::AsyncResult>()
                .to_glib_none()
                .0;
            let source_object = source_object
                .map(|p| p.unsafe_cast_ref::<glib::Object>())
                .to_glib_none()
                .0;
            callback.unwrap()(source_object, result, user_data)
        };

        let t = gio::LocalTask::new(Some(&wrap), cancellable.as_ref(), closure);

        glib::MainContext::default().spawn_local(async move {
            let res = imp.populate_future(&context).await;
            t.return_result(res);
        });
    }
}

unsafe extern "C" fn completion_provider_populate_finish<T: CompletionProviderImpl>(
    _ptr: *mut ffi::GtkSourceCompletionProvider,
    res_ptr: *mut gio::ffi::GAsyncResult,
    error_ptr: *mut *mut glib::ffi::GError,
) -> *mut gio::ffi::GListModel {
    unsafe {
        let res: gio::AsyncResult = from_glib_none(res_ptr);
        let t = res.downcast::<gio::LocalTask<gio::ListModel>>().unwrap();
        let ret = t.propagate();
        match ret {
            Ok(model) => model.to_glib_full(),
            Err(e) => {
                *error_ptr = e.into_glib_ptr();
                std::ptr::null_mut()
            }
        }
    }
}
