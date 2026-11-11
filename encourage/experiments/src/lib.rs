//! Experimental generation only: no production CLI, terminal, state, or I/O.
//! Executed-stage counts distinguish content work from explicitly requested
//! reversible transitions; no production integration or frozen protocol is claimed.

pub mod corpus;
pub mod expanded;
pub mod pipeline;
pub mod revision;
pub mod rng;
pub mod trace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    Zh,
    En,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::Zh, Self::En];

    pub fn name(self) -> &'static str {
        match self {
            Self::Zh => "zh-CN",
            Self::En => "en-US",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub draws: usize,
    pub atoms: usize,
    pub bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            draws: 4096,
            atoms: 64,
            bytes: 512,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NoExplicitArguments,
    EmptyCandidates,
    RandomSourceEnded,
    DrawBudget,
    AtomBudget,
    ByteBudget,
    InvalidCorpus(&'static str),
    InvalidSelection,
    MissingEnd,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}
