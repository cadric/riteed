use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::{gio, glib, prelude::*};

use crate::document_limits::VIEWER_PAGE_BYTES;
use crate::error::AppError;
use crate::large_file::{page_text::decode_page_window, reader, search, usize_to_u64};

type VoidCallback = Rc<dyn Fn()>;

pub(crate) struct LargeFileViewer {
    root: gtk4::Box,
    buffer: gtk4::TextBuffer,
    file: gio::File,
    file_size: Cell<u64>,
    generation: Cell<u64>,
    page_offset: Cell<u64>,
    next_page_offset: Cell<u64>,
    current_cancellable: RefCell<Option<gio::Cancellable>>,
    search_cancellable: RefCell<Option<gio::Cancellable>>,
    status_label: gtk4::Label,
    search_entry: gtk4::SearchEntry,
    line_entry: gtk4::Entry,
    previous_button: gtk4::Button,
    next_button: gtk4::Button,
    refresh_button: gtk4::Button,
    edit_button: gtk4::Button,
}

impl LargeFileViewer {
    #[must_use]
    pub(crate) fn new(
        file: &gio::File,
        file_size: u64,
        edit_allowed: bool,
        edit_warning: &str,
        on_edit: VoidCallback,
    ) -> Rc<Self> {
        let buffer = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
        let text_view = build_text_view(&buffer);
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&text_view)
            .build();
        scrolled.set_hexpand(true);
        scrolled.set_vexpand(true);

        let previous_button = gtk4::Button::with_label(&pgettext("viewer button", "Previous"));
        let next_button = gtk4::Button::with_label(&pgettext("viewer button", "Next"));
        let refresh_button = gtk4::Button::with_label(&pgettext("viewer button", "Refresh"));
        let edit_button = gtk4::Button::with_label(&pgettext("viewer button", "Edit Anyway"));
        edit_button.set_sensitive(edit_allowed);
        edit_button.set_tooltip_text(Some(edit_warning));

        let search_entry = gtk4::SearchEntry::new();
        search_entry.set_placeholder_text(Some(&pgettext("viewer search", "Find")));
        let search_button = gtk4::Button::with_label(&pgettext("viewer button", "Find"));

        let line_entry = gtk4::Entry::new();
        line_entry.set_placeholder_text(Some(&pgettext("viewer line jump", "Line")));
        line_entry.set_width_chars(8);
        let line_button = gtk4::Button::with_label(&pgettext("viewer button", "Go"));

        let status_label = gtk4::Label::new(Some(&gettext("Loading file page...")));
        status_label.set_xalign(0.0);
        status_label.set_hexpand(true);
        status_label.set_tooltip_text(Some(&viewer_memory_tooltip()));

        let toolbar = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        toolbar.append(&previous_button);
        toolbar.append(&next_button);
        toolbar.append(&refresh_button);
        toolbar.append(&search_entry);
        toolbar.append(&search_button);
        toolbar.append(&line_entry);
        toolbar.append(&line_button);
        toolbar.append(&status_label);
        toolbar.append(&edit_button);

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.append(&toolbar);
        root.append(&scrolled);

