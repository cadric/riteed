use std::cell::Cell;
use std::rc::Rc;

use crate::settings::{AppSettings, LargeFileLimitValues};
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

#[derive(Clone)]
struct LargeFileRows {
    full_feature: libadwaita::SpinRow,
    editor: libadwaita::SpinRow,
    strong_warning: libadwaita::SpinRow,
    viewer_only: libadwaita::SpinRow,
    always_edit: libadwaita::SwitchRow,
}

pub(crate) fn install(shell: &WindowShell, settings: &AppSettings, workspace: &Rc<Workspace>) {
    let rows = LargeFileRows::from_shell(shell);
    let syncing = Rc::new(Cell::new(false));
    sync_rows(&rows, settings, &syncing);
    let workspace = Rc::downgrade(workspace);
    install_spin_handler(
        &rows.full_feature,
        rows.clone(),
        settings,
        &syncing,
        workspace.clone(),
    );
    install_spin_handler(
        &rows.editor,
        rows.clone(),
        settings,
        &syncing,
        workspace.clone(),
    );
    install_spin_handler(
        &rows.strong_warning,
        rows.clone(),
        settings,
        &syncing,
        workspace.clone(),
    );
    install_spin_handler(
        &rows.viewer_only,
        rows.clone(),
        settings,
        &syncing,
        workspace,
    );

    let settings = settings.clone();
    let switch_syncing = Rc::clone(&syncing);
    rows.always_edit.connect_active_notify(move |row| {
        if switch_syncing.get() {
            return;
        }
        settings.set_always_allow_large_file_edit(row.is_active());
    });
}

impl LargeFileRows {
    fn from_shell(shell: &WindowShell) -> Self {
        Self {
            full_feature: shell.large_file_full_feature_limit_row.clone(),
            editor: shell.large_file_editor_limit_row.clone(),
            strong_warning: shell.large_file_strong_warning_limit_row.clone(),
            viewer_only: shell.large_file_viewer_only_limit_row.clone(),
            always_edit: shell.large_file_always_edit_row.clone(),
        }
    }

    fn values(&self) -> LargeFileLimitValues {
        LargeFileLimitValues {
            full_feature: rounded_spin_value(self.full_feature.value()),
            editor: rounded_spin_value(self.editor.value()),
            strong_warning: rounded_spin_value(self.strong_warning.value()),
            viewer_only: rounded_spin_value(self.viewer_only.value()),
        }
    }
}

fn install_spin_handler(
    row: &libadwaita::SpinRow,
    rows: LargeFileRows,
    settings: &AppSettings,
    syncing: &Rc<Cell<bool>>,
    workspace: std::rc::Weak<Workspace>,
) {
    let settings = settings.clone();
    let syncing = Rc::clone(syncing);
    row.connect_value_notify(move |_| {
        if syncing.get() {
            return;
        }
        settings.set_large_file_limit_values(rows.values());
        sync_rows(&rows, &settings, &syncing);
        if let Some(workspace) = workspace.upgrade() {
            workspace.reapply_large_file_feature_gates_to_tabs();
        }
    });
}

fn sync_rows(rows: &LargeFileRows, settings: &AppSettings, syncing: &Rc<Cell<bool>>) {
    let limits = settings.large_file_limit_values();
    syncing.set(true);
    rows.full_feature.set_value(f64::from(limits.full_feature));
    rows.editor.set_value(f64::from(limits.editor));
    rows.strong_warning
        .set_value(f64::from(limits.strong_warning));
    rows.viewer_only.set_value(f64::from(limits.viewer_only));
    rows.always_edit
        .set_active(settings.always_allow_large_file_edit());
    syncing.set(false);
}

fn rounded_spin_value(value: f64) -> i32 {
    let text = format!(
        "{:.0}",
        value
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
    );
    match text.parse::<i32>() {
        Ok(value) => value,
        Err(_) if value.is_sign_negative() => i32::MIN,
        Err(_) => i32::MAX,
    }
}
