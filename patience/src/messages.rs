//! The comfort deck: localized messages drawn while the bar stalls. Cards are
//! shuffled up front and drawn in order; the deck reshuffles when empty, never
//! repeating the card that was just shown.

use cli_common::Language;
use rand::seq::SliceRandom;
use rand::Rng;

const ENGLISH: [&str; 7] = [
    "Good things take time",
    "It's loading. Breathe.",
    "Not frozen, just thinking",
    "Waiting is also progress",
    "Almost there — like always",
    "The bar will move, promise",
    "Just a moment… any moment now",
];

const CHINESE: [&str; 7] = [
    "别急，好饭不怕晚",
    "它在加载，你先深呼吸",
    "程序没死，它在思考",
    "等待也是一种修行",
    "马上就好，就像每次说的那样",
    "进度条会动的，我保证",
    "再等等，快了快了",
];

/// Conservative cell budgets for this finite deck only. Ambiguous em dash and
/// ellipsis are budgeted as two cells; this is not a general Unicode width API.
pub fn max_width(language: Language) -> usize {
    let deck = match language {
        Language::English => &ENGLISH,
        Language::Chinese => &CHINESE,
    };
    deck.iter().map(|s| width(s)).max().unwrap_or(0)
}
fn width(text: &str) -> usize {
    text.chars()
        .map(|c| match c {
            ' '..='~' => 1,
            '—'
            | '…'
            | '\u{3000}'..='\u{303f}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff01}'..='\u{ff60}' => 2,
            _ => panic!("unbudgeted comfort-deck character: {c:?}"),
        })
        .sum()
}

pub struct Comfort {
    deck: Vec<&'static str>,
    drawn: usize,
}

impl Comfort {
    pub fn new(language: Language, rng: &mut impl Rng) -> Self {
        let mut deck: Vec<&'static str> = match language {
            Language::English => ENGLISH.into(),
            Language::Chinese => CHINESE.into(),
        };
        deck.shuffle(rng);
        Self { deck, drawn: 0 }
    }

    /// Draw the next comfort message, reshuffling without immediate repeats.
    pub fn draw(&mut self, rng: &mut impl Rng) -> &'static str {
        if self.drawn >= self.deck.len() {
            let last = self.deck[self.deck.len() - 1];
            self.deck.shuffle(rng);
            if self.deck[0] == last && self.deck.len() > 1 {
                let last_index = self.deck.len() - 1;
                self.deck.swap(0, last_index);
            }
            self.drawn = 0;
        }
        let card = self.deck[self.drawn];
        self.drawn += 1;
        card
    }
}

#[cfg(test)]
mod tests {
    use super::Comfort;
    use cli_common::Language;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn comfort_for(seed: u64) -> (Comfort, StdRng) {
        let mut rng = StdRng::seed_from_u64(seed);
        let comfort = Comfort::new(Language::English, &mut rng);
        (comfort, rng)
    }

    #[test]
    fn builtin_width_budgets_are_pinned() {
        assert_eq!(super::max_width(Language::English), 30);
        assert_eq!(super::max_width(Language::Chinese), 26);
    }

    #[test]
    fn a_full_cycle_covers_every_card() {
        let (mut comfort, mut rng) = comfort_for(11);
        let mut drawn: Vec<&str> = (0..7).map(|_| comfort.draw(&mut rng)).collect();
        drawn.sort_unstable();
        drawn.dedup();
        assert_eq!(drawn.len(), 7);
    }

    #[test]
    fn the_same_card_never_shows_twice_in_a_row() {
        for seed in 0..20u64 {
            let (mut comfort, mut rng) = comfort_for(seed);
            let mut previous = "";
            for _ in 0..30 {
                let card = comfort.draw(&mut rng);
                assert_ne!(card, previous, "seed {seed}: repeated card {card:?}");
                previous = card;
            }
        }
    }

    #[test]
    fn language_selects_the_right_deck() {
        let mut rng = StdRng::seed_from_u64(1);
        let english = Comfort::new(Language::English, &mut rng);
        let chinese = Comfort::new(Language::Chinese, &mut rng);
        assert!(english.deck.contains(&"Good things take time"));
        assert!(chinese.deck.contains(&"别急，好饭不怕晚"));
    }
}
