// Take a look at the license at the top of the repository in the LICENSE file.

// rustdoc-stripper-ignore-next
//! Traits intended for implementing the [`HoverProvider`](crate::HoverProvider) interface.

use crate::{HoverContext, HoverDisplay, HoverProvider, prelude::*};
use glib::subclass::prelude::*;
use glib::translate::*;
use std::pin::Pin;

pub trait HoverProviderImpl: ObjectImpl + ObjectSubclass<Type: IsA<glib::Object>> {
    fn populate_future(
        &self,
        context: &HoverContext,
        display: &HoverDisplay,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), glib::Error>> + 'static>> {
        self.parent_populate_future(context, display)
    }
}

pub trait HoverProviderImplExt: HoverProviderImpl {
    fn parent_populate_async<P: FnOnce(Result<(), glib::Error>) + 'static>(
        &self,
        context: &HoverContext,
        display: &HoverDisplay,
        cancellable: Option<&impl IsA<gio::Cancellable>>,
        callback: P,
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
            let parent_iface = type_data.as_ref().parent_interface::<HoverProvider>()
                as *const ffi::GtkSourceHoverProviderInterface;
            let f = (*parent_iface)
                .populate_async
                .expect("no parent \"populate_async\" implementation");
            let finish = (*parent_iface)
                .populate_finish
                .expect("no parent \"populate_finish\" implementation");

            let user_data: Box<(glib::thread_guard::ThreadGuard<P>, _)> =
                Box::new((glib::thread_guard::ThreadGuard::new(callback), finish));

            unsafe extern "C" fn parent_populate_async_trampoline<
                P: FnOnce(Result<(), glib::Error>) + 'static,
            >(
                source_object_ptr: *mut glib::gobject_ffi::GObject,
                res: *mut gio::ffi::GAsyncResult,
                user_data: glib::ffi::gpointer,
            ) {
                unsafe {
                    let mut error = std::ptr::null_mut();
                    let cb: Box<(
                        glib::thread_guard::ThreadGuard<P>,
                        fn(
                            *mut ffi::GtkSourceHoverProvider,
                            *mut gio::ffi::GAsyncResult,
                            *mut *mut glib::ffi::GError,
                        ),
                    )> = Box::from_raw(user_data as *mut _);
                    cb.1(source_object_ptr as _, res, &mut error);
                    let result = if error.is_null() {
                        Ok(())
                    } else {
                        Err(from_glib_full(error))
                    };
                    let cb = cb.0.into_inner();
                    cb(result);
                }
            }

            let cancellable = cancellable.map(|p| p.as_ref());
            let callback = parent_populate_async_trampoline::<P>;
            f(
                self.obj()
                    .unsafe_cast_ref::<HoverProvider>()
                    .to_glib_none()
                    .0,
                context.to_glib_none().0,
                display.to_glib_none().0,
                cancellable.to_glib_none().0,
                Some(callback),
                Box::into_raw(user_data) as *mut _,
            );
        }
    }

    fn parent_populate_future(
        &self,
        context: &HoverContext,
        display: &HoverDisplay,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), glib::Error>> + 'static>> {
        let context = context.clone();
        let display = display.clone();
        Box::pin(gio::GioFuture::new(
            &self.ref_counted(),
            move |imp, cancellable, send| {
                imp.parent_populate_async(&context, &display, Some(cancellable), move |res| {
                    send.resolve(res);
                });
            },
        ))
    }
}

impl<T: HoverProviderImpl> HoverProviderImplExt for T {}

unsafe impl<T: HoverProviderImpl> IsImplementable<T> for HoverProvider {
    fn interface_init(iface: &mut glib::Interface<Self>) {
        let iface = iface.as_mut();

        iface.populate_async = Some(hover_provider_populate_async::<T>);
        iface.populate_finish = Some(hover_provider_populate_finish::<T>);
    }
}

unsafe extern "C" fn hover_provider_populate_async<T: HoverProviderImpl>(
    provider: *mut ffi::GtkSourceHoverProvider,
    context: *mut ffi::GtkSourceHoverContext,
    display: *mut ffi::GtkSourceHoverDisplay,
    cancellable_ptr: *mut gio::ffi::GCancellable,
    callback: gio::ffi::GAsyncReadyCallback,
    user_data: glib::ffi::gpointer,
) {
    unsafe {
        let instance = &*(provider as *mut T::Instance);
        let imp = instance.imp();
        let wrap: HoverProvider = from_glib_none(provider);

        let context: HoverContext = from_glib_none(context);
        let display: HoverDisplay = from_glib_none(display);
        let cancellable: Option<gio::Cancellable> = from_glib_none(cancellable_ptr);

        let closure = move |result: gio::LocalTask<bool>, source_object: Option<&HoverProvider>| {
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
            let res = imp.populate_future(&context, &display).await;
            t.return_result(res.map(|_| true));
        });
    }
}

unsafe extern "C" fn hover_provider_populate_finish<T: HoverProviderImpl>(
    _provider: *mut ffi::GtkSourceHoverProvider,
    res_ptr: *mut gio::ffi::GAsyncResult,
    error_ptr: *mut *mut glib::ffi::GError,
) -> glib::ffi::gboolean {
    unsafe {
        let res: gio::AsyncResult = from_glib_none(res_ptr);
        let t = res.downcast::<gio::LocalTask<bool>>().unwrap();
        let ret = t.propagate();
        match ret {
            Ok(v) => v.into_glib(),
            Err(e) => {
                *error_ptr = e.into_glib_ptr();
                false.into_glib()
            }
        }
    }
}
