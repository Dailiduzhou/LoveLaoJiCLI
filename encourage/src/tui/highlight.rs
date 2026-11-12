//! A deliberately small lexer for the fixed embedded pipeline excerpt.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    enabled: bool,
}

impl Palette {
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn style(self, color: Color) -> Style {
        if self.enabled {
            Style::default().fg(color)
        } else {
            Style::default()
        }
    }

    pub(crate) fn keyword(self) -> Style {
        self.style(Color::Magenta)
    }
    pub(crate) fn function(self) -> Style {
        self.style(Color::Cyan)
    }
    pub(crate) fn known_type(self) -> Style {
        self.style(Color::Yellow)
    }
    pub(crate) fn binding(self) -> Style {
        self.style(Color::Green)
    }
    pub(crate) fn string(self) -> Style {
        self.style(Color::LightGreen)
    }
    pub(crate) fn number(self) -> Style {
        self.style(Color::LightYellow)
    }
    pub(crate) fn punctuation(self) -> Style {
        self.style(Color::Gray)
    }
    pub(crate) fn comment(self) -> Style {
        self.style(Color::DarkGray)
    }
    pub(crate) fn plain(self) -> Style {
        Style::default()
    }
}

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while",
];
const TYPES: &[&str] = &[
    "u8",
    "u16",
    "u32",
    "u64",
    "usize",
    "i32",
    "bool",
    "char",
    "str",
    "String",
    "Option",
    "Result",
    "Vec",
    "Generation",
    "GenerationError",
    "Language",
    "Trace",
];
const BINDINGS: &[&str] = &[
    "seed", "language", "trace", "config", "frame", "slot", "atom", "item", "value", "state",
    "acc", "result", "index", "position", "parent", "node", "word",
];

pub(crate) fn line(source: &str, palette: Palette) -> Line<'static> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut cursor = 0;

    while cursor < bytes.len() {
        let (end, style) = if source[cursor..].starts_with("//") {
            (bytes.len(), palette.comment())
        } else if bytes[cursor] == b'"' || is_char_literal(source, cursor) {
            (quoted_end(source, cursor), palette.string())
        } else if is_ident_start(bytes[cursor]) {
            let end = ident_end(bytes, cursor);
            let word = &source[cursor..end];
            let next = source[end..].trim_start().as_bytes().first();
            let style = if KEYWORDS.contains(&word) {
                palette.keyword()
            } else if TYPES.contains(&word) || word.chars().next().is_some_and(char::is_uppercase) {
                palette.known_type()
            } else if next == Some(&b'(') {
                palette.function()
            } else if BINDINGS.contains(&word) {
                palette.binding()
            } else {
                palette.plain()
            };
            (end, style)
        } else if bytes[cursor].is_ascii_digit() {
            (number_end(bytes, cursor), palette.number())
        } else if is_punctuation(bytes[cursor]) {
            (cursor + 1, palette.punctuation())
        } else {
            cursor += char_len(bytes[cursor]);
            continue;
        };

        if plain_start < cursor {
            spans.push(Span::styled(
                source[plain_start..cursor].to_owned(),
                palette.plain(),
            ));
        }
        spans.push(Span::styled(source[cursor..end].to_owned(), style));
        cursor = end;
        plain_start = end;
    }
    if plain_start < source.len() {
        spans.push(Span::styled(
            source[plain_start..].to_owned(),
            palette.plain(),
        ));
    }
    Line::from(spans)
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn ident_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start + 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    end
}

fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start + 1;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'.'))
    {
        end += 1;
    }
    end
}

fn is_punctuation(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')'
            | b'{'
            | b'}'
            | b'['
            | b']'
            | b'.'
            | b','
            | b':'
            | b';'
            | b'|'
            | b'&'
            | b'='
            | b'>'
            | b'<'
            | b'!'
            | b'?'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
    )
}

fn is_char_literal(source: &str, start: usize) -> bool {
    let bytes = source.as_bytes();
    if bytes[start] != b'\'' {
        return false;
    }
    // Lifetimes such as `'a` and `'static` have no closing quote.
    let Some(relative_end) = source[start + 1..].find('\'') else {
        return false;
    };
    let end = start + 1 + relative_end;
    end <= start + 5 || source[start + 1..].starts_with('\\')
}

fn quoted_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let quote = bytes[start];
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[cursor] == b'\\' {
            escaped = true;
        } else if bytes[cursor] == quote {
            return cursor + 1;
        }
        cursor += 1;
    }
    bytes.len()
}

fn char_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first < 0xE0 {
        2
    } else if first < 0xF0 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(source: &str) -> Vec<(String, Option<Color>)> {
        line(source, Palette::new(true))
            .spans
            .into_iter()
            .map(|span| (span.content.into_owned(), span.style.fg))
            .collect()
    }

    #[test]
    fn distinguishes_fixed_rust_tokens_and_escaped_strings() {
        let tokens = spans("let seed = 0x2a_u64; // comment\n")
            .into_iter()
            .filter(|(text, _)| !text.trim().is_empty())
            .collect::<Vec<_>>();
        assert!(tokens.contains(&("let".into(), Some(Color::Magenta))));
        assert!(tokens.contains(&("seed".into(), Some(Color::Green))));
        assert!(tokens.contains(&("0x2a_u64".into(), Some(Color::LightYellow))));
        assert!(tokens.iter().any(|(text, color)| {
            text.trim_end() == "// comment" && *color == Some(Color::DarkGray)
        }));

        let tokens = spans("map(|value| format!(\"value: \\\"{}\\\"\", value))");
        assert!(tokens.iter().any(|(text, color)| {
            text == "\"value: \\\"{}\\\"\"" && *color == Some(Color::LightGreen)
        }));
        assert!(tokens.contains(&("map".into(), Some(Color::Cyan))));
        assert!(tokens.contains(&("value".into(), Some(Color::Green))));
    }

    #[test]
    fn lifetimes_are_not_char_strings_and_no_color_preserves_text() {
        let source = "fn next<'a>(value: &'a str) -> &'a str { value }";
        assert!(!spans(source)
            .iter()
            .any(|(text, color)| text == "'a" && *color == Some(Color::LightGreen)));
        let colored = line(source, Palette::new(true));
        let plain = line(source, Palette::new(false));
        assert_eq!(colored.to_string(), source);
        assert_eq!(plain.to_string(), source);
        assert!(plain.spans.iter().all(|span| span.style.fg.is_none()));
    }
}
