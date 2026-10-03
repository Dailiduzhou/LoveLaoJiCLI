//! Ratatui renders only. This module owns the terminal and signal lifecycle;
//! crossterm's global raw-mode state and event reader are deliberately unused.
use cli_common::{Language, Result};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        cursor::Show,
        execute,
        style::ResetColor,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::Rect,
    Terminal, TerminalOptions, Viewport,
};
use rustix::termios::{self, InputModes, LocalModes, OptionalActions, SpecialCodeIndex, Termios};
use signal_hook::{consts::signal::*, SigId};
use std::{
    fs::{File, OpenOptions},
    io::{self, IsTerminal},
    os::unix::fs::OpenOptionsExt,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const FRAME: Duration = Duration::from_millis(100);
const MIN_COLUMNS: u16 = 24;
const MIN_ROWS: u16 = 10;

pub fn run(wait: Duration, language: Language) -> Result<Option<i32>> {
    let start = Instant::now();
    if !eligible() {
        if let Some(left) = wait.checked_sub(start.elapsed()) {
            std::thread::sleep(left);
        }
        return Ok(None);
    }
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
fn foreground() -> bool {
    termios::tcgetpgrp(io::stdin()).is_ok_and(|p| p == rustix::process::getpgrp())
}
fn eligible() -> bool {
    if !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || !io::stderr().is_terminal()
        || !foreground()
        || std::env::var_os("TERM").is_none_or(|t| t.is_empty() || t == "dumb")
    {
        return false;
    }
    let same_terminal = (|| {
        let input = rustix::fs::fstat(io::stdin())?;
        let output = rustix::fs::fstat(io::stdout())?;
        let error = rustix::fs::fstat(io::stderr())?;
        Ok::<_, io::Error>(input.st_rdev == output.st_rdev && input.st_rdev == error.st_rdev)
    })()
    .unwrap_or(false);
    same_terminal
        && termios::tcgetwinsize(io::stdout())
            .is_ok_and(|size| size.ws_col >= MIN_COLUMNS && size.ws_row >= MIN_ROWS)
}
fn viewport(tty: &File) -> Result<Rect> {
    let size = termios::tcgetwinsize(tty)?;
    Ok(Rect::new(0, 0, size.ws_col.min(240), size.ws_row.min(100)))
}
fn fault_name() -> Option<String> {
    std::env::var("AFK_TEST_FAILURE").ok()
}
fn fault(at: &str) -> Result<()> {
    if fault_name().as_deref() == Some(at) {
        return Err(io::Error::other(format!("AFK_TEST_FAILURE={at}")));
    }
    Ok(())
}
struct Session {
    terminal: Terminal<CrosstermBackend<ScreenWriter>>,
    input: File,
    mode: ModeGuard,
    fail: Arc<AtomicBool>,
}
impl Session {
    fn terminal(
        tty: &File,
        fail: Arc<AtomicBool>,
        area: Rect,
    ) -> Result<Terminal<CrosstermBackend<ScreenWriter>>> {
        Terminal::with_options(
            CrosstermBackend::new(ScreenWriter {
                file: tty.try_clone()?,
                fail,
            }),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )
    }
    fn new() -> Result<Self> {
        // A stalled terminal must not block inside a frame with cbreak live.
        // On output backpressure, fail and restore termios; screen cleanup is
        // best-effort if the terminal can no longer accept escape sequences.
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        if !termios::tcgetpgrp(&tty).is_ok_and(|p| p == rustix::process::getpgrp()) {
            return Err(io::Error::other("Terminal is no longer in the foreground"));
        }
        // Separately open, not dup+fcntl: O_NONBLOCK must never affect the shell's
        // inherited open file description. We only read the controlling terminal.
        let input = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        let saved = termios::tcgetattr(&tty)?;
        let fail = Arc::new(AtomicBool::new(false));
        // Avoid Terminal::clear/cursor-position requests: crossterm services
        // those by temporarily entering raw mode and waiting for terminal input.
        // A bounded fixed viewport also prevents giant resize events allocating
        // unbounded frame buffers while we own the terminal.
        let mut terminal = Self::terminal(&tty, fail.clone(), viewport(&tty)?)?;
        let mut mode = ModeGuard {
            tty,
            saved,
            changed: false,
            screen: false,
        };
        mode.enter()?;
        fault("enter")?;
        terminal.backend_mut().clear()?;
        fail.store(fault_name().as_deref() == Some("draw"), Ordering::SeqCst);
        Ok(Self {
            terminal,
            input,
            mode,
            fail,
        })
    }
}
// Keep cleanup on a separate writer: the test hook exercises a real backend
// Write failure, while ModeGuard can still restore the live controlling tty.
struct ScreenWriter {
    file: File,
    fail: Arc<AtomicBool>,
}
impl io::Write for ScreenWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(io::Error::other("AFK_TEST_FAILURE=draw"));
        }
        io::Write::write(&mut self.file, bytes)
    }
    fn flush(&mut self) -> Result<()> {
        io::Write::flush(&mut self.file)
    }
}
struct ModeGuard {
    tty: File,
    saved: Termios,
    changed: bool,
    screen: bool,
}
impl ModeGuard {
    fn enter(&mut self) -> Result<()> {
        let mut input = self.saved.clone();
        // Cbreak-style input, not raw mode: keep normal terminal signal delivery.
        input.local_modes.remove(
            LocalModes::ICANON | LocalModes::ECHO | LocalModes::ECHONL | LocalModes::IEXTEN,
        );
        input.local_modes.insert(LocalModes::ISIG);
        input
            .input_modes
            .remove(InputModes::IXON | InputModes::IXOFF);
        input.special_codes[SpecialCodeIndex::VMIN] = 1;
        input.special_codes[SpecialCodeIndex::VTIME] = 0;
        self.changed = true; // Restore even if a setup syscall partially fails.
        termios::tcsetattr(&self.tty, OptionalActions::Now, &input)?;
        self.screen = true;
        execute!(&mut self.tty, EnterAlternateScreen)?;
        Ok(())
    }
    fn restore(&mut self) -> Result<()> {
        let attrs = if self.changed {
            // Do not pass queued TUI keys/paste back to the shell. Never flush
            // another foreground job's input if ownership has already changed.
            let pending = if termios::tcgetpgrp(&self.tty)
                .is_ok_and(|p| p == rustix::process::getpgrp())
            {
                termios::tcflush(&self.tty, termios::QueueSelector::IFlush).map_err(io::Error::from)
            } else {
                Ok(())
            };
            let result = termios::tcsetattr(&self.tty, OptionalActions::Now, &self.saved)
                .map_err(io::Error::from);
            if result.is_ok() {
                self.changed = false;
            }
            result.and(pending)
        } else {
            Ok(())
        };
        let screen = if self.screen {
            // Try every cleanup operation even when an earlier one fails.
            let color = execute!(&mut self.tty, ResetColor);
            let cursor = execute!(&mut self.tty, Show);
            let alternate = execute!(&mut self.tty, LeaveAlternateScreen);
            let result = color.and(cursor).and(alternate);
            if result.is_ok() {
                self.screen = false;
            }
            result
        } else {
            Ok(())
        };
        attrs.and(screen)
    }
}
impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
struct Signals {
    ids: Vec<SigId>,
    terminate: Arc<AtomicUsize>,
    suspend: Arc<AtomicBool>,
    restored: Arc<AtomicBool>,
}
impl Signals {
    fn new() -> Result<Self> {
        let mut result = Self {
            ids: vec![],
            terminate: Arc::new(AtomicUsize::new(0)),
            suspend: Arc::new(AtomicBool::new(false)),
            restored: Arc::new(AtomicBool::new(false)),
        };
        // unregister() removes actions, not the OS disposition. Keep tiny
        // conditional-default actions until process exit so a signal during a
        // final stdout/stderr write still works after terminal cleanup.
        for signal in [SIGINT, SIGTERM, SIGHUP, SIGQUIT, SIGTSTP] {
            signal_hook::flag::register_conditional_default(signal, result.restored.clone())?;
        }
        for signal in [SIGINT, SIGTERM, SIGHUP, SIGQUIT] {
            result.ids.push(signal_hook::flag::register_usize(
                signal,
                result.terminate.clone(),
                signal as usize,
            )?);
        }
        result.ids.push(signal_hook::flag::register(
            SIGTSTP,
            result.suspend.clone(),
        )?);
        Ok(result)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        self.restored.store(true, Ordering::SeqCst);
        for id in self.ids.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}
/// tcsetattr itself can generate SIGTTOU if foreground ownership changes. Block
/// job-control I/O signals on this single UI thread until restoration is done.
/// A racing background read fails with EIO instead of stopping with cbreak live.
struct JobControlMask {
    previous: libc::sigset_t,
}
impl JobControlMask {
    fn new() -> Result<Self> {
        // SAFETY: POSIX sigset_t values are initialized with sigemptyset. All
        // pointers are live for each call; pthread_sigmask writes `previous` on
        // success. This changes only the calling thread's mask, not dispositions.
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            let mut previous: libc::sigset_t = std::mem::zeroed();
            if libc::sigemptyset(&mut set) != 0
                || libc::sigaddset(&mut set, SIGTTOU) != 0
                || libc::sigaddset(&mut set, SIGTTIN) != 0
            {
                return Err(io::Error::last_os_error());
            }
            let error = libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut previous);
            if error != 0 {
                return Err(io::Error::from_raw_os_error(error));
            }
            Ok(Self { previous })
        }
    }
}
impl Drop for JobControlMask {
    fn drop(&mut self) {
        // SAFETY: `previous` is the initialized mask captured on this same thread.
        // Session guards have already restored the terminal before this drop.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}
