use gtk4::prelude::*;
use libadwaita as adw;

use super::EditorTab;
use crate::error::AppError;

/// Identifies the document state that an asynchronous disk read may replace.
///
/// The I/O generation rejects superseded requests. This guard additionally
/// rejects a still-current request when the tab was detached, its document
/// identity changed, or GTK accepted an edit while the read was pending.
#[derive(Clone)]
pub(crate) struct DocumentReadGuard {
    dirty_generation: u64,
    document_uri: Option<String>,
    page: Option<adw::TabPage>,
    attachment_root: Option<gtk4::Root>,
}

impl DocumentReadGuard {
    pub(super) fn capture(tab: &EditorTab) -> Self {
        Self {
            dirty_generation: tab.dirty_generation(),
            document_uri: tab.document_uri(),
            page: tab.page(),
            attachment_root: tab.page().and_then(|page| page.child().root()),
        }
    }

    pub(crate) fn page(&self) -> Option<&adw::TabPage> {
        self.page.as_ref()
    }

    pub(super) fn verify(&self, tab: &EditorTab) -> Result<(), AppError> {
        let current_attachment_root = tab.page().and_then(|page| page.child().root());
        if tab.page().as_ref() != self.page.as_ref()
            || self.attachment_root.is_none()
            || current_attachment_root != self.attachment_root
        {
            return Err(AppError::Cancelled);
        }
        if tab.dirty_generation() != self.dirty_generation
            || tab.document_uri() != self.document_uri
        {
            return Err(AppError::DocumentChangedDuringRead);
        }
        Ok(())
    }
}

impl EditorTab {
    pub(crate) fn capture_document_read_guard(&self) -> DocumentReadGuard {
        DocumentReadGuard::capture(self)
    }

    pub(crate) fn verify_document_read_guard(
        &self,
        guard: &DocumentReadGuard,
    ) -> Result<(), AppError> {
        guard.verify(self)
    }
}
