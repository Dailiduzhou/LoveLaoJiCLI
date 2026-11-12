//! Typed, bounded text observations. Never parse Debug strings or borrow words
//! from a future stage to fill a missing observation.

use crate::{
    corpus::{self, Atom},
    Language,
};

pub const MAX_MATERIAL_PIECES: usize = 256;
pub const MAX_MATERIAL_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Assembly {
    /// Alternatives/selectors do not yet form one sentence.
    #[default]
    None,
    /// A real ordered word/punctuation stream, with locale-specific joining.
    Words,
    /// Spacing decisions or actual emitted text pieces already supply spaces.
    Exact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub is_word: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Material {
    pieces: Vec<Piece>,
    bytes: usize,
    omitted: usize,
    assembly: Assembly,
}

impl Material {
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn omitted(&self) -> usize {
        self.omitted
    }

    pub fn push(&mut self, text: &str, is_word: bool, assembly: Assembly) {
        self.assembly = assembly;
        if text.is_empty() {
            return;
        }
        // Keep a prefix, never silently skip a large piece and append its tail.
        if self.omitted != 0
            || self.pieces.len() >= MAX_MATERIAL_PIECES
            || self.bytes + text.len() > MAX_MATERIAL_BYTES
        {
            self.omitted += 1;
            return;
        }
        self.bytes += text.len();
        self.pieces.push(Piece {
            text: text.to_owned(),
            is_word,
        });
    }

    pub fn atom(&mut self, atom: Atom, language: Language, assembly: Assembly) {
        match atom {
            Atom::Word(word) => self.push(word, true, assembly),
            Atom::Punctuation(mark) => {
                self.push(corpus::punctuation(mark, language), false, assembly)
            }
            // The end boundary has no text; do not invent a visible token.
            Atom::End => {}
        }
    }

    /// Fold's output is one scalar, not a list of every accumulator prefix.
    pub fn accumulated(&mut self, text: &str) {
        *self = Self::default();
        self.push(text, false, Assembly::Exact);
    }

    pub fn raw(&self) -> String {
        self.pieces
            .iter()
            .map(|piece| piece.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Only selected ordered material can be joined. No final-sentence string is
    /// accepted here, so an early preview cannot accidentally reveal the result.
    pub fn preview(&self, language: Language) -> String {
        if self.assembly == Assembly::None {
            return String::new();
        }
        let mut text = String::new();
        for piece in &self.pieces {
            if self.assembly == Assembly::Words
                && language == Language::English
                && piece.is_word
                && !text.is_empty()
            {
                text.push(' ');
            }
            text.push_str(&piece.text);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_text_keeps_duplicates_and_never_renders_types_or_end_boundaries() {
        let mut material = Material::default();
        for atom in [
            Atom::Word("try"),
            Atom::Word("try"),
            Atom::Punctuation(corpus::Mark::Period),
            Atom::End,
        ] {
            material.atom(atom, Language::English, Assembly::Words);
        }
        assert_eq!(material.raw(), "try try .");
        assert_eq!(material.preview(Language::English), "try try.");
        let mut alternatives = Material::default();
        alternatives.push("try", true, Assembly::None);
        assert_eq!(alternatives.preview(Language::English), "");
        alternatives.accumulated("Try.");
        assert_eq!(alternatives.raw(), "Try.");
    }

    #[test]
    fn storage_is_bounded_and_omission_counts_are_explicit() {
        let mut material = Material::default();
        for _ in 0..MAX_MATERIAL_PIECES + 5 {
            material.push("饭", true, Assembly::Words);
        }
        assert_eq!(material.pieces().len(), MAX_MATERIAL_PIECES);
        assert_eq!(material.omitted(), 5);
        let mut oversized = Material::default();
        oversized.push(&"x".repeat(MAX_MATERIAL_BYTES + 1), true, Assembly::Words);
        oversized.push("tail", true, Assembly::Words);
        assert!(oversized.raw().is_empty());
        assert_eq!(oversized.omitted(), 2);
    }
}
