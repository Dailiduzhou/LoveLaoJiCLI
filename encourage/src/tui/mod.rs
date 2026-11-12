//! Ratatui stage replay and terminal lifecycle.

mod highlight;
pub mod layout;
pub mod playback;
mod progress;
mod signals;
mod terminal;

use crate::error::{Error, Result};
use crate::{duration::PlaybackDuration, pipeline::Generation, source::Source};
use cli_common::Language;
use playback::{Action, Key, Playback, PlaybackState};
use signal_hook::consts::signal::{SIGINT, SIGQUIT, SIGTSTP};
use signals::{JobControlMask, Signals};
use std::{
    io,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use terminal::{available, eligible, foreground, viewport, Session};

const FRAME: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Startup was not interactive; the caller should print the same sentence.
    Plain,
    /// The terminal is restored and the caller may print the sentence once.
    Confirmed,
    /// The terminal is restored and no result may be printed.
    Cancelled,
    /// The terminal is restored; the caller should restore default disposition.
    Signal(i32),
}

/// Read-only startup check, allowing the CLI to skip trace collection in plain
/// mode. `run` checks again before acquiring any terminal resources.
pub fn is_interactive() -> bool {
    eligible()
}

pub fn run(
    generation: &Generation,
    language: Language,
    duration: PlaybackDuration,
) -> Result<Outcome> {
    if !eligible() {
        return Ok(Outcome::Plain);
    }
    let source = Source::embedded()?;
    run_ready(generation, &source, language, duration)
}

/// Replay a selected real pipeline. Both execution and display must embed the
/// same file; tests and the opt-in P0 driver use this seam without changing the
/// production generator or terminal state machine.
pub fn run_with_source(
    generation: &Generation,
    source: &Source,
    language: Language,
    duration: PlaybackDuration,
) -> Result<Outcome> {
    if !eligible() {
        return Ok(Outcome::Plain);
    }
    run_ready(generation, source, language, duration)
}

fn run_ready(
    generation: &Generation,
    source: &Source,
    language: Language,
    duration: PlaybackDuration,
) -> Result<Outcome> {
    source.validate_trace(&generation.trace)?;
    let colors = !std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());

    // Declaration order is a safety contract: Session drops before the signal
    // handlers and this thread's job-control mask are removed.
    let signals = Signals::new()?;
    let _job_control = JobControlMask::new()?;
    let mut session: Option<Session> = None;
    let mut playback = Playback::with_duration(generation.trace.stages.len(), duration);
    let mut last_tick = Instant::now();

    loop {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(last_tick);
        last_tick = now;
        let was_playing = playback.state() == PlaybackState::Playing;

        let signal = signals.terminate.load(Ordering::SeqCst) as i32;
        if signal != 0 {
            restore_best_effort(&mut session);
            return Ok(Outcome::Signal(signal));
        }
        let mut discard_elapsed = signals.resumed.swap(false, Ordering::SeqCst);
        if discard_elapsed {
            // SIGSTOP cannot be handled. SIGCONT is the point where we discard
            // stopped wall time instead of skipping stages after `fg`.
            last_tick = Instant::now();
        }
        if signals.suspend.swap(false, Ordering::SeqCst) {
            playback.set_suspended(true);
            restore(&mut session)?;
            // The alternate screen and original termios are gone before SIGSTOP.
            signal_hook::low_level::emulate_default_handler(SIGTSTP)?;
            playback.set_suspended(false);
            playback.set_background(!foreground());
            last_tick = Instant::now();
            continue;
        }

        if !foreground() {
            // No key reads, drawing, cleanup escapes, or stale snapshot writes
            // while another process group owns the terminal.
            drop(session.take());
            playback.set_background(true);
            std::thread::sleep(FRAME);
            continue;
        }
        playback.set_background(false);
        if session.is_none() {
            if !available() {
                return Err(Error::TerminalUnavailable);
            }
            // Resize below the minimum is an in-session pause, never a plain
            // fallback. Session supports the bounded tiny viewport for a hint.
            session = Some(Session::new()?);
        }

        let action = {
            let s = session.as_mut().expect("session installed above");
            let mut input = [0_u8; 1];
            match io::Read::read(&mut s.input, &mut input) {
                Ok(1) => match input[0] {
                    3 => {
                        restore_best_effort(&mut session);
                        return Ok(Outcome::Signal(SIGINT));
                    }
                    26 => {
                        signals.suspend.store(true, Ordering::SeqCst);
                        Action::Continue
                    }
                    28 => {
                        restore_best_effort(&mut session);
                        return Ok(Outcome::Signal(SIGQUIT));
                    }
                    b' ' => playback.key(Key::Space),
                    b'\r' | b'\n' => playback.key(Key::Enter),
                    b'q' => playback.key(Key::Quit),
                    _ => Action::Continue,
                },
                Ok(_) => Action::Continue,
                Err(_) if !foreground() => {
                    drop(session.take());
                    continue;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    Action::Continue
                }
                Err(error) => return Err(error.into()),
            }
        };
        match action {
            Action::Confirm => {
                restore(&mut session)?;
                return Ok(Outcome::Confirmed);
            }
            Action::Cancel => {
                restore(&mut session)?;
                return Ok(Outcome::Cancelled);
            }
            Action::Continue => {}
        }

        let s = session.as_mut().expect("session retained for drawing");
        let area = viewport(&s.mode.tty)?;
        playback.set_too_small(area.width < 80 || area.height < 24);
        if signals.resumed.swap(false, Ordering::SeqCst) {
            discard_elapsed = true;
            last_tick = Instant::now();
        }
        if !discard_elapsed && was_playing {
            playback.advance(elapsed);
        }
        if s.terminal.get_frame().area() != area {
            // Avoid Terminal::resize's backend fallback, which may spawn tput.
            let mut replacement = Session::terminal(&s.mode.tty, area)?;
            ratatui::backend::Backend::clear(replacement.backend_mut())?;
            s.terminal = replacement;
        }
        s.terminal.draw(|frame| {
            layout::draw(frame, generation, source, &playback, language, colors);
        })?;
        std::thread::sleep(FRAME);
    }
}

fn restore(session: &mut Option<Session>) -> Result<()> {
    if let Some(session) = session.take() {
        session.close()?;
    }
    Ok(())
}

fn restore_best_effort(session: &mut Option<Session>) {
    if let Some(session) = session.take() {
        let _ = session.close();
    }
}
