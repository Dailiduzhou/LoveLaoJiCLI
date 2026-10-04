//! Fixed-cell seven-segment ASCII digits. Scale stroke lengths, not stroke thickness.
use std::time::Duration;

const MAX_SCALE: u16 = 4;
// Bits: top, upper-right, lower-right, bottom, lower-left, upper-left, middle.
const DIGITS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];

pub(super) fn remaining(elapsed: Duration, wait: Duration) -> String {
    let left = wait.saturating_sub(elapsed);
    let seconds = left.as_secs() + u64::from(left.subsec_nanos() != 0);
    // Keep the format tied to the original timer, not the shrinking remainder.
    text(seconds, wait.as_secs() >= 3600)
}

fn text(seconds: u64, hours: bool) -> String {
    if hours {
        format!(
            "{:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

fn slot(scale: u16) -> u16 {
    4 * scale + 1
}
pub(super) fn height(scale: u16) -> u16 {
    if scale == 0 {
        0
    } else {
        slot(scale)
    }
}
pub(super) fn width(cells: usize, scale: u16) -> u16 {
    (slot(scale) + scale) * cells as u16 - scale
}
pub(super) fn fit(columns: u16, rows: u16, cells: usize) -> u16 {
    (1..=MAX_SCALE)
        .rev()
        .find(|&scale| width(cells, scale) <= columns && height(scale) <= rows)
        .unwrap_or(0)
}

pub(super) fn lines(time: &str, scale: u16) -> Vec<String> {
    debug_assert!((1..=MAX_SCALE).contains(&scale));
    let size = usize::from(slot(scale));
    let middle = size / 2;
    let bottom = size - 1;
    let mut canvas = vec![vec![b' '; usize::from(width(time.len(), scale))]; size];
    for (i, byte) in time.bytes().enumerate() {
        let x = i * (size + usize::from(scale));
        if byte == b':' {
            canvas[usize::from(scale)][x + middle] = b'o';
            canvas[usize::from(scale) * 3][x + middle] = b'o';
            continue;
        }
        let segments = DIGITS[usize::from(byte - b'0')];
        for (bit, y) in [(0, 0), (6, middle), (3, bottom)] {
            if segments & (1 << bit) != 0 {
                canvas[y][x + 1..x + bottom].fill(b'_');
            }
        }
        for (bit, column, from, to) in [
            (5, x, 1, middle),
            (1, x + bottom, 1, middle),
            (4, x, middle + 1, bottom),
            (2, x + bottom, middle + 1, bottom),
        ] {
            if segments & (1 << bit) != 0 {
                for row in &mut canvas[from..=to] {
                    row[column] = b'|';
                }
            }
        }
    }
    canvas
        .into_iter()
        .map(|row| String::from_utf8(row).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ten_digits_have_readable_native_strokes() {
        let expected = [
            [" ___ ", "|   |", "|   |", "|   |", "|___|"],
            ["     ", "    |", "    |", "    |", "    |"],
            [" ___ ", "    |", " ___|", "|    ", "|___ "],
            [" ___ ", "    |", " ___|", "    |", " ___|"],
            ["     ", "|   |", "|___|", "    |", "    |"],
            [" ___ ", "|    ", "|___ ", "    |", " ___|"],
            [" ___ ", "|    ", "|___ ", "|   |", "|___|"],
            [" ___ ", "    |", "    |", "    |", "    |"],
            [" ___ ", "|   |", "|___|", "|   |", "|___|"],
            [" ___ ", "|   |", "|___|", "    |", " ___|"],
        ];
        for (digit, expected) in expected.iter().enumerate() {
            assert_eq!(lines(&digit.to_string(), 1), *expected, "digit {digit}");
        }
    }

    #[test]
    fn scaling_keeps_every_stroke_one_cell_thick_and_connected() {
        for scale in 1..=MAX_SCALE {
            for digit in 0..10 {
                let lines = lines(&digit.to_string(), scale);
                let size = usize::from(slot(scale));
                assert_eq!(lines.len(), size);
                assert!(lines.iter().all(|row| row.len() == size));
                // Sample each native stroke location: scaling must not change its identity.
                let native = super::lines(&digit.to_string(), 1);
                for (y, row) in lines.iter().enumerate() {
                    for (x, byte) in row.bytes().enumerate() {
                        if byte == b'_' {
                            assert!([0, size / 2, size - 1].contains(&y));
                            assert_eq!(native[y / usize::from(scale)].as_bytes()[2], b'_');
                        } else if byte == b'|' {
                            assert!([0, size - 1].contains(&x));
                        }
                    }
                }
                for x in [0, size - 1] {
                    for (from, to, native_y) in [(1, size / 2, 1), (size / 2 + 1, size - 1, 3)] {
                        assert!(lines[from..=to].iter().all(|row| row.as_bytes()[x]
                            == native[native_y].as_bytes()[x / usize::from(scale)]));
                    }
                }
            }
        }
    }

    #[test]
    fn colons_and_slot_widths_stay_fixed_for_all_scales() {
        for scale in 1..=MAX_SCALE {
            for time in ["01:31", "00:11", "00:51", "00:00", "11:11:11", "00:59:59"] {
                let rows = lines(time, scale);
                assert_eq!(rows.len(), usize::from(height(scale)));
                assert!(rows
                    .iter()
                    .all(|row| row.len() == usize::from(width(time.len(), scale))));
                for (i, byte) in time.bytes().enumerate() {
                    if byte == b':' {
                        let x = i * usize::from(slot(scale) + scale) + usize::from(slot(scale) / 2);
                        for y in [scale, 3 * scale] {
                            assert_eq!(rows[usize::from(y)].as_bytes()[x], b'o');
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn format_does_not_shrink_at_the_hour_boundary() {
        assert_eq!(text(3600, true), "01:00:00");
        assert_eq!(text(3599, true), "00:59:59");
        assert_eq!(text(0, true), "00:00:00");
        assert_eq!(text(86400, true), "24:00:00");
        assert_eq!(text(3599, false), "59:59");
        assert_eq!(text(0, false), "00:00");
    }
}
