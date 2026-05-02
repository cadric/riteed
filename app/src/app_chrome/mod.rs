use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk4::{gdk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::settings::{AppSettings, SettingsSubscription};

mod css;

pub(crate) use css::chrome_css_for_settings;

#[cfg(test)]
pub(crate) use css::{chrome_css, exercise_chrome_css_for_tests};

#[derive(Clone)]
pub(crate) struct AppChromeController {
    state: Rc<ChromeState>,
}

struct ChromeState {
    settings: AppSettings,
    display: gdk::Display,
    provider: gtk4::CssProvider,
    observers: RefCell<Vec<Weak<ObserverInner>>>,
    subscriptions: RefCell<Vec<Subscription>>,
}

pub(crate) struct ChromeObserver {
    _inner: Rc<ObserverInner>,
}

struct ObserverInner {
    callback: Box<dyn Fn()>,
}

enum Subscription {
    Settings(SettingsSubscription),
    Style(StyleSubscription),
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self {
            Self::Settings(_subscription) => {}
            Self::Style(_subscription) => {}
        }
    }
}

struct StyleSubscription {
    manager: adw::StyleManager,
    handler: Option<glib::SignalHandlerId>,
}

impl AppChromeController {
    #[must_use]
    pub(crate) fn install(display: &gdk::Display, settings: &AppSettings) -> Self {
        let provider = gtk4::CssProvider::new();
        gtk4::style_context_add_provider_for_display(
            display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let state = Rc::new(ChromeState {
            settings: settings.clone(),
            display: display.clone(),
            provider,
            observers: RefCell::new(Vec::new()),
            subscriptions: RefCell::new(Vec::new()),
        });
        state.refresh();
        install_callbacks(&state);
        Self { state }
    }

    pub(crate) fn refresh(&self) {
        self.state.refresh();
    }

    pub(crate) fn add_observer(&self, callback: impl Fn() + 'static) -> ChromeObserver {
        let inner = Rc::new(ObserverInner {
            callback: Box::new(callback),
        });
        let mut observers = self.state.observers.borrow_mut();
        observers.retain(|observer| observer.strong_count() > 0);
        observers.push(Rc::downgrade(&inner));
        ChromeObserver { _inner: inner }
    }
}

impl ChromeState {
    fn refresh(&self) {
        let stylesheet = chrome_css_for_settings(&self.settings);
        self.provider.load_from_data(&stylesheet);
        for observer in self.live_observers() {
            (observer.callback)();
        }
    }

    fn live_observers(&self) -> Vec<Rc<ObserverInner>> {
        let mut observers = self.observers.borrow_mut();
        observers.retain(|observer| observer.strong_count() > 0);
        observers.iter().filter_map(Weak::upgrade).collect()
    }
}

impl Drop for ChromeState {
    fn drop(&mut self) {
        gtk4::style_context_remove_provider_for_display(&self.display, &self.provider);
    }
}

impl StyleSubscription {
    fn new(manager: &adw::StyleManager, handler: glib::SignalHandlerId) -> Self {
        Self {
            manager: manager.clone(),
            handler: Some(handler),
        }
    }
}

impl Drop for StyleSubscription {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            self.manager.disconnect(handler);
        }
    }
}

fn install_callbacks(state: &Rc<ChromeState>) {
    let style_manager = adw::StyleManager::default();
    let weak = Rc::downgrade(state);
    let dark_handler = style_manager.connect_dark_notify(move |_| {
        if let Some(state) = weak.upgrade() {
            state.refresh();
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Style(StyleSubscription::new(
            &style_manager,
            dark_handler,
        )));

    let weak = Rc::downgrade(state);
    let high_contrast_handler = style_manager.connect_high_contrast_notify(move |_| {
        if let Some(state) = weak.upgrade() {
            state.refresh();
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Style(StyleSubscription::new(
            &style_manager,
            high_contrast_handler,
        )));

    let weak = Rc::downgrade(state);
    let window_palette_subscription = state.settings.connect_window_palette_changed(move || {
        if let Some(state) = weak.upgrade() {
            state.refresh();
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Settings(window_palette_subscription));

    let weak = Rc::downgrade(state);
    let editor_palette_subscription = state.settings.connect_editor_palette_changed(move || {
        if let Some(state) = weak.upgrade() {
            state.refresh();
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Settings(editor_palette_subscription));
}
