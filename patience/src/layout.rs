//! A bounded, single-physical-line layout; never query the cursor or run stty.
use crate::{messages, plan::BAR_WIDTH};
use cli_common::Language;
use std::io::{self, IsTerminal};

const READOUT: usize = 7; // " 100.0%"
const MIN_BAR: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub rows: usize,
    pub columns: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub bar_width: usize,
    pub message: bool,
}

impl Layout {
    pub fn new(size: Size, bars: usize, message_width: usize) -> Option<Self> {
        // Leave a spare row and a spare column; never enter pending autowrap.
        if bars == 0 || bars >= size.rows {
            return None;
        }
        let available = size.columns.checked_sub(1 + READOUT)?;
        let bar_width = available.min(BAR_WIDTH);
        (bar_width >= MIN_BAR).then_some(Self {
            bar_width,
            // Omit the message before sacrificing the normal bar width. Keep
            // this decision fixed across cards so the region never changes size.
            message: available >= BAR_WIDTH + 1 + message_width,
        })
    }
}

pub fn terminal_size() -> Option<Size> {
    let stderr = io::stderr();
    if !stderr.is_terminal() {
        return None;
    }
    let size = rustix::termios::tcgetwinsize(stderr).ok()?;
    Some(Size {
        rows: size.ws_row.into(),
        columns: size.ws_col.into(),
    })
}

pub fn detect(bars: usize, language: Language) -> Option<(Size, Layout)> {
    let term = std::env::var("TERM").unwrap_or_default();
    if !supports_animation(&term) {
        return None;
    }
    let size = terminal_size()?;
    Some((
        size,
        Layout::new(size, bars, messages::max_width(language))?,
    ))
}

fn supports_animation(term: &str) -> bool {
    [
        "xterm",
        "screen",
        "tmux",
        "rxvt",
        "linux",
        "vt100",
        "alacritty",
        "foot",
        "wezterm",
        "kitty",
    ]
    .iter()
    .any(|prefix| term == *prefix || term.starts_with(&format!("{prefix}-")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_every_builtin_card_at_common_sizes() {
        for language in [Language::English, Language::Chinese] {
            for columns in [20, 24, 40, 80, 120] {
                for bars in [1, 2] {
                    let layout = Layout::new(
                        Size { rows: 24, columns },
                        bars,
                        messages::max_width(language),
                    )
                    .unwrap();
                    let width = layout.bar_width
                        + READOUT
                        + if layout.message {
                            1 + messages::max_width(language)
                        } else {
                            0
                        };
                    assert!(width < columns, "{language:?}, {columns}, {bars}: {width}");
                    assert_eq!(layout.bar_width, (columns - 1 - READOUT).min(BAR_WIDTH));
                    assert_eq!(
                        layout.message,
                        columns >= BAR_WIDTH + READOUT + 2 + messages::max_width(language)
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_small_or_overcrowded_regions_are_not_animated() {
        for size in [
            Size {
                rows: 0,
                columns: 80,
            },
            Size {
                rows: 24,
                columns: 0,
            },
            Size {
                rows: 24,
                columns: 15,
            },
        ] {
            assert!(Layout::new(size, 1, 30).is_none());
        }
        assert!(Layout::new(
            Size {
                rows: 2,
                columns: 80
            },
            2,
            30
        )
        .is_none());
        assert!(Layout::new(
            Size {
                rows: 3,
                columns: 80
            },
            2,
            30
        )
        .is_some());
        for term in ["", "dumb", "unknown", "made-up"] {
            assert!(!supports_animation(term));
        }
        assert!(supports_animation("xterm-256color"));
    }
}
