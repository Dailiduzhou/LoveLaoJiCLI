//! The rainbow marquee: a looping palette sampled from `colors.png`, diagonal
//! color-band boundaries rendered through half-block cells, and a full
//! color-capability fallback cascade (truecolor → 256 → 16 → plain).
//!
//! The bar paints `color(x, y) = palette(x + k·y + phase)` where each cell is
//! one `▀` half-block: the foreground colors the upper half (y = 1.5), the
//! background the lower half (y = 0.5). With standard 2:1 cell metrics a
//! boundary has visual slope tan(θ) = 1/k, so the default 45° slant is k = 1
//! and the whole pattern slides one cell per tick as `phase` grows.

use crate::plan::BAR_WIDTH;

/// Phase units for one full trip around the palette. Equals the bar width so
/// the bar spans exactly one rainbow cycle at any moment.
pub const PERIOD: f64 = BAR_WIDTH as f64;

/// Palette anchors sampled from colors.png, pinned at their position in the
/// cycle. The first and last yellow are the same color, so the loop closes
/// without a seam.
const STOPS: [(f64, [u8; 3]); 13] = [
    (0.00, [244, 207, 90]),
    (0.13, [193, 239, 104]),
    (0.21, [111, 233, 168]),
    (0.29, [82, 224, 199]),
    (0.43, [95, 195, 235]),
    (0.56, [148, 154, 255]),
    (0.64, [173, 140, 255]),
    (0.70, [227, 104, 172]),
    (0.73, [253, 86, 128]),
    (0.78, [255, 119, 114]),
    (0.84, [255, 166, 98]),
    (0.90, [254, 201, 87]),
    (1.00, [244, 207, 90]),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorLevel {
    Truecolor,
    Ansi256,
    Ansi16,
    None,
}

/// The frozen look of one run: which escape dialect to speak and how slanted
/// the color bands are.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub level: ColorLevel,
    /// Phase offset between the two halves of a cell: cot(angle in degrees).
    pub k: f64,
}

impl Style {
    /// Read the environment once and freeze the show's look.
    pub fn detect() -> Self {
        Self {
            level: detect_level(),
            k: angle_k(),
        }
    }
}

/// The palette color at `phase` (phase units, wrapping every PERIOD).
pub fn sample(phase: f64) -> [u8; 3] {
    let position = phase.rem_euclid(PERIOD) / PERIOD;
    for pair in STOPS.windows(2) {
        let &(from, start) = &pair[0];
        let &(to, stop) = &pair[1];
        if position < to {
            return mix(start, stop, (position - from) / (to - from));
        }
    }
    STOPS[0].1
}

fn mix(from: [u8; 3], to: [u8; 3], t: f64) -> [u8; 3] {
    std::array::from_fn(|channel| {
        (f64::from(from[channel]) * (1.0 - t) + f64::from(to[channel]) * t).round() as u8
    })
}

/// Build one frame of the bar line: filled cells as half-block marquee cells,
/// empty cells plain, the readout and comfort message untouched by color.
pub fn render_line(style: &Style, progress: f64, phase: f64, message: &str) -> String {
    let filled = ((progress / 100.0) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.clamp(0, BAR_WIDTH);
    let mut line = String::from("\r");
    match style.level {
        ColorLevel::None => line.push_str(&"█".repeat(filled)),
        level => {
            for cell in 0..filled {
                let x = cell as f64;
                level.push_fg(sample(x + 1.5 * style.k + phase), &mut line);
                level.push_bg(sample(x + 0.5 * style.k + phase), &mut line);
                line.push('▀');
            }
            if filled > 0 {
                line.push_str("\u{1b}[0m");
            }
        }
    }
    line.push_str(&"░".repeat(BAR_WIDTH - filled));
    line.push_str(&format!(" {:>5.1}% {message}\u{1b}[K", progress));
    line
}

/// Get the terminal ready for colored frames. On classic Windows conhost the
/// VT escape mode must be switched on before any SGR reaches stderr; Windows
/// Terminal and POSIX terminals need nothing, and pipes are left alone.
pub fn prepare_terminal(style: &Style) {
    if style.level != ColorLevel::None {
        enable_vt();
    }
}

fn detect_level() -> ColorLevel {
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let forced = std::env::var("PATIENCE_COLOR").ok();
    level_from(no_color, &term, &colorterm, forced.as_deref())
}

fn level_from(no_color: bool, term: &str, colorterm: &str, forced: Option<&str>) -> ColorLevel {
    // NO_COLOR (any nonempty value) and TERM=dumb force plain output,
    // overriding even an explicit PATIENCE_COLOR.
    if no_color || term == "dumb" {
        return ColorLevel::None;
    }
    if let Some(level) = forced.and_then(parse_level) {
        return level;
    }
    let colorterm = colorterm.to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        ColorLevel::Truecolor
    } else if term.contains("256color") {
        ColorLevel::Ansi256
    } else if !term.is_empty() {
        ColorLevel::Ansi16
    } else if cfg!(windows) {
        // Windows terminals usually leave TERM unset; modern ConPTY and
        // conhost both handle at least 256 colors once VT mode is on.
        ColorLevel::Ansi256
    } else {
        ColorLevel::None
    }
}

