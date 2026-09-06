#[derive(Clone, Copy)]
pub(crate) enum TempFileFixture {
    BoundaryChunkedClose,
    BoundaryChunkedFull,
    BoundaryLargePlaceholderClose,
    BoundaryLargePlaceholderRemove,
    BoundaryLargeViewerClose,
    BoundaryLargeViewerEdit,
    BoundaryLargeViewerEditFail,
    BoundaryLargeViewerOpen,
    BoundaryLargeViewerRefresh,
    BoundaryLargeViewerRestore,
    BoundaryLongLine,
    BoundaryMediumMinimap,
    BoundaryOpenCap,
    BoundaryRestoreBig,
    BoundaryRestoreExtra,
    BoundaryRestoreSmall,
    BoundaryThresholdReapply,
    CloseSave,
    MarkdownPreview,
    OpenA,
    OpenB,
    RestoreOne,
    RestoreTwo,
    TabsLargeViewerTransfer,
    V2First,
    V2Second,
    V2Third,
    V4AutoA,
    V4AutoB,
    V4Banner,
    V4Stale,
    V4Syntax,
    V5Ascii,
    V5Latin1,
    V6AppOpenFirst,
    V6AppOpenSecond,
    V6Open,
    V7Editable,
    V7ExitCompare,
    V7ExitReference,
    V7MinimapLong,
    V7MinimapLongRef,
    V7MinimapShort,
    V7MinimapShortRef,
    V7Nav,
    V7NavRef,
    V7Reference,
    V7Replacement,
    V7TabActionReference,
    V7TabActions,
    V7TwoLeft,
    V7TwoRight,
    V8Autosave,
    V8RecentFirst,
    V8RecentSecond,
    V11AutosaveCompare,
    V11AutosaveReference,
    V11Editable,
    V11GutterEditable,
    V11GutterReference,
    V11NarrowReference,
    V11Reference,
    V11SaveSync,
    V11WideCurrent,
    V13StatusPresentation,
    WindowCloseA,
    WindowCloseB,
}

impl TempFileFixture {
    pub(crate) const CLOSE_SAVE: Self = Self::CloseSave;
    pub(crate) const MARKDOWN_PREVIEW: Self = Self::MarkdownPreview;
    pub(crate) const OPEN_A: Self = Self::OpenA;
    pub(crate) const OPEN_B: Self = Self::OpenB;
    pub(crate) const RESTORE_ONE: Self = Self::RestoreOne;
    pub(crate) const RESTORE_TWO: Self = Self::RestoreTwo;
    pub(crate) const V2_FIRST: Self = Self::V2First;
    pub(crate) const V2_SECOND: Self = Self::V2Second;
    pub(crate) const V2_THIRD: Self = Self::V2Third;
    pub(crate) const V4_AUTO_A: Self = Self::V4AutoA;
    pub(crate) const V4_AUTO_B: Self = Self::V4AutoB;
    pub(crate) const V4_BANNER: Self = Self::V4Banner;
    pub(crate) const V4_STALE: Self = Self::V4Stale;
    pub(crate) const V4_SYNTAX: Self = Self::V4Syntax;
    pub(crate) const V5_ASCII: Self = Self::V5Ascii;
    pub(crate) const V5_LATIN1: Self = Self::V5Latin1;
    pub(crate) const V6_APP_OPEN_FIRST: Self = Self::V6AppOpenFirst;
    pub(crate) const V6_APP_OPEN_SECOND: Self = Self::V6AppOpenSecond;
    pub(crate) const V6_OPEN: Self = Self::V6Open;
    pub(crate) const V7_EDITABLE: Self = Self::V7Editable;
    pub(crate) const V7_EXIT_COMPARE: Self = Self::V7ExitCompare;
    pub(crate) const V7_EXIT_REFERENCE: Self = Self::V7ExitReference;
    pub(crate) const V7_MINIMAP_LONG: Self = Self::V7MinimapLong;
    pub(crate) const V7_MINIMAP_LONG_REF: Self = Self::V7MinimapLongRef;
    pub(crate) const V7_MINIMAP_SHORT: Self = Self::V7MinimapShort;
    pub(crate) const V7_MINIMAP_SHORT_REF: Self = Self::V7MinimapShortRef;
    pub(crate) const V7_NAV: Self = Self::V7Nav;
    pub(crate) const V7_NAV_REF: Self = Self::V7NavRef;
    pub(crate) const V7_REFERENCE: Self = Self::V7Reference;
    pub(crate) const V7_REPLACEMENT: Self = Self::V7Replacement;
    pub(crate) const V7_TAB_ACTION_REFERENCE: Self = Self::V7TabActionReference;
    pub(crate) const V7_TAB_ACTIONS: Self = Self::V7TabActions;
    pub(crate) const V7_TWO_LEFT: Self = Self::V7TwoLeft;
    pub(crate) const V7_TWO_RIGHT: Self = Self::V7TwoRight;
    pub(crate) const V8_AUTOSAVE: Self = Self::V8Autosave;
    pub(crate) const V8_RECENT_FIRST: Self = Self::V8RecentFirst;
    pub(crate) const V8_RECENT_SECOND: Self = Self::V8RecentSecond;
    pub(crate) const V11_AUTOSAVE_COMPARE: Self = Self::V11AutosaveCompare;
    pub(crate) const V11_AUTOSAVE_REFERENCE: Self = Self::V11AutosaveReference;
    pub(crate) const V11_EDITABLE: Self = Self::V11Editable;
    pub(crate) const V11_GUTTER_EDITABLE: Self = Self::V11GutterEditable;
    pub(crate) const V11_GUTTER_REFERENCE: Self = Self::V11GutterReference;
    pub(crate) const V11_NARROW_REFERENCE: Self = Self::V11NarrowReference;
    pub(crate) const V11_REFERENCE: Self = Self::V11Reference;
    pub(crate) const V11_SAVE_SYNC: Self = Self::V11SaveSync;
    pub(crate) const V11_WIDE_CURRENT: Self = Self::V11WideCurrent;
    pub(crate) const V13_STATUS_PRESENTATION: Self = Self::V13StatusPresentation;
    pub(crate) const WINDOW_CLOSE_A: Self = Self::WindowCloseA;
    pub(crate) const WINDOW_CLOSE_B: Self = Self::WindowCloseB;

