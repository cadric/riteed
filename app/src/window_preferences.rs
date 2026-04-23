use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, pango, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::editor_format::LineEndingMode;
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{
    EditorZoomController, font_row_subtitle, resolve_editor_font_description, resolve_font_family,
    resolve_font_family_in_map,
};
use crate::settings::{AppSettings, ThemePreference};
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

pub struct WindowPreferencesController {
    _state: Rc<RefCell<PreferencesState>>,
}

#[derive(Default)]
struct PreferencesState {
    syncing: bool,
    tab_width_staged: bool,
    indent_width_staged: bool,
}

const MIN_EDITOR_WIDTH: f64 = 1.0;
const MAX_EDITOR_WIDTH: f64 = 16.0;
const EDITOR_WIDTH_STEP: f64 = 1.0;
const EDITOR_WIDTH_PAGE: f64 = 4.0;

impl WindowPreferencesController {
    #[must_use]
    pub fn new(
        shell: &WindowShell,
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
        zoom: &Rc<EditorZoomController>,
    ) -> Self {
        let state = Rc::new(RefCell::new(PreferencesState::default()));
        initialize_rows(shell, settings, &state);
        install_theme_preferences(shell, settings, &state);
        install_toggle_preferences(shell, settings, workspace, &state);
        install_spin_preferences(shell, settings, workspace, &state);
        install_document_format_preferences(shell, workspace, &state);
        install_font_preference(shell, settings, zoom);
        Self { _state: state }
    }
}

fn initialize_rows(
    shell: &WindowShell,
    settings: &AppSettings,
    state: &Rc<RefCell<PreferencesState>>,
) {
    let themes = gtk4::StringList::new(&[
        &pgettext("theme choice", "System Default"),
        &pgettext("theme choice", "Light"),
        &pgettext("theme choice", "Dark"),
    ]);
    let lf = LineEndingMode::Lf.menu_label();
    let crlf = LineEndingMode::CrLf.menu_label();
    let cr = LineEndingMode::Cr.menu_label();
    let line_endings = gtk4::StringList::new(&[lf.as_str(), crlf.as_str(), cr.as_str()]);
    with_syncing(state, || {
        shell.theme_row.set_model(Some(&themes));
        shell.theme_row.set_selected(settings.theme().index());
        shell.line_ending_row.set_model(Some(&line_endings));
        shell.line_ending_row.set_selected(0);
        shell.word_wrap_row.set_active(settings.word_wrap());
        shell
            .line_numbers_row
            .set_active(settings.show_line_numbers());
        shell.minimap_row.set_active(settings.show_minimap());
        shell
            .insert_spaces_row
            .set_active(settings.insert_spaces_instead_of_tabs());
        configure_spin_row(&shell.tab_width_row, settings.tab_width().cast_signed());
        configure_spin_row(&shell.indent_width_row, settings.indent_width());
        shell
            .editor_font_row
            .set_subtitle(&font_row_subtitle(&settings.editor_font()));
    });
}

fn configure_spin_row(row: &adw::SpinRow, value: i32) {
    row.adjustment().configure(
        f64::from(value),
        MIN_EDITOR_WIDTH,
        MAX_EDITOR_WIDTH,
        EDITOR_WIDTH_STEP,
        EDITOR_WIDTH_PAGE,
        0.0,
    );
    row.set_editable(true);
    row.set_numeric(true);
    row.set_snap_to_ticks(true);
    row.set_digits(0);
    row.set_value(f64::from(value));
}

fn install_theme_preferences(
    shell: &WindowShell,
    settings: &AppSettings,
    state: &Rc<RefCell<PreferencesState>>,
) {
    let state = Rc::clone(state);
    let settings = settings.clone();
    shell.theme_row.connect_selected_notify(move |row| {
        if state.borrow().syncing {
            return;
        }
        let theme = ThemePreference::from_index(row.selected());
        settings.set_theme(theme);
        settings.apply_theme();
    });
}

