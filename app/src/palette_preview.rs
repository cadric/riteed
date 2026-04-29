use std::f64::consts::{FRAC_PI_2, PI};

use gtk4::{cairo, gdk, prelude::*};

const PREVIEW_WIDTH: i32 = 132;
const PREVIEW_HEIGHT: i32 = 92;
const PREVIEW_RADIUS: f64 = 9.0;
const CODE_LEFT_MARGIN: f64 = 12.0;
const CODE_TOP_BASELINE: f64 = 18.0;
const CODE_LINE_HEIGHT: f64 = 10.0;
const CODE_INDENT: f64 = 8.0;
const CODE_FONT_SIZE: f64 = 6.8;

#[derive(Clone)]
pub(crate) struct PalettePreview {
    widget: gtk4::Overlay,
    selected_ring: gtk4::Box,
    selected_badge: gtk4::Image,
}

#[derive(Clone, Copy)]
struct PreviewColors {
    background: gdk::RGBA,
    text: gdk::RGBA,
    keyword: gdk::RGBA,
    function: gdk::RGBA,
    string: gdk::RGBA,
    constant: gdk::RGBA,
    comment: gdk::RGBA,
}

struct CodeRun {
    text: &'static str,
    kind: TokenKind,
}

struct CodeLine {
    indent: f64,
    runs: &'static [CodeRun],
}

#[derive(Clone, Copy)]
enum TokenKind {
    Text,
    Keyword,
    Function,
    String,
    Constant,
    Comment,
}

#[derive(Clone, Copy)]
enum StyleColorSource {
    Foreground,
    Background,
}

const CODE_COMMENT_LINE: [CodeRun; 1] = [CodeRun {
    text: "// Riteed",
    kind: TokenKind::Comment,
}];
const CODE_MAIN_LINE: [CodeRun; 4] = [
    CodeRun {
        text: "fn",
        kind: TokenKind::Keyword,
    },
    CodeRun {
        text: " ",
        kind: TokenKind::Text,
    },
    CodeRun {
        text: "main",
        kind: TokenKind::Function,
    },
    CodeRun {
        text: "() {",
        kind: TokenKind::Text,
    },
];
const CODE_TITLE_LINE: [CodeRun; 4] = [
    CodeRun {
        text: "let",
        kind: TokenKind::Keyword,
    },
    CodeRun {
        text: " title = ",
        kind: TokenKind::Text,
    },
    CodeRun {
        text: "\"Riteed\"",
        kind: TokenKind::String,
    },
    CodeRun {
        text: ";",
        kind: TokenKind::Text,
    },
];
const CODE_IF_LINE: [CodeRun; 4] = [
    CodeRun {
        text: "if",
        kind: TokenKind::Keyword,
    },
    CodeRun {
        text: " saved == ",
        kind: TokenKind::Text,
    },
    CodeRun {
        text: "true",
        kind: TokenKind::Constant,
    },
    CodeRun {
        text: " {",
        kind: TokenKind::Text,
    },
];
const CODE_OPEN_LINE: [CodeRun; 2] = [
    CodeRun {
        text: "open",
        kind: TokenKind::Function,
    },
    CodeRun {
        text: "(title);",
        kind: TokenKind::Text,
    },
];
const CODE_CLOSE_LINE: [CodeRun; 1] = [CodeRun {
    text: "}",
    kind: TokenKind::Text,
}];
const CODE_LINES: [CodeLine; 7] = [
    CodeLine {
        indent: 0.0,
        runs: &CODE_COMMENT_LINE,
    },
    CodeLine {
        indent: 0.0,
        runs: &CODE_MAIN_LINE,
    },
    CodeLine {
        indent: CODE_INDENT,
        runs: &CODE_TITLE_LINE,
    },
    CodeLine {
        indent: CODE_INDENT,
        runs: &CODE_IF_LINE,
    },
    CodeLine {
        indent: CODE_INDENT * 2.0,
        runs: &CODE_OPEN_LINE,
    },
    CodeLine {
        indent: CODE_INDENT,
        runs: &CODE_CLOSE_LINE,
    },
    CodeLine {
        indent: 0.0,
        runs: &CODE_CLOSE_LINE,
    },
];

impl PalettePreview {
    pub(crate) fn new(scheme: &sourceview5::StyleScheme) -> Self {
        let colors = PreviewColors::from_scheme(scheme);
        let area = gtk4::DrawingArea::new();
        area.set_content_width(PREVIEW_WIDTH);
        area.set_content_height(PREVIEW_HEIGHT);
        area.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        area.set_can_focus(false);
        area.set_can_target(false);
        let draw_colors = colors;
        area.set_draw_func(move |_, context, width, height| {
            draw_preview(context, f64::from(width), f64::from(height), &draw_colors);
        });

        let selected_ring = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        selected_ring.add_css_class("riteed-palette-selected-ring");
        selected_ring.set_halign(gtk4::Align::Fill);
        selected_ring.set_valign(gtk4::Align::Fill);
        selected_ring.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        selected_ring.set_visible(false);
        selected_ring.set_can_focus(false);
        selected_ring.set_can_target(false);

        let selected_badge = gtk4::Image::from_icon_name("object-select-symbolic");
        selected_badge.add_css_class("riteed-palette-selected-badge");
        selected_badge.set_halign(gtk4::Align::End);
        selected_badge.set_valign(gtk4::Align::End);
        selected_badge.set_pixel_size(14);
        selected_badge.set_visible(false);
        selected_badge.set_can_focus(false);
        selected_badge.set_can_target(false);

        let widget = gtk4::Overlay::new();
        widget.add_css_class("riteed-palette-preview");
        widget.set_halign(gtk4::Align::Center);
        widget.set_valign(gtk4::Align::Center);
        widget.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        widget.set_can_focus(false);
        widget.set_can_target(false);
        widget.set_child(Some(&area));
        widget.add_overlay(&selected_ring);
        widget.add_overlay(&selected_badge);

        Self {
            widget,
            selected_ring,
            selected_badge,
        }
    }