    pub(crate) const BOUNDARY_CHUNKED_CLOSE: Self = Self::BoundaryChunkedClose;
    pub(crate) const BOUNDARY_CHUNKED_FULL: Self = Self::BoundaryChunkedFull;
    pub(crate) const BOUNDARY_LARGE_PLACEHOLDER_CLOSE: Self = Self::BoundaryLargePlaceholderClose;
    pub(crate) const BOUNDARY_LARGE_PLACEHOLDER_REMOVE: Self = Self::BoundaryLargePlaceholderRemove;
    pub(crate) const BOUNDARY_LARGE_VIEWER_CLOSE: Self = Self::BoundaryLargeViewerClose;
    pub(crate) const BOUNDARY_LARGE_VIEWER_EDIT: Self = Self::BoundaryLargeViewerEdit;
    pub(crate) const BOUNDARY_LARGE_VIEWER_EDIT_FAIL: Self = Self::BoundaryLargeViewerEditFail;
    pub(crate) const BOUNDARY_LARGE_VIEWER_OPEN: Self = Self::BoundaryLargeViewerOpen;
    pub(crate) const BOUNDARY_LARGE_VIEWER_REFRESH: Self = Self::BoundaryLargeViewerRefresh;
    pub(crate) const BOUNDARY_LARGE_VIEWER_RESTORE: Self = Self::BoundaryLargeViewerRestore;
    pub(crate) const BOUNDARY_LONG_LINE: Self = Self::BoundaryLongLine;
    pub(crate) const BOUNDARY_MEDIUM_MINIMAP: Self = Self::BoundaryMediumMinimap;
    pub(crate) const BOUNDARY_OPEN_CAP: Self = Self::BoundaryOpenCap;
    pub(crate) const BOUNDARY_RESTORE_BIG: Self = Self::BoundaryRestoreBig;
    pub(crate) const BOUNDARY_RESTORE_EXTRA: Self = Self::BoundaryRestoreExtra;
    pub(crate) const BOUNDARY_RESTORE_SMALL: Self = Self::BoundaryRestoreSmall;
    pub(crate) const BOUNDARY_THRESHOLD_REAPPLY: Self = Self::BoundaryThresholdReapply;
    pub(crate) const TABS_LARGE_VIEWER_TRANSFER: Self = Self::TabsLargeViewerTransfer;

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::BoundaryChunkedClose => "riteed-chunked-close.txt",
            Self::BoundaryChunkedFull => "riteed-chunked-full.txt",
            Self::BoundaryLargePlaceholderClose => "riteed-large-placeholder-close.txt",
            Self::BoundaryLargePlaceholderRemove => "riteed-large-placeholder-remove.txt",
            Self::BoundaryLargeViewerClose => "riteed-large-viewer-close.txt",
            Self::BoundaryLargeViewerEdit => "riteed-large-viewer-edit.txt",
            Self::BoundaryLargeViewerEditFail => "riteed-large-viewer-edit-fail.txt",
            Self::BoundaryLargeViewerOpen => "riteed-large-viewer-open.txt",
            Self::BoundaryLargeViewerRefresh => "riteed-large-viewer-refresh.txt",
            Self::BoundaryLargeViewerRestore => "riteed-large-viewer-restore.txt",
            Self::BoundaryLongLine => "riteed-long-line.txt",
            Self::BoundaryMediumMinimap => "riteed-medium-minimap.txt",
            Self::BoundaryOpenCap => "riteed-open-boundary-cap.txt",
            Self::BoundaryRestoreBig => "riteed-restore-big.txt",
            Self::BoundaryRestoreExtra => "riteed-restore-extra.txt",
            Self::BoundaryRestoreSmall => "riteed-restore-small.txt",
            Self::BoundaryThresholdReapply => "riteed-threshold-reapply.rs",
            Self::CloseSave => "riteed-close-save.txt",
            Self::MarkdownPreview => "riteed-markdown-preview.md",
            Self::OpenA => "riteed-open-a.txt",
            Self::OpenB => "riteed-open-b.txt",
            Self::RestoreOne => "riteed-restore-one.txt",
            Self::RestoreTwo => "riteed-restore-two.txt",
            Self::TabsLargeViewerTransfer => "riteed-large-viewer-transfer.txt",
            Self::V2First => "riteed-v2-first.txt",
            Self::V2Second => "riteed-v2-second.txt",
            Self::V2Third => "riteed-v2-third.txt",
            Self::V4AutoA => "riteed-v4-auto-a.txt",
            Self::V4AutoB => "riteed-v4-auto-b.txt",
            Self::V4Banner => "riteed-v4-banner.rs",
            Self::V4Stale => "riteed-v4-stale.txt",
            Self::V4Syntax => "riteed-v4-syntax.rs",
            Self::V5Ascii => "riteed-v5-ascii.txt",
            Self::V5Latin1 => "riteed-v5-latin1.txt",
            Self::V6AppOpenFirst => "riteed-v6-app-open-first.txt",
            Self::V6AppOpenSecond => "riteed-v6-app-open-second.txt",
            Self::V6Open => "riteed-v6-open.txt",
            Self::V7Editable => "riteed-v7-editable.txt",
            Self::V7ExitCompare => "riteed-v7-exit-compare.txt",
            Self::V7ExitReference => "riteed-v7-exit-reference.txt",
            Self::V7MinimapLong => "riteed-v7-minimap-long.txt",
            Self::V7MinimapLongRef => "riteed-v7-minimap-long-ref.txt",
            Self::V7MinimapShort => "riteed-v7-minimap-short.txt",
            Self::V7MinimapShortRef => "riteed-v7-minimap-short-ref.txt",
            Self::V7Nav => "riteed-v7-nav.txt",
            Self::V7NavRef => "riteed-v7-nav-ref.txt",
            Self::V7Reference => "riteed-v7-reference.txt",
            Self::V7Replacement => "riteed-v7-replacement.txt",
            Self::V7TabActionReference => "riteed-v7-tab-action-ref.txt",
            Self::V7TabActions => "riteed-v7-tab-actions.txt",
            Self::V7TwoLeft => "riteed-v7-two-left.txt",
            Self::V7TwoRight => "riteed-v7-two-right.txt",
            Self::V8Autosave => "riteed-v8-autosave.txt",
            Self::V8RecentFirst => "riteed-v8-recent-first.txt",
            Self::V8RecentSecond => "riteed-v8-recent-second.txt",
            Self::V11AutosaveCompare => "riteed-v11-autosave-compare.txt",
            Self::V11AutosaveReference => "riteed-v11-autosave-reference.txt",
            Self::V11Editable => "riteed-v11-editable.rs",
            Self::V11GutterEditable => "riteed-v11-gutter-editable.txt",
            Self::V11GutterReference => "riteed-v11-gutter-reference.txt",
            Self::V11NarrowReference => "riteed-v11-narrow-reference.txt",
            Self::V11Reference => "riteed-v11-reference.rs",
            Self::V11SaveSync => "riteed-v11-save-sync.txt",
            Self::V11WideCurrent => "riteed-v11-wide-current.txt",
            Self::V13StatusPresentation => "riteed-v13-status-presentation.txt",
            Self::WindowCloseA => "riteed-window-close-a.txt",
            Self::WindowCloseB => "riteed-window-close-b.txt",
        }
    }
}
