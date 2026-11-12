//! A deliberately compact baseline: 12 real main-chain operations, not 32.
//! Header choices and iterator-internal helpers are NOT added to that count.
use crate::corpus::{self, Atom, Frame, Slot};
use crate::rng::{accepted_index, Attempt, Attempts, Budgeted, SplitMix64};
use crate::trace::{Trace, TraceConfig};
use crate::{Error, Language, Limits};
use std::cell::{Cell, RefCell};

pub const OPERATIONS: [(&str, &str); 12] = [
    (
        "filter",
        "reject out-of-range random words without advancing a slot",
    ),
    (
        "zip",
        "pair accepted tickets with their ordered grammar slots",
    ),
    ("map", "select the indexed compatible lexical alternative"),
    (
        "flat_map",
        "expand fixed prefix and selected word-level rule",
    ),
    ("chain", "append the grammar suffix and its end boundary"),
    (
        "take_while",
        "consume the end boundary, never emit it as text",
    ),
    (
        "enumerate",
        "assign stable output positions and enforce atom limits",
    ),
    ("map", "resolve words and language-specific punctuation"),
    ("scan", "derive word spacing from prior emitted content"),
    ("flat_map", "expand spacing decisions into text pieces"),
    ("scan", "capitalize only the first English word"),
    ("fold", "assemble the sentence under the byte budget"),
];

#[derive(Debug)]
pub struct Generation {
    pub frame_index: usize,
    pub sentence: String,
    pub draws: usize,
    pub trace: Trace,
}

#[derive(Clone, Debug)]
struct Node {
    atom: Atom,
    // The originating decision draw, not a made-up timestamp or random ID.
    // Fixed suffix words have no decision parent.
    parent_draw: Option<usize>,
}

#[derive(Debug)]
struct Selected<'a> {
    slot: &'a Slot,
    words: &'static [Atom],
    parent_draw: usize,
}

#[derive(Debug)]
struct TextAtom {
    text: String,
    is_word: bool,
    parent_draw: Option<usize>,
}

#[derive(Debug)]
struct Spaced {
    atom: TextAtom,
    leading_space: bool,
}

fn expand(selected: Result<Selected<'_>, Error>) -> Vec<Result<Node, Error>> {
    match selected {
        Err(e) => vec![Err(e)],
        Ok(selected) => selected
            .slot
            .before
            .iter()
            .chain(selected.words)
            .map(|&atom| {
                Ok(Node {
                    atom,
                    parent_draw: Some(selected.parent_draw),
                })
            })
            .collect(),
    }
}

fn pieces(spaced: Result<Spaced, Error>) -> Vec<Result<TextAtom, Error>> {
    match spaced {
        Err(e) => vec![Err(e)],
        Ok(spaced) => {
            let mut result = Vec::with_capacity(2);
            if spaced.leading_space {
                result.push(Ok(TextAtom {
                    text: " ".into(),
                    is_word: false,
                    parent_draw: spaced.atom.parent_draw,
                }));
            }
            result.push(Ok(spaced.atom));
            result
        }
    }
}

fn header<I: Iterator<Item = u64>>(
    rng: &mut Budgeted<I>,
    bound: usize,
    name: &'static str,
    trace: &RefCell<Trace>,
) -> Result<usize, Error> {
    loop {
        let word = rng.word()?;
        let index = accepted_index(word, bound)?;
        trace
            .borrow_mut()
            .record(name, || format!("{word:#018x} / {bound} -> {index:?}"));
        if let Some(index) = index {
            return Ok(index);
        }
    }
}

pub fn generate(seed: u64, language: Language, config: TraceConfig) -> Result<Generation, Error> {
    generate_words(SplitMix64::new(seed), language, config, Limits::default())
}

