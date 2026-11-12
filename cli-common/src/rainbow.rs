//! Backend-independent rainbow sampling and progress-bar geometry.

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

pub fn sample(phase: f64, period: f64) -> [u8; 3] {
    if !phase.is_finite() || !period.is_finite() || period <= 0.0 {
        return STOPS[0].1;
    }
    let position = phase.rem_euclid(period) / period;
    for pair in STOPS.windows(2) {
        let &(from, start) = &pair[0];
        let &(to, stop) = &pair[1];
        if position < to {
            let t = (position - from) / (to - from);
            return std::array::from_fn(|channel| {
                (f64::from(start[channel]) * (1.0 - t) + f64::from(stop[channel]) * t).round() as u8
            });
        }
    }
    STOPS[0].1
}

pub fn filled_cells(progress_percent: f64, width: usize) -> usize {
    (((progress_percent / 100.0) * width as f64).round() as usize).min(width)
}

/// Upper/lower RGB samples for one diagonal half-block cell.
pub fn cell_colors(
    cell: usize,
    width: usize,
    phase: f64,
    period: f64,
    slant: f64,
) -> ([u8; 3], [u8; 3]) {
    let x = if width == 0 {
        0.0
    } else {
        cell as f64 * period / width as f64
    };
    (
        sample(x + 1.5 * slant + phase, period),
        sample(x + 0.5 * slant + phase, period),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_matches_patience_anchors_and_wraps_smoothly() {
        let period = 30.0;
        assert_eq!(sample(0.0, period), [244, 207, 90]);
        assert_eq!(sample(period, period), sample(0.0, period));
        assert_eq!(sample(-period, period), sample(0.0, period));
        for (position, rgb) in STOPS {
            assert_eq!(sample(position * period, period), rgb);
        }
        let before = sample(period - 0.05, period);
        let after = sample(0.05, period);
        for channel in 0..3 {
            assert!((i32::from(before[channel]) - i32::from(after[channel])).abs() <= 3);
        }
        assert_ne!(sample(0.0, period), sample(1.0, period));
        assert_eq!(sample(f64::NAN, period), STOPS[0].1);
        assert_eq!(sample(1.0, 0.0), STOPS[0].1);
    }

    #[test]
    fn filled_cells_are_bounded_for_empty_invalid_and_extreme_inputs() {
        for width in [0, 1, 15, 30, 100] {
            assert_eq!(filled_cells(0.0, width), 0);
            assert_eq!(filled_cells(-10.0, width), 0);
            assert_eq!(filled_cells(f64::NAN, width), 0);
            assert_eq!(filled_cells(100.0, width), width);
            assert_eq!(filled_cells(250.0, width), width);
            assert!(filled_cells(f64::INFINITY, width) <= width);
        }
        assert_eq!(filled_cells(50.0, 30), 15);
    }

    #[test]
    fn half_blocks_keep_the_original_diagonal_and_scale_with_width() {
        let period = 30.0;
        let diagonal = cell_colors(0, 30, 0.0, period, 1.0);
        assert_eq!(diagonal, (sample(1.5, period), sample(0.5, period)));
        assert_ne!(diagonal.0, diagonal.1);
        let vertical = cell_colors(0, 30, 0.0, period, 0.0);
        assert_eq!(vertical.0, vertical.1);
        assert_eq!(
            cell_colors(5, 30, 2.0, period, 1.0),
            cell_colors(10, 60, 2.0, period, 1.0)
        );
        assert_eq!(cell_colors(0, 0, 0.0, period, 1.0), diagonal);
    }
}
