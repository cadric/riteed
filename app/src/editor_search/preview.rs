use gtk4::{gdk, pango, prelude::*};
use libadwaita as adw;

const MAX_PREVIEW_MATCHES: usize = 5_000;
const PREVIEW_SEARCH_TAG: &str = "riteed-preview-search-match";
const PREVIEW_ACTIVE_SEARCH_TAG: &str = "riteed-preview-search-current-match";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreviewMatch {
    start: i32,
    end: i32,
}

pub(super) struct PreviewSearchBinding {
    buffer: gtk4::TextBuffer,
    view: gtk4::TextView,
    scrolled: gtk4::ScrolledWindow,
    matches: Vec<PreviewMatch>,
    current: Option<usize>,
    limit_hit: bool,
    normal_tag: gtk4::TextTag,
    active_tag: gtk4::TextTag,
}

impl PreviewSearchBinding {
    #[must_use]
    pub(super) fn new(
        buffer: &gtk4::TextBuffer,
        view: &gtk4::TextView,
        scrolled: &gtk4::ScrolledWindow,
        query: &str,
        match_case: bool,
    ) -> Self {
        let normal_tag = search_tag(buffer, PREVIEW_SEARCH_TAG);
        let active_tag = search_tag(buffer, PREVIEW_ACTIVE_SEARCH_TAG);
        let (matches, limit_hit) = collect_matches(buffer, query, match_case);
        let current = (!matches.is_empty()).then_some(0);
        let binding = Self {
            buffer: buffer.clone(),
            view: view.clone(),
            scrolled: scrolled.clone(),
            matches,
            current,
            limit_hit,
            normal_tag,
            active_tag,
        };
        binding.refresh_style();
        binding.reapply_highlights();
        binding.select_current();
        binding
    }

    pub(super) fn clear_highlights(&self) {
        let start = self.buffer.start_iter();
        let end = self.buffer.end_iter();
        self.buffer.remove_tag(&self.normal_tag, &start, &end);
        self.buffer.remove_tag(&self.active_tag, &start, &end);
    }

    pub(super) fn refresh_style(&self) {
        let palette = SearchPalette::current();
        self.normal_tag.set_background_rgba(Some(&palette.match_bg));
        self.normal_tag.set_underline(pango::Underline::Single);
        self.normal_tag
            .set_underline_rgba(Some(&palette.match_line));
        self.normal_tag.set_weight(400);
        self.active_tag
            .set_background_rgba(Some(&palette.active_bg));
        self.active_tag
            .set_foreground_rgba(Some(&palette.active_fg));
        self.active_tag.set_underline(pango::Underline::Single);
        self.active_tag
            .set_underline_rgba(Some(&palette.active_line));
        self.active_tag.set_weight(700);
    }

    pub(super) fn reapply_highlights(&self) {
        self.clear_highlights();
        for (index, search_match) in self.matches.iter().enumerate() {
            let start = self.buffer.iter_at_offset(search_match.start);
            let end = self.buffer.iter_at_offset(search_match.end);
            self.buffer.apply_tag(&self.normal_tag, &start, &end);
            if Some(index) == self.current {
                self.buffer.apply_tag(&self.active_tag, &start, &end);
            }
        }
    }

    fn move_active_tag(&self, previous: Option<usize>) {
        if previous == self.current {
            return;
        }
        if let Some(search_match) = previous.and_then(|index| self.matches.get(index)) {
            let start = self.buffer.iter_at_offset(search_match.start);
            let end = self.buffer.iter_at_offset(search_match.end);
            self.buffer.remove_tag(&self.active_tag, &start, &end);
        }
        if let Some(search_match) = self.current.and_then(|index| self.matches.get(index)) {
            let start = self.buffer.iter_at_offset(search_match.start);
            let end = self.buffer.iter_at_offset(search_match.end);
            self.buffer.apply_tag(&self.active_tag, &start, &end);
        }
    }

    pub(super) fn occurrence_count(&self) -> usize {
        self.matches.len()
    }

    pub(super) fn limit_hit(&self) -> bool {
        self.limit_hit
    }

    pub(super) fn select_next(&mut self) {
        let previous = self.current;
        let Some(current) = self.current else {
            self.current = (!self.matches.is_empty()).then_some(0);
            self.move_active_tag(previous);
            self.select_current();
            return;
        };
        if self.matches.is_empty() {
            self.current = None;
            self.move_active_tag(previous);
            return;
        }
        self.current = Some((current + 1) % self.matches.len());
        self.move_active_tag(previous);
        self.select_current();
    }

    pub(super) fn select_previous(&mut self) {
        let previous = self.current;
        let Some(current) = self.current else {
            self.current = (!self.matches.is_empty()).then_some(0);
            self.move_active_tag(previous);
            self.select_current();
            return;
        };
        if self.matches.is_empty() {
            self.current = None;
            self.move_active_tag(previous);
            return;
        }
        self.current = Some(if current == 0 {
            self.matches.len() - 1
        } else {
            current - 1
        });
        self.move_active_tag(previous);
        self.select_current();
    }