    pub(crate) fn widget(&self) -> gtk4::Widget {
        self.widget.clone().upcast::<gtk4::Widget>()
    }

    pub(crate) fn set_selected(&self, selected: bool) {
        if selected {
            self.widget.add_css_class("selected");
        } else {
            self.widget.remove_css_class("selected");
        }
        self.selected_ring.set_visible(selected);
        self.selected_badge.set_visible(selected);
    }

    pub(crate) fn queue_resize(&self) {
        self.widget.queue_resize();
    }
}

impl PreviewColors {
    fn from_scheme(scheme: &sourceview5::StyleScheme) -> Self {
        let dark = scheme
            .metadata("variant")
            .is_some_and(|variant| variant.as_str() == "dark");
        let fallback_background = if dark {
            gdk::RGBA::new(0.11, 0.12, 0.13, 1.0)
        } else {
            gdk::RGBA::WHITE
        };
        let fallback_text = if dark {
            gdk::RGBA::new(0.88, 0.88, 0.86, 1.0)
        } else {
            gdk::RGBA::new(0.16, 0.16, 0.15, 1.0)
        };
        let background = style_color(
            scheme,
            &["text", "view"],
            StyleColorSource::Background,
            &fallback_background,
        );
        let text = style_color(
            scheme,
            &["text", "def:base-n-integer"],
            StyleColorSource::Foreground,
            &fallback_text,
        );
        let keyword = style_color(
            scheme,
            &["def:keyword", "keyword"],
            StyleColorSource::Foreground,
            &text,
        );
        let function = style_color(
            scheme,
            &["def:function", "def:type", "def:identifier"],
            StyleColorSource::Foreground,
            &text,
        );
        let string = style_color(
            scheme,
            &["def:string", "string"],
            StyleColorSource::Foreground,
            &text,
        );
        let constant = style_color(
            scheme,
            &["def:constant", "def:number", "def:base-n-integer"],
            StyleColorSource::Foreground,
            &text,
        );
        let comment = style_color(
            scheme,
            &["def:comment", "comment"],
            StyleColorSource::Foreground,
            &text,
        );
        Self {
            background,
            text,
            keyword,
            function,
            string,
            constant,
            comment,
        }
    }

    fn token_color(&self, kind: TokenKind) -> &gdk::RGBA {
        match kind {
            TokenKind::Text => &self.text,
            TokenKind::Keyword => &self.keyword,
            TokenKind::Function => &self.function,
            TokenKind::String => &self.string,
            TokenKind::Constant => &self.constant,
            TokenKind::Comment => &self.comment,
        }
    }
}

fn draw_preview(context: &cairo::Context, width: f64, height: f64, colors: &PreviewColors) {
    if width <= 1.0 || height <= 1.0 {
        return;
    }
    add_rounded_rectangle(context, 0.5, 0.5, width - 1.0, height - 1.0, PREVIEW_RADIUS);
    set_source_rgba(context, &colors.background);
    if context.fill().is_err() {
        return;
    }
    context.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    context.set_font_size(CODE_FONT_SIZE);
    draw_sample_code(context, colors);
}

fn draw_sample_code(context: &cairo::Context, colors: &PreviewColors) {
    let mut baseline = CODE_TOP_BASELINE;
    for line in &CODE_LINES {
        draw_code_line(context, baseline, line.indent, line.runs, colors);
        baseline += CODE_LINE_HEIGHT;
    }
}

fn draw_code_line(
    context: &cairo::Context,
    baseline: f64,
    indent: f64,
    runs: &[CodeRun],
    colors: &PreviewColors,
) {
    let mut cursor_position = CODE_LEFT_MARGIN + indent;
    for run in runs {
        set_source_rgba(context, colors.token_color(run.kind));
        context.move_to(cursor_position, baseline);
        if context.show_text(run.text).is_err() {
            return;
        }
        if let Ok(extents) = context.text_extents(run.text) {
            cursor_position += extents.x_advance();
        }
    }
}

fn add_rounded_rectangle(
    context: &cairo::Context,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let right = left + width;
    let bottom = top + height;
    context.new_sub_path();
    context.arc(right - radius, top + radius, radius, -FRAC_PI_2, 0.0);
    context.arc(right - radius, bottom - radius, radius, 0.0, FRAC_PI_2);
    context.arc(left + radius, bottom - radius, radius, FRAC_PI_2, PI);
    context.arc(left + radius, top + radius, radius, PI, PI + FRAC_PI_2);
    context.close_path();
}

fn style_color(
    scheme: &sourceview5::StyleScheme,
    style_ids: &[&str],
    source: StyleColorSource,
    fallback: &gdk::RGBA,
) -> gdk::RGBA {
    style_ids
        .iter()
        .find_map(|style_id| {
            let style = scheme.style(style_id)?;
            let color = match source {
                StyleColorSource::Foreground => style.foreground(),
                StyleColorSource::Background => {
                    style.background().or_else(|| style.line_background())
                }
            }?;
            gdk::RGBA::parse(color.as_str()).ok()
        })
        .unwrap_or(*fallback)
}

fn set_source_rgba(context: &cairo::Context, color: &gdk::RGBA) {
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()),
    );
}
