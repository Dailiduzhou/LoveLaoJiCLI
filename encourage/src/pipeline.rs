//! Generation v2: v1 text selection plus typed, bounded output observations.
//! 34 executed operations = 28 content + 6 explicitly
//! reversible presentation transitions, followed by real lexical insert/delete.
//! The isolated experiments retain the original 12-operation reference.
use crate::collection::{Trace, TraceConfig};
use crate::corpus::{self, Atom, Frame, Slot};
use crate::material::Assembly;
use crate::revision::{self, Rule, Site};
use crate::rng::{accepted_index, Budgeted, SplitMix64};
use crate::{GenerationError, Language, Limits};
use std::cell::{Cell, RefCell};

#[cfg(test)]
mod tests;

pub const GENERATION_VERSION: &str = "encourage-gen-v2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generation {
    pub generation_version: &'static str,
    pub seed: u64,
    pub frame_index: usize,
    pub sentence: String,
    pub draws: usize,
    pub trace: crate::trace::Trace,
}

impl Generation {
    /// Focused TUI fixture constructor; production callers use `generate`.
    pub fn from_trace(seed: u64, sentence: impl Into<String>, trace: crate::trace::Trace) -> Self {
        Self {
            generation_version: GENERATION_VERSION,
            seed,
            frame_index: 0,
            sentence: sentence.into(),
            draws: 0,
            trace,
        }
    }
}

#[derive(Debug)]
struct CollectedGeneration {
    frame_index: usize,
    sentence: String,
    draws: usize,
    trace: Trace,
}

