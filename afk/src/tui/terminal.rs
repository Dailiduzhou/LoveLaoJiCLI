//! Terminal eligibility, bounded backend, and RAII restoration. No event loop.
use cli_common::Result;
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
use std::{
    fs::{File, OpenOptions},
    io::{self, IsTerminal},
    os::unix::fs::OpenOptionsExt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const MIN_COLUMNS: u16 = 24;
const MIN_ROWS: u16 = 10;

pub(super) fn foreground() -> bool {
    termios::tcgetpgrp(io::stdin()).is_ok_and(|p| p == rustix::process::getpgrp())
}
pub(super) fn eligible() -> bool {
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
pub(super) fn viewport(tty: &File) -> Result<Rect> {
    let size = termios::tcgetwinsize(tty)?;
    Ok(Rect::new(0, 0, size.ws_col.min(240), size.ws_row.min(100)))
}
pub(super) fn fault_name() -> Option<String> {
    std::env::var("AFK_TEST_FAILURE").ok()
}
fn fault(at: &str) -> Result<()> {
    if fault_name().as_deref() == Some(at) {
        return Err(io::Error::other(format!("AFK_TEST_FAILURE={at}")));
    }
    Ok(())
}
pub(super) struct Session {
    pub(super) terminal: Terminal<CrosstermBackend<ScreenWriter>>,
    pub(super) input: File,
    pub(super) mode: ModeGuard,
    pub(super) fail: Arc<AtomicBool>,
}
impl Session {
    pub(super) fn terminal(
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
    pub(super) fn new() -> Result<Self> {
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
pub(super) struct ScreenWriter {
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
pub(super) struct ModeGuard {
    pub(super) tty: File,
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
    pub(super) fn restore(&mut self) -> Result<()> {
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