        let viewer = Rc::new(Self {
            root,
            buffer,
            file: file.clone(),
            file_size: Cell::new(file_size),
            generation: Cell::new(0),
            page_offset: Cell::new(0),
            next_page_offset: Cell::new(0),
            current_cancellable: RefCell::new(None),
            search_cancellable: RefCell::new(None),
            status_label,
            search_entry,
            line_entry,
            previous_button,
            next_button,
            refresh_button,
            edit_button,
        });
        viewer.install_callbacks(&search_button, &line_button, on_edit);
        viewer.load_page(0);
        viewer
    }

    #[must_use]
    pub(crate) fn widget(&self) -> gtk4::Widget {
        self.root.clone().upcast()
    }

    pub(crate) fn cancel(&self) {
        self.cancel_current_request();
        self.cancel_search_request();
    }

    fn install_callbacks(
        self: &Rc<Self>,
        search_button: &gtk4::Button,
        line_button: &gtk4::Button,
        on_edit: VoidCallback,
    ) {
        let weak = Rc::downgrade(self);
        self.previous_button.connect_clicked(move |_| {
            let Some(viewer) = weak.upgrade() else {
                return;
            };
            let next_offset = viewer
                .page_offset
                .get()
                .saturating_sub(usize_to_u64(VIEWER_PAGE_BYTES));
            viewer.load_page(next_offset);
        });

        let weak = Rc::downgrade(self);
        self.next_button.connect_clicked(move |_| {
            let Some(viewer) = weak.upgrade() else {
                return;
            };
            viewer.load_page(viewer.next_page_offset.get());
        });

        let weak = Rc::downgrade(self);
        self.refresh_button.connect_clicked(move |_| {
            if let Some(viewer) = weak.upgrade() {
                viewer.load_page(viewer.page_offset.get());
            }
        });

        let weak = Rc::downgrade(self);
        search_button.connect_clicked(move |_| {
            if let Some(viewer) = weak.upgrade() {
                viewer.start_search();
            }
        });

        let weak = Rc::downgrade(self);
        self.search_entry.connect_activate(move |_| {
            if let Some(viewer) = weak.upgrade() {
                viewer.start_search();
            }
        });

        let weak = Rc::downgrade(self);
        line_button.connect_clicked(move |_| {
            if let Some(viewer) = weak.upgrade() {
                viewer.jump_to_line_from_entry();
            }
        });

        let weak = Rc::downgrade(self);
        self.line_entry.connect_activate(move |_| {
            if let Some(viewer) = weak.upgrade() {
                viewer.jump_to_line_from_entry();
            }
        });

        self.edit_button.connect_clicked(move |_| {
            on_edit();
        });
    }

    fn load_page(self: &Rc<Self>, offset: u64) {
        let generation = self.next_generation();
        let cancellable = self.replace_current_cancellable();
        self.status_label.set_text(&gettext("Loading file page..."));
        self.previous_button.set_sensitive(offset > 0);
        self.next_button.set_sensitive(false);

        let weak = Rc::downgrade(self);
        let file = self.file.clone();
        let cancellable_for_query = cancellable.clone();
        file.query_info_async(
            "standard::type,standard::size",
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::default(),
            Some(&cancellable_for_query),
            move |result| {
                let Some(viewer) = weak.upgrade() else {
                    return;
                };
                if viewer.generation.get() != generation {
                    return;
                }
                match result {
                    Ok(info) => viewer.update_file_size_from_info(&info),
                    Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {
                        viewer.current_cancellable.borrow_mut().take();
                        return;
                    }
                    Err(_error) => {}
                }
                viewer.read_page_after_size_refresh(offset, generation, &cancellable);
            },
        );
    }

    fn read_page_after_size_refresh(
        self: &Rc<Self>,
        offset: u64,
        generation: u64,
        cancellable: &gio::Cancellable,
    ) {
        let clamped_offset = offset.min(self.file_size.get());
        self.previous_button.set_sensitive(clamped_offset > 0);
        let weak = Rc::downgrade(self);
        reader::read_window(
            &self.file,
            clamped_offset,
            VIEWER_PAGE_BYTES,
            Some(cancellable),
            Rc::new(move |result| {
                let Some(viewer) = weak.upgrade() else {
                    return;
                };
                if viewer.generation.get() != generation {
                    return;
                }
                viewer.current_cancellable.borrow_mut().take();
                viewer.apply_page_result(result);
            }),
        );
    }

    fn update_file_size_from_info(&self, info: &gio::FileInfo) {
        if info.file_type() != gio::FileType::Regular || !info.has_attribute("standard::size") {
            return;
        }
        if let Ok(size) = u64::try_from(info.size()) {
            self.file_size.set(size);
        }
    }

    fn apply_page_result(&self, result: Result<reader::ReadWindow, AppError>) {
        match result {
            Ok(window) => {
                let decoded = decode_page_window(window.offset, &window.bytes);
                self.buffer.set_text(&decoded.text);
                self.page_offset.set(decoded.visible_start);
                self.next_page_offset.set(decoded.next_offset);
                let file_size = self.file_size.get();
                let end = decoded.visible_end.min(file_size);
                self.status_label.set_text(&format_page_status(
                    decoded.visible_start,
                    end,
                    file_size,
                ));
                self.previous_button
                    .set_sensitive(decoded.visible_start > 0);
                self.next_button.set_sensitive(
                    !window.eof
                        && decoded.next_offset > decoded.visible_start
                        && decoded.next_offset < file_size,
                );
            }
            Err(AppError::Cancelled) => {}
            Err(error) => {
                self.buffer.set_text("");
                self.status_label.set_text(&error.body());
            }
        }
    }

    fn start_search(self: &Rc<Self>) {
        let needle = self.search_entry.text().to_string();
        if needle.is_empty() {
            self.cancel_search_request();
            return;
        }
        let generation = self.next_generation();
        let cancellable = self.replace_search_cancellable();
        self.status_label.set_text(&gettext("Searching file..."));

        let weak = Rc::downgrade(self);
        search::search_file(
            &self.file,
            &needle,
            Some(&cancellable),
            Rc::new(move |result| {
                let Some(viewer) = weak.upgrade() else {
                    return;
                };
                if viewer.generation.get() != generation {
                    return;
                }
                viewer.search_cancellable.borrow_mut().take();
                match result {
                    Ok(outcome) => viewer.apply_search_outcome(&outcome),
                    Err(AppError::Cancelled) => {}
                    Err(error) => viewer.status_label.set_text(&error.body()),
                }
            }),
        );
    }

    fn apply_search_outcome(self: &Rc<Self>, outcome: &search::SearchOutcome) {
        let Some(first_match) = outcome.matches.first().copied() else {
            self.status_label.set_text(&gettext("No matches found."));
            return;
        };
        let message = search_match_message(outcome.matches.len(), outcome.reached_cap);
        self.status_label.set_text(&message);
        self.load_page(first_match);
    }

    #[cfg(test)]
    pub(crate) fn text_for_tests(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
            .to_string()
    }

    #[cfg(test)]
    pub(crate) fn status_for_tests(&self) -> String {
        self.status_label.text().to_string()
    }

    #[cfg(test)]
    pub(crate) fn activate_edit_for_tests(&self) -> bool {
        if !self.edit_button.is_sensitive() {
            return false;
        }
        self.edit_button.emit_clicked();
        true
    }

    #[cfg(test)]
    pub(crate) fn activate_refresh_for_tests(&self) {
        self.refresh_button.emit_clicked();
    }

    fn jump_to_line_from_entry(self: &Rc<Self>) {
        let Ok(line) = self.line_entry.text().parse::<u64>() else {
            self.status_label
                .set_text(&gettext("Enter a valid line number."));
            return;
        };
        if line == 0 {
            self.status_label
                .set_text(&gettext("Enter a valid line number."));
            return;
        }
        self.status_label.set_text(&gettext("Finding line..."));
        let generation = self.next_generation();
        let cancellable = self.replace_current_cancellable();
        find_line_offset(
            &self.file,
            line,
            Some(cancellable),
            Rc::new({
                let weak = Rc::downgrade(self);
                move |result| {
                    let Some(viewer) = weak.upgrade() else {
                        return;
                    };
                    if viewer.generation.get() != generation {
                        return;
                    }
                    viewer.current_cancellable.borrow_mut().take();
                    match result {
                        Ok(offset) => viewer.load_page(offset),
                        Err(AppError::Cancelled) => {}
                        Err(error) => viewer.status_label.set_text(&error.body()),
                    }
                }
            }),
        );
    }

    fn next_generation(&self) -> u64 {
        let next = self.generation.get().saturating_add(1);
        self.generation.set(next);
        next
    }

    fn replace_current_cancellable(&self) -> gio::Cancellable {
        self.cancel_current_request();
        self.cancel_search_request();
        let cancellable = gio::Cancellable::new();
        self.current_cancellable.replace(Some(cancellable.clone()));
        cancellable
    }

    fn replace_search_cancellable(&self) -> gio::Cancellable {
        self.cancel_current_request();
        self.cancel_search_request();
        let cancellable = gio::Cancellable::new();
        self.search_cancellable.replace(Some(cancellable.clone()));
        cancellable
    }

    fn cancel_current_request(&self) {
        cancel_cancellable(&self.current_cancellable);
    }

    fn cancel_search_request(&self) {
        cancel_cancellable(&self.search_cancellable);
    }
}

