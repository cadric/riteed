use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use gtk4::{gdk, glib, prelude::*};
use libadwaita as adw;

use crate::settings::{AppSettings, SettingsSubscription, WindowPalette};
use crate::window_appearance::WindowAppearanceController;
use crate::workspace::Workspace;

mod css;
mod scope;

#[cfg(test)]
pub(crate) use css::chrome_css;
pub(crate) use scope::{
    SIDEBAR_CONTENT_CLASS, SIDEBAR_HEADER_CLASS, SIDEBAR_STACK_CLASS, SIDEBAR_SWITCHER_CLASS,
    TAB_BAR_CLASS, TAB_VIEW_CLASS,
};

static NEXT_CHROME_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(crate) struct WindowChromeController {
    state: Rc<ChromeState>,
}

struct ChromeState {
    window: adw::ApplicationWindow,
    settings: AppSettings,
    workspace: Weak<Workspace>,
    appearance: WindowAppearanceController,
    menu_button: gtk4::MenuButton,
    display: gdk::Display,
    provider: gtk4::CssProvider,
    css_class: String,
    current_popover: RefCell<Option<gtk4::Popover>>,
    subscriptions: RefCell<Vec<Subscription>>,
}

enum Subscription {
    Settings(SettingsSubscription),
    Style(StyleSubscription),
    MenuButton(MenuButtonSubscription),
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self {
            Self::Settings(_subscription) => {}
            Self::Style(_subscription) => {}
            Self::MenuButton(_subscription) => {}
        }
    }
}

struct StyleSubscription {
    manager: adw::StyleManager,
    handler: Option<glib::SignalHandlerId>,
}

struct MenuButtonSubscription {
    menu_button: gtk4::MenuButton,
    handler: Option<glib::SignalHandlerId>,
}

impl WindowChromeController {
    #[must_use]
    pub(crate) fn new(
        window: &adw::ApplicationWindow,
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
        appearance: &WindowAppearanceController,
        menu_button: &gtk4::MenuButton,
    ) -> Self {
        let provider = gtk4::CssProvider::new();
        let display = gtk4::prelude::WidgetExt::display(window);
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let css_class = format!(
            "riteed-window-chrome-{}",
            NEXT_CHROME_ID.fetch_add(1, Ordering::Relaxed)
        );
        scope::add_class_once(window, &css_class);

        let state = Rc::new(ChromeState {
            window: window.clone(),
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            appearance: appearance.clone(),
            menu_button: menu_button.clone(),
            display,
            provider,
            css_class,
            current_popover: RefCell::new(None),
            subscriptions: RefCell::new(Vec::new()),
        });
        state.refresh();
        state.sync_popover_class();
        install_callbacks(&state);
        Self { state }
    }

    pub(crate) fn refresh(&self) {
        self.state.refresh();
    }

    #[cfg(test)]
    pub(crate) fn css_class_for_tests(&self) -> String {
        self.state.css_class.clone()
    }

    #[cfg(test)]
    pub(crate) fn css_for_tests(&self) -> String {
        css::chrome_css_for_settings(&self.state.css_class, &self.state.settings)
    }
}

impl ChromeState {
    fn refresh(&self) {
        let stylesheet = css::chrome_css_for_settings(&self.css_class, &self.settings);
        self.provider.load_from_data(&stylesheet);
        self.sync_popover_class();
    }

    fn refresh_editor_surfaces(&self) {
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
        self.appearance.sync();
    }

    fn sync_popover_class(&self) {
        let next = self.menu_button.popover();
        let mut current = self.current_popover.borrow_mut();
        if let Some(previous) = current.take()
            && next.as_ref().is_none_or(|popover| popover != &previous)
        {
            scope::remove_class(&previous, &self.css_class);
        }
        if let Some(popover) = next {
            scope::add_class_once(&popover, &self.css_class);
            *current = Some(popover);
        }
    }
}

impl Drop for ChromeState {
    fn drop(&mut self) {
        scope::remove_class(&self.window, &self.css_class);
        if let Some(popover) = self.current_popover.borrow_mut().take() {
            scope::remove_class(&popover, &self.css_class);
        }
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

impl MenuButtonSubscription {
    fn new(menu_button: &gtk4::MenuButton, handler: glib::SignalHandlerId) -> Self {
        Self {
            menu_button: menu_button.clone(),
            handler: Some(handler),
        }
    }
}

impl Drop for MenuButtonSubscription {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            self.menu_button.disconnect(handler);
        }
    }
}

fn install_callbacks(state: &Rc<ChromeState>) {
    let style_manager = adw::StyleManager::default();
    let weak = Rc::downgrade(state);
    let dark_handler = style_manager.connect_dark_notify(move |_| {
        if let Some(state) = weak.upgrade() {
            state.refresh();
            state.refresh_editor_surfaces();
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
    let popover_handler = state
        .menu_button
        .connect_notify_local(Some("popover"), move |_, _| {
            if let Some(state) = weak.upgrade() {
                state.sync_popover_class();
            }
        });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::MenuButton(MenuButtonSubscription::new(
            &state.menu_button,
            popover_handler,
        )));

    let weak = Rc::downgrade(state);
    let window_palette_subscription = state.settings.connect_window_palette_changed(move || {
        if let Some(state) = weak.upgrade() {
            state.refresh();
            state.appearance.sync();
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Settings(window_palette_subscription));

    let weak = Rc::downgrade(state);
    let editor_palette_subscription = state.settings.connect_editor_palette_changed(move || {
        if let Some(state) = weak.upgrade() {
            state.refresh_editor_surfaces();
            if state.settings.window_palette() == WindowPalette::FollowEditor {
                state.refresh();
            }
        }
    });
    state
        .subscriptions
        .borrow_mut()
        .push(Subscription::Settings(editor_palette_subscription));
}

#[cfg(test)]
pub(crate) fn css_is_scoped_for_tests(stylesheet: &str, css_class: &str) -> bool {
    css::css_is_scoped_for_tests(stylesheet, css_class)
}

#[cfg(test)]
pub(crate) fn exercise_chrome_css_for_tests() {
    css::exercise_chrome_css_for_tests();
}
