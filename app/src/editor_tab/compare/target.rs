use gtk4::gio;
use gtk4::prelude::FileExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompareTargetKind {
    Disk,
    File,
    Text,
}

#[derive(Clone)]
pub(super) struct CompareTarget {
    pub(super) kind: CompareTargetKind,
    pub(super) uri: Option<String>,
    pub(super) title: String,
    pub(super) file: Option<gio::File>,
    pub(super) text: Option<String>,
    pub(super) implicit_trailing_newline: bool,
}

impl CompareTarget {
    pub(super) fn disk(file: gio::File) -> Self {
        Self::from_file(file, CompareTargetKind::Disk)
    }

    pub(super) fn file(file: gio::File) -> Self {
        Self::from_file(file, CompareTargetKind::File)
    }

    pub(super) fn text(title: String, text: String) -> Self {
        Self {
            kind: CompareTargetKind::Text,
            uri: None,
            title,
            file: None,
            text: Some(text),
            implicit_trailing_newline: false,
        }
    }

    #[must_use]
    pub(super) fn is_refreshable(&self) -> bool {
        self.kind != CompareTargetKind::Text
    }

    fn from_file(file: gio::File, kind: CompareTargetKind) -> Self {
        let uri = file.uri().to_string();
        let title = file
            .basename()
            .map_or_else(|| uri.clone(), |name| name.to_string_lossy().to_string());
        Self {
            kind,
            uri: Some(uri),
            title,
            file: Some(file),
            text: None,
            implicit_trailing_newline: false,
        }
    }
}
