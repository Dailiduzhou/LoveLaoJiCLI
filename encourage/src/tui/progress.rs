use super::playback::Playback;
use cli_common::rainbow;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};
use std::time::Duration;

pub(super) const WIDTH: u16 = 38;
const READOUT_WIDTH: u16 = 6;
const HEART_SLOT_WIDTH: u16 = 4;
const HEARTS_WIDTH: u16 = HEART_SLOT_WIDTH * HEART_OFFSETS.len() as u16;
const TAIL_WIDTH: usize = (READOUT_WIDTH + HEARTS_WIDTH) as usize;
const PALETTE_PERIOD: f64 = 30.0;
// Every frame occupies two cells. Mixing one- and two-cell glyphs would move
// the center by half a cell even when a fixed container centers its content.
const HEART_FRAMES: [&str; 8] = ["♥️", "❤️", "💗", "💗", "💗", "❤️", "♥️", "♥️"];
const HEART_OFFSETS: [usize; 3] = [0, 2, 5];
const HEART_STEP_MS: u128 = 150;

pub(super) fn draw(frame: &mut Frame, area: Rect, playback: &Playback, colors: bool) {
    if area.height == 0 || area.width as usize <= TAIL_WIDTH {
        return;
    }
    let area = Rect { height: 1, ..area };
    let [meter, hearts] = Layout::horizontal([
        Constraint::Min(READOUT_WIDTH + 1),
        Constraint::Length(HEARTS_WIDTH),
    ])
    .areas(area);
    frame.render_widget(
        line(meter.width, playback.elapsed(), playback.total(), colors),
        meter,
    );
    let slots: [Rect; 3] =
        Layout::horizontal([Constraint::Length(HEART_SLOT_WIDTH); 3]).areas(hearts);
    for (index, slot) in slots.into_iter().enumerate() {
        frame.render_widget(heart_line(playback.elapsed(), index, colors), slot);
    }
}

fn heart(elapsed: Duration, offset: usize) -> &'static str {
    let tick = (elapsed.as_millis() / HEART_STEP_MS) % HEART_FRAMES.len() as u128;
    HEART_FRAMES[(tick as usize + offset) % HEART_FRAMES.len()]
}

fn heart_line(elapsed: Duration, index: usize, colors: bool) -> Line<'static> {
    let phase = elapsed.as_secs_f64() * 10.0;
    let style = if colors {
        Style::default().fg(rgb(rainbow::sample(
            phase + index as f64 * 2.0 + 22.0,
            PALETTE_PERIOD,
        )))
    } else {
        Style::default()
    };
    Line::from(Span::styled(
        heart(elapsed, HEART_OFFSETS[index]),
        style.add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center)
}

fn line(width: u16, elapsed: Duration, total: Duration, colors: bool) -> Line<'static> {
    let width = usize::from(width);
    if width <= READOUT_WIDTH as usize {
        return Line::default();
    }
    let progress = if total.is_zero() {
        100.0
    } else {
        (elapsed.as_secs_f64() / total.as_secs_f64() * 100.0).clamp(0.0, 100.0)
    };
    let bar_width = width - READOUT_WIDTH as usize;
    let filled = rainbow::filled_cells(progress, bar_width);
    let phase = elapsed.as_secs_f64() * 10.0;
    let mut spans = Vec::with_capacity(bar_width + 2);
    for cell in 0..filled {
        let style = if colors {
            let (upper, lower) = rainbow::cell_colors(cell, bar_width, phase, PALETTE_PERIOD, 1.0);
            Style::default().fg(rgb(upper)).bg(rgb(lower))
        } else {
            Style::default()
        };
        spans.push(Span::styled("▀", style));
    }
    spans.push(Span::raw("░".repeat(bar_width - filled)));
    // Never announce 100% until the credited playback budget is complete.
    spans.push(Span::raw(format!(" {:3.0}% ", progress.floor())));
    Line::from(spans)
}

