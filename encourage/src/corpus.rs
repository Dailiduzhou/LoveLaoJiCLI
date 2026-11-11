//! Words and slot rules only. No runtime sentence strings or string templates.
use crate::{Error, Language};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mark {
    Comma,
    Semicolon,
    Period,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Atom {
    Word(&'static str),
    Punctuation(Mark),
    End,
}

#[derive(Debug)]
pub struct Slot {
    pub before: &'static [Atom],
    pub variants: &'static [&'static [Atom]],
}

#[derive(Debug)]
pub struct Frame {
    pub id: &'static str,
    pub language: Language,
    pub slots: &'static [Slot],
    pub suffix: &'static [Atom],
}

use Atom::{End, Punctuation as P, Word as W};
use Mark::{Comma, Period, Semicolon};

macro_rules! words {
    ($($word:literal),+ $(,)?) => { &[$(W($word)),+] };
}

macro_rules! slot {
    ($before:expr; $($variant:expr),+ $(,)?) => {
        Slot { before: $before, variants: &[$($variant),+] }
    };
}

pub static ZH: [Frame; 8] = [
    Frame {
        id: "body.meal",
        language: Language::Chinese,
        slots: &[
            slot!(&[]; words!("好好"), words!("记得"), words!("安心")),
            slot!(&[W("吃"), W("饭"), P(Comma)]; words!("慢慢"), words!("用心")),
        ],
        suffix: &[W("照顾"), W("自己"), P(Period), End],
    },
    Frame {
        id: "body.water",
        language: Language::Chinese,
        slots: &[
            slot!(&[]; words!("先"), words!("记得")),
            slot!(&[W("喝"), W("点"), W("水"), P(Comma), W("再")]; words!("轻轻"), words!("慢慢")),
        ],
        suffix: &[W("伸展"), W("身体"), P(Period), End],
    },
    Frame {
        id: "rest.pause",
        language: Language::Chinese,
        slots: &[
            slot!(&[]; words!("累"), words!("困")),
            slot!(&[W("了"), W("就"), W("歇"), W("一会儿"), P(Comma), W("不必"), W("一直")]; words!("赶路"), words!("硬撑")),
        ],
        suffix: &[P(Period), End],
    },
    Frame {
        id: "rest.night",
        language: Language::Chinese,
        slots: &[
            slot!(words!("今晚"); words!("早点"), words!("安心")),
            slot!(&[W("休息"), P(Comma), W("明天"), W("再")]; words!("慢慢"), words!("从容")),
        ],
        suffix: &[W("开始"), P(Period), End],
    },
    Frame {
        id: "step.begin",
        language: Language::Chinese,
        slots: &[
            slot!(words!("先"); words!("迈出"), words!("走出")),
            slot!(&[]; words!("一小步"), words!("这一步")),
        ],
        suffix: &[
            P(Comma),
            W("按"),
            W("自己"),
            W("的"),
            W("节奏"),
            W("来"),
            P(Period),
            End,
        ],
    },
    Frame {
        id: "step.credit",
        language: Language::Chinese,
        slots: &[
            slot!(words!("今天", "有", "一点"); words!("进展"), words!("收获")),
            slot!(&[P(Comma), W("就"), W("给"), W("自己"), W("一点")]; words!("肯定"), words!("鼓励")),
        ],
        suffix: &[P(Period), End],
    },
    Frame {
        id: "accept.kindness",
        language: Language::Chinese,
        slots: &[
            slot!(words!("暂时", "没"); words!("做好"), words!("做成")),
            slot!(&[P(Comma), W("也"), W("值得"), W("被")]; words!("温柔"), words!("认真")),
        ],
        suffix: &[W("对待"), P(Period), End],
    },
    Frame {
        id: "accept.pace",
        language: Language::Chinese,
        slots: &[
            slot!(words!("允许", "自己"); words!("慢一点"), words!("缓一缓")),
            slot!(&[P(Comma), W("不必")]; words!("急着"), words!("忙着")),
        ],
        suffix: &[W("证明"), W("什么"), P(Period), End],
    },
];

pub static EN: [Frame; 8] = [
    Frame {
        id: "body.meal",
        language: Language::English,
        slots: &[
            slot!(words!("enjoy", "a"); words!("warm"), words!("simple"), words!("hearty")),
            slot!(words!("meal", "and", "take", "a"); words!("quiet"), words!("short")),
        ],
        suffix: &[W("break"), P(Period), End],
    },
    Frame {
        id: "body.water",
        language: Language::English,
        slots: &[
            slot!(words!("have", "some"); words!("cool"), words!("warm")),
            slot!(words!("water", "and"); words!("gently"), words!("slowly")),
        ],
        suffix: &[W("stretch"), W("your"), W("body"), P(Period), End],
    },
    Frame {
        id: "rest.pause",
        language: Language::English,
        slots: &[
            slot!(words!("take", "a"); words!("short"), words!("quiet")),
            slot!(&[W("break"), P(Semicolon), W("you"), W("do"), W("not"), W("have"), W("to")]; words!("rush"), words!("hurry")),
        ],
        suffix: &[P(Period), End],
    },
    Frame {
        id: "rest.night",
        language: Language::English,
        slots: &[
            slot!(words!("get", "some"); words!("good"), words!("quiet")),
            slot!(words!("rest", "tonight", "and", "start", "tomorrow", "at", "a"); words!("gentle"), words!("steady")),
        ],
        suffix: &[W("pace"), P(Period), End],
    },
    Frame {
        id: "step.begin",
        language: Language::English,
        slots: &[
            slot!(words!("take", "one"); words!("small"), words!("manageable")),
            slot!(words!("step"); words!("at", "your", "own", "pace"), words!("to", "your", "own", "rhythm")),
        ],
        suffix: &[P(Period), End],
    },
    Frame {
        id: "step.credit",
        language: Language::English,
        slots: &[
            slot!(words!("give", "yourself"); words!("credit"), words!("recognition")),
            slot!(words!("for", "the"); words!("small"), words!("steady")),
        ],
        suffix: &[W("steps"), W("you"), W("take"), P(Period), End],
    },
    Frame {
        id: "accept.kindness",
        language: Language::English,
        slots: &[
            slot!(words!("you", "deserve"); words!("kindness"), words!("patience")),
            slot!(&[P(Comma), W("even"), W("when"), W("things"), W("feel")]; words!("difficult"), words!("uncertain")),
        ],
        suffix: &[P(Period), End],
    },
    Frame {
        id: "accept.pace",
        language: Language::English,
        slots: &[
            slot!(words!("let", "yourself"); words!("slow", "down"), words!("take", "a", "breath")),
            slot!(&[P(Semicolon), W("you"), W("do"), W("not"), W("need",), W("to")]; words!("rush"), words!("hurry")),
        ],
        suffix: &[P(Period), End],
    },
];

pub fn frames(language: Language) -> &'static [Frame; 8] {
    match language {
        Language::Chinese => &ZH,
        Language::English => &EN,
    }
}

