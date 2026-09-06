use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;

use super::{BannerActionKind, EditorTab, EditorTabState, VisibleBannerState, Writability};
use crate::editor_monitor::PendingExternalState;

impl EditorTab {
    #[must_use]
    pub fn should_present_dirty_reload_prompt(&self) -> bool {
        let state = self.state.borrow();
        self.is_dirty()
            && matches!(
                state.external.pending,
                PendingExternalState::ContentPossiblyChanged {
                    acknowledged: false
                }
            )
            && !state.ui.external_prompt_active
            && !state.io.loading
            && !state.external.reload_deferred_by_edit
            && state.compare.active.is_none()
    }

    #[must_use]
    pub fn banner_action_kind(&self) -> Option<BannerActionKind> {
        match self.state.borrow().ui.visible_banner {
            VisibleBannerState::External => Some(BannerActionKind::Reload),
            VisibleBannerState::Missing | VisibleBannerState::AutosavePaused => {
                Some(BannerActionKind::Save)
            }
            VisibleBannerState::ReadOnly => Some(BannerActionKind::SaveAs),
            VisibleBannerState::ReviewStale => Some(BannerActionKind::RefreshReview),
            VisibleBannerState::None => None,
        }
    }

    pub fn pause_autosave(&self, message: String) {
        {
            let mut state = self.state.borrow_mut();
            state.autosave.pause(message);
        }
        self.sync_external_banner(true, true);
        self.notify_external_state_change();
    }

    pub fn clear_autosave_pause(&self) {
        let changed = {
            let mut state = self.state.borrow_mut();
            state.autosave.clear_pause()
        };
        if changed {
            self.sync_external_banner(true, true);
            self.notify_external_state_change();
        }
    }

    pub fn mark_external_prompt_active(&self, active: bool) {
        self.state.borrow_mut().ui.external_prompt_active = active;
    }

    pub fn acknowledge_pending_external(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.external.pending.acknowledge();
            state.external.reload_deferred_by_edit = false;
            state.ui.external_prompt_active = false;
        }
        self.sync_external_banner(true, true);
        self.notify_external_state_change();
    }

    pub fn resolve_pending_external(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.external.pending = PendingExternalState::Idle;
            state.external.reload_deferred_by_edit = false;
            state.ui.external_prompt_active = false;
            state.ui.visible_banner = VisibleBannerState::None;
            state.io.external_reload_in_progress = false;
        }
        self.set_attention(false);
        self.set_banner_revealed(false);
        self.notify_external_state_change();
    }

    pub fn sync_external_banner(&self, is_selected: bool, window_active: bool) {
        let (visible, title, action) = {
            let state = self.state.borrow();
            let is_dirty = state.is_dirty(self.text_buffer.is_modified());
            let should_offer_reload = is_selected
                && window_active
                && (!is_dirty || state.external.reload_deferred_by_edit);
            visible_banner_state(
                &state,
                should_offer_reload,
                is_selected,
                self.settings.autosave_enabled(),
            )
        };

        if let Some(title) = title {
            self.state.borrow_mut().ui.visible_banner = visible;
            self.banner.set_title(&title);
            self.banner.set_button_label(action.as_deref());
            self.set_banner_revealed(true);
        } else {
            self.state.borrow_mut().ui.visible_banner = VisibleBannerState::None;
            self.banner.set_button_label(None);
            self.set_banner_revealed(false);
        }
    }

    #[cfg(test)]
    pub(crate) fn banner_visible_for_tests(&self) -> bool {
        self.banner.is_revealed()
    }

    #[cfg(test)]
    pub(crate) fn sync_banner_for_tests(&self, is_selected: bool, window_active: bool) {
        self.sync_external_banner(is_selected, window_active);
    }

    #[cfg(test)]
    pub(crate) fn trigger_external_action_for_tests(&self) {
        let callback = self.on_external_action.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    #[cfg(test)]
    pub(crate) fn force_external_banner_for_tests(&self) {
        self.state.borrow_mut().ui.visible_banner = VisibleBannerState::External;
    }
}

fn visible_banner_state(
    state: &EditorTabState,
    should_offer_reload: bool,
    is_selected: bool,
    autosave_enabled: bool,
) -> (VisibleBannerState, Option<String>, Option<String>) {
    if state.compare.active.is_some() {
        return (VisibleBannerState::None, None, None);
    }

    if state
        .review
        .session
        .as_ref()
        .is_some_and(|session| session.borrow().is_stale())
    {
        return (
            VisibleBannerState::ReviewStale,
            Some(gettext("This review is out of date.")),
            Some(gettext("Refresh Review")),
        );
    }

    match &state.external.pending {
        PendingExternalState::ContentPossiblyChanged {
            acknowledged: false,
        } if should_offer_reload => {
            return (
                VisibleBannerState::External,
                Some(pgettext("external banner", "This File Changed on Disk.")),
                Some(pgettext("external action", "Reload")),
            );
        }
        PendingExternalState::Missing {
            acknowledged: false,
        } if is_selected => {
            return (
                VisibleBannerState::Missing,
                Some(pgettext("external banner", "This File Is Missing on Disk.")),
                Some(pgettext("external action", "Save")),
            );
        }
        PendingExternalState::Idle
        | PendingExternalState::Moved { .. }
        | PendingExternalState::ContentPossiblyChanged { .. }
        | PendingExternalState::Missing { .. } => {}
    }

    if state.external.writability == Writability::Unwritable
        && state.document.document.path().is_some()
    {
        return (
            VisibleBannerState::ReadOnly,
            Some(pgettext("save safety banner", "This File Is Read-Only.")),
            Some(pgettext("save safety action", "Save As")),
        );
    }

    if autosave_enabled && let Some(message) = &state.autosave.paused_message {
        return (
            VisibleBannerState::AutosavePaused,
            Some(message.clone()),
            Some(pgettext("save safety action", "Save")),
        );
    }

    (VisibleBannerState::None, None, None)
}
