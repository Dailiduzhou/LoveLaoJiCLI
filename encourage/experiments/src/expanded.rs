//! Third experiment: 34 executed operations = 28 content + 6 explicitly
//! reversible presentation transitions, followed by real lexical insert/delete.
//! The original 12-operation pipeline remains an unedited reference.
use crate::corpus::{self, Atom, Frame, Slot};
use crate::pipeline::Generation;
use crate::revision::{self, Rule, Site};
use crate::rng::{accepted_index, Budgeted, SplitMix64};
use crate::trace::{Trace, TraceConfig};
use crate::{Error, Language, Limits};
use std::cell::{Cell, RefCell};

pub const OPERATIONS: [(&str, &str); 34] = [
    ("map", "derive the theme from the first random word"),
    ("map", "draw the structure within the chosen theme"),
    ("flat_map", "open the bilingual corpus catalogues"),
    ("filter", "select the requested language catalogue"),
    ("flat_map", "open the four theme groups"),
    ("filter", "select the sampled theme"),
    ("flat_map", "open the theme's two structures"),
    ("filter", "select the sampled structure"),
    ("flat_map", "expand the selected frame into grammar slots"),
    (
        "enumerate",
        "assign slot ordinals used for candidate ownership",
    ),
    (
        "flat_map",
        "query the frame's candidate bank for each requested slot",
    ),
    (
        "filter",
        "retain only candidates owned by that grammar slot",
    ),
    (
        "scan",
        "group compatible candidates until the slot boundary",
    ),
    (
        "flatten",
        "emit completed candidate groups, not partial groups",
    ),
    ("zip", "pair each complete slot with its first random word"),
    (
        "map",
        "sample within the compatible group, retrying in the same slot",
    ),
    (
        "flat_map",
        "emit fixed-prefix and chosen-rule fragments with distinct origins",
    ),
    ("chain", "append the selected frame's fixed suffix"),
    (
        "zip",
        "TRANSITION: pair each fragment with a display ordinal",
    ),
    (
        "map",
        "TRANSITION: discard the ordinal and restore the fragment",
    ),
    (
        "map",
        "TRANSITION: wrap each fragment in a singleton iterator",
    ),
    (
        "flatten",
        "TRANSITION: unwrap the singleton without losing errors",
    ),
    (
        "map",
        "TRANSITION: Fisher-Yates shuffle with independent fixed seed",
    ),
    (
        "map",
        "TRANSITION: restore atoms by their original positions",
    ),
    (
        "flat_map",
        "expand restored fragments into word and boundary atoms",
    ),
    (
        "flat_map",
        "insert actual editorial words before their original anchor",
    ),
    (
        "filter",
        "remove the original anchored word, never the inserted words",
    ),
    (
        "take_while",
        "consume the sentence-end boundary without rendering it",
    ),
    ("enumerate", "assign output positions for the atom budget"),
    ("map", "resolve words and locale-specific punctuation"),
    ("scan", "derive word spacing from prior emitted content"),
    ("flat_map", "expand spacing decisions into text pieces"),
    ("scan", "capitalize the first English word only"),
    ("fold", "assemble the final sentence under the byte budget"),
];

pub const TRANSITION_STAGES: [usize; 6] = [19, 20, 21, 22, 23, 24];
pub const CONTENT_OPERATION_COUNT: usize = OPERATIONS.len() - TRANSITION_STAGES.len();
/// Presentation only: never advance the RNG which chooses themes or words.
pub const SHUFFLE_SEED: u64 = 2;

#[derive(Clone, Copy, Debug)]
struct Word {
    value: u64,
    draw: usize,
}

#[derive(Clone, Copy, Debug)]
struct Choice {
    theme: usize,
    structure: usize,
}

#[derive(Clone, Copy, Debug)]
struct Catalogue {
    choice: Choice,
    language: Language,
}

#[derive(Clone, Copy, Debug)]
struct ThemeRow {
    catalogue: Catalogue,
    theme: usize,
}

#[derive(Clone, Copy)]
struct FrameRow {
    choice: Choice,
    index: usize,
    frame: &'static Frame,
}

impl std::fmt::Debug for FrameRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:?} index={} choice={:?}",
            self.frame.id, self.frame.language, self.index, self.choice
        )
    }
}

#[derive(Clone, Copy)]
struct Request {
    frame: &'static Frame,
    slot: &'static Slot,
}