type LineCallback = Rc<dyn Fn(Result<u64, AppError>)>;

fn find_line_offset(
    file: &gio::File,
    target_line: u64,
    cancellable: Option<gio::Cancellable>,
    callback: LineCallback,
) {
    if cancellable
        .as_ref()
        .is_some_and(gio::Cancellable::is_cancelled)
    {
        callback(Err(AppError::Cancelled));
        return;
    }
    let cancellable_for_open = cancellable.clone();
    reader::open_stream(
        file,
        cancellable_for_open.as_ref(),
        Rc::new(move |result| match result {
            Ok(opened) => find_line_offset_in_stream(
                &opened,
                target_line,
                1,
                0,
                cancellable.clone(),
                callback.clone(),
            ),
            Err(error) => callback(Err(error)),
        }),
    );
}

fn find_line_offset_in_stream(
    opened: &reader::OpenedStream,
    target_line: u64,
    current_line: u64,
    offset: u64,
    cancellable: Option<gio::Cancellable>,
    callback: LineCallback,
) {
    if cancellable
        .as_ref()
        .is_some_and(gio::Cancellable::is_cancelled)
    {
        callback(Err(AppError::Cancelled));
        return;
    }
    let cancellable_for_read = cancellable.clone();
    let opened_for_callback = opened.clone();
    reader::read_open_stream_window(
        opened,
        offset,
        VIEWER_PAGE_BYTES,
        cancellable_for_read.as_ref(),
        Rc::new(move |result| match result {
            Ok(window) => {
                if let Some(line_offset) =
                    locate_line_in_chunk(target_line, current_line, window.offset, &window.bytes)
                {
                    callback(Ok(line_offset));
                    return;
                }
                if window.eof {
                    callback(Ok(window.offset));
                    return;
                }
                let next_line = current_line.saturating_add(count_newlines(&window.bytes));
                find_line_offset_in_stream(
                    &opened_for_callback,
                    target_line,
                    next_line,
                    window
                        .offset
                        .saturating_add(usize_to_u64(window.bytes.len())),
                    cancellable.clone(),
                    callback.clone(),
                );
            }
            Err(error) => callback(Err(error)),
        }),
    );
}

