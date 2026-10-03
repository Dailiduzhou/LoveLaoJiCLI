//! Basic ANSI colors only: no truecolor probing or terminal background assumptions.
use ratatui::style::{Color, Style};
use std::ffi::OsStr;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(super) title: Style,
    pub(super) clock: Style,
    pub(super) grass: Style,
    pub(super) sun: Style,
    pub(super) flower: Style,
    pub(super) center: Style,
    pub(super) ground: Style,
}

impl Palette {
    pub(crate) fn detect() -> Self {
        Self::from_no_color(std::env::var_os("NO_COLOR").as_deref())
    }

    fn from_no_color(value: Option<&OsStr>) -> Self {
        Self::new(value.is_none_or(OsStr::is_empty))
    }

    pub(super) fn new(enabled: bool) -> Self {
        let style = |color| {
            if enabled {
                Style::default().fg(color)
            } else {
                Style::default()
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_nonempty_no_color_disables_colors() {
        for value in ["1", "0", "false"] {
            assert_eq!(
                Palette::from_no_color(Some(OsStr::new(value))).clock,
                Style::default()
            );
        }
        for value in [None, Some(OsStr::new(""))] {
            assert_eq!(
                Palette::from_no_color(value).clock.fg,
                Some(Color::LightCyan)
            );
        }
    }
}