#[derive(Clone, Copy)]
struct Candidate {
    owner: usize,
    rank: usize,
    words: &'static [Atom],
}

#[derive(Clone, Copy)]
struct Row {
    request: Request,
    ordinal: usize,
    candidate: Option<Candidate>,
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} slot={} ", self.request.frame.id, self.ordinal)?;
        match self.candidate {
            Some(c) => write!(f, "owner={} rank={} {:?}", c.owner, c.rank, c.words),
            None => write!(f, "slot-boundary"),
        }
    }
}

struct Group {
    request: Request,
    ordinal: usize,
    candidates: Vec<&'static [Atom]>,
}

#[derive(Default)]
struct Pending {
    ordinal: Option<usize>,
    candidates: Vec<&'static [Atom]>,
}

impl Pending {
    fn push(&mut self, row: Result<Row, Error>) -> Option<Result<Group, Error>> {
        let row = match row {
            Ok(row) => row,
            Err(error) => {
                *self = Self::default();
                return Some(Err(error));
            }
        };
        if self.ordinal.is_some_and(|ordinal| ordinal != row.ordinal) {
            *self = Self::default();
            return Some(Err(Error::InvalidSelection));
        }
        if let Some(candidate) = row.candidate {
            if candidate.owner != row.ordinal || candidate.rank != self.candidates.len() {
                *self = Self::default();
                return Some(Err(Error::InvalidSelection));
            }
            self.ordinal = Some(row.ordinal);
            self.candidates.push(candidate.words);
            None
        } else {
            self.ordinal = None;
            let candidates = std::mem::take(&mut self.candidates);
            if candidates.is_empty() {
                return Some(Err(Error::EmptyCandidates));
            }
            if candidates.len() != row.request.slot.variants.len() {
                return Some(Err(Error::InvalidSelection));
            }
            Some(Ok(Group {
                request: row.request,
                ordinal: row.ordinal,
                candidates,
            }))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Fixed { slot: usize },
    Choice { slot: usize, draw: usize },
    Suffix,
    Inserted { rule: &'static str, site: Site },
}

impl Origin {
    fn site(self) -> Option<Site> {
        match self {
            Self::Fixed { slot } => Some(Site::Fixed(slot)),
            Self::Suffix => Some(Site::Suffix),
            Self::Choice { .. } | Self::Inserted { .. } => None,
        }
    }
}

#[derive(Debug)]
struct Fragment {
    atoms: &'static [Atom],
    origin: Origin,
}

#[derive(Debug)]
struct Shuffled {
    items: Vec<(usize, Atom)>,
    origin: Origin,
    seed: u64,
}

#[derive(Debug)]
struct Restored {
    atoms: Vec<Atom>,
    origin: Origin,
}

struct Selected {
    group: Group,
    words: &'static [Atom],
    draw: usize,
}

#[derive(Debug)]
struct Node {
    atom: Atom,
    origin: Origin,
}

#[derive(Debug)]
struct TextAtom {
    text: String,
    is_word: bool,
    origin: Origin,
}

#[derive(Debug)]
struct Spaced {
    atom: TextAtom,
    leading_space: bool,
}

fn record(trace: &RefCell<Trace>, stage: &'static str, value: impl FnOnce() -> String) {
    trace.borrow_mut().record(stage, value);
}

fn read_word<I: Iterator<Item = u64>>(rng: &RefCell<Budgeted<I>>) -> Result<Word, Error> {
    let mut rng = rng.borrow_mut();
    let value = rng.word()?;
    Ok(Word {
        value,
        draw: rng.used - 1,
    })
}

/// Retry consumes words for THIS group, never another item from the slot stream.
fn choose<I: Iterator<Item = u64>>(
    first: Result<Word, Error>,
    bound: usize,
    rng: &RefCell<Budgeted<I>>,
    trace: &RefCell<Trace>,
    stage: &'static str,
) -> Result<(usize, usize), Error> {
    let mut word = first?;
    loop {
        let index = accepted_index(word.value, bound)?;
        record(trace, stage, || {
            format!(
                "draw={} word={:#018x} bound={bound} -> {index:?}",
                word.draw, word.value
            )
        });
        if let Some(index) = index {
            return Ok((index, word.draw));
        }
        record(trace, "random.rejected", || {
            format!("draw={} bound={bound}", word.draw)
        });
        word = read_word(rng)?;
    }
}

fn catalogues(choice: Result<Choice, Error>) -> Vec<Result<Catalogue, Error>> {
    match choice {
        Ok(choice) => Language::ALL
            .into_iter()
            .map(|language| Ok(Catalogue { choice, language }))
            .collect(),
        Err(error) => vec![Err(error)],
    }
}

fn themes(catalogue: Result<Catalogue, Error>) -> Vec<Result<ThemeRow, Error>> {
    match catalogue {
        Ok(catalogue) => (0..4)
            .map(|theme| Ok(ThemeRow { catalogue, theme }))
            .collect(),
        Err(error) => vec![Err(error)],
    }
}

fn frames(theme: Result<ThemeRow, Error>) -> Vec<Result<FrameRow, Error>> {
    match theme {
        Ok(theme) => (0..2)
            .map(|structure| {
                let index = theme.theme * 2 + structure;
                Ok(FrameRow {
                    choice: theme.catalogue.choice,
                    index,
                    frame: &corpus::frames(theme.catalogue.language)[index],
                })
            })
            .collect(),
        Err(error) => vec![Err(error)],
    }
}

/// A frame-local bank contains real alternatives for BOTH slots, not fabricated
/// noise. Ownership filtering below must retain the correct role before drawing.
fn candidate_rows((ordinal, request): (usize, Result<Request, Error>)) -> Vec<Result<Row, Error>> {
    match request {
        Err(error) => vec![Err(error)],
        Ok(request) => {
            let mut rows = Vec::new();
            for (owner, slot) in request.frame.slots.iter().enumerate() {
                for (rank, &words) in slot.variants.iter().enumerate() {
                    rows.push(Ok(Row {
                        request,
                        ordinal,
                        candidate: Some(Candidate { owner, rank, words }),
                    }));
                }
            }
            rows.push(Ok(Row {
                request,
                ordinal,
                candidate: None,
            }));
            rows
        }
    }
}

fn fragments(selected: Result<Selected, Error>) -> Vec<Result<Fragment, Error>> {
    match selected {
        Err(error) => vec![Err(error)],
        Ok(selected) => vec![
            Ok(Fragment {
                atoms: selected.group.request.slot.before,
                origin: Origin::Fixed {
                    slot: selected.group.ordinal,
                },
            }),
            Ok(Fragment {
                atoms: selected.words,
                origin: Origin::Choice {
                    slot: selected.group.ordinal,
                    draw: selected.draw,
                },
            }),
        ],
    }
}

fn shuffle_fragment(fragment: Result<Fragment, Error>) -> Result<Shuffled, Error> {
    let fragment = fragment?;
    let mut items: Vec<_> = fragment.atoms.iter().copied().enumerate().collect();
    let mut local = Budgeted::new(SplitMix64::new(SHUFFLE_SEED), 4096);
    for end in (1..items.len()).rev() {
        let other = local.choose(end + 1)?;
        items.swap(end, other);
    }
    Ok(Shuffled {
        items,
        origin: fragment.origin,
        seed: SHUFFLE_SEED,
    })
}

fn restore_fragment(shuffled: Result<Shuffled, Error>) -> Result<Restored, Error> {
    let mut shuffled = shuffled?;
    if shuffled.seed != SHUFFLE_SEED {
        return Err(Error::InvalidSelection);
    }
    shuffled
        .items
        .sort_by_key(|(original_position, _)| *original_position);
    Ok(Restored {
        atoms: shuffled.items.into_iter().map(|(_, atom)| atom).collect(),
        origin: shuffled.origin,
    })
}

fn atoms(fragment: Result<Restored, Error>) -> Vec<Result<Node, Error>> {
    match fragment {
        Err(error) => vec![Err(error)],
        Ok(fragment) => fragment
            .atoms
            .into_iter()
            .map(|atom| {
                Ok(Node {
                    atom,
                    origin: fragment.origin,
                })
            })
            .collect(),
    }
}

fn insert_words(
    node: Result<Node, Error>,
    rule: Option<&Rule>,
    insertions: &Cell<usize>,
) -> Vec<Result<Node, Error>> {
    let node = match node {
        Ok(node) => node,
        Err(error) => return vec![Err(error)],
    };
    let Some(rule) = rule else {
        return vec![Err(Error::InvalidSelection)];
    };
    let mut output = Vec::new();
    if rule.matches(node.origin.site(), node.atom) {
        insertions.set(insertions.get() + 1);
        for &atom in rule.insert {
            output.push(Ok(Node {
                atom,
                origin: Origin::Inserted {
                    rule: rule.id,
                    site: rule.site,
                },
            }));
        }
    }
    // Keep the original here so the subsequent filter performs a real deletion.
    output.push(Ok(node));
    output
}

fn text_pieces(spaced: Result<Spaced, Error>) -> Vec<Result<TextAtom, Error>> {
    match spaced {
        Err(error) => vec![Err(error)],
        Ok(spaced) => {
            let mut pieces = Vec::with_capacity(2);
            if spaced.leading_space {
                pieces.push(Ok(TextAtom {
                    text: " ".into(),
                    is_word: false,
                    origin: spaced.atom.origin,
                }));
            }
            pieces.push(Ok(spaced.atom));
            pieces
        }
    }
}

pub fn generate(seed: u64, language: Language, config: TraceConfig) -> Result<Generation, Error> {
    generate_words(SplitMix64::new(seed), language, config, Limits::default())
}

#[expect(
    clippy::manual_try_fold,
    reason = "retain the specified bounded fold experiment"
)]
pub fn generate_words<I: Iterator<Item = u64>>(
    words: I,
    language: Language,
    config: TraceConfig,
    limits: Limits,
) -> Result<Generation, Error> {
    let rng = RefCell::new(Budgeted::new(words, limits.draws));
    let trace = RefCell::new(Trace::new(config));
    let chosen_frame = Cell::new(None::<FrameRow>);
    let selected_slots = Cell::new(0);
    let saw_end = Cell::new(false);
    let insertions = Cell::new(0);
    let deletions = Cell::new(0);

    // encourage:display:start
    let sentence = std::iter::once_with(|| read_word(&rng))
        // op01: map
        .map(|first| choose(first, 4, &rng, &trace, "01.theme").map(|(theme, _)| theme))
        // op02: map
        .map(|theme| {
            let theme = theme?;
            let (structure, _) = choose(read_word(&rng), 2, &rng, &trace, "02.structure")?;
            Ok(Choice { theme, structure })
        })
        // op03: flat_map
        .flat_map(catalogues)
        .inspect(|row| record(&trace, "03.catalogues", || format!("{row:?}")))
        // op04: filter
        .filter(|row| {
            let keep = row.as_ref().map_or(true, |row| row.language == language);
            record(&trace, if keep { "04.kept" } else { "04.rejected" }, || {
                format!("{row:?}")
            });
            keep
        })
        // op05: flat_map
        .flat_map(themes)
        .inspect(|row| record(&trace, "05.themes", || format!("{row:?}")))
        // op06: filter
        .filter(|row| {
            let keep = row
                .as_ref()
                .map_or(true, |row| row.theme == row.catalogue.choice.theme);
            record(&trace, if keep { "06.kept" } else { "06.rejected" }, || {
                format!("{row:?}")
            });
            keep
        })
        // op07: flat_map
        .flat_map(frames)
        .inspect(|row| record(&trace, "07.structures", || format!("{row:?}")))
        // op08: filter
        .filter(|row| {
            let keep = row
                .as_ref()
                .map_or(true, |row| row.index % 2 == row.choice.structure);
            record(&trace, if keep { "08.kept" } else { "08.rejected" }, || {
                format!("{row:?}")
            });
            keep
        })
        // op09: flat_map
        .flat_map(|frame: Result<FrameRow, Error>| match frame {
            Err(error) => vec![Err(error)],
            Ok(row) => {
                if let Err(error) = corpus::validate(row.frame)
                    .and_then(|()| revision::validate(row.frame, row.index))
                {
                    return vec![Err(error)];
                }
                chosen_frame.set(Some(row));
                row.frame
                    .slots
                    .iter()
                    .map(|slot| {
                        Ok(Request {
                            frame: row.frame,
                            slot,
                        })
                    })
                    .collect()
            }
        })
        .inspect(|row| {
            record(&trace, "09.slots", || match row {
                Ok(r) => format!("{} candidates={}", r.frame.id, r.slot.variants.len()),
                Err(e) => format!("{e:?}"),
            })
        })
        // op10: enumerate
        .enumerate()
        .inspect(|(ordinal, _)| record(&trace, "10.slot_id", || ordinal.to_string()))
        // op11: flat_map
        .flat_map(candidate_rows)
        .inspect(|row| record(&trace, "11.bank_rows", || format!("{row:?}")))
        // op12: filter
        .filter(|row| {
            let keep = row.as_ref().map_or(true, |row| {
                row.candidate.is_none_or(|c| c.owner == row.ordinal)
            });
            record(&trace, if keep { "12.kept" } else { "12.rejected" }, || {
                format!("{row:?}")
            });
            keep
        })
        // op13: scan
        .scan(Pending::default(), |pending, row| Some(pending.push(row)))
        .inspect(|group| {
            record(&trace, "13.grouping", || match group {
                None => "pending".into(),
                Some(Err(e)) => format!("{e:?}"),
                Some(Ok(g)) => format!("slot={} complete={}", g.ordinal, g.candidates.len()),
            })
        })
        // op14: flatten
        .flatten()
        .inspect(|group| {
            record(&trace, "14.complete", || match group {
                Ok(g) => format!("slot={} candidates={}", g.ordinal, g.candidates.len()),
                Err(e) => format!("{e:?}"),
            })
        })
        // op15: zip
        .zip(std::iter::from_fn(|| Some(read_word(&rng))))
        .inspect(|(_, word)| record(&trace, "15.paired_word", || format!("{word:?}")))
        // op16: map
        .map(|(group, word)| {
            let group = group?;
            if group.ordinal != selected_slots.get() {
                return Err(Error::InvalidSelection);
            }
            let (index, draw) = choose(word, group.candidates.len(), &rng, &trace, "16.choice")?;
            let words = group.candidates[index];
            selected_slots.set(selected_slots.get() + 1);
            Ok(Selected { group, words, draw })
        })
        // op17: flat_map
        .flat_map(fragments)
        .inspect(|fragment| record(&trace, "17.fragments", || format!("{fragment:?}")))
        // op18: chain
        .chain(std::iter::once_with(|| {
            chosen_frame
                .get()
                .map(|row| Fragment {
                    atoms: row.frame.suffix,
                    origin: Origin::Suffix,
                })
                .ok_or(Error::MissingEnd)
        }))
        .inspect(|fragment| record(&trace, "18.with_suffix", || format!("{fragment:?}")))
        // op19: zip (TRANSITION)
        .zip(0_u64..)
        .inspect(|paired| record(&trace, "19.transition.paired", || format!("{paired:?}")))
        // op20: map (TRANSITION)
        .map(|(fragment, _display_ordinal)| fragment)
        .inspect(|fragment| record(&trace, "20.transition.unpaired", || format!("{fragment:?}")))
        // op21: map (TRANSITION)
        .map(std::iter::once)
        .inspect(|wrapped| record(&trace, "21.transition.singleton", || format!("{wrapped:?}")))
        // op22: flatten (TRANSITION)
        .flatten()
        .inspect(|fragment| {
            record(&trace, "22.transition.unwrapped", || {
                format!("{fragment:?}")
            })
        })
        // op23: map (TRANSITION)
        .map(shuffle_fragment)
        .inspect(|chunk| record(&trace, "23.transition.shuffled", || format!("{chunk:?}")))
        // op24: map (TRANSITION)
        .map(restore_fragment)
        .inspect(|chunk| record(&trace, "24.transition.restored", || format!("{chunk:?}")))
        // op25: flat_map
        .flat_map(atoms)
        .inspect(|node| record(&trace, "25.atoms", || format!("{node:?}")))
        // op26: flat_map
        .flat_map(|node| {
            insert_words(
                node,
                chosen_frame
                    .get()
                    .map(|row| revision::rule(language, row.index)),
                &insertions,
            )
        })
        .inspect(|node| record(&trace, "26.with_insertions", || format!("{node:?}")))
        // op27: filter
        .filter(|node| {
            let remove = node.as_ref().is_ok_and(|node| {
                chosen_frame.get().is_some_and(|row| {
                    revision::rule(language, row.index).matches(node.origin.site(), node.atom)
                })
            });
            if remove {
                deletions.set(deletions.get() + 1);
            }
            record(
                &trace,
                if remove { "27.removed" } else { "27.kept" },
                || format!("{node:?}"),
            );
            !remove
        })
        // op28: take_while
        .take_while(|node| {
            let end = node.as_ref().is_ok_and(|n| n.atom == Atom::End);
            if end {
                saw_end.set(true);
            }
            record(&trace, if end { "28.stopped" } else { "28.passed" }, || {
                format!("{node:?}")
            });
            !end
        })
        // op29: enumerate
        .enumerate()
        .inspect(|(position, _)| record(&trace, "29.position", || position.to_string()))
        // op30: map
        .map(|(position, node)| {
            if position >= limits.atoms {
                return Err(Error::AtomBudget);
            }
            let Node { atom, origin } = node?;
            let (text, is_word) = match atom {
                Atom::Word(word) => (word.to_owned(), true),
                Atom::Punctuation(mark) => (corpus::punctuation(mark, language).to_owned(), false),
                Atom::End => return Err(Error::InvalidSelection),
            };
            Ok(TextAtom {
                text,
                is_word,
                origin,
            })
        })
        .inspect(|atom| record(&trace, "30.localized", || format!("{atom:?}")))
        // op31: scan
        .scan(false, |has_content, atom| {
            Some(atom.map(|atom| {
                let leading_space = language == Language::En && atom.is_word && *has_content;
                *has_content = true;
                Spaced {
                    atom,
                    leading_space,
                }
            }))
        })
        .inspect(|spaced| record(&trace, "31.spacing", || format!("{spaced:?}")))
        // op32: flat_map
        .flat_map(text_pieces)
        .inspect(|atom| record(&trace, "32.pieces", || format!("{atom:?}")))
        // op33: scan
        .scan(false, |seen_word, atom| {
            Some(atom.map(|mut atom| {
                if atom.is_word && !*seen_word {
                    if language == Language::En {
                        atom.text[..1].make_ascii_uppercase();
                    }
                    *seen_word = true;
                }
                atom
            }))
        })
        .inspect(|atom| record(&trace, "33.capitalized", || format!("{atom:?}")))
        // op34: fold
        .fold(Ok(String::new()), |result, atom| {
            let mut sentence = result?;
            let atom = atom?;
            if sentence.len() + atom.text.len() > limits.bytes {
                return Err(Error::ByteBudget);
            }
            sentence.push_str(&atom.text);
            record(&trace, "34.accumulated", || sentence.clone());
            Ok(sentence)
        });
    // encourage:display:end

    let sentence = sentence?;
    let frame = chosen_frame.get().ok_or(Error::InvalidSelection)?;
    if selected_slots.get() != frame.frame.slots.len() {
        return Err(Error::InvalidSelection);
    }
    if !saw_end.get() {
        return Err(Error::MissingEnd);
    }
    if insertions.get() != 1 || deletions.get() != 1 {
        return Err(Error::InvalidSelection);
    }
    let draws = rng.borrow().used;
    Ok(Generation {
        frame_index: frame.index,
        sentence,
        draws,
        trace: trace.into_inner(),
    })
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn fixed_shuffle_has_a_real_known_permutation() {
        let original = &[Atom::Word("enjoy"), Atom::Word("a")];
        let shuffled = shuffle_fragment(Ok(Fragment {
            atoms: original,
            origin: Origin::Fixed { slot: 0 },
        }))
        .unwrap();
        assert_eq!(
            shuffled.items,
            [(1, Atom::Word("a")), (0, Atom::Word("enjoy"))]
        );
        assert_eq!(shuffled.seed, 2);
        let restored = restore_fragment(Ok(shuffled)).unwrap();
        assert_eq!(restored.atoms, original);
        assert_eq!(restored.origin, Origin::Fixed { slot: 0 });
    }

    #[test]
    fn every_corpus_fragment_restores_exactly_including_end_markers() {
        for language in Language::ALL {
            for frame in corpus::frames(language) {
                let mut chunks = vec![frame.suffix];
                for slot in frame.slots {
                    chunks.push(slot.before);
                    chunks.extend_from_slice(slot.variants);
                }
                for atoms in chunks {
                    let origin = Origin::Choice { slot: 1, draw: 3 };
                    let a = shuffle_fragment(Ok(Fragment { atoms, origin })).unwrap();
                    let b = shuffle_fragment(Ok(Fragment { atoms, origin })).unwrap();
                    assert_eq!(a.items, b.items);
                    let restored = restore_fragment(Ok(a)).unwrap();
                    assert_eq!(restored.atoms, atoms);
                    assert_eq!(restored.origin, origin);
                }
            }
        }
    }

    #[test]
    fn shuffle_and_restore_do_not_swallow_errors() {
        assert_eq!(
            shuffle_fragment(Err(Error::DrawBudget)).unwrap_err(),
            Error::DrawBudget
        );
        assert_eq!(
            restore_fragment(Err(Error::RandomSourceEnded)).unwrap_err(),
            Error::RandomSourceEnded
        );
    }
}
