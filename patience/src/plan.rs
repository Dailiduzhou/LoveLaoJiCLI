//! The show schedule: which speed curve runs, where the bar stalls, and how
//! long every act lasts. A `Plan` is pure data with a pure evaluator, so the
//! whole comedy is deterministic once the RNG is seeded.

use rand::Rng;

use crate::curve::Curve;

/// Width of the rendered bar, in cells.
pub const BAR_WIDTH: usize = 30;

/// Cap on captured child output, per stream.
pub const OUTPUT_CAP: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /// Progress moves from `from` to `to` over `duration` seconds of wall
    /// clock time, following `curve`.
    Climb {
        curve: Curve,
        from: f64,
        to: f64,
        duration: f64,
    },
    /// Progress holds at `point` for `duration` seconds. The final stall has
    /// an infinite duration: it ends only when the child process exits.
    Stall { point: f64, duration: f64 },
}

impl Segment {
    fn duration(&self) -> f64 {
        match *self {
            Segment::Climb { duration, .. } | Segment::Stall { duration, .. } => duration,
        }
    }

    fn point(&self) -> f64 {
        match *self {
            Segment::Climb { to, .. } => to,
            Segment::Stall { point, .. } => point,
        }
    }

    pub fn is_stall(&self) -> bool {
        matches!(self, Segment::Stall { .. })
    }
}

/// One run's entire script, scaled by `scale` (0.01 under `PATIENCE_FAST=1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub segments: Vec<Segment>,
    /// The show runs at least this long even if the child exits instantly.
    pub min_show: f64,
    /// Seconds between animation frames.
    pub tick: f64,
    /// Seconds per cell during the rapid fill on success.
    pub fill_frame: f64,
}

impl Plan {
    /// Total scripted time; the final infinite stall is excluded.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn scripted_time(&self) -> f64 {
        self.segments
            .iter()
            .map(Segment::duration)
            .filter(|duration| duration.is_finite())
            .sum()
    }

    /// Progress (0..100) to display `elapsed` seconds into the show.
    pub fn progress(&self, elapsed: f64) -> f64 {
        let mut remaining = elapsed;
        for segment in &self.segments {
            if remaining < segment.duration() {
                return match *segment {
                    Segment::Climb {
                        curve,
                        from,
                        to,
                        duration,
                    } => from + curve.apply(remaining / duration) * (to - from),
                    Segment::Stall { point, .. } => point,
                };
            }
            remaining -= segment.duration();
        }
        // Past the script: hold at the final stall forever.
        self.final_point()
    }

    /// Index of the segment playing `elapsed` seconds into the show.
    pub fn segment_at(&self, elapsed: f64) -> usize {
        let mut remaining = elapsed;
        for (index, segment) in self.segments.iter().enumerate() {
            if remaining < segment.duration() {
                return index;
            }
            remaining -= segment.duration();
        }
        self.segments.len() - 1
    }

    /// Where the bar sits once the script runs out (the final stall point).
    pub fn final_point(&self) -> f64 {
        self.segments.last().map(Segment::point).unwrap_or(100.0)
    }
}