fn parse_level(raw: &str) -> Option<ColorLevel> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "truecolor" | "24bit" => Some(ColorLevel::Truecolor),
        "256" | "256color" => Some(ColorLevel::Ansi256),
        "16" | "ansi16" => Some(ColorLevel::Ansi16),
        "none" | "off" => Some(ColorLevel::None),
        _ => None,
    }
}

const DEFAULT_ANGLE: f64 = 45.0;
/// Steepest slant worth rendering: with only two half-cell rows to sample,
/// larger offsets between the halves just alias the palette.
const MAX_K: f64 = 15.0;

fn angle_k() -> f64 {
    let raw = std::env::var("PATIENCE_ANGLE").unwrap_or_default();
    k_from_degrees(parse_angle(&raw).unwrap_or(DEFAULT_ANGLE))
}

fn parse_angle(raw: &str) -> Option<f64> {
    let degrees: f64 = raw.trim().parse().ok()?;
    (0.0..=90.0).contains(&degrees).then_some(degrees)
}

fn k_from_degrees(degrees: f64) -> f64 {
    // A boundary has visual slope tan(θ) = 1/k in a 2:1 cell grid, so
    // k = cot(θ): 90° → 0 (vertical bands), 45° → 1 (the default slant).
    degrees.to_radians().tan().recip().clamp(0.0, MAX_K)
}

impl ColorLevel {
    fn push_fg(self, rgb: [u8; 3], out: &mut String) {
        match self {
            Self::Truecolor => {
                out.push_str(&format!("\u{1b}[38;2;{};{};{}m", rgb[0], rgb[1], rgb[2]))
            }
            Self::Ansi256 => out.push_str(&format!("\u{1b}[38;5;{}m", ansi256(rgb))),
            Self::Ansi16 => {
                let index = ansi16(rgb);
                if index < 8 {
                    out.push_str(&format!("\u{1b}[3{index}m"));
                } else {
                    out.push_str(&format!("\u{1b}[9{}m", index - 8));
                }
            }
            Self::None => {}
        }
    }

    fn push_bg(self, rgb: [u8; 3], out: &mut String) {
        match self {
            Self::Truecolor => {
                out.push_str(&format!("\u{1b}[48;2;{};{};{}m", rgb[0], rgb[1], rgb[2]))
            }
            Self::Ansi256 => out.push_str(&format!("\u{1b}[48;5;{}m", ansi256(rgb))),
            Self::Ansi16 => {
                let index = ansi16(rgb);
                if index < 8 {
                    out.push_str(&format!("\u{1b}[4{index}m"));
                } else {
                    out.push_str(&format!("\u{1b}[10{}m", index - 8));
                }
            }
            Self::None => {}
        }
    }
}

/// Nearest xterm-256 index for `rgb`, searching the 6×6×6 cube and the gray
/// ramp (indices 16..=255; the terminal-defined 0..=15 are left alone).
fn ansi256(rgb: [u8; 3]) -> u8 {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let mut best = 16usize;
    let mut best_distance = u32::MAX;
    for (red, &red_value) in CUBE.iter().enumerate() {
        for (green, &green_value) in CUBE.iter().enumerate() {
            for (blue, &blue_value) in CUBE.iter().enumerate() {
                let candidate = [red_value, green_value, blue_value];
                let distance = squared_distance(rgb, candidate);
                if distance < best_distance {
                    best_distance = distance;
                    best = 16 + 36 * red + 6 * green + blue;
                }
            }
        }
    }
    for step in 0..=23usize {
        let gray = (8 + 10 * step) as u8;
        let distance = squared_distance(rgb, [gray; 3]);
        if distance < best_distance {
            best_distance = distance;
            best = 232 + step;
        }
    }
    best as u8
}

/// Typical RGB values of the 16 base ANSI colors (xterm defaults).
const ANSI16: [[u8; 3]; 16] = [
    [0, 0, 0],
    [205, 0, 0],
    [0, 205, 0],
    [205, 205, 0],
    [0, 0, 238],
    [205, 0, 205],
    [0, 205, 205],
    [229, 229, 229],
    [127, 127, 127],
    [255, 85, 85],
    [85, 255, 85],
    [255, 255, 85],
    [85, 85, 255],
    [255, 85, 255],
    [85, 255, 255],
    [255, 255, 255],
];

