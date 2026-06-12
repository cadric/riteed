use crate::large_file::usize_to_u64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PageText {
    pub(crate) text: String,
    pub(crate) visible_start: u64,
    pub(crate) visible_end: u64,
    pub(crate) next_offset: u64,
}

#[must_use]
pub(crate) fn decode_page_window(offset: u64, bytes: &[u8]) -> PageText {
    let leading_trim = leading_incomplete_len(offset, bytes);
    let trailing_trim = trailing_incomplete_len(&bytes[leading_trim..]);
    let content_end = bytes.len().saturating_sub(trailing_trim).max(leading_trim);
    let content = &bytes[leading_trim..content_end];
    let visible_start = offset.saturating_add(usize_to_u64(leading_trim));
    let visible_end = offset.saturating_add(usize_to_u64(content_end));
    let raw_end = offset.saturating_add(usize_to_u64(bytes.len()));
    let mut next_offset = if trailing_trim > 0 {
        visible_end
    } else {
        raw_end
    };
    if next_offset <= offset && raw_end > offset {
        next_offset = raw_end;
    }
    PageText {
        text: String::from_utf8_lossy(content).to_string(),
        visible_start,
        visible_end,
        next_offset,
    }
}

fn leading_incomplete_len(offset: u64, bytes: &[u8]) -> usize {
    if offset == 0 {
        return 0;
    }
    bytes
        .iter()
        .take_while(|byte| is_continuation_byte(**byte))
        .take(3)
        .count()
}

fn trailing_incomplete_len(bytes: &[u8]) -> usize {
    let Some((lead_index, lead)) = trailing_lead_candidate(bytes) else {
        return 0;
    };
    let Some(expected_len) = utf8_sequence_len(lead) else {
        return 0;
    };
    let available = bytes.len().saturating_sub(lead_index);
    if available < expected_len {
        available
    } else {
        0
    }
}

fn trailing_lead_candidate(bytes: &[u8]) -> Option<(usize, u8)> {
    let mut index = bytes.len().checked_sub(1)?;
    let mut continuation_count = 0_usize;
    while continuation_count < 3 && is_continuation_byte(bytes[index]) {
        continuation_count = continuation_count.saturating_add(1);
        let previous = index.checked_sub(1)?;
        index = previous;
    }
    Some((index, bytes[index]))
}

fn is_continuation_byte(byte: u8) -> bool {
    (0x80..=0xbf).contains(&byte)
}

fn utf8_sequence_len(byte: u8) -> Option<usize> {
    match byte {
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::decode_page_window;

    #[test]
    fn trims_danish_letter_split_at_page_end() {
        let mut bytes = b"aaaaaaaaaa".to_vec();
        bytes.push("å".as_bytes()[0]);

        let decoded = decode_page_window(100, &bytes);

        assert_eq!(decoded.text, "aaaaaaaaaa");
        assert_eq!(decoded.next_offset, 110);
        assert!(!decoded.text.contains('\u{fffd}'));
    }

    #[test]
    fn renders_danish_letter_when_next_page_starts_at_boundary() {
        let decoded = decode_page_window(110, "åb".as_bytes());

        assert_eq!(decoded.text, "åb");
        assert!(!decoded.text.contains('\u{fffd}'));
    }

    #[test]
    fn trims_cjk_and_emoji_splits_without_replacement() {
        let cjk = "界".as_bytes();
        let emoji = "🙂".as_bytes();

        let cjk_decoded = decode_page_window(20, &cjk[..2]);
        let emoji_decoded = decode_page_window(30, &emoji[..3]);

        assert_eq!(cjk_decoded.text, "");
        assert_eq!(emoji_decoded.text, "");
        assert!(!cjk_decoded.text.contains('\u{fffd}'));
        assert!(!emoji_decoded.text.contains('\u{fffd}'));
    }

    #[test]
    fn trims_leading_continuation_bytes_for_mid_codepoint_windows() {
        let bytes = &"øx".as_bytes()[1..];

        let decoded = decode_page_window(41, bytes);

        assert_eq!(decoded.text, "x");
        assert_eq!(decoded.visible_start, 42);
    }

    #[test]
    fn keeps_replacement_for_invalid_file_bytes() {
        let decoded = decode_page_window(0, b"a\xffb");

        assert!(decoded.text.contains('\u{fffd}'));
    }
}
