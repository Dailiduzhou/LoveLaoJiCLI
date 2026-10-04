//! Basic ANSI colors only; no terminal probing or background assumptions.
use cli_common::Language;
use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
};
use std::{ffi::OsStr, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorMode {
    Parts,
    Rainbow,
    White,
}
impl ColorMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "parts" => Ok(Self::Parts),
            "rainbow" => Ok(Self::Rainbow),
            "white" => Ok(Self::White),
            _ => Err(Language::detect()
                .text(
                    "Expected rainbow, parts or white",
                    "请输入 rainbow、parts 或 white",
                )
                .to_owned()),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(super) title: Style,
    pub(super) clock: Style,
    pub(super) grass: Style,
    pub(super) sun: Style,
    pub(super) flower: Style,
    pub(super) center: Style,
    pub(super) ground: Style,
    pub(super) text: Style,
    rainbow: bool,
}

impl Palette {
    pub(crate) fn detect(mode: ColorMode) -> Self {
        Self::from_no_color(mode, std::env::var_os("NO_COLOR").as_deref())
    }

    fn from_no_color(mode: ColorMode, value: Option<&OsStr>) -> Self {
        Self::new(mode, value.is_none_or(OsStr::is_empty))
    }

    pub(super) fn new(mode: ColorMode, enabled: bool) -> Self {
        let style = |color| {
            if !enabled {
                Style::default()
            } else if mode == ColorMode::White {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(color)
            }
        };
        Self {
            title: style(Color::Cyan),
            clock: style(Color::LightCyan),
            grass: style(Color::Green),
            sun: style(Color::Yellow),
            flower: style(Color::LightMagenta),
            center: style(Color::LightYellow),
            ground: style(Color::Yellow),
            text: style(Color::Reset),
            rainbow: enabled && mode == ColorMode::Rainbow,
        }
    }

    /// Recolor only artwork (already marked with a foreground color), leaving
    /// prose/hints and every character/background unchanged. Diagonal bands move
    /// one column per 100ms of monotonic time, not per rendered frame/key press.
    pub(super) fn animate(self, buffer: &mut Buffer, elapsed: Duration) {
        if !self.rainbow || buffer.area.width == 0 {
            return;
        }
        const COLORS: [Color; 6] = [
            Color::LightRed,
            Color::LightYellow,
            Color::LightGreen,
            Color::LightCyan,
            Color::LightBlue,
            Color::LightMagenta,
        ];
        const BAND: usize = 4;
        const PERIOD: usize = BAND * COLORS.len();
        let phase = (elapsed.as_millis() / 100 % PERIOD as u128) as usize;
        let width = usize::from(buffer.area.width);
        for (i, cell) in buffer.content.iter_mut().enumerate() {
            if cell.fg != Color::Reset && cell.symbol() != " " {
                // Two horizontal cells per row approximates a 45-degree band.
                let position = i % width + 2 * (i / width) + phase;
                cell.set_fg(COLORS[position / BAND % COLORS.len()]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_nonempty_no_color_disables_every_mode() {
        for mode in [ColorMode::Parts, ColorMode::Rainbow, ColorMode::White] {
            for value in ["1", "0", "false"] {
                let palette = Palette::from_no_color(mode, Some(OsStr::new(value)));
                assert_eq!(palette.clock, Style::default());
                assert_eq!(palette.text, Style::default());
                assert!(!palette.rainbow);
            }
            for value in [None, Some(OsStr::new(""))] {
                let palette = Palette::from_no_color(mode, value);
                assert_eq!(palette.rainbow, mode == ColorMode::Rainbow);
                assert_eq!(
                    palette.clock.fg,
                    Some(if mode == ColorMode::White {
                        Color::White
                    } else {
                        Color::LightCyan
                    })
                );
            }
        }
    }

    #[test]
    fn rainbow_is_spatial_time_based_and_periodic() {
        use ratatui::layout::Rect;
        let palette = Palette::new(ColorMode::Rainbow, true);
        let mut base = Buffer::empty(Rect::new(0, 0, 48, 2));
        base.set_string(0, 0, "_".repeat(48), palette.ground);
        base.set_string(0, 1, "hint", palette.text);
        let sample = |millis| {
            let mut buffer = base.clone();
            palette.animate(&mut buffer, Duration::from_millis(millis));
            buffer
        };
        assert_ne!(sample(0), sample(100));
        assert_eq!(sample(0), sample(99));
        assert_eq!(sample(0), sample(2400));
        assert_ne!(sample(0)[(0, 0)].fg, sample(0)[(4, 0)].fg);
        assert_eq!(sample(100)[(0, 1)].fg, Color::Reset);
        assert!(sample(100)
            .content
            .iter()
            .all(|cell| cell.bg == Color::Reset));
    }
}