/// Diagnostic type names retained for trace consumers, never rendered in the
/// raw-material panel. Operation IDs and counts still come from source markers.
const VALUE_TYPES: &[&str] = &[
    "usize",
    "Choice",
    "Catalogue",
    "Catalogue",
    "ThemeRow",
    "ThemeRow",
    "FrameRow",
    "FrameRow",
    "Request",
    "(usize, Request)",
    "Row",
    "Row",
    "Option<Group>",
    "Group",
    "(Group, Word)",
    "Selected",
    "Fragment",
    "Fragment",
    "(Fragment, u64)",
    "Fragment",
    "Once<Fragment>",
    "Fragment",
    "Shuffled",
    "Restored",
    "Node",
    "Node",
    "Node",
    "Node",
    "(usize, Node)",
    "TextAtom",
    "Spaced",
    "TextAtom",
    "TextAtom",
    "String",
];

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
    fn push(
        &mut self,
        row: Result<Row, GenerationError>,
    ) -> Option<Result<Group, GenerationError>> {
        let row = match row {
            Ok(row) => row,
            Err(error) => {
                *self = Self::default();
                return Some(Err(error));
            }
        };
        if self.ordinal.is_some_and(|ordinal| ordinal != row.ordinal) {
            *self = Self::default();
            return Some(Err(GenerationError::InvalidSelection));
        }
        if let Some(candidate) = row.candidate {
            if candidate.owner != row.ordinal || candidate.rank != self.candidates.len() {
                *self = Self::default();
                return Some(Err(GenerationError::InvalidSelection));
            }
            self.ordinal = Some(row.ordinal);
            self.candidates.push(candidate.words);
            None
        } else {
            self.ordinal = None;
            let candidates = std::mem::take(&mut self.candidates);
            if candidates.is_empty() {
                return Some(Err(GenerationError::EmptyCandidates));
            }
            if candidates.len() != row.request.slot.variants.len() {
                return Some(Err(GenerationError::InvalidSelection));
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

fn observe_atoms(
    trace: &RefCell<Trace>,
    stage: u16,
    language: Language,
    assembly: Assembly,
    atoms: impl IntoIterator<Item = Atom>,
) {
    trace.borrow_mut().material(stage, |material| {
        for atom in atoms {
            material.atom(atom, language, assembly);
        }
    });
}

fn observe_row(
    trace: &RefCell<Trace>,
    stage: u16,
    row: &Result<Row, GenerationError>,
    language: Language,
) {
    if let Ok(Row {
        candidate: Some(candidate),
        ..
    }) = row
    {
        observe_atoms(
            trace,
            stage,
            language,
            Assembly::None,
            candidate.words.iter().copied(),
        );
    }
}

fn observe_group(
    trace: &RefCell<Trace>,
    stage: u16,
    group: &Result<Group, GenerationError>,
    language: Language,
) {
    if let Ok(group) = group {
        observe_atoms(
            trace,
            stage,
            language,
            Assembly::None,
            group
                .candidates
                .iter()
                .flat_map(|words| words.iter().copied()),
        );
    }
}

fn observe_fragment(
    trace: &RefCell<Trace>,
    stage: u16,
    fragment: &Result<Fragment, GenerationError>,
    language: Language,
) {
    if let Ok(fragment) = fragment {
        observe_atoms(
            trace,
            stage,
            language,
            Assembly::Words,
            fragment.atoms.iter().copied(),
        );
    }
}

fn observe_node(
    trace: &RefCell<Trace>,
    stage: u16,
    node: &Result<Node, GenerationError>,
    language: Language,
) {
    if let Ok(node) = node {
        observe_atoms(trace, stage, language, Assembly::Words, [node.atom]);
    }
}

fn observe_text(
    trace: &RefCell<Trace>,
    stage: u16,
    atom: &Result<TextAtom, GenerationError>,
    assembly: Assembly,
) {
    if let Ok(atom) = atom {
        trace.borrow_mut().material(stage, |material| {
            material.push(&atom.text, atom.is_word, assembly)
        });
    }
}

fn read_word<I: Iterator<Item = u64>>(rng: &RefCell<Budgeted<I>>) -> Result<Word, GenerationError> {
    let mut rng = rng.borrow_mut();
    let value = rng.word()?;
    Ok(Word {
        value,
        draw: rng.used - 1,
    })
}

/// Retry consumes words for THIS group, never another item from the slot stream.
fn choose<I: Iterator<Item = u64>>(
    first: Result<Word, GenerationError>,
    bound: usize,
    rng: &RefCell<Budgeted<I>>,
    trace: &RefCell<Trace>,
    stage: &'static str,
) -> Result<(usize, usize), GenerationError> {
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

fn catalogues(choice: Result<Choice, GenerationError>) -> Vec<Result<Catalogue, GenerationError>> {
    match choice {
        Ok(choice) => [Language::Chinese, Language::English]
            .into_iter()
            .map(|language| Ok(Catalogue { choice, language }))
            .collect(),
        Err(error) => vec![Err(error)],
    }
}

fn themes(catalogue: Result<Catalogue, GenerationError>) -> Vec<Result<ThemeRow, GenerationError>> {
    match catalogue {
        Ok(catalogue) => (0..4)
            .map(|theme| Ok(ThemeRow { catalogue, theme }))
            .collect(),
        Err(error) => vec![Err(error)],
    }
}

fn frames(theme: Result<ThemeRow, GenerationError>) -> Vec<Result<FrameRow, GenerationError>> {
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
fn candidate_rows(
    (ordinal, request): (usize, Result<Request, GenerationError>),
) -> Vec<Result<Row, GenerationError>> {
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

fn fragments(
    selected: Result<Selected, GenerationError>,
) -> Vec<Result<Fragment, GenerationError>> {
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

fn shuffle_fragment(
    fragment: Result<Fragment, GenerationError>,
) -> Result<Shuffled, GenerationError> {
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

fn restore_fragment(
    shuffled: Result<Shuffled, GenerationError>,
) -> Result<Restored, GenerationError> {
    let mut shuffled = shuffled?;
    if shuffled.seed != SHUFFLE_SEED {
        return Err(GenerationError::InvalidSelection);
    }
    shuffled
        .items
        .sort_by_key(|(original_position, _)| *original_position);
    Ok(Restored {
        atoms: shuffled.items.into_iter().map(|(_, atom)| atom).collect(),
        origin: shuffled.origin,
    })
}

fn atoms(fragment: Result<Restored, GenerationError>) -> Vec<Result<Node, GenerationError>> {
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
    node: Result<Node, GenerationError>,
    rule: Option<&Rule>,
    insertions: &Cell<usize>,
) -> Vec<Result<Node, GenerationError>> {
    let node = match node {
        Ok(node) => node,
        Err(error) => return vec![Err(error)],
    };
    let Some(rule) = rule else {
        return vec![Err(GenerationError::InvalidSelection)];
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

fn text_pieces(spaced: Result<Spaced, GenerationError>) -> Vec<Result<TextAtom, GenerationError>> {
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

pub fn generate(seed: u64, language: Language) -> Result<Generation, GenerationError> {
    let output = generate_words(
        SplitMix64::new(seed),
        language,
        TraceConfig::default(),
        Limits::default(),
    )?;
    Ok(Generation {
        generation_version: GENERATION_VERSION,
        seed,
        frame_index: output.frame_index,
        sentence: output.sentence,
        draws: output.draws,
        trace: replay_trace(output.trace, language)?,
    })
}

pub fn generate_plain(seed: u64, language: Language) -> Result<String, GenerationError> {
    generate_words(
        SplitMix64::new(seed),
        language,
        TraceConfig::off(),
        Limits::default(),
    )
    .map(|output| output.sentence)
}

#[expect(
    clippy::manual_try_fold,
    reason = "retain the specified bounded fold experiment"
)]
fn generate_words<I: Iterator<Item = u64>>(
    words: I,
    language: Language,
    config: TraceConfig,
    limits: Limits,
) -> Result<CollectedGeneration, GenerationError> {
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
        .flat_map(|frame: Result<FrameRow, GenerationError>| match frame {
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
        .inspect(|row| {
            record(&trace, "11.bank_rows", || format!("{row:?}"));
            observe_row(&trace, 11, row, language);
        })
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
        .inspect(|row| observe_row(&trace, 12, row, language))
        // op13: scan
        .scan(Pending::default(), |pending, row| Some(pending.push(row)))
        .inspect(|group| {
            record(&trace, "13.grouping", || match group {
                None => "pending".into(),
                Some(Err(e)) => format!("{e:?}"),
                Some(Ok(g)) => format!("slot={} complete={}", g.ordinal, g.candidates.len()),
            });
            if let Some(group) = group {
                observe_group(&trace, 13, group, language);
            }
        })
        // op14: flatten
        .flatten()
        .inspect(|group| {
            record(&trace, "14.complete", || match group {
                Ok(g) => format!("slot={} candidates={}", g.ordinal, g.candidates.len()),
                Err(e) => format!("{e:?}"),
            });
            observe_group(&trace, 14, group, language);
        })
        // op15: zip
        .zip(std::iter::from_fn(|| Some(read_word(&rng))))
        .inspect(|(group, word)| {
            record(&trace, "15.paired_word", || format!("{word:?}"));
            observe_group(&trace, 15, group, language);
        })
        // op16: map
        .map(|(group, word)| {
            let group = group?;
            if group.ordinal != selected_slots.get() {
                return Err(GenerationError::InvalidSelection);
            }
            let (index, draw) = choose(word, group.candidates.len(), &rng, &trace, "16.choice")?;
            let words = group.candidates[index];
            selected_slots.set(selected_slots.get() + 1);
            Ok(Selected { group, words, draw })
        })
        .inspect(|selected| {
            if let Ok(selected) = selected {
                observe_atoms(
                    &trace,
                    16,
                    language,
                    Assembly::Words,
                    selected.words.iter().copied(),
                );
            }
        })
        // op17: flat_map
        .flat_map(fragments)
        .inspect(|fragment| {
            record(&trace, "17.fragments", || format!("{fragment:?}"));
            observe_fragment(&trace, 17, fragment, language);
        })
        // op18: chain
        .chain(std::iter::once_with(|| {
            chosen_frame
                .get()
                .map(|row| Fragment {
                    atoms: row.frame.suffix,
                    origin: Origin::Suffix,
                })
                .ok_or(GenerationError::MissingEnd)
        }))
        .inspect(|fragment| {
            record(&trace, "18.with_suffix", || format!("{fragment:?}"));
            observe_fragment(&trace, 18, fragment, language);
        })
        // op19: zip (TRANSITION)
        .zip(0_u64..)
        .inspect(|paired| {
            record(&trace, "19.transition.paired", || format!("{paired:?}"));
            observe_fragment(&trace, 19, &paired.0, language);
        })
        // op20: map (TRANSITION)
        .map(|(fragment, _display_ordinal)| fragment)
        .inspect(|fragment| {
            record(&trace, "20.transition.unpaired", || format!("{fragment:?}"));
            observe_fragment(&trace, 20, fragment, language);
        })
        // op21: map (TRANSITION)
        .map(|fragment| {
            // Observe the actual singleton payload without pulling its iterator.
            observe_fragment(&trace, 21, &fragment, language);
            std::iter::once(fragment)
        })
        .inspect(|wrapped| record(&trace, "21.transition.singleton", || format!("{wrapped:?}")))
        // op22: flatten (TRANSITION)
        .flatten()
        .inspect(|fragment| {
            record(&trace, "22.transition.unwrapped", || {
                format!("{fragment:?}")
            });
            observe_fragment(&trace, 22, fragment, language);
        })
        // op23: map (TRANSITION)
        .map(shuffle_fragment)
        .inspect(|chunk| {
            record(&trace, "23.transition.shuffled", || format!("{chunk:?}"));
            if let Ok(chunk) = chunk {
                observe_atoms(
                    &trace,
                    23,
                    language,
                    Assembly::Words,
                    chunk.items.iter().map(|(_, atom)| *atom),
                );
            }
        })
        // op24: map (TRANSITION)
        .map(restore_fragment)
        .inspect(|chunk| {
            record(&trace, "24.transition.restored", || format!("{chunk:?}"));
            if let Ok(chunk) = chunk {
                observe_atoms(
                    &trace,
                    24,
                    language,
                    Assembly::Words,
                    chunk.atoms.iter().copied(),
                );
            }
        })
        // op25: flat_map
        .flat_map(atoms)
        .inspect(|node| {
            record(&trace, "25.atoms", || format!("{node:?}"));
            observe_node(&trace, 25, node, language);
        })
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
        .inspect(|node| {
            record(&trace, "26.with_insertions", || format!("{node:?}"));
            observe_node(&trace, 26, node, language);
        })
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
        .inspect(|node| observe_node(&trace, 27, node, language))
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
        .inspect(|node| observe_node(&trace, 28, node, language))
        // op29: enumerate
        .enumerate()
        .inspect(|(position, node)| {
            record(&trace, "29.position", || position.to_string());
            observe_node(&trace, 29, node, language);
        })
        // op30: map
        .map(|(position, node)| {
            if position >= limits.atoms {
                return Err(GenerationError::AtomBudget);
            }
            let Node { atom, origin } = node?;
            let (text, is_word) = match atom {
                Atom::Word(word) => (word.to_owned(), true),
                Atom::Punctuation(mark) => (corpus::punctuation(mark, language).to_owned(), false),
                Atom::End => return Err(GenerationError::InvalidSelection),
            };
            Ok(TextAtom {
                text,
                is_word,
                origin,
            })
        })
        .inspect(|atom| {
            record(&trace, "30.localized", || format!("{atom:?}"));
            observe_text(&trace, 30, atom, Assembly::Words);
        })
        // op31: scan
        .scan(false, |has_content, atom| {
            Some(atom.map(|atom| {
                let leading_space = language == Language::English && atom.is_word && *has_content;
                *has_content = true;
                Spaced {
                    atom,
                    leading_space,
                }
            }))
        })
        .inspect(|spaced| {
            record(&trace, "31.spacing", || format!("{spaced:?}"));
            if let Ok(spaced) = spaced {
                trace.borrow_mut().material(31, |material| {
                    if spaced.leading_space {
                        material.push(" ", false, Assembly::Exact);
                    }
                    material.push(&spaced.atom.text, spaced.atom.is_word, Assembly::Exact);
                });
            }
        })
        // op32: flat_map
        .flat_map(text_pieces)
        .inspect(|atom| {
            record(&trace, "32.pieces", || format!("{atom:?}"));
            observe_text(&trace, 32, atom, Assembly::Exact);
        })
        // op33: scan
        .scan(false, |seen_word, atom| {
            Some(atom.map(|mut atom| {
                if atom.is_word && !*seen_word {
                    if language == Language::English {
                        atom.text[..1].make_ascii_uppercase();
                    }
                    *seen_word = true;
                }
                atom
            }))
        })
        .inspect(|atom| {
            record(&trace, "33.capitalized", || format!("{atom:?}"));
            observe_text(&trace, 33, atom, Assembly::Exact);
        })
        // op34: fold
        .fold(Ok(String::new()), |result, atom| {
            let mut sentence = result?;
            let atom = atom?;
            if sentence.len() + atom.text.len() > limits.bytes {
                return Err(GenerationError::ByteBudget);
            }
            sentence.push_str(&atom.text);
            record(&trace, "34.accumulated", || sentence.clone());
            trace
                .borrow_mut()
                .material(34, |material| material.accumulated(&sentence));
            Ok(sentence)
        });
    // encourage:display:end

    let sentence = sentence?;
    let frame = chosen_frame
        .get()
        .ok_or(GenerationError::InvalidSelection)?;
    if selected_slots.get() != frame.frame.slots.len() {
        return Err(GenerationError::InvalidSelection);
    }
    if !saw_end.get() {
        return Err(GenerationError::MissingEnd);
    }
    if insertions.get() != 1 || deletions.get() != 1 {
        return Err(GenerationError::InvalidSelection);
    }
    let draws = rng.borrow().used;
    Ok(CollectedGeneration {
        frame_index: frame.index,
        sentence,
        draws,
        trace: trace.into_inner(),
    })
}

/// Convert the collector's actual labelled observations into bounded replay
/// stages. Missing structured counts/provenance stay unknown rather than being
/// inferred from preview indices or by assuming input and output counts match.
fn replay_trace(
    mut trace: Trace,
    language: Language,
) -> Result<crate::trace::Trace, GenerationError> {
    use crate::source::Source;
    use crate::trace::{OperationKind, Stage, StageId, StageRole, ValuePreview};

    let source = Source::embedded().map_err(|_| GenerationError::InvalidSelection)?;
    let mut stages = Vec::with_capacity(OPERATIONS.len());
    for (index, (operation_name, _)) in OPERATIONS.iter().enumerate() {
        let number = index + 1;
        let raw_id = u16::try_from(number).map_err(|_| GenerationError::InvalidSelection)?;
        let id = StageId(raw_id);
        let operation =
            OperationKind::from_name(operation_name).ok_or(GenerationError::InvalidSelection)?;
        let range = source.range(id).ok_or(GenerationError::InvalidSelection)?;
        let role = source.role(id).unwrap_or(StageRole::Content);
        let value_type = VALUE_TYPES.get(index).copied().unwrap_or("observed value");
        let mut stage = Stage::new(raw_id, operation, range, value_type).with_role(role);
        stage.material = trace.materials.remove(&raw_id).unwrap_or_default();
        let prefix = format!("{number:02}.");

        for (name, snapshot) in trace
            .stages
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
        {
            stage.summary.observed_events += snapshot.observed;
            for value in &snapshot.preview {
                stage
                    .summary
                    .push_preview(ValuePreview::unidentified(format!("{name}: {value}")));
            }
            stage.summary.omit_previews(snapshot.omitted());
        }

        match operation {
            OperationKind::Filter => {
                let kept = observed(&trace, &format!("{prefix}kept"));
                let rejected = observed(&trace, &format!("{prefix}rejected"))
                    + observed(&trace, &format!("{prefix}removed"));
                stage.summary.observed_inputs = Some(kept + rejected);
                stage.summary.observed_outputs = Some(kept);
                stage.summary.rejected = Some(rejected);
            }
            OperationKind::TakeWhile => {
                let passed = observed(&trace, &format!("{prefix}passed"));
                let stopped = observed(&trace, &format!("{prefix}stopped"));
                stage.summary.observed_inputs = Some(passed + stopped);
                stage.summary.observed_outputs = Some(passed);
                if stopped != 0 {
                    stage.summary.termination = Some(
                        language
                            .text(
                                "End boundary observed; no values after it are fabricated",
                                "已观测句末边界；不补造其后的值",
                            )
                            .to_owned(),
                    );
                }
            }
            OperationKind::Enumerate | OperationKind::Fold => {
                let count = stage.summary.observed_events;
                stage.summary.observed_inputs = Some(count);
                stage.summary.observed_outputs = Some(count);
            }
            _ => {}
        }
        stages.push(stage);
    }
    Ok(crate::trace::Trace { stages })
}

fn observed(trace: &Trace, name: &str) -> usize {
    trace
        .stages
        .get(name)
        .map(|snapshot| snapshot.observed)
        .unwrap_or(0)
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
        for language in [Language::Chinese, Language::English] {
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
            shuffle_fragment(Err(GenerationError::DrawBudget)).unwrap_err(),
            GenerationError::DrawBudget
        );
        assert_eq!(
            restore_fragment(Err(GenerationError::RandomSourceEnded)).unwrap_err(),
            GenerationError::RandomSourceEnded
        );
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    use crate::{source::Source, trace::StageRole};

    #[test]
    fn real_trace_matches_the_oracle_and_embedded_stage_metadata() {
        let source = Source::embedded().unwrap();
        for seed in 0..128 {
            let mut frame = None;
            for language in [Language::Chinese, Language::English] {
                let output = generate(seed, language).unwrap();
                let expected = revision::reference(seed, language).unwrap();
                assert_eq!(generate_plain(seed, language).unwrap(), output.sentence);
                assert_eq!(
                    (output.frame_index, output.sentence.clone(), output.draws),
                    expected
                );
                assert_eq!(output.trace.stages.len(), OPERATIONS.len());
                assert_eq!(
                    output
                        .trace
                        .stages
                        .iter()
                        .filter(|stage| stage.role == StageRole::Transition)
                        .count(),
                    TRANSITION_STAGES.len()
                );
                source.validate_trace(&output.trace).unwrap();
                assert!(output
                    .trace
                    .stages
                    .iter()
                    .flat_map(|stage| stage.summary.previews())
                    .all(|preview| preview.id.is_none()));
                if let Some(expected) = frame {
                    assert_eq!(output.frame_index, expected);
                }
                frame = Some(output.frame_index);
            }
        }
    }
}