/// Roll a random show: one curve, 1..=4 short stalls, and a final stall that
/// ends only when the child does. All durations are multiplied by `scale`.
pub fn generate(rng: &mut impl Rng, scale: f64) -> Plan {
    let curve = Curve::pick(rng);
    let final_point: f64 = rng.gen_range(97.0..99.9);

    // Short stall points strictly increase with at least 3% between them.
    let top = final_point - 6.0;
    let mut points: Vec<f64> = Vec::new();
    let mut previous = 5.0_f64;
    for _ in 0..rng.gen_range(1..=4) {
        let point: f64 = rng.gen_range(8.0..top).max(previous + 3.0);
        if point > top {
            break;
        }
        points.push(point);
        previous = point;
    }
    if points.is_empty() {
        points.push((8.0 + top) / 2.0);
    }

    let mut segments: Vec<Segment> = Vec::new();
    let mut from = 0.0;
    for &point in &points {
        segments.push(Segment::Climb {
            curve,
            from,
            to: point,
            duration: rng.gen_range(2.0..6.0) * scale,
        });
        segments.push(Segment::Stall {
            point,
            duration: rng.gen_range(0.5..2.0) * scale,
        });
        from = point;
    }
    segments.push(Segment::Climb {
        curve,
        from,
        to: final_point,
        duration: rng.gen_range(2.0..6.0) * scale,
    });
    segments.push(Segment::Stall {
        point: final_point,
        duration: f64::INFINITY,
    });

    Plan {
        segments,
        min_show: rng.gen_range(0.5..1.5) * scale,
        tick: 0.05 * scale,
        fill_frame: 0.015 * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::{generate, Segment};
    use crate::curve::{Curve, ALL};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn plan_for(seed: u64, scale: f64) -> super::Plan {
        let mut rng = StdRng::seed_from_u64(seed);
        generate(&mut rng, scale)
    }

    #[test]
    fn script_alternates_climbs_and_stalls_and_ends_high() {
        for seed in 0..50u64 {
            let plan = plan_for(seed, 1.0);
            assert!(
                plan.segments.len() >= 4,
                "seed {seed}: need at least one short stall"
            );
            assert!(matches!(plan.segments[0], Segment::Climb { .. }));
            for (index, segment) in plan.segments.iter().enumerate() {
                let should_stall = index % 2 == 1;
                assert_eq!(
                    segment.is_stall(),
                    should_stall,
                    "seed {seed}: segment {index} out of pattern"
                );
            }
            let Segment::Stall { point, duration } = plan.segments[plan.segments.len() - 1] else {
                panic!("seed {seed}: must end on a stall");
            };
            assert_eq!(duration, f64::INFINITY);
            assert!((97.0..99.9).contains(&point), "seed {seed}: {point}");
        }
    }

    #[test]
    fn progress_starts_at_zero_and_holds_at_final_point() {
        let plan = plan_for(7, 1.0);
        assert!(plan.progress(0.0).abs() < 1e-9);
        let after = plan.scripted_time() + 10.0;
        assert!((plan.progress(after) - plan.final_point()).abs() < 1e-9);
    }

    #[test]
    fn progress_is_monotonic_and_never_exceeds_the_final_point() {
        for seed in 0..50u64 {
            let plan = plan_for(seed, 1.0);
            let total = plan.scripted_time() + 1.0;
            let mut previous = plan.progress(0.0);
            for step in 1..=(total as usize * 20) {
                let elapsed = step as f64 / 20.0;
                let progress = plan.progress(elapsed);
                assert!(
                    progress >= previous - 1e-9,
                    "seed {seed}: progress went backwards at {elapsed}"
                );
                assert!(
                    progress <= plan.final_point() + 1e-9,
                    "seed {seed}: overshot at {elapsed}"
                );
                previous = progress;
            }
        }
    }

    #[test]
    fn fast_mode_scales_every_timing_down() {
        let slow = plan_for(3, 1.0);
        let fast = plan_for(3, 0.01);
        // An upper bound alone accepts zero timings or an incorrect 50x
        // compression. Require the documented 100x scale for every segment.
        let scaled = |actual: f64, original: f64| {
            assert!((actual - original * 0.01).abs() < 1e-9);
        };
        assert_eq!(fast.segments.len(), slow.segments.len());
        for (fast, slow) in fast.segments.iter().zip(&slow.segments) {
            match (fast, slow) {
                (
                    Segment::Climb {
                        curve: fc,
                        from: ff,
                        to: ft,
                        duration: fd,
                    },
                    Segment::Climb {
                        curve: sc,
                        from: sf,
                        to: st,
                        duration: sd,
                    },
                ) => {
                    assert_eq!((fc, ff, ft), (sc, sf, st));
                    scaled(*fd, *sd);
                }
                (
                    Segment::Stall {
                        point: fp,
                        duration: fd,
                    },
                    Segment::Stall {
                        point: sp,
                        duration: sd,
                    },
                ) => {
                    assert_eq!(fp, sp);
                    if sd.is_infinite() {
                        assert_eq!(fd, sd);
                    } else {
                        scaled(*fd, *sd);
                    }
                }
                _ => panic!("scaling changed the show's structure"),
            }
        }
        scaled(fast.scripted_time(), slow.scripted_time());
        scaled(fast.min_show, slow.min_show);
        scaled(fast.tick, slow.tick);
        scaled(fast.fill_frame, slow.fill_frame);
    }

    #[test]
    fn different_seeds_produce_different_shows() {
        let first = plan_for(1, 1.0);
        let mut differs = 0;
        for seed in 2..20u64 {
            if plan_for(seed, 1.0) != first {
                differs += 1;
            }
        }
        assert!(differs >= 10, "only {differs} distinct plans in 18 seeds");
    }

    #[test]
    fn every_curve_appears_across_seeds() {
        let mut seen: Vec<Curve> = Vec::new();
        for seed in 0..200u64 {
            if let Segment::Climb { curve, .. } = plan_for(seed, 1.0).segments[0] {
                if !seen.contains(&curve) {
                    seen.push(curve);
                }
            }
        }
        assert_eq!(seen.len(), ALL.len(), "some curve never got picked");
    }
}