fn install_toggle_preferences(
    shell: &WindowShell,
    settings: &AppSettings,
    workspace: &Rc<Workspace>,
    state: &Rc<RefCell<PreferencesState>>,
) {
    install_switch_handler(
        &shell.word_wrap_row,
        settings,
        workspace,
        state,
        AppSettings::set_word_wrap,
        Workspace::apply_word_wrap_to_tabs,
    );
    install_switch_handler(
        &shell.line_numbers_row,
        settings,
        workspace,
        state,
        AppSettings::set_show_line_numbers,
        Workspace::apply_line_numbers_to_tabs,
    );
    install_switch_handler(
        &shell.minimap_row,
        settings,
        workspace,
        state,
        AppSettings::set_show_minimap,
        Workspace::apply_minimap_to_tabs,
    );
    install_switch_handler(
        &shell.insert_spaces_row,
        settings,
        workspace,
        state,
        AppSettings::set_insert_spaces_instead_of_tabs,
        Workspace::apply_indentation_to_tabs,
    );
}

fn install_switch_handler(
    row: &adw::SwitchRow,
    settings: &AppSettings,
    workspace: &Rc<Workspace>,
    state: &Rc<RefCell<PreferencesState>>,
    write: impl Fn(&AppSettings, bool) + 'static,
    apply: impl Fn(&Workspace) + 'static,
) {
    let row_state = Rc::clone(state);
    let settings = settings.clone();
    let workspace = Rc::downgrade(workspace);
    row.connect_active_notify(move |row| {
        if row_state.borrow().syncing {
            return;
        }
        write(&settings, row.is_active());
        if let Some(workspace) = workspace.upgrade() {
            apply(&workspace);
        }
    });
}

fn install_spin_preferences(
    shell: &WindowShell,
    settings: &AppSettings,
    workspace: &Rc<Workspace>,
    state: &Rc<RefCell<PreferencesState>>,
) {
    let tab_commit: Rc<dyn Fn(i32) -> i32> = Rc::new({
        let settings = settings.clone();
        let workspace = Rc::downgrade(workspace);
        move |value| {
            settings.set_tab_width(value);
            if let Some(workspace) = workspace.upgrade() {
                workspace.apply_indentation_to_tabs();
            }
            settings.tab_width().cast_signed()
        }
    });
    install_spin_row_handler(
        &shell.tab_width_row,
        state,
        DirtySpin::TabWidth,
        &tab_commit,
    );
    let indent_commit: Rc<dyn Fn(i32) -> i32> = Rc::new({
        let settings = settings.clone();
        let workspace = Rc::downgrade(workspace);
        move |value| {
            settings.set_indent_width(value);
            if let Some(workspace) = workspace.upgrade() {
                workspace.apply_indentation_to_tabs();
            }
            settings.indent_width()
        }
    });
    install_spin_row_handler(
        &shell.indent_width_row,
        state,
        DirtySpin::IndentWidth,
        &indent_commit,
    );
}

fn install_spin_row_handler(
    row: &adw::SpinRow,
    state: &Rc<RefCell<PreferencesState>>,
    dirty_spin: DirtySpin,
    commit: &Rc<dyn Fn(i32) -> i32>,
) {
    let row_state = Rc::clone(state);
    row.connect_changed(move |_| {
        if row_state.borrow().syncing {
            return;
        }
        set_spin_dirty(&row_state, dirty_spin, true);
    });

    let row_clone = row.clone();
    let value_state = Rc::clone(state);
    let value_commit = Rc::clone(commit);
    row.connect_value_notify(move |row| {
        if value_state.borrow().syncing {
            return;
        }
        set_spin_dirty(&value_state, dirty_spin, true);
        commit_spin_row(&row_clone, &value_state, dirty_spin, &value_commit);
        row.queue_draw();
    });

    if let Some(delegate) = row
        .delegate()
        .and_then(|item| item.downcast::<gtk4::Text>().ok())
    {
        let row_clone = row.clone();
        let activate_state = Rc::clone(state);
        let activate_commit = Rc::clone(commit);
        delegate.connect_activate(move |_| {
            commit_spin_row(&row_clone, &activate_state, dirty_spin, &activate_commit);
        });

        let row_clone = row.clone();
        let focus_state = Rc::clone(state);
        let focus_commit = Rc::clone(commit);
        delegate.connect_has_focus_notify(move |delegate| {
            if !delegate.has_focus() {
                commit_spin_row(&row_clone, &focus_state, dirty_spin, &focus_commit);
            }
        });
    }

    let row_clone = row.clone();
    let focus_state = Rc::clone(state);
    let focus_commit = Rc::clone(commit);
    row.connect_has_focus_notify(move |row| {
        if !row.has_focus() {
            commit_spin_row(&row_clone, &focus_state, dirty_spin, &focus_commit);
        }
    });
}

