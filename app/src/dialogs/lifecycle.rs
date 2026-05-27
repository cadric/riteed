use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
pub(crate) enum DialogLeakKind {
    PasteText,
    RecentFiles,
    Encoding,
}

pub(crate) struct DialogLeakCanary {
    kind: DialogLeakKind,
}

static PASTE_TEXT_STATES: AtomicUsize = AtomicUsize::new(0);
static RECENT_FILES_STATES: AtomicUsize = AtomicUsize::new(0);
static ENCODING_STATES: AtomicUsize = AtomicUsize::new(0);

impl DialogLeakCanary {
    pub(crate) fn new(kind: DialogLeakKind) -> Self {
        counter(kind).fetch_add(1, Ordering::SeqCst);
        Self { kind }
    }
}

impl Drop for DialogLeakCanary {
    fn drop(&mut self) {
        counter(self.kind).fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn reset_dialog_leak_counters_for_tests() {
    PASTE_TEXT_STATES.store(0, Ordering::SeqCst);
    RECENT_FILES_STATES.store(0, Ordering::SeqCst);
    ENCODING_STATES.store(0, Ordering::SeqCst);
}

pub(crate) fn assert_dialog_leak_counters_clear_for_tests() {
    assert_eq!(PASTE_TEXT_STATES.load(Ordering::SeqCst), 0);
    assert_eq!(RECENT_FILES_STATES.load(Ordering::SeqCst), 0);
    assert_eq!(ENCODING_STATES.load(Ordering::SeqCst), 0);
}

pub(crate) fn dialog_leak_counters_clear_for_tests() -> bool {
    PASTE_TEXT_STATES.load(Ordering::SeqCst) == 0
        && RECENT_FILES_STATES.load(Ordering::SeqCst) == 0
        && ENCODING_STATES.load(Ordering::SeqCst) == 0
}

fn counter(kind: DialogLeakKind) -> &'static AtomicUsize {
    match kind {
        DialogLeakKind::PasteText => &PASTE_TEXT_STATES,
        DialogLeakKind::RecentFiles => &RECENT_FILES_STATES,
        DialogLeakKind::Encoding => &ENCODING_STATES,
    }
}