pub fn generate_words<I: Iterator<Item = u64>>(
    words: I,
    language: Language,
    config: TraceConfig,
    limits: Limits,
) -> Result<Generation, Error> {
    let trace = RefCell::new(Trace::new(config));
    let mut rng = Budgeted::new(words, limits.draws);
    let theme = header(&mut rng, 4, "header.theme", &trace)?;
    let structure = header(&mut rng, 2, "header.structure", &trace)?;
    let frame_index = theme * 2 + structure;
    let frame = &corpus::frames(language)[frame_index];
    let sentence = run_frame(frame, &mut rng, &trace, limits)?;
    Ok(Generation {
        frame_index,
        sentence,
        draws: rng.used,
        trace: trace.into_inner(),
    })
}

/// Also exposed to tests so a non-power-of-two candidate set can exercise real
/// rejection before zip. No production argv or hidden environment hook needed.
// Deliberately retain the requested `fold`: it preserves the first error but
// drains the bounded stream. lazy_semantics.rs contrasts this with try_fold.
#[expect(
    clippy::manual_try_fold,
    reason = "P0 measures the specified fold semantics"
)]
pub fn run_frame<I: Iterator<Item = u64>>(
    frame: &Frame,
    rng: &mut Budgeted<I>,
    trace: &RefCell<Trace>,
    limits: Limits,
) -> Result<String, Error> {
    corpus::validate(frame)?;
    let bounds: Vec<_> = frame.slots.iter().map(|s| s.variants.len()).collect();
    let selected_slots = Cell::new(0);
    let saw_end = Cell::new(false);

    // encourage:display:start
    let sentence = Attempts::new(rng, &bounds)
        .inspect(|attempt| {
            trace
                .borrow_mut()
                .record("source.attempt", || format!("{attempt:?}"))
        })
        // op01: filter
        .filter(|attempt| {
            let keep = match attempt {
                Ok(a) => a.index.is_some(),
                Err(_) => true,
            };
            trace
                .borrow_mut()
                .record(if keep { "01.accepted" } else { "01.rejected" }, || {
                    format!("{attempt:?}")
                });
            keep
        })
        // op02: zip
        .zip(frame.slots.iter())
        .inspect(|(ticket, _)| {
            trace
                .borrow_mut()
                .record("02.paired", || format!("{ticket:?}"))
        })
        // op03: map
        .map(|(ticket, slot)| {
            let Attempt {
                index,
                draw,
                slot: ordinal,
                ..
            } = ticket?;
            if ordinal != selected_slots.get() {
                return Err(Error::InvalidSelection);
            }
            let words = *slot
                .variants
                .get(index.ok_or(Error::InvalidSelection)?)
                .ok_or(Error::InvalidSelection)?;
            selected_slots.set(selected_slots.get() + 1);
            Ok(Selected {
                slot,
                words,
                parent_draw: draw,
            })
        })
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("03.selected", || format!("{value:?}"))
        })
        // op04: flat_map
        .flat_map(expand)
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("04.expanded", || format!("{value:?}"))
        })
        // op05: chain
        .chain(frame.suffix.iter().map(|&atom| {
            Ok(Node {
                atom,
                parent_draw: None,
            })
        }))
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("05.with_suffix", || format!("{value:?}"))
        })
        // op06: take_while
        .take_while(|value| {
            let end = value.as_ref().is_ok_and(|n| n.atom == Atom::End);
            if end {
                saw_end.set(true);
            }
            trace
                .borrow_mut()
                .record(if end { "06.stopped" } else { "06.passed" }, || {
                    format!("{value:?}")
                });
            !end
        })
        // op07: enumerate
        .enumerate()
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("07.positioned", || format!("{value:?}"))
        })
        // op08: map
        .map(|(position, node)| {
            if position >= limits.atoms {
                return Err(Error::AtomBudget);
            }
            let Node { atom, parent_draw } = node?;
            let (text, is_word) = match atom {
                Atom::Word(word) => (word.to_owned(), true),
                Atom::Punctuation(mark) => {
                    (corpus::punctuation(mark, frame.language).to_owned(), false)
                }
                Atom::End => return Err(Error::InvalidSelection),
            };
            Ok(TextAtom {
                text,
                is_word,
                parent_draw,
            })
        })
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("08.localized", || format!("{value:?}"))
        })
        // op09: scan
        .scan(false, |has_content, atom| {
            Some(atom.map(|atom| {
                let leading_space = frame.language == Language::En && atom.is_word && *has_content;
                *has_content = true;
                Spaced {
                    atom,
                    leading_space,
                }
            }))
        })
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("09.spacing", || format!("{value:?}"))
        })
        // op10: flat_map
        .flat_map(pieces)
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("10.pieces", || format!("{value:?}"))
        })
        // op11: scan
        .scan(false, |seen_word, atom| {
            Some(atom.map(|mut atom| {
                if atom.is_word && !*seen_word {
                    if frame.language == Language::En {
                        atom.text[..1].make_ascii_uppercase();
                    }
                    *seen_word = true;
                }
                atom
            }))
        })
        .inspect(|value| {
            trace
                .borrow_mut()
                .record("11.capitalized", || format!("{value:?}"))
        })
        // op12: fold
        .fold(Ok(String::new()), |result, atom| {
            let mut sentence = result?;
            let atom = atom?;
            if sentence.len() + atom.text.len() > limits.bytes {
                return Err(Error::ByteBudget);
            }
            sentence.push_str(&atom.text);
            trace
                .borrow_mut()
                .record("12.accumulated", || sentence.clone());
            Ok(sentence)
        });
    // encourage:display:end

    let sentence = sentence?;
    if selected_slots.get() != frame.slots.len() {
        return Err(Error::InvalidSelection);
    }
    if !saw_end.get() {
        return Err(Error::MissingEnd);
    }
    Ok(sentence)
}