    fn select_current(&self) {
        let Some(search_match) = self.current.and_then(|index| self.matches.get(index)) else {
            return;
        };
        let start = self.buffer.iter_at_offset(search_match.start);
        let end = self.buffer.iter_at_offset(search_match.end);
        self.buffer.select_range(&start, &end);
        let mut scroll_iter = start;
        self.view
            .scroll_to_iter(&mut scroll_iter, 0.2, false, 0.0, 0.0);
        self.scroll_outer_to_iter(&start);
    }

    fn scroll_outer_to_iter(&self, iter: &gtk4::TextIter) {
        let location = self.view.iter_location(iter);
        let adjustment = self.scrolled.vadjustment();
        let page_size = adjustment.page_size();
        let target = f64::from(location.y()) - (page_size * 0.2);
        let upper = (adjustment.upper() - page_size).max(adjustment.lower());
        adjustment.set_value(target.clamp(adjustment.lower(), upper));
    }
}

struct SearchPalette {
    match_bg: gdk::RGBA,
    match_line: gdk::RGBA,
    active_bg: gdk::RGBA,
    active_line: gdk::RGBA,
    active_fg: gdk::RGBA,
}

impl SearchPalette {
    fn current() -> Self {
        let manager = adw::StyleManager::default();
        let accent = manager.accent_color_rgba();
        let foreground = if manager.is_dark() {
            gdk::RGBA::new(1.0, 1.0, 1.0, 1.0)
        } else {
            gdk::RGBA::new(0.0, 0.0, 0.0, 1.0)
        };
        let (normal_alpha, active_alpha) = if manager.is_high_contrast() {
            (0.28, 0.58)
        } else {
            (0.16, 0.34)
        };
        Self {
            match_bg: with_alpha(&accent, normal_alpha),
            match_line: with_alpha(&accent, 1.0),
            active_bg: with_alpha(&accent, active_alpha),
            active_line: with_alpha(&accent, 1.0),
            active_fg: foreground,
        }
    }
}

fn collect_matches(
    buffer: &gtk4::TextBuffer,
    query: &str,
    match_case: bool,
) -> (Vec<PreviewMatch>, bool) {
    if query.is_empty() {
        return (Vec::new(), false);
    }

    let mut flags = gtk4::TextSearchFlags::TEXT_ONLY | gtk4::TextSearchFlags::VISIBLE_ONLY;
    if !match_case {
        flags |= gtk4::TextSearchFlags::CASE_INSENSITIVE;
    }

    let mut matches = Vec::new();
    let mut cursor = buffer.start_iter();
    let limit = buffer.end_iter();
    while matches.len() < MAX_PREVIEW_MATCHES {
        let Some((start, end)) = cursor.forward_search(query, flags, Some(&limit)) else {
            return (matches, false);
        };
        if start.offset() == end.offset() {
            return (matches, false);
        }
        matches.push(PreviewMatch {
            start: start.offset(),
            end: end.offset(),
        });
        cursor = end;
    }

    let limit_hit = cursor.forward_search(query, flags, Some(&limit)).is_some();
    (matches, limit_hit)
}

fn search_tag(buffer: &gtk4::TextBuffer, name: &str) -> gtk4::TextTag {
    let table = buffer.tag_table();
    if let Some(tag) = table.lookup(name) {
        return tag;
    }
    let tag = gtk4::TextTag::builder().name(name).build();
    let _added = table.add(&tag);
    tag
}

fn with_alpha(color: &gdk::RGBA, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(color.red(), color.green(), color.blue(), alpha)
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::*;

    use super::{MAX_PREVIEW_MATCHES, PreviewSearchBinding};

    #[test]
    fn preview_search_limit_is_bounded() {
        assert_eq!(MAX_PREVIEW_MATCHES, 5_000);
    }

    #[test]
    fn navigation_keeps_active_tag_on_current_match_only() {
        let _guard = crate::test_support::init_gtk_for_tests();
        if gtk4::gdk::Display::default().is_none() {
            return;
        }
        let buffer = gtk4::TextBuffer::new(None);
        buffer.set_text("match match match");
        let view = gtk4::TextView::with_buffer(&buffer);
        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_child(Some(&view));
        let mut binding = PreviewSearchBinding::new(&buffer, &view, &scrolled, "match", true);
        assert_eq!(binding.occurrence_count(), 3);
        assert!(buffer.iter_at_offset(0).has_tag(&binding.active_tag));

        binding.select_next();
        assert!(buffer.iter_at_offset(6).has_tag(&binding.active_tag));
        assert!(!buffer.iter_at_offset(0).has_tag(&binding.active_tag));
        assert!(buffer.iter_at_offset(0).has_tag(&binding.normal_tag));
        assert!(buffer.iter_at_offset(12).has_tag(&binding.normal_tag));

        binding.select_previous();
        assert!(buffer.iter_at_offset(0).has_tag(&binding.active_tag));
        assert!(!buffer.iter_at_offset(6).has_tag(&binding.active_tag));
    }
}