fn rgb([red, green, blue]: [u8; 3]) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::playback::Key;
    use ratatui::{backend::TestBackend, Terminal};
    use std::collections::BTreeSet;

    const METER_WIDTH: u16 = WIDTH - HEARTS_WIDTH;

    #[test]
    fn real_progress_fills_monotonically_and_only_finishes_at_the_budget() {
        let total = Duration::from_secs(15);
        let start = line(METER_WIDTH, Duration::ZERO, total, true).to_string();
        assert_eq!(start.matches('░').count(), WIDTH as usize - TAIL_WIDTH);
        assert!(start.contains("  0% "));
        let half = line(METER_WIDTH, total / 2, total, true).to_string();
        assert!(half.contains(" 50% "));
        assert_eq!(half.matches('▀').count(), 10);
        let almost = line(METER_WIDTH, total - Duration::from_nanos(1), total, true).to_string();
        assert!(almost.contains(" 99% ") && !almost.contains("100%"));
        let end = line(METER_WIDTH, total, total, true).to_string();
        assert!(end.contains("100%"));
        assert!(!end.contains('░'));
        assert_eq!(end.matches('▀').count(), WIDTH as usize - TAIL_WIDTH);
        assert_eq!(
            line(METER_WIDTH, Duration::MAX, total, true).width(),
            METER_WIDTH as usize
        );
        assert!(line(METER_WIDTH, Duration::ZERO, Duration::ZERO, false)
            .to_string()
            .contains("100%"));
    }

    #[test]
    fn three_hearts_grow_shrink_and_repeat_without_width_jitter() {
        let cycle = Duration::from_millis((HEART_STEP_MS * HEART_FRAMES.len() as u128) as u64);
        for offset in HEART_OFFSETS {
            let mut seen = BTreeSet::new();
            for tick in 0..HEART_FRAMES.len() {
                let elapsed = Duration::from_millis((tick as u128 * HEART_STEP_MS) as u64);
                seen.insert(heart(elapsed, offset));
                assert_eq!(heart(elapsed, offset), heart(elapsed + cycle, offset));
                assert_eq!(Line::from(heart(elapsed, offset)).width(), 2);
                for index in 0..HEART_OFFSETS.len() {
                    let output = heart_line(elapsed, index, true);
                    assert_eq!(output.width(), 2);
                    assert_eq!(output.alignment, Some(Alignment::Center));
                }
            }
            assert_eq!(seen, BTreeSet::from(["♥️", "❤️", "💗"]));
        }
        let initial: BTreeSet<_> = HEART_OFFSETS
            .into_iter()
            .map(|offset| heart(Duration::ZERO, offset))
            .collect();
        assert_eq!(initial.len(), 3, "the hearts have staggered size phases");
    }

    #[test]
    fn marquee_uses_shared_diagonal_colors_and_no_color_changes_only_styles() {
        let elapsed = Duration::from_secs(3);
        let total = Duration::from_secs(15);
        let colored = line(METER_WIDTH, elapsed, total, true);
        let plain = line(METER_WIDTH, elapsed, total, false);
        assert_eq!(colored.to_string(), plain.to_string());
        let (upper, lower) =
            rainbow::cell_colors(0, WIDTH as usize - TAIL_WIDTH, 30.0, PALETTE_PERIOD, 1.0);
        assert_eq!(colored.spans[0].style.fg, Some(rgb(upper)));
        assert_eq!(colored.spans[0].style.bg, Some(rgb(lower)));
        assert_ne!(upper, lower);
        assert!(plain
            .spans
            .iter()
            .all(|span| span.style.fg.is_none() && span.style.bg.is_none()));
        assert_ne!(
            colored.spans[0].style,
            line(
                METER_WIDTH,
                elapsed + Duration::from_millis(20),
                total,
                true
            )
            .spans[0]
                .style
        );
    }

    #[test]
    fn pauses_interruptions_and_completion_freeze_the_visual_clock() {
        let mut playback = Playback::new(34);
        playback.advance(Duration::from_millis(750));
        let snapshot = |playback: &Playback| {
            (
                line(METER_WIDTH, playback.elapsed(), playback.total(), true),
                std::array::from_fn::<_, 3, _>(|index| heart_line(playback.elapsed(), index, true)),
            )
        };
        let before = snapshot(&playback);
        playback.key(Key::Space);
        playback.advance(Duration::from_secs(600));
        assert_eq!(snapshot(&playback), before);
        playback.key(Key::Space);
        playback.set_too_small(true);
        playback.advance(Duration::from_secs(600));
        assert_eq!(snapshot(&playback), before);
        playback.set_too_small(false);
        playback.set_suspended(true);
        playback.advance(Duration::from_secs(600));
        assert_eq!(snapshot(&playback), before);
        playback.set_suspended(false);
        playback.set_background(true);
        playback.advance(Duration::from_secs(600));
        assert_eq!(snapshot(&playback), before);
        playback.set_background(false);
        playback.key(Key::Enter);
        let completed = snapshot(&playback);
        assert!(completed.0.to_string().contains("100%"));
        playback.advance(Duration::from_secs(600));
        assert_eq!(snapshot(&playback), completed);
    }

    #[test]
    fn heart_centers_stay_fixed_across_every_pulse_and_progress_update() {
        for width in [WIDTH, WIDTH + 12] {
            for colors in [true, false] {
                let area = Rect::new(3, 1, width, 1);
                let mut terminal = Terminal::new(TestBackend::new(width + 6, 3)).unwrap();
                let mut playback = Playback::new(34);
                for _ in 0..=110 {
                    terminal
                        .draw(|frame| draw(frame, area, &playback, colors))
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    for (index, offset) in HEART_OFFSETS.into_iter().enumerate() {
                        let left = area.right() - HEARTS_WIDTH + index as u16 * HEART_SLOT_WIDTH;
                        assert_eq!(buffer[(left, area.y)].symbol(), " ");
                        assert_eq!(
                            buffer[(left + 1, area.y)].symbol(),
                            heart(playback.elapsed(), offset)
                        );
                        assert_eq!(buffer[(left + 3, area.y)].symbol(), " ");
                        // All two-cell glyphs have exactly the same cell center.
                        assert_eq!(
                            left + 1
                                + Line::from(buffer[(left + 1, area.y)].symbol()).width() as u16
                                    / 2,
                            left + HEART_SLOT_WIDTH / 2
                        );
                    }
                    playback.advance(Duration::from_millis(150));
                }
            }
        }
    }

    #[test]
    fn narrow_and_empty_areas_stay_bounded() {
        for width in 0..=80 {
            let output = line(width, Duration::from_secs(4), Duration::from_secs(15), true);
            assert!(output.width() <= width as usize);
            for height in [0, 1, 2] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| draw(frame, frame.area(), &Playback::new(34), true))
                    .unwrap();
                assert_eq!(
                    terminal.backend().buffer().area,
                    Rect::new(0, 0, width, height)
                );
            }
        }
    }
}
