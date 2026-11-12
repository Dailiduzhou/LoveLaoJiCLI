//! Pure playback state. Terminal I/O and wall-clock reads live in `tui::mod`.

use crate::duration::PlaybackDuration;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    TooSmall,
    Suspended,
    Background,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaseState {
    Playing,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interruption {
    TooSmall,
    Suspended,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Space,
    Enter,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Confirm,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct Playback {
    base: BaseState,
    interruption: Option<Interruption>,
    stage_count: usize,
    elapsed: Duration,
    total: Duration,
}

impl Playback {
    pub fn new(stage_count: usize) -> Self {
        Self::with_duration(stage_count, PlaybackDuration::default())
    }

    pub fn with_duration(stage_count: usize, duration: PlaybackDuration) -> Self {
        Self {
            base: if stage_count == 0 {
                BaseState::Completed
            } else {
                BaseState::Playing
            },
            interruption: None,
            stage_count,
            elapsed: if stage_count == 0 {
                duration.get()
            } else {
                Duration::ZERO
            },
            total: duration.get(),
        }
    }

    pub fn state(&self) -> PlaybackState {
        match self.interruption {
            Some(Interruption::TooSmall) => PlaybackState::TooSmall,
            Some(Interruption::Suspended) => PlaybackState::Suspended,
            Some(Interruption::Background) => PlaybackState::Background,
            None => match self.base {
                BaseState::Playing => PlaybackState::Playing,
                BaseState::Paused => PlaybackState::Paused,
                BaseState::Completed => PlaybackState::Completed,
                BaseState::Cancelled => PlaybackState::Cancelled,
                BaseState::Failed => PlaybackState::Failed,
            },
        }
    }

    pub fn stage_index(&self) -> Option<usize> {
        if self.stage_count == 0 {
            return None;
        }
        // Rational scheduling avoids rounding each of the 34 intervals: even
        // a budget not divisible by the stage count completes at exactly total.
        Some(
            ((self.elapsed.as_nanos() * self.stage_count as u128 / self.total.as_nanos()) as usize)
                .min(self.stage_count - 1),
        )
    }

    pub fn stage_count(&self) -> usize {
        self.stage_count
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn total(&self) -> Duration {
        self.total
    }

    pub fn is_completed(&self) -> bool {
        self.base == BaseState::Completed
    }

    /// Advance only uninterrupted playing time; duration is not a wall-clock
    /// deadline, nor a mandatory minimum before the user may skip/cancel.
    pub fn advance(&mut self, elapsed: Duration) {
        if self.base != BaseState::Playing || self.interruption.is_some() {
            return;
        }
        self.elapsed = self.elapsed.saturating_add(elapsed).min(self.total);
        if self.elapsed == self.total {
            self.base = BaseState::Completed;
        }
    }

    pub fn key(&mut self, key: Key) -> Action {
        if matches!(self.base, BaseState::Cancelled | BaseState::Failed) {
            return Action::Continue;
        }
        match key {
            Key::Space => {
                match self.base {
                    BaseState::Playing => self.base = BaseState::Paused,
                    BaseState::Paused => self.base = BaseState::Playing,
                    BaseState::Completed | BaseState::Cancelled | BaseState::Failed => {}
                }
                Action::Continue
            }
            Key::Enter if self.base == BaseState::Completed => Action::Confirm,
            Key::Enter => {
                self.base = BaseState::Completed;
                self.elapsed = self.total;
                Action::Continue
            }
            Key::Quit => {
                self.base = BaseState::Cancelled;
                self.interruption = None;
                Action::Cancel
            }
        }
    }

    pub fn set_too_small(&mut self, too_small: bool) {
        self.set_interruption(Interruption::TooSmall, too_small);
    }

    pub fn set_suspended(&mut self, suspended: bool) {
        self.set_interruption(Interruption::Suspended, suspended);
    }

    pub fn set_background(&mut self, background: bool) {
        self.set_interruption(Interruption::Background, background);
    }

    fn set_interruption(&mut self, interruption: Interruption, active: bool) {
        if matches!(self.base, BaseState::Cancelled | BaseState::Failed) {
            return;
        }
        if active {
            self.interruption = Some(interruption);
        } else if self.interruption == Some(interruption) {
            self.interruption = None;
        }
    }

    pub fn fail(&mut self) {
        self.base = BaseState::Failed;
        self.interruption = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_is_fifteen_seconds_regardless_of_stage_count() {
        for count in [1, 3, 12, 34, 35, 1000] {
            let mut playback = Playback::new(count);
            playback.advance(Duration::from_secs(15) - Duration::from_nanos(1));
            assert_eq!(playback.state(), PlaybackState::Playing);
            assert_eq!(playback.stage_index(), Some(count - 1));
            playback.advance(Duration::from_nanos(1));
            assert_eq!(playback.state(), PlaybackState::Completed);
            assert_eq!(playback.elapsed(), Duration::from_secs(15));
        }
    }

    #[test]
    fn configured_budget_has_no_per_stage_rounding_drift_or_overflow() {
        for duration in ["10s", "17s", "600s"] {
            let duration = PlaybackDuration::parse(duration).unwrap();
            let mut playback = Playback::with_duration(34, duration);
            playback.advance(duration.get() / 2);
            assert_eq!(playback.stage_index(), Some(17));
            playback.advance(duration.get() / 2 - Duration::from_nanos(1));
            assert!(!playback.is_completed());
            playback.advance(Duration::MAX);
            assert!(playback.is_completed());
            assert_eq!(playback.elapsed(), duration.get());
        }
        assert_eq!(Playback::new(0).stage_index(), None);
        assert!(Playback::new(0).is_completed());
    }

    #[test]
    fn pauses_and_interruptions_never_consume_the_playback_budget() {
        let mut playback = Playback::new(3);
        playback.advance(Duration::from_secs(5));
        assert_eq!(playback.stage_index(), Some(1));
        playback.key(Key::Space);
        playback.advance(Duration::from_secs(30));
        assert_eq!(playback.state(), PlaybackState::Paused);
        playback.key(Key::Space);
        for interruption in [
            Interruption::TooSmall,
            Interruption::Suspended,
            Interruption::Background,
        ] {
            playback.set_interruption(interruption, true);
            playback.advance(Duration::from_secs(100));
            assert_eq!(playback.elapsed(), Duration::from_secs(5));
            playback.set_interruption(interruption, false);
        }
        playback.advance(Duration::from_secs(10));
        assert!(playback.is_completed());
    }

    #[test]
    fn suspend_background_and_manual_pause_restore_the_original_mode() {
        for interruption in [Interruption::Suspended, Interruption::Background] {
            let mut playback = Playback::new(4);
            playback.key(Key::Space);
            playback.set_interruption(interruption, true);
            playback.advance(Duration::from_secs(100));
            assert_eq!(playback.stage_index(), Some(0));
            playback.set_interruption(interruption, false);
            assert_eq!(playback.state(), PlaybackState::Paused);
        }
    }

    #[test]
    fn enter_skips_to_the_last_stage_then_confirms_but_q_always_cancels() {
        let mut playback = Playback::new(34);
        assert_eq!(playback.key(Key::Enter), Action::Continue);
        assert_eq!(playback.stage_index(), Some(33));
        assert!(playback.is_completed());
        playback.key(Key::Space);
        assert!(playback.is_completed());
        assert_eq!(playback.key(Key::Enter), Action::Confirm);
        for completed in [false, true] {
            let mut playback = Playback::new(34);
            if completed {
                playback.advance(Duration::from_secs(15));
            }
            assert_eq!(playback.key(Key::Quit), Action::Cancel);
            assert_eq!(playback.state(), PlaybackState::Cancelled);
            playback.key(Key::Enter);
            assert_eq!(playback.state(), PlaybackState::Cancelled);
        }
    }

    #[test]
    fn space_in_small_window_changes_the_mode_to_restore() {
        let mut playback = Playback::new(2);
        playback.set_too_small(true);
        playback.key(Key::Space);
        assert_eq!(playback.state(), PlaybackState::TooSmall);
        playback.set_too_small(false);
        assert_eq!(playback.state(), PlaybackState::Paused);
    }
}