fn commit_spin_row(
    row: &adw::SpinRow,
    state: &Rc<RefCell<PreferencesState>>,
    dirty_spin: DirtySpin,
    commit: &Rc<dyn Fn(i32) -> i32>,
) {
    if state.borrow().syncing || !spin_dirty(state, dirty_spin) {
        return;
    }
    row.update();
    let value = row.text().parse::<i32>().unwrap_or_default();
    let applied = commit(value);
    with_syncing(state, || {
        row.set_value(f64::from(applied));
    });
    set_spin_dirty(state, dirty_spin, false);
}

fn install_document_format_preferences(
    shell: &WindowShell,
    workspace: &Rc<Workspace>,
    state: &Rc<RefCell<PreferencesState>>,
) {
    let weak = Rc::downgrade(workspace);
    shell.encoding_row.connect_activated(move |_| {
        if let Some(workspace) = weak.upgrade() {
            workspace.request_selected_encoding_action();
        }
    });

    let weak = Rc::downgrade(workspace);
    let line_state = Rc::clone(state);
    shell.line_ending_row.connect_selected_notify(move |row| {
        if line_state.borrow().syncing {
            return;
        }
        if let Some(workspace) = weak.upgrade()
            && let Some(mode) = line_ending_mode_from_index(row.selected())
        {
            workspace.set_selected_line_ending_mode(mode);
        }
    });

    let encoding_row = shell.encoding_row.clone();
    let line_ending_row = shell.line_ending_row.clone();
    let format_state = Rc::clone(state);
    workspace.set_format_preferences_handler(Rc::new(move |tab| {
        sync_document_format_rows(
            &encoding_row,
            &line_ending_row,
            &format_state,
            tab.as_deref(),
        );
    }));
    sync_document_format_rows(
        &shell.encoding_row,
        &shell.line_ending_row,
        state,
        workspace.selected_tab().as_deref(),
    );
}

fn sync_document_format_rows(
    encoding_row: &adw::ActionRow,
    line_ending_row: &adw::ComboRow,
    state: &Rc<RefCell<PreferencesState>>,
    tab: Option<&EditorTab>,
) {
    with_syncing(state, || {
        if let Some(tab) = tab {
            let format = tab.current_format();
            encoding_row.set_sensitive(tab.uri().is_none() || tab.can_reopen_with_encoding());
            encoding_row.set_subtitle(format.encoding().charset());
            line_ending_row.set_sensitive(true);
            line_ending_row.set_selected(line_ending_index(format.line_ending_mode()));
        } else {
            encoding_row.set_sensitive(false);
            encoding_row.set_subtitle(&pgettext("document format", "No Document"));
            line_ending_row.set_sensitive(false);
            line_ending_row.set_selected(line_ending_index(LineEndingMode::Lf));
        }
    });
}

fn line_ending_index(mode: LineEndingMode) -> u32 {
    match mode {
        LineEndingMode::Lf => 0,
        LineEndingMode::CrLf => 1,
        LineEndingMode::Cr => 2,
    }
}

fn line_ending_mode_from_index(index: u32) -> Option<LineEndingMode> {
    match index {
        0 => Some(LineEndingMode::Lf),
        1 => Some(LineEndingMode::CrLf),
        2 => Some(LineEndingMode::Cr),
        _ => None,
    }
}

