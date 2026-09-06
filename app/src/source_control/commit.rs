use std::rc::Rc;

use gettextrs::gettext;
use gtk4::prelude::*;

use crate::git_process::{GitIdentity, GitProcessError};

use super::slots::MutationTicket;
use super::{
    SourceStateRef, actions, git_error_is_cancelled, operation_bridge, set_commit_controls_enabled,
    set_status_label,
};

pub(super) fn run(state: &SourceStateRef) {
    let (process, repo, entry, settings) = {
        let state = state.borrow();
        if !actions::commit_is_eligible(&state) {
            return;
        }
        let (Some(process), Some(repo)) = (state.process.clone(), state.repo.clone()) else {
            return;
        };
        (
            process,
            repo,
            state.commit_entry.clone(),
            state.settings.clone(),
        )
    };
    let message = entry.text().to_string();
    if message.trim().is_empty() {
        set_status_label(state, &gettext("Enter a commit message first."));
        return;
    }
    if operation_bridge::native_index_lock_exists(state) {
        actions::show_locked_wait(state);
        return;
    }
    let Some(ticket) = operation_bridge::try_begin_mutation(state, &repo) else {
        return;
    };
    set_commit_controls_enabled(state, false);
    actions::fire_state_change_handler(state);
    if !actions::mutation_can_spawn(state, &ticket) {
        actions::finish_mutation(state, &ticket, None);
        return;
    }

    let settings_identity = settings.git_identity();
    let weak = Rc::downgrade(state);
    let cancellable = ticket.cancellable().clone();
    let process_for_commit = process.clone();
    process.read_git_identity(
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !operation_bridge::is_mutation_current(&state, &ticket) {
                actions::finish_mutation(&state, &ticket, None);
                return;
            }
            let configured = match result {
                Ok(identity) => identity,
                Err(error)
                    if ticket.cancellable().is_cancelled() || git_error_is_cancelled(&error) =>
                {
                    actions::finish_mutation(&state, &ticket, Some(&error));
                    return;
                }
                Err(_error) => None,
            };
            let identity = configured.or_else(|| preference_identity(&settings_identity));
            let Some(identity) = identity else {
                actions::finish_mutation_message(
                    &state,
                    &ticket,
                    &gettext(
                        "Set a Git identity in the Source Control preferences before committing.",
                    ),
                );
                return;
            };
            if !actions::mutation_can_spawn(&state, &ticket) {
                actions::finish_mutation(&state, &ticket, None);
                return;
            }
            let weak = Rc::downgrade(&state);
            let ticket_for_callback = ticket.clone();
            let entry_for_callback = entry.clone();
            process_for_commit.commit(
                &identity,
                &message,
                ticket.cancellable(),
                Rc::new(move |result| {
                    let Some(state) = weak.upgrade() else {
                        return;
                    };
                    finish(&state, &ticket_for_callback, &entry_for_callback, result);
                }),
            );
        }),
    );
}

fn preference_identity(settings_identity: &(String, String)) -> Option<GitIdentity> {
    if settings_identity.0.trim().is_empty() || settings_identity.1.trim().is_empty() {
        return None;
    }
    GitIdentity::new(settings_identity.0.clone(), settings_identity.1.clone()).ok()
}

fn finish(
    state: &SourceStateRef,
    ticket: &MutationTicket,
    entry: &gtk4::Entry,
    result: Result<(), GitProcessError>,
) {
    let matched = operation_bridge::finish_mutation(state, ticket);
    let current = operation_bridge::mutation_root_is_current(state, ticket);
    if !matched {
        return;
    }
    if current {
        match result {
            Ok(()) => entry.set_text(""),
            Err(error) if git_error_is_cancelled(&error) => {}
            Err(error) => super::refresh::finish_error(state, &super::git_error_text(&error)),
        }
    }
    super::live::schedule(state);
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
