// Take a look at the license at the top of the repository in the LICENSE file.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::needless_doctest_main)]
#![doc(
    html_logo_url = "https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/logo.svg",
    html_favicon_url = "https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/logo.svg"
)]

//! # GtkSourceView 5 Rust bindings
//!
//! This library contains safe Rust bindings for [GtkSourceView](https://gitlab.gnome.org/GNOME/gtksourceview).
//!
//! See also
//!
//! - [GTK 4 Rust bindings documentation](mod@gtk)
//! - [The C API documentation](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/)
//! - [gtk-rs project overview](https://gtk-rs.org/)

// Re-export -sys
pub use ffi;
#[doc(hidden)]
pub use gdk_pixbuf;
#[doc(hidden)]
pub use gio;
#[doc(hidden)]
pub use glib;
#[doc(hidden)]
pub use gtk;

macro_rules! assert_initialized_main_thread {
    () => {
        if !::gtk::is_initialized_main_thread() {
            if ::gtk::is_initialized() {
                panic!("GtkSourceView may only be used from the main thread.");
            } else {
                panic!("Gtk has to be initialized before using GtkSourceView.");
            }
        }
    };
}

macro_rules! skip_assert_initialized {
    () => {};
}

#[allow(clippy::clone_on_copy)]
#[allow(clippy::let_and_return)]
#[allow(clippy::type_complexity)]
#[allow(unused_doc_comments)]
#[allow(unused_imports)]
mod auto;
pub use auto::functions::*;
mod file_loader;
mod file_saver;
mod hover_context;
pub use auto::*;
pub mod subclass;

mod region_iter;
mod search_context;
mod view;

pub mod prelude;
pub use region_iter::RegionIter;