/// Independent imperative oracle; shares data and the RNG definition, not the
/// rendering pipeline. Keep this simple, rather than making it another chain.
pub fn reference(seed: u64, language: Language) -> Result<(usize, String, usize), Error> {
    let mut rng = Budgeted::new(SplitMix64::new(seed), Limits::default().draws);
    let frame_index = rng.choose(4)? * 2 + rng.choose(2)?;
    let frame = &corpus::frames(language)[frame_index];
    let mut choices = Vec::new();
    for slot in frame.slots {
        choices.push(rng.choose(slot.variants.len())?);
    }
    Ok((frame_index, render_choices(frame, &choices)?, rng.used))
}

pub fn render_choices(frame: &Frame, choices: &[usize]) -> Result<String, Error> {
    corpus::validate(frame)?;
    if choices.len() != frame.slots.len() {
        return Err(Error::InvalidSelection);
    }
    let mut atoms = Vec::new();
    for (slot, &index) in frame.slots.iter().zip(choices) {
        atoms.extend_from_slice(slot.before);
        atoms.extend_from_slice(slot.variants.get(index).ok_or(Error::InvalidSelection)?);
    }
    atoms.extend_from_slice(frame.suffix);
    let mut text = String::new();
    let mut first_word = true;
    for atom in atoms {
        match atom {
            Atom::Word(word) => {
                if frame.language == Language::En && !text.is_empty() {
                    text.push(' ');
                }
                if first_word && frame.language == Language::En {
                    let mut letters = word.chars();
                    text.push(
                        letters
                            .next()
                            .ok_or(Error::InvalidSelection)?
                            .to_ascii_uppercase(),
                    );
                    text.extend(letters);
                } else {
                    text.push_str(word);
                }
                first_word = false;
            }
            Atom::Punctuation(mark) => text.push_str(corpus::punctuation(mark, frame.language)),
            Atom::End => break,
        }
    }
    Ok(text)
}

pub fn choice_vectors(frame: &Frame) -> Vec<Vec<usize>> {
    let mut choices = vec![vec![]];
    for slot in frame.slots {
        let mut next = Vec::new();
        for prefix in choices {
            for index in 0..slot.variants.len() {
                let mut branch = prefix.clone();
                branch.push(index);
                next.push(branch);
            }
        }
        choices = next;
    }
    choices
}