fn install_font_preference(
    shell: &WindowShell,
    settings: &AppSettings,
    zoom: &Rc<EditorZoomController>,
) {
    let window = shell.window.downgrade();
    let dialog = shell.preferences_dialog.downgrade();
    let row = shell.editor_font_row.downgrade();
    let settings = settings.clone();
    let zoom = Rc::downgrade(zoom);
    shell.editor_font_row.connect_activated(move |_| {
        let Some(window) = window.upgrade() else {
            return;
        };
        let monospace_filter = monospace_font_filter();
        let dialog_font_map = window.font_map();
        let font_dialog = gtk4::FontDialog::new();
        font_dialog.set_filter(Some(&monospace_filter));
        if let Some(font_map) = dialog_font_map.as_ref() {
            font_dialog.set_font_map(Some(font_map));
        }
        font_dialog.set_modal(true);
        font_dialog.set_title(&gettext("Choose Editor Font"));
        let initial = resolve_editor_font_description(&settings.editor_font());
        let settings = settings.clone();
        let dialog = dialog.clone();
        let row = row.clone();
        let zoom = zoom.clone();
        let validation_window = window.clone();
        let validation_font_map = dialog_font_map.clone();
        font_dialog.choose_font(
            Some(&window),
            Some(&initial),
            None::<&gio::Cancellable>,
            move |result| {
                let Ok(description) = result else {
                    return;
                };
                let Some(dialog_ref) = dialog.upgrade() else {
                    return;
                };
                if !font_description_is_monospace(
                    validation_font_map.as_ref(),
                    &validation_window,
                    &description,
                ) {
                    dialog_ref.add_toast(adw::Toast::new(&gettext(
                        "Only monospace fonts are supported",
                    )));
                    return;
                }
                let stored = description.to_string();
                settings.set_editor_font(&stored);
                if let Some(row) = row.upgrade() {
                    row.set_subtitle(&font_row_subtitle(&stored));
                }
                if let Some(zoom) = zoom.upgrade() {
                    zoom.set_editor_font(&stored);
                }
            },
        );
    });
}

fn monospace_font_filter() -> gtk4::CustomFilter {
    gtk4::CustomFilter::new(|item| {
        item.downcast_ref::<pango::FontFamily>()
            .is_some_and(pango::FontFamily::is_monospace)
            || item
                .downcast_ref::<pango::FontFace>()
                .is_some_and(|face| face.family().is_monospace())
    })
}

fn font_description_is_monospace(
    font_map: Option<&pango::FontMap>,
    parent: &adw::ApplicationWindow,
    description: &pango::FontDescription,
) -> bool {
    font_map
        .and_then(|font_map| resolve_font_family_in_map(font_map, description))
        .or_else(|| resolve_font_family(parent, description))
        .is_none_or(|family| family.is_monospace())
}

#[cfg(test)]
pub(crate) fn font_description_is_monospace_for_tests(
    font_map: Option<&pango::FontMap>,
    parent: &adw::ApplicationWindow,
    description: &pango::FontDescription,
) -> bool {
    font_description_is_monospace(font_map, parent, description)
}

fn with_syncing(state: &Rc<RefCell<PreferencesState>>, operation: impl FnOnce()) {
    state.borrow_mut().syncing = true;
    operation();
    state.borrow_mut().syncing = false;
}

fn spin_dirty(state: &Rc<RefCell<PreferencesState>>, dirty_spin: DirtySpin) -> bool {
    let state = state.borrow();
    match dirty_spin {
        DirtySpin::TabWidth => state.tab_width_staged,
        DirtySpin::IndentWidth => state.indent_width_staged,
    }
}

fn set_spin_dirty(state: &Rc<RefCell<PreferencesState>>, dirty_spin: DirtySpin, value: bool) {
    let mut state = state.borrow_mut();
    match dirty_spin {
        DirtySpin::TabWidth => state.tab_width_staged = value,
        DirtySpin::IndentWidth => state.indent_width_staged = value,
    }
}

#[derive(Clone, Copy)]
enum DirtySpin {
    TabWidth,
    IndentWidth,
}
