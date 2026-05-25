use std::borrow::Cow;

pub(crate) fn parser_input(body: &str) -> Cow<'_, str> {
    if body.chars().any(markdown_character_needs_normalization) {
        Cow::Owned(body.chars().map(normalize_markdown_character).collect())
    } else {
        Cow::Borrowed(body)
    }
}

fn normalize_markdown_character(character: char) -> char {
    match character {
        '\r' => '\n',
        std::char::REPLACEMENT_CHARACTER => ' ',
        character if markdown_character_needs_normalization(character) => ' ',
        character => character,
    }
}

fn markdown_character_needs_normalization(character: char) -> bool {
    character == std::char::REPLACEMENT_CHARACTER
        || character.is_ascii_control() && !matches!(character, '\n' | '\t')
}
