use std::{cell::RefCell, pin::Pin, ptr};

use glib::translate::{IntoGlib, ToGlibPtr, from_glib_full};

use crate::{FileSaver, prelude::*};

impl FileSaver {
    #[doc(alias = "gtk_source_file_saver_save_async")]
    pub fn save_async<Q: FnOnce(Result<(), glib::Error>) + Send + 'static>(
        &self,
        io_priority: glib::Priority,
        cancellable: Option<&impl IsA<gio::Cancellable>>,
        callback: Q,
    ) {
        self.save_async_impl(io_priority, cancellable, None, callback)
    }

    #[doc(alias = "gtk_source_file_saver_save_async")]
    pub fn save_async_with_callback<
        P: FnMut(i64, i64) + Send + 'static,
        Q: FnOnce(Result<(), glib::Error>) + Send + 'static,
    >(
        &self,
        io_priority: glib::Priority,
        cancellable: Option<&impl IsA<gio::Cancellable>>,
        progress_callback: P,
        callback: Q,
    ) {
        self.save_async_impl(
            io_priority,
            cancellable,
            Some(Box::new(progress_callback)),
            callback,
        )
    }

    fn save_async_impl<Q: FnOnce(Result<(), glib::Error>) + Send + 'static>(
        &self,
        io_priority: glib::Priority,
        cancellable: Option<&impl IsA<gio::Cancellable>>,
        progress_callback: Option<Box<dyn FnMut(i64, i64) + Send>>,
        callback: Q,
    ) {
        let progress_trampoline = if progress_callback.is_some() {
            Some(save_async_progress_trampoline::<Q> as _)
        } else {
            None
        };

        let user_data: Box<(Q, RefCell<Option<Box<dyn FnMut(i64, i64) + Send>>>)> = Box::new((
            callback,
            RefCell::new(
                progress_callback.map(|p| -> Box<dyn FnMut(i64, i64) + Send> { Box::new(p) }),
            ),
        ));
        unsafe extern "C" fn save_async_trampoline<
            Q: FnOnce(Result<(), glib::Error>) + Send + 'static,
        >(
            _source_object: *mut glib::gobject_ffi::GObject,
            res: *mut gio::ffi::GAsyncResult,
            user_data: glib::ffi::gpointer,
        ) {
            unsafe {
                let mut error = ptr::null_mut();
                ffi::gtk_source_file_saver_save_finish(_source_object as *mut _, res, &mut error);
                let result = if error.is_null() {
                    Ok(())
                } else {
                    Err(from_glib_full(error))
                };
                let callback: Box<(Q, RefCell<Option<Box<dyn FnMut(i64, i64) + Send>>>)> =
                    Box::from_raw(user_data as *mut _);
                callback.0(result);
            }
        }
        unsafe extern "C" fn save_async_progress_trampoline<
            Q: FnOnce(Result<(), glib::Error>) + Send + 'static,
        >(
            current_num_bytes: i64,
            total_num_bytes: i64,
            user_data: glib::ffi::gpointer,
        ) {
            unsafe {
                let callback: &(Q, RefCell<Option<Box<dyn FnMut(i64, i64) + Send>>>) =
                    &*(user_data as *const _);
                (callback.1.borrow_mut().as_mut().expect("no closure"))(
                    current_num_bytes,
                    total_num_bytes,
                );
            }
        }

        let user_data = Box::into_raw(user_data) as *mut _;

        unsafe {
            ffi::gtk_source_file_saver_save_async(
                self.to_glib_none().0,
                io_priority.into_glib(),
                cancellable.map(|p| p.as_ref()).to_glib_none().0,
                progress_trampoline,
                user_data,
                None,
                Some(save_async_trampoline::<Q>),
                user_data,
            );
        }
    }

    pub fn save_future(
        &self,
        io_priority: glib::Priority,
    ) -> (
        Pin<Box<dyn std::future::Future<Output = Result<(), glib::Error>> + 'static>>,
        Pin<Box<dyn futures_core::stream::Stream<Item = (i64, i64)> + 'static>>,
    ) {
        let (sender, receiver) = futures_channel::mpsc::unbounded();

        let fut = Box::pin(gtk::gio::GioFuture::new(
            self,
            move |obj, cancellable, send| {
                obj.save_async_with_callback(
                    io_priority,
                    Some(cancellable),
                    move |current_num_bytes, total_num_bytes| {
                        let _ = sender.unbounded_send((current_num_bytes, total_num_bytes));
                    },
                    move |res| {
                        send.resolve(res);
                    },
                );
            },
        ));

        (fut, Box::pin(receiver))
    }
}