pub(super) fn locate_line_in_chunk(
    target_line: u64,
    current_line: u64,
    offset: u64,
    bytes: &[u8],
) -> Option<u64> {
    if target_line <= current_line {
        return Some(offset);
    }
    let mut line = current_line;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            line = line.saturating_add(1);
            if line == target_line {
                return Some(offset.saturating_add(usize_to_u64(index)).saturating_add(1));
            }
        }
    }
    None
}

pub(super) fn count_newlines(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .filter(|byte| **byte == b'\n')
        .fold(0_u64, |count, _| count.saturating_add(1))
}

fn build_text_view(buffer: &gtk4::TextBuffer) -> gtk4::TextView {
    let view = gtk4::TextView::with_buffer(buffer);
    view.set_cursor_visible(false);
    view.set_editable(false);
    view.set_hexpand(true);
    view.set_left_margin(12);
    view.set_monospace(true);
    view.set_right_margin(12);
    view.set_top_margin(12);
    view.set_vexpand(true);
    view.set_wrap_mode(gtk4::WrapMode::None);
    view
}

pub(super) fn format_page_status(start: u64, end: u64, size: u64) -> String {
    let template = gettext("Viewing bytes %1$s-%2$s of %3$s.");
    template
        .replace("%1$s", &start.to_string())
        .replace("%2$s", &end.to_string())
        .replace("%3$s", &size.to_string())
}

pub(super) fn viewer_memory_tooltip() -> String {
    gettext("Viewer keeps only the current file page in memory.")
}

pub(super) fn search_match_message(match_count: usize, reached_cap: bool) -> String {
    if reached_cap {
        return gettext("Many matches found; showing the first match.");
    }
    let count = u32::try_from(match_count).map_or(u32::MAX, |value| value);
    ngettext(
        "%d match found; showing the first match.",
        "%d matches found; showing the first match.",
        count,
    )
    .replace("%d", &match_count.to_string())
}

pub(super) fn cancel_cancellable(cell: &RefCell<Option<gio::Cancellable>>) {
    if let Some(cancellable) = cell.borrow_mut().take() {
        cancellable.cancel();
    }
}
