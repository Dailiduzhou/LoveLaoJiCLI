//! Monotonic event loop. Guard declaration/drop order is part of terminal safety.
//! Crossterm raw mode and event readers are deliberately unused.
mod signals;
mod terminal;

use cli_common::{Language, Result};
use ratatui::backend::Backend;
use signal_hook::consts::signal::{SIGINT, SIGQUIT, SIGTSTP};
use signals::{JobControlMask, Signals};
use std::{
    io,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use terminal::{eligible, fault_name, foreground, viewport, Session};

const FRAME: Duration = Duration::from_millis(100);

pub fn run(wait: Duration, language: Language) -> Result<Option<i32>> {
    let start = Instant::now();
    if !eligible() {
        if let Some(left) = wait.checked_sub(start.elapsed()) {
            std::thread::sleep(left);
        }
        return Ok(None);
    }
    let palette = super::scene::Palette::detect();
    // Registration and masks precede every terminal mutation. Reverse drop order
    // ensures a panicking Session restores termios before masks/handlers go away.
    let signals = Signals::new()?;
    let _job_control = JobControlMask::new()?;
    let mut session: Option<Session> = None;
    let mut noticed_at: Option<Instant> = None;
    loop {
        let signal = signals.terminate.load(Ordering::SeqCst) as i32;
        if signal != 0 {
            if let Some(mut s) = session.take() {
                let _ = s.mode.restore();
            }
            return Ok(Some(signal));
        }
        if signals.suspend.swap(false, Ordering::SeqCst) {
            if let Some(mut s) = session.take() {
                s.mode.restore()?;
            }
            // signal-hook emulates a stop with SIGSTOP, including in orphaned
            // process groups. No UI/termios state is left live while stopped.
            signal_hook::low_level::emulate_default_handler(SIGTSTP)?;
            continue;
        }
        if start.elapsed() >= wait {
            if let Some(mut s) = session.take() {
                s.mode.restore()?;
            }
            return Ok(None);
        }
        if !foreground() {
            if let Some(mut s) = session.take() {
                s.mode.restore()?;
            }
            // A background continuation never reads keys, draws, or reacquires
            // the terminal. The original deadline still advances.
        } else {
            if session.is_none() && eligible() {
                session = Some(Session::new()?);
            }
            if let Some(s) = session.as_mut() {
                let mut input = [0; 256];
                // At most one bounded read/frame: key/paste floods cannot starve
                // the timer or turn into an unbounded input buffer.
                match io::Read::read(&mut s.input, &mut input) {
                    Ok(n) if n > 0 => {
                        if input[..n].contains(&3) {
                            signals.terminate.store(SIGINT as usize, Ordering::SeqCst);
                            continue;
                        }
                        if input[..n].contains(&26) {
                            signals.suspend.store(true, Ordering::SeqCst);
                            continue;
                        }
                        if input[..n].contains(&28) {
                            signals.terminate.store(SIGQUIT as usize, Ordering::SeqCst);
                            continue;
                        }
                        noticed_at = Some(Instant::now());
                    }
                    Ok(_) => (),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e),
                }
                let area = viewport(&s.mode.tty)?;
                if s.terminal.get_frame().area() != area {
                    // Rebuild only on resize instead of Terminal::resize: its
                    // backend size fallback can spawn `tput` after an ioctl error.
                    let mut replacement = Session::terminal(&s.mode.tty, s.fail.clone(), area)?;
                    replacement.backend_mut().clear()?;
                    s.terminal = replacement;
                }
                let elapsed = start.elapsed();
                s.terminal.draw(|frame| {
                    super::scene::draw(
                        frame,
                        elapsed,
                        wait,
                        noticed_at.is_some_and(|t| t.elapsed() < Duration::from_secs(2)),
                        language,
                        palette,
                    )
                })?;
                if fault_name().as_deref() == Some("panic") {
                    panic!("AFK_TEST_FAILURE=panic");
                }
            }
        }
        std::thread::sleep(wait.saturating_sub(start.elapsed()).min(FRAME));
    }
}
