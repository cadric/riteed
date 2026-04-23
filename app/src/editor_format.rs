use gettextrs::pgettext;
use sourceview5::{Encoding, NewlineType};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodingInfo {
    charset: String,
    display_name: String,
    is_utf8: bool,
}

impl EncodingInfo {
    #[must_use]
    pub fn utf8() -> Self {
        Self {
            charset: String::from("UTF-8"),
            display_name: String::from("Unicode (UTF-8)"),
            is_utf8: true,
        }
    }

    #[must_use]
    pub fn from_encoding(encoding: &Encoding) -> Self {
        Self {
            charset: encoding.charset().to_string(),
            display_name: encoding.to_str().to_string(),
            is_utf8: encoding.charset().eq_ignore_ascii_case("utf-8")
                || encoding.charset().eq_ignore_ascii_case("utf8"),
        }
    }

    #[must_use]
    pub fn charset(&self) -> &str {
        &self.charset
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub const fn is_utf8(&self) -> bool {
        self.is_utf8
    }

    #[must_use]
    pub fn to_source_encoding(&self) -> Option<Encoding> {
        Encoding::from_charset(&self.charset)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEndingMode {
    Lf,
    CrLf,
    Cr,
}

impl LineEndingMode {
    #[must_use]
    pub const fn from_source(newline_type: NewlineType) -> Self {
        match newline_type {
            NewlineType::CrLf => Self::CrLf,
            NewlineType::Cr => Self::Cr,
            _ => Self::Lf,
        }
    }

    #[must_use]
    pub const fn into_source(self) -> NewlineType {
        match self {
            Self::Lf => NewlineType::Lf,
            Self::CrLf => NewlineType::CrLf,
            Self::Cr => NewlineType::Cr,
        }
    }

    #[must_use]
    pub fn short_label(self) -> String {
        match self {
            Self::Lf => pgettext("line ending", "LF"),
            Self::CrLf => pgettext("line ending", "CRLF"),
            Self::Cr => pgettext("line ending", "CR"),
        }
    }

    #[must_use]
    pub fn menu_label(self) -> String {
        match self {
            Self::Lf => pgettext("line ending menu", "Unix (LF)"),
            Self::CrLf => pgettext("line ending menu", "Windows (CRLF)"),
            Self::Cr => pgettext("line ending menu", "Classic Mac (CR)"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedTextFormat {
    line_ending_mode: LineEndingMode,
    encoding: EncodingInfo,
    implicit_trailing_newline: bool,
}

impl SavedTextFormat {
    #[must_use]
    pub fn new_document_defaults() -> Self {
        Self {
            line_ending_mode: LineEndingMode::Lf,
            encoding: EncodingInfo::utf8(),
            implicit_trailing_newline: true,
        }
    }

    #[must_use]
    pub const fn new(
        line_ending_mode: LineEndingMode,
        encoding: EncodingInfo,
        implicit_trailing_newline: bool,
    ) -> Self {
        Self {
            line_ending_mode,
            encoding,
            implicit_trailing_newline,
        }
    }

    #[must_use]
    pub const fn line_ending_mode(&self) -> LineEndingMode {
        self.line_ending_mode
    }

    #[must_use]
    pub fn encoding(&self) -> &EncodingInfo {
        &self.encoding
    }

    #[must_use]
    pub const fn implicit_trailing_newline(&self) -> bool {
        self.implicit_trailing_newline
    }

    pub fn set_line_ending_mode(&mut self, line_ending_mode: LineEndingMode) {
        self.line_ending_mode = line_ending_mode;
    }

    pub fn set_encoding(&mut self, encoding: EncodingInfo) {
        self.encoding = encoding;
    }

    pub fn set_implicit_trailing_newline(&mut self, implicit_trailing_newline: bool) {
        self.implicit_trailing_newline = implicit_trailing_newline;
    }

    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} · {}",
            self.encoding.charset(),
            self.line_ending_mode.short_label()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedTextFormat {
    pub text: String,
    pub format: SavedTextFormat,
}

impl LoadedTextFormat {
    #[must_use]
    pub fn from_disk_text(
        mut text: String,
        line_ending_mode: LineEndingMode,
        encoding: EncodingInfo,
    ) -> Self {
        let trailing_newlines = text
            .as_bytes()
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\n')
            .count();
        let implicit_trailing_newline = if trailing_newlines == 1 {
            let _removed = text.pop();
            true
        } else {
            false
        };
        Self {
            text,
            format: SavedTextFormat::new(line_ending_mode, encoding, implicit_trailing_newline),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EncodingInfo, LineEndingMode, LoadedTextFormat, SavedTextFormat};

    #[test]
    fn new_document_defaults_to_utf8_lf_with_implicit_newline() {
        let format = SavedTextFormat::new_document_defaults();
        assert_eq!(format.line_ending_mode(), LineEndingMode::Lf);
        assert!(format.encoding().is_utf8());
        assert!(format.implicit_trailing_newline());
    }

    #[test]
    fn one_trailing_newline_becomes_implicit() {
        let loaded = LoadedTextFormat::from_disk_text(
            String::from("alpha\n"),
            LineEndingMode::Lf,
            EncodingInfo::utf8(),
        );
        assert_eq!(loaded.text, "alpha");
        assert!(loaded.format.implicit_trailing_newline());
    }

    #[test]
    fn two_trailing_newlines_stay_explicit() {
        let loaded = LoadedTextFormat::from_disk_text(
            String::from("alpha\n\n"),
            LineEndingMode::Lf,
            EncodingInfo::utf8(),
        );
        assert_eq!(loaded.text, "alpha\n\n");
        assert!(!loaded.format.implicit_trailing_newline());
    }
}
