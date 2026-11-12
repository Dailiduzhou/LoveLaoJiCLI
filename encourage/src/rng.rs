use crate::GenerationError;
use rand::RngCore;
use std::io;

/// Explicit text never touches the OS entropy source. An empty argument is
/// explicit text; only a genuinely empty argv requests eight fresh bytes.
pub fn seed(args: &[&str]) -> io::Result<u64> {
    seed_with_entropy(args, |bytes| {
        rand::rngs::OsRng
            .try_fill_bytes(bytes)
            .map_err(io::Error::other)
    })
}

fn seed_with_entropy(
    args: &[&str],
    entropy: impl FnOnce(&mut [u8; 8]) -> io::Result<()>,
) -> io::Result<u64> {
    if !args.is_empty() {
        return seed_from_argv(args).map_err(io::Error::other);
    }
    let mut bytes = [0_u8; 8];
    entropy(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests;

pub fn encode_argv(args: &[&str]) -> Result<Vec<u8>, GenerationError> {
    if args.is_empty() {
        return Err(GenerationError::NoExplicitArguments);
    }
    let mut bytes = b"encourage/argv/v1\0".to_vec();
    bytes.extend_from_slice(&(args.len() as u64).to_le_bytes());
    for arg in args {
        bytes.extend_from_slice(&(arg.len() as u64).to_le_bytes());
        bytes.extend_from_slice(arg.as_bytes());
    }
    Ok(bytes)
}

pub fn seed_from_argv(args: &[&str]) -> Result<u64, GenerationError> {
    let mut key = [0_u8; 32];
    let domain = b"encourage/argv-seed/v1";
    key[..domain.len()].copy_from_slice(domain);
    let digest = blake3::keyed_hash(&key, &encode_argv(args)?);
    Ok(u64::from_le_bytes(
        digest.as_bytes()[..8].try_into().expect("eight bytes"),
    ))
}

#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl Iterator for SplitMix64 {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Some(z ^ (z >> 31))
    }
}

pub struct Budgeted<I> {
    source: I,
    limit: usize,
    pub used: usize,
}

impl<I: Iterator<Item = u64>> Budgeted<I> {
    pub fn new(source: I, limit: usize) -> Self {
        Self {
            source,
            limit,
            used: 0,
        }
    }

    pub fn word(&mut self) -> Result<u64, GenerationError> {
        if self.used >= self.limit {
            return Err(GenerationError::DrawBudget);
        }
        let value = self
            .source
            .next()
            .ok_or(GenerationError::RandomSourceEnded)?;
        self.used += 1;
        Ok(value)
    }

    pub fn choose(&mut self, bound: usize) -> Result<usize, GenerationError> {
        loop {
            if let Some(index) = accepted_index(self.word_after_check(bound)?, bound)? {
                return Ok(index);
            }
        }
    }

    fn word_after_check(&mut self, bound: usize) -> Result<u64, GenerationError> {
        if bound == 0 {
            return Err(GenerationError::EmptyCandidates);
        }
        self.word()
    }
}

/// None means rejection, not an absent grammar slot. Check BEFORE modulo.
pub fn accepted_index(word: u64, bound: usize) -> Result<Option<usize>, GenerationError> {
    if bound == 0 {
        return Err(GenerationError::EmptyCandidates);
    }
    let n = bound as u64;
    let threshold = n.wrapping_neg() % n;
    Ok((word >= threshold).then(|| (word % n) as usize))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub draw: usize,
    pub slot: usize,
    pub word: u64,
    pub bound: usize,
    pub index: Option<usize>,
}

/// Emits every actual attempt, including rejected words. A rejected word does
/// NOT advance the slot. Ending the schedule does NOT pull an extra random word.
/// This source does no lexical selection, token expansion, or sentence assembly.
pub struct Attempts<'a, I> {
    rng: &'a mut Budgeted<I>,
    bounds: &'a [usize],
    slot: usize,
    failed: bool,
}

impl<'a, I> Attempts<'a, I> {
    pub fn new(rng: &'a mut Budgeted<I>, bounds: &'a [usize]) -> Self {
        Self {
            rng,
            bounds,
            slot: 0,
            failed: false,
        }
    }
}

impl<I: Iterator<Item = u64>> Iterator for Attempts<'_, I> {
    type Item = Result<Attempt, GenerationError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.slot == self.bounds.len() {
            return None;
        }
        let bound = self.bounds[self.slot];
        let result = (|| {
            if bound == 0 {
                return Err(GenerationError::EmptyCandidates);
            }
            let word = self.rng.word()?;
            Ok(Attempt {
                draw: self.rng.used - 1,
                slot: self.slot,
                word,
                bound,
                index: accepted_index(word, bound)?,
            })
        })();
        match &result {
            Ok(attempt) if attempt.index.is_some() => self.slot += 1,
            Err(_) => self.failed = true,
            _ => {}
        }
        Some(result)
    }
}
