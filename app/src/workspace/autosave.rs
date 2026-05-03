#[cfg(test)]
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{glib, prelude::*};

use crate::editor_tab::{EditorTab, SaveKind, SaveResult};
use crate::workspace::Workspace;

#[cfg(not(test))]
const AUTOSAVE_DELAY: Duration = Duration::from_secs(3);
#[cfg(test)]
const AUTOSAVE_DELAY: Duration = Duration::from_millis(20);

#[cfg(test)]
pub(crate) struct AutosaveRequestForTests {
    pub(crate) requested: bool,
    pub(crate) result: Rc<RefCell<Option<SaveResult>>>,
}

pub(crate) fn install_tab_autosave(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let buffer = tab.text_buffer();
    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    let _handler = buffer.connect_changed(move |_| {
        let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) else {
            return;
        };
        tab.clear_autosave_pause();
        schedule_autosave(&workspace, &tab);
    });
}

fn schedule_autosave(workspace: &Rc<Workspace>, tab: &Rc<EditorTab>) {
    let generation = tab.next_autosave_generation();
    let weak_workspace = Rc::downgrade(workspace);
    let weak_tab = Rc::downgrade(tab);
    let _source = glib::timeout_add_local_once(AUTOSAVE_DELAY, move || {
        let (Some(workspace), Some(tab)) = (weak_workspace.upgrade(), weak_tab.upgrade()) else {
            return;
        };
        let _requested =
            request_autosave_if_eligible(&workspace, &tab, generation, Rc::new(|_result| {}));
    });
}

#[cfg(test)]
pub(crate) fn request_tab_autosave_for_tests(
    workspace: &Rc<Workspace>,
    tab: &Rc<EditorTab>,
) -> AutosaveRequestForTests {
    let result = Rc::new(RefCell::new(None));
    let result_for_callback = Rc::clone(&result);
    let generation = tab.next_autosave_generation();
    let requested = request_autosave_if_eligible(
        workspace,
        tab,
        generation,
        Rc::new(move |save_result| {
            *result_for_callback.borrow_mut() = Some(save_result);
        }),
    );
    AutosaveRequestForTests { requested, result }
}

fn request_autosave_if_eligible(
    workspace: &Rc<Workspace>,
    tab: &Rc<EditorTab>,
    generation: u64,
    callback: Rc<dyn Fn(SaveResult)>,
) -> bool {
    if tab.autosave_generation() != generation
        || !workspace_contains_tab(workspace, tab)
        || !tab.is_autosave_eligible()
    {
        return false;
    }
    workspace.request_save_tab_kind(tab, false, SaveKind::Autosave, callback);
    true
}

fn workspace_contains_tab(workspace: &Workspace, tab: &Rc<EditorTab>) -> bool {
    workspace
        .state
        .borrow()
        .tabs
        .iter()
        .any(|item| Rc::ptr_eq(item, tab))
}
