//! Small, frame-specific editorial changes AFTER the reversible transitions.
//! These are short word/grammar rules, not complete replacement sentences.
use crate::corpus::{self, Atom, Frame, Mark};
use crate::rng::{Budgeted, SplitMix64};
use crate::{GenerationError, Language, Limits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Fixed(usize),
    Suffix,
}

#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub site: Site,
    pub remove: &'static str,
    pub insert: &'static [Atom],
}

impl Rule {
    pub fn matches(&self, site: Option<Site>, atom: Atom) -> bool {
        site == Some(self.site) && atom == Atom::Word(self.remove)
    }
}

use Atom::{Punctuation as P, Word as W};
use Site::{Fixed, Suffix};

static ZH: [Rule; 8] = [
    Rule {
        id: "body.meal",
        site: Suffix,
        remove: "照顾",
        insert: &[W("善待")],
    },
    Rule {
        id: "body.water",
        site: Suffix,
        remove: "伸展",
        insert: &[W("舒展")],
    },
    Rule {
        id: "rest.pause",
        site: Fixed(1),
        remove: "歇",
        insert: &[W("休息")],
    },
    Rule {
        id: "rest.night",
        site: Suffix,
        remove: "开始",
        insert: &[W("出发")],
    },
    Rule {
        id: "step.begin",
        site: Suffix,
        remove: "节奏",
        insert: &[W("步调")],
    },
    Rule {
        id: "step.credit",
        site: Fixed(1),
        remove: "给",
        insert: &[W("送给")],
    },
    Rule {
        id: "accept.kindness",
        site: Fixed(0),
        remove: "暂时",
        insert: &[W("这次")],
    },
    Rule {
        id: "accept.pace",
        site: Suffix,
        remove: "证明",
        insert: &[W("解释")],
    },
];

static EN: [Rule; 8] = [
    Rule {
        id: "body.meal",
        site: Suffix,
        remove: "break",
        insert: &[W("pause")],
    },
    Rule {
        id: "body.water",
        site: Fixed(1),
        remove: "and",
        insert: &[P(Mark::Comma), W("then")],
    },
    Rule {
        id: "rest.pause",
        site: Fixed(1),
        remove: "have",
        insert: &[W("need")],
    },
    Rule {
        id: "rest.night",
        site: Fixed(1),
        remove: "start",
        insert: &[W("approach")],
    },
    Rule {
        id: "step.begin",
        site: Fixed(0),
        remove: "take",
        insert: &[W("try")],
    },
    Rule {
        id: "step.credit",
        site: Suffix,
        remove: "take",
        insert: &[W("have"), W("taken")],
    },
    Rule {
        id: "accept.kindness",
        site: Fixed(1),
        remove: "feel",
        insert: &[W("seem")],
    },
    Rule {
        id: "accept.pace",
        site: Fixed(1),
        remove: "need",
        insert: &[W("have")],
    },
];

pub fn rule(language: Language, frame_index: usize) -> &'static Rule {
    match language {
        Language::Chinese => &ZH[frame_index],
        Language::English => &EN[frame_index],
    }
}

pub fn validate(frame: &Frame, index: usize) -> Result<(), GenerationError> {
    let rule = rule(frame.language, index);
    if rule.id != frame.id {
        return Err(GenerationError::InvalidCorpus("revision frame mismatch"));
    }
    let anchor = match rule.site {
        Fixed(slot) => {
            frame
                .slots
                .get(slot)
                .ok_or(GenerationError::InvalidSelection)?
                .before
        }
        Suffix => frame.suffix,
    };
    if anchor
        .iter()
        .filter(|&&atom| atom == W(rule.remove))
        .count()
        != 1
    {
        return Err(GenerationError::InvalidCorpus(
            "revision anchor must occur exactly once",
        ));
    }
    if rule.insert.is_empty() || !rule.insert.iter().any(|a| matches!(a, W(_))) {
        return Err(GenerationError::InvalidCorpus(
            "revision must insert real words",
        ));
    }
    for &atom in rule.insert {
        match atom {
            W(word) => {
                let safe = match frame.language {
                    Language::Chinese => {
                        (1..=4).contains(&word.chars().count())
                            && word.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
                    }
                    Language::English => {
                        !word.is_empty() && word.bytes().all(|c| c.is_ascii_lowercase())
                    }
                };
                if !safe || word == rule.remove {
                    return Err(GenerationError::InvalidCorpus(
                        "unsafe or ineffective revision",
                    ));
                }
            }
            P(Mark::Comma | Mark::Semicolon) => {}
            _ => {
                return Err(GenerationError::InvalidCorpus(
                    "revision cannot introduce an end boundary",
                ))
            }
        }
    }
    Ok(())
}

/// Independent imperative oracle: directly SUBSTITUTE at the anchored site.
/// It does not call the production insert/filter helpers or transition code.
pub fn render_choices(
    frame_index: usize,
    language: Language,
    choices: &[usize],
) -> Result<String, GenerationError> {
    let frame = &corpus::frames(language)[frame_index];
    corpus::validate(frame)?;
    validate(frame, frame_index)?;
    if choices.len() != frame.slots.len() {
        return Err(GenerationError::InvalidSelection);
    }
    let rule = rule(language, frame_index);
    let mut original = Vec::new();
    for (ordinal, (slot, &index)) in frame.slots.iter().zip(choices).enumerate() {
        for &atom in slot.before {
            original.push((Some(Fixed(ordinal)), atom));
        }
        for &atom in *slot
            .variants
            .get(index)
            .ok_or(GenerationError::InvalidSelection)?
        {
            original.push((None, atom));
        }
    }
    for &atom in frame.suffix {
        original.push((Some(Suffix), atom));
    }
    let mut revised = Vec::new();
    let mut replaced = 0;
    for (site, atom) in original {
        if rule.matches(site, atom) {
            revised.extend_from_slice(rule.insert);
            replaced += 1;
        } else {
            revised.push(atom);
        }
    }
    if replaced != 1 {
        return Err(GenerationError::InvalidSelection);
    }
    let mut text = String::new();
    let mut first_word = true;
    for atom in revised {
        match atom {
            W(word) => {
                if language == Language::English && !text.is_empty() {
                    text.push(' ');
                }
                if first_word && language == Language::English {
                    let mut letters = word.chars();
                    text.push(
                        letters
                            .next()
                            .ok_or(GenerationError::InvalidSelection)?
                            .to_ascii_uppercase(),
                    );
                    text.extend(letters);
                } else {
                    text.push_str(word);
                }
                first_word = false;
            }
            P(mark) => text.push_str(corpus::punctuation(mark, language)),
            Atom::End => break,
        }
    }
    Ok(text)
}

pub fn reference(seed: u64, language: Language) -> Result<(usize, String, usize), GenerationError> {
    let mut rng = Budgeted::new(SplitMix64::new(seed), Limits::default().draws);
    let index = rng.choose(4)? * 2 + rng.choose(2)?;
    let mut choices = Vec::new();
    for slot in corpus::frames(language)[index].slots {
        choices.push(rng.choose(slot.variants.len())?);
    }
    Ok((index, render_choices(index, language, &choices)?, rng.used))
}
