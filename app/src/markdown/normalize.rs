use std::borrow::Cow;

pub(crate) fn parser_input(body: &str) -> Cow<'_, str> {
    if body.chars().any(markdown_control_needs_normalization) {
        Cow::Owned(body.chars().map(normalize_markdown_control).collect())
    } else {
        Cow::Borrowed(body)
    }
}

fn normalize_markdown_control(character: char) -> char {
    match character {
        '\r' => '\n',
        character if markdown_control_needs_normalization(character) => ' ',
        character => character,
    }
}

fn markdown_control_needs_normalization(character: char) -> bool {
    character.is_ascii_control() && !matches!(character, '\n' | '\t')
}
