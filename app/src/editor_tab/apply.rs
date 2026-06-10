use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::{glib, prelude::*};
use sourceview5::prelude::*;

use crate::editor_io::LoadedDocument;
use crate::editor_tab::EditorTab;
use crate::editor_tab::state::{PendingApplyRestore, PendingApplySource};
use crate::editor_view::ReloadSnapshot;

type ApplyFinishedCallback = Rc<dyn Fn(&Rc<EditorTab>)>;
type ApplyCancelledCallback = Rc<dyn Fn()>;

/// Documents at or below this size keep the synchronous apply path.
pub(super) const CHUNKED_APPLY_MIN_BYTES: usize = 1024 * 1024;
/// Upper bound for a single buffer insertion.
#[cfg(not(test))]
const CHUNK_TARGET_BYTES: usize = 2 * 1024 * 1024;
#[cfg(test)]
const CHUNK_TARGET_BYTES: usize = 256 * 1024;
/// Budget for one idle tick before yielding back to the main loop.
#[cfg(not(test))]
const CHUNK_TICK_BUDGET: Duration = Duration::from_millis(15);
#[cfg(test)]
const CHUNK_TICK_BUDGET: Duration = Duration::ZERO;

/// Largest `end > start` with `end <= start + target`, `end <= text.len()`,
/// and `end` on a `char` boundary. Returns `start` only when already at the
/// end or the target cannot include a full character.
fn chunk_end_at_char_boundary(text: &str, start: usize, target: usize) -> usize {
    let mut end = start.saturating_add(target).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    end
}

struct PendingApply {
    document: LoadedDocument,
    snapshot: Option<ReloadSnapshot>,
    offset: usize,
    restore: PendingApplyRestore,
}

impl EditorTab {
    /// Applies a loaded document without blocking the main loop for large text.
    ///
    /// Exactly one of `on_applied` and `on_cancelled` runs: `on_cancelled`
    /// fires (deferred to an idle) when the pending apply is torn down before
    /// completion, so workspace flows waiting on the load callback always
    /// finish.
    pub(super) fn apply_loaded_document_async(
        self: &Rc<Self>,
        document: LoadedDocument,
        snapshot: Option<ReloadSnapshot>,
        on_applied: ApplyFinishedCallback,
        on_cancelled: ApplyCancelledCallback,
    ) {
        self.begin_loaded_document_state(&document);
        let restore_undo = self.text_buffer.enables_undo();
        self.text_buffer.set_enable_undo(false);
        self.text_buffer
            .set_implicit_trailing_newline(document.format.implicit_trailing_newline());

        if document.text.len() <= CHUNKED_APPLY_MIN_BYTES {
            self.text_buffer.set_text(&document.text);
            self.text_buffer.set_enable_undo(restore_undo);
            self.finish_loaded_document_presentation(&document, snapshot.as_ref());
            on_applied(self);
            return;
        }

        self.text_buffer.set_text("");
        let restore = PendingApplyRestore {
            editable: self.text_view.is_editable(),
            undo: restore_undo,
        };
        self.text_view.set_editable(false);
        let generation = self.state.borrow().io.generation;
        let pending = Rc::new(RefCell::new(PendingApply {
            document,
            snapshot,
            offset: 0,
            restore,
        }));
        let weak = Rc::downgrade(self);
        let cancel_notifier = on_cancelled.clone();
        let source = glib::idle_add_local(move || {
            let Some(tab) = weak.upgrade() else {
                notify_apply_cancelled(&cancel_notifier);
                return glib::ControlFlow::Break;
            };
            if tab.state.borrow().io.generation != generation {
                tab.state.borrow_mut().io.pending_apply = None;
                restore_pending_apply(&tab, pending.borrow().restore);
                tab.apply_minimap_visibility();
                notify_apply_cancelled(&cancel_notifier);
                return glib::ControlFlow::Break;
            }
            let tick_started = Instant::now();
            loop {
                if insert_next_chunk(&tab, &pending) {
                    tab.state.borrow_mut().io.pending_apply = None;
                    finish_chunked_apply(&tab, &pending, &on_applied);
                    return glib::ControlFlow::Break;
                }
                if tick_started.elapsed() >= CHUNK_TICK_BUDGET {
                    return glib::ControlFlow::Continue;
                }
            }
        });
        self.state.borrow_mut().io.pending_apply = Some(PendingApplySource {
            source,
            restore,
            on_cancelled,
        });
        // Hide the minimap while chunks land; a GtkSourceMap re-layouts the
        // whole growing buffer on every insertion otherwise.
        self.apply_minimap_visibility();
    }

    pub(super) fn cancel_pending_apply(&self, pending: PendingApplySource) {
        let PendingApplySource {
            source,
            restore,
            on_cancelled,
        } = pending;
        source.remove();
        restore_pending_apply(self, restore);
        self.apply_minimap_visibility();
        notify_apply_cancelled(&on_cancelled);
    }
}

fn insert_next_chunk(tab: &Rc<EditorTab>, pending: &Rc<RefCell<PendingApply>>) -> bool {
    let (start, end, done) = {
        let pending = pending.borrow();
        let text = &pending.document.text;
        let end = chunk_end_at_char_boundary(text, pending.offset, CHUNK_TARGET_BYTES);
        (pending.offset, end, end >= text.len())
    };
    if end > start {
        let slice_owner = pending.borrow();
        let mut iter = tab.text_buffer.end_iter();
        tab.text_buffer
            .insert(&mut iter, &slice_owner.document.text[start..end]);
    }
    pending.borrow_mut().offset = end;
    done
}

fn finish_chunked_apply(
    tab: &Rc<EditorTab>,
    pending: &Rc<RefCell<PendingApply>>,
    on_applied: &ApplyFinishedCallback,
) {
    let pending = pending.borrow();
    restore_pending_apply(tab, pending.restore);
    tab.finish_loaded_document_presentation(&pending.document, pending.snapshot.as_ref());
    on_applied(tab);
}

fn restore_pending_apply(tab: &EditorTab, restore: PendingApplyRestore) {
    tab.text_view.set_editable(restore.editable);
    tab.text_buffer.set_enable_undo(restore.undo);
}

/// Defers the cancel notification to an idle so callers inside a
/// `RefCell` borrow (for example page-detach teardown) cannot re-enter
/// workspace state through the load callback.
fn notify_apply_cancelled(on_cancelled: &ApplyCancelledCallback) {
    let on_cancelled = on_cancelled.clone();
    glib::idle_add_local_once(move || on_cancelled());
}

#[cfg(test)]
mod tests {
    use super::chunk_end_at_char_boundary;

    #[test]
    fn chunk_end_respects_target_and_length() {
        assert_eq!(chunk_end_at_char_boundary("abcdef", 0, 4), 4);
        assert_eq!(chunk_end_at_char_boundary("abcdef", 4, 4), 6);
        assert_eq!(chunk_end_at_char_boundary("abcdef", 6, 4), 6);
    }

    #[test]
    fn chunk_end_backs_up_to_char_boundary() {
        let text = "abé";
        assert_eq!(chunk_end_at_char_boundary(text, 0, 3), 2);
        assert_eq!(chunk_end_at_char_boundary(text, 2, 3), 4);
    }

    #[test]
    fn chunk_end_handles_multibyte_run() {
        let text = "ééé";
        assert_eq!(chunk_end_at_char_boundary(text, 0, 1), 0);
        assert_eq!(chunk_end_at_char_boundary(text, 0, 2), 2);
    }
}