pub fn punctuation(mark: Mark, language: Language) -> &'static str {
    match (mark, language) {
        (Mark::Comma, Language::Chinese) => "，",
        (Mark::Semicolon, Language::Chinese) => "；",
        (Mark::Period, Language::Chinese) => "。",
        (Mark::Comma, Language::English) => ",",
        (Mark::Semicolon, Language::English) => ";",
        (Mark::Period, Language::English) => ".",
    }
}

pub fn validate(frame: &Frame) -> Result<(), Error> {
    if frame.slots.len() < 2 {
        return Err(Error::InvalidCorpus("at least two variable slots"));
    }
    for slot in frame.slots {
        if slot.variants.is_empty() {
            return Err(Error::EmptyCandidates);
        }
        if slot.variants.len() < 2 {
            return Err(Error::InvalidCorpus("not a variable slot"));
        }
        let unique: BTreeSet<_> = slot.variants.iter().copied().collect();
        if unique.len() != slot.variants.len() {
            return Err(Error::InvalidCorpus(
                "duplicate implicitly weighted alternative",
            ));
        }
        for variant in slot.variants {
            if variant.is_empty() || variant.iter().any(|a| !matches!(a, Atom::Word(_))) {
                return Err(Error::InvalidCorpus("alternatives must expand to words"));
            }
            for atom in *variant {
                validate_atom(*atom, frame.language)?;
            }
        }
        for atom in slot.before {
            if matches!(atom, Atom::End | Atom::Punctuation(Mark::Period)) {
                return Err(Error::InvalidCorpus("premature sentence end"));
            }
            validate_atom(*atom, frame.language)?;
        }
    }
    if !frame.suffix.ends_with(&[P(Period), End]) {
        return Err(Error::InvalidCorpus("missing period and end boundary"));
    }
    for atom in &frame.suffix[..frame.suffix.len() - 1] {
        if *atom == End {
            return Err(Error::InvalidCorpus("early end boundary"));
        }
        validate_atom(*atom, frame.language)?;
    }
    Ok(())
}

fn validate_atom(atom: Atom, language: Language) -> Result<(), Error> {
    if let Atom::Word(text) = atom {
        let valid = match language {
            Language::Chinese => {
                (1..=4).contains(&text.chars().count())
                    && text.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
            }
            Language::English => !text.is_empty() && text.bytes().all(|b| b.is_ascii_lowercase()),
        };
        if !valid {
            return Err(Error::InvalidCorpus("expected short, safe word atom"));
        }
    }
    Ok(())
}
