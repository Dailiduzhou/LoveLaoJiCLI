//! Speed curves for the fake progress bar.
//!
//! Each curve maps normalized elapsed time (0..=1) of a climb segment to
//! normalized progress (0..=1) through that segment.

use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// Constant speed.
    Linear,
    /// Starts slow, ends fast.
    EaseIn,
    /// Starts fast, ends slow.
    EaseOut,
    /// S-shaped: slow at both ends, fast in the middle.
    Sigmoid,
    /// Jumps in discrete steps like a badly written installer.
    Stepped,
    /// Gains shrink as it goes; looks busy but never quite arrives.
    Exponential,
    /// Square root of time: fast off the line, then a long slow creep.
    Sqrt,
}

pub const ALL: [Curve; 7] = [
    Curve::Linear,
    Curve::EaseIn,
    Curve::EaseOut,
    Curve::Sigmoid,
    Curve::Stepped,
    Curve::Exponential,
    Curve::Sqrt,
];

impl Curve {
    /// Pick a curve uniformly at random.
    pub fn pick(rng: &mut impl Rng) -> Self {
        *ALL.choose(rng).expect("ALL is non-empty")
    }

    /// Apply the curve to normalized time, clamped to 0..=1.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::EaseIn => t * t * t,
            Curve::EaseOut => 1.0 - (1.0 - t).powi(3),
            Curve::Sigmoid => t * t * (3.0 - 2.0 * t),
            Curve::Stepped => (t * 6.0).floor() / 6.0,
            Curve::Exponential => (1.0 - (-4.0 * t).exp()) / (1.0 - (-4.0_f64).exp()),
            Curve::Sqrt => t.sqrt(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Curve, ALL};

    #[test]
    fn curves_start_at_zero_and_end_at_one() {
        for curve in ALL {
            assert!((curve.apply(0.0)).abs() < 1e-9, "{curve:?}");
            assert!((curve.apply(1.0) - 1.0).abs() < 1e-9, "{curve:?}");
        }
    }

    #[test]
    fn curves_are_monotonic_and_bounded() {
        for curve in ALL {
            let mut previous = curve.apply(0.0);
            for step in 1..=100 {
                let t = step as f64 / 100.0;
                let value = curve.apply(t);
                assert!((0.0..=1.0).contains(&value), "{curve:?} at {t}");
                assert!(value >= previous - 1e-9, "{curve:?} not monotonic at {t}");
                previous = value;
            }
        }
    }

    #[test]
    fn curve_family_has_distinct_shapes() {
        // The classic ordering at the midpoint keeps the family from collapsing
        // into duplicates with different names.
        assert!(Curve::EaseIn.apply(0.5) < Curve::Linear.apply(0.5));
        assert!(Curve::Linear.apply(0.5) < Curve::EaseOut.apply(0.5));
        assert!(Curve::Stepped.apply(0.5) < Curve::Exponential.apply(0.5));
        // Sqrt sits between Linear and EaseOut: still an ease-out, but a
        // gentler one. Two curves sharing a midpoint would be one curve.
        assert!(Curve::Linear.apply(0.5) < Curve::Sqrt.apply(0.5));
        assert!(Curve::Sqrt.apply(0.5) < Curve::EaseOut.apply(0.5));
    }

    #[test]
    fn sqrt_curve_is_wired_up_and_pinned() {
        assert!(
            ALL.contains(&Curve::Sqrt),
            "Sqrt must be reachable by pick()"
        );
        // apply(t)^2 == t is the curve's defining property, so check it as a
        // relative error across the range instead of comparing the curve
        // against the same expression it is implemented with. A literal
        // f64::EPSILON would be only two ULPs at this magnitude.
        for step in 1..=9 {
            let t = step as f64 / 10.0;
            let squared = Curve::Sqrt.apply(t).powi(2);
            let error = (squared - t).abs();
            assert!(error < 1e-12, "t={t}: squared error {error}");
        }
        // 0.25 is 2^-2, so its square root is exactly representable.
        assert_eq!(Curve::Sqrt.apply(0.25), 0.5);
    }
}