fn ansi16(rgb: [u8; 3]) -> usize {
    let mut best = 0usize;
    let mut best_distance = u32::MAX;
    for (index, candidate) in ANSI16.iter().enumerate() {
        let distance = squared_distance(rgb, *candidate);
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    best
}

fn squared_distance(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let delta = i32::from(x) - i32::from(y);
            (delta * delta) as u32
        })
        .sum()
}

#[cfg(not(windows))]
fn enable_vt() {}

#[cfg(windows)]
fn enable_vt() {
    use std::os::raw::{c_ulong, c_void};

    const STD_ERROR_HANDLE: c_ulong = -12i32 as c_ulong;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: c_ulong = 0x0004;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(handle: c_ulong) -> *mut c_void;
        fn GetConsoleMode(console: *mut c_void, mode: *mut c_ulong) -> i32;
        fn SetConsoleMode(console: *mut c_void, mode: c_ulong) -> i32;
    }

    // Best effort: when stderr is not a console (a pipe, a test runner) the
    // console calls simply fail and the mode is left untouched.
    unsafe {
        let console = GetStdHandle(STD_ERROR_HANDLE);
        if console.is_null() {
            return;
        }
        let mut mode = 0;
        if GetConsoleMode(console, &mut mode) != 0 {
            SetConsoleMode(console, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ansi16, ansi256, k_from_degrees, level_from, parse_angle, parse_level, render_line, sample,
        ColorLevel, Style, MAX_K, PERIOD,
    };

    fn style(level: ColorLevel, k: f64) -> Style {
        Style { level, k }
    }

    #[test]
    fn palette_wraps_and_matches_colors_png() {
        assert_eq!(sample(0.0), [244, 207, 90]);
        assert_eq!(sample(PERIOD), [244, 207, 90]);
        assert_eq!(sample(0.13 * PERIOD), [193, 239, 104]);
        assert_eq!(sample(0.56 * PERIOD), [148, 154, 255]);
        assert_eq!(sample(0.70 * PERIOD), [227, 104, 172]);
        assert_eq!(sample(0.90 * PERIOD), [254, 201, 87]);
    }

    #[test]
    fn palette_moves_smoothly_across_the_seam() {
        let before = sample(PERIOD - 0.05);
        let after = sample(0.05);
        for channel in 0..3 {
            let jump = (i32::from(before[channel]) - i32::from(after[channel])).abs();
            assert!(jump <= 3, "channel {channel} jumps {jump}");
        }
    }

    #[test]
    fn angle_validation_and_cotangent_mapping() {
        assert_eq!(parse_angle("45"), Some(45.0));
        assert_eq!(parse_angle("  30.5 "), Some(30.5));
        assert_eq!(parse_angle("0"), Some(0.0));
        assert_eq!(parse_angle("90"), Some(90.0));
        assert_eq!(parse_angle("bogus"), None);
        assert_eq!(parse_angle("120"), None);
        assert_eq!(parse_angle("-5"), None);
        assert!((k_from_degrees(45.0) - 1.0).abs() < 1e-9);
        assert!(k_from_degrees(90.0).abs() < 1e-9);
        assert_eq!(k_from_degrees(0.0), MAX_K);
        assert!((k_from_degrees(30.0) - 3f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn color_level_cascade() {
        use ColorLevel::{Ansi16, Ansi256, Truecolor};
        let none = ColorLevel::None;
        assert_eq!(parse_level("TRUECOLOR"), Some(Truecolor));
        assert_eq!(parse_level("24bit"), Some(Truecolor));
        assert_eq!(parse_level("256"), Some(Ansi256));
        assert_eq!(parse_level("16"), Some(Ansi16));
        assert_eq!(parse_level("none"), Some(none));
        assert_eq!(parse_level("junk"), None);
        // NO_COLOR and TERM=dumb outrank an explicit override.
        assert_eq!(
            level_from(true, "xterm-256color", "truecolor", Some("256")),
            none
        );
        assert_eq!(level_from(false, "dumb", "", Some("truecolor")), none);
        // An explicit override wins over autodetection...
        assert_eq!(
            level_from(false, "xterm-256color", "truecolor", Some("16")),
            Ansi16
        );
        assert_eq!(level_from(false, "xterm", "", Some("none")), none);
        // ...but a bad value falls back to autodetection.
        assert_eq!(
            level_from(false, "xterm-256color", "", Some("junk")),
            Ansi256
        );
        // Autodetection: COLORTERM first, then TERM richness.
        assert_eq!(level_from(false, "xterm", "truecolor", None), Truecolor);
        assert_eq!(level_from(false, "xterm", "24bit", None), Truecolor);
        assert_eq!(level_from(false, "xterm-256color", "", None), Ansi256);
        assert_eq!(level_from(false, "screen", "", None), Ansi16);
        #[cfg(not(windows))]
        assert_eq!(level_from(false, "", "", None), none);
        #[cfg(windows)]
        assert_eq!(level_from(false, "", "", None), Ansi256);
    }

    #[test]
    fn uncolored_renders_the_classic_plain_bar() {
        let line = render_line(&style(ColorLevel::None, 1.0), 50.0, 3.0, "hang on");
        assert!(line.starts_with('\r'));
        assert!(line.contains(&"█".repeat(15)));
        assert!(line.contains(&"░".repeat(15)));
        assert!(line.contains("50.0% hang on"));
        assert!(!line.contains("38;2;"));
        assert!(!line.contains("38;5;"));
        assert!(line.ends_with("\u{1b}[K"));
    }

    #[test]
    fn truecolor_renders_half_block_marquee_cells() {
        let line = render_line(&style(ColorLevel::Truecolor, 1.0), 100.0, 0.0, "");
        assert_eq!(line.matches('▀').count(), 30);
        assert!(!line.contains('█'));
        assert!(!line.contains('░'));
        assert!(line.contains("38;2;"));
        assert!(line.contains("48;2;"));
        assert!(line.contains("\u{1b}[0m"));
        assert!(line.contains("100.0% "));
    }

    #[test]
    fn marquee_slides_with_phase_and_cycles() {
        let style = style(ColorLevel::Truecolor, 1.0);
        let first = render_line(&style, 100.0, 0.0, "");
        let second = render_line(&style, 100.0, 1.0, "");
        assert_ne!(first, second);
        // One full period later the show is byte-identical again.
        assert_eq!(first, render_line(&style, 100.0, PERIOD, ""));
    }

    #[test]
    fn forty_five_degrees_splits_the_cell_diagonally() {
        // k = cot(θ): at 90° both halves of a cell share one color (vertical
        // bands); at 45° the halves differ (the visible diagonal boundary).
        let vertical = render_line(&style(ColorLevel::Truecolor, 0.0), 100.0, 0.0, "");
        let diagonal = render_line(&style(ColorLevel::Truecolor, 1.0), 100.0, 0.0, "");
        let (fg, bg) = first_cell_codes(&vertical);
        assert_eq!(fg, bg.replace("48;2;", "38;2;"));
        let (fg, bg) = first_cell_codes(&diagonal);
        assert_ne!(fg, bg.replace("48;2;", "38;2;"));
    }

    fn first_cell_codes(line: &str) -> (String, String) {
        let cells = &line[1..line.find('▀').expect("marquee cells")];
        let split = cells.find('m').expect("fg code") + 1;
        (cells[..split].to_owned(), cells[split..].to_owned())
    }

    #[test]
    fn fallback_levels_use_their_own_escape_syntax() {
        let line = render_line(&style(ColorLevel::Ansi256, 1.0), 100.0, 0.0, "");
        assert!(line.contains("38;5;"));
        assert!(line.contains("48;5;"));
        let line = render_line(&style(ColorLevel::Ansi16, 1.0), 100.0, 0.0, "");
        assert!(line.contains("\u{1b}[9") || line.contains("\u{1b}[3"));
        assert!(line.contains("\u{1b}[10") || line.contains("\u{1b}[4"));
    }

    #[test]
    fn ansi256_picks_known_cube_and_gray_indices() {
        assert_eq!(ansi256([0, 0, 0]), 16);
        assert_eq!(ansi256([95, 0, 0]), 52);
        assert_eq!(ansi256([255, 255, 255]), 231);
        assert_eq!(ansi256([128, 128, 128]), 244);
    }

    #[test]
    fn ansi16_picks_bright_neighbours() {
        assert_eq!(ansi16([255, 85, 85]), 9);
        assert_eq!(ansi16([244, 207, 90]), 11);
        assert_eq!(ansi16([148, 154, 255]), 12);
    }

    #[test]
    fn empty_bar_wastes_no_color() {
        let line = render_line(&style(ColorLevel::Truecolor, 1.0), 0.0, 0.0, "");
        assert!(!line.contains("38;2;"));
        assert!(!line.contains("\u{1b}[0m"));
        assert_eq!(line.matches('░').count(), 30);
    }

    #[test]
    fn progress_is_clamped_to_the_bar() {
        let over = render_line(&style(ColorLevel::Truecolor, 1.0), 250.0, 0.0, "");
        assert_eq!(over.matches('▀').count(), 30);
        assert!(!over.contains('░'));
        let under = render_line(&style(ColorLevel::None, 1.0), -5.0, 0.0, "");
        assert!(!under.contains('█'));
        assert_eq!(under.matches('░').count(), 30);
    }
}
