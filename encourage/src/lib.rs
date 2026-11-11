//! `encourage` deterministic generation and stage replay.
//!
//! `encourage-gen-v2` retains v1 text generation and adds typed material snapshots.
//! It freezes argv encoding, word selection, corpus, formatting
//! and labelled observations. The 34-operation chain has 28 content operations
//! and 6 explicitly marked reversible presentation transitions.

pub use cli_common::Language;

mod collection;
pub mod corpus;
pub mod duration;
pub mod material;
pub mod pipeline;
pub mod revision;
pub mod rng;
pub mod source;
pub mod trace;
pub mod tui;

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
