//! Independent `/dev/tty`, cbreak input, bounded viewport and RAII restoration.

use cli_common::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        cursor::{Hide, Show},
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
};

const MIN_COLUMNS: u16 = 80;
const MIN_ROWS: u16 = 24;
const MAX_COLUMNS: u16 = 240;
const MAX_ROWS: u16 = 100;

pub(super) fn foreground() -> bool {
    termios::tcgetpgrp(io::stdin()).is_ok_and(|group| group == rustix::process::getpgrp())
}

/// Shared-terminal checks that remain necessary when reentering after `fg`.
pub(super) fn available() -> bool {
    if !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || !io::stderr().is_terminal()
        || !foreground()
        || std::env::var_os("TERM").is_none_or(|term| term.is_empty() || term == "dumb")
    {
        return false;
    }
    (|| {
        let input = rustix::fs::fstat(io::stdin())?;
        let output = rustix::fs::fstat(io::stdout())?;
        let error = rustix::fs::fstat(io::stderr())?;
        // Opening /dev/tty proves that a controlling terminal exists. Do not
        // compare its st_rdev with the slave: Darwin reports a distinct device
        // number for /dev/tty even when it refers to this exact terminal.
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        Ok::<_, io::Error>(input.st_rdev == output.st_rdev && input.st_rdev == error.st_rdev)
    })()
    .unwrap_or(false)
}

pub(super) fn eligible() -> bool {
    available()
        && termios::tcgetwinsize(io::stdout())
            .is_ok_and(|size| size.ws_col >= MIN_COLUMNS && size.ws_row >= MIN_ROWS)
}

pub(super) fn viewport(tty: &File) -> Result<Rect> {
    let size = termios::tcgetwinsize(tty)?;
    Ok(Rect::new(
        0,
        0,
        size.ws_col.min(MAX_COLUMNS),
        size.ws_row.min(MAX_ROWS),
    ))
}

pub(super) struct Session {
    pub(super) terminal: Terminal<CrosstermBackend<ScreenWriter>>,
    pub(super) input: File,
    pub(super) mode: ModeGuard,
}

impl Session {
    pub(super) fn terminal(
        tty: &File,
        area: Rect,
    ) -> Result<Terminal<CrosstermBackend<ScreenWriter>>> {
        Terminal::with_options(
            CrosstermBackend::new(ScreenWriter {
                file: tty.try_clone()?,
            }),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )
    }

    pub(super) fn new() -> Result<Self> {
        // Separate opens keep O_NONBLOCK off the shell's inherited descriptions.
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        if !termios::tcgetpgrp(&tty).is_ok_and(|group| group == rustix::process::getpgrp()) {
            return Err(io::Error::other("terminal is no longer in the foreground"));
        }
        let input = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        let saved = termios::tcgetattr(&tty)?;
        // Fixed viewport: no cursor query, tput fallback, or unbounded allocation.
        let mut terminal = Self::terminal(&tty, viewport(&tty)?)?;
        let mut mode = ModeGuard {
            tty,
            saved,
            changed: false,
            screen: false,
        };
        mode.enter()?;
        ratatui::backend::Backend::clear(terminal.backend_mut())?;
        Ok(Self {
            terminal,
            input,
            mode,
        })
    }

    /// Drop Ratatui before the final guard cleanup. `Terminal::draw` hides the
    /// cursor internally, so Terminal::drop may otherwise emit Show after the
    /// alternate screen has already been left.
    pub(super) fn close(self) -> Result<()> {
        let Self {
            terminal,
            input,
            mut mode,
        } = self;
        drop(terminal);
        drop(input);
        mode.restore()
    }
}

pub(super) struct ScreenWriter {
    file: File,
}

impl ScreenWriter {
    fn owned(&self) -> bool {
        termios::tcgetpgrp(&self.file).is_ok_and(|group| group == rustix::process::getpgrp())
    }
}

impl io::Write for ScreenWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.owned() {
            // Ratatui Drop may still try to show the cursor. Discard all stale
            // output rather than disturbing the new foreground job.
            return Ok(bytes.len());
        }
        io::Write::write(&mut self.file, bytes)
    }

    fn flush(&mut self) -> Result<()> {
        if self.owned() {
            io::Write::flush(&mut self.file)
        } else {
            Ok(())
        }
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
        // Cbreak, not raw: terminal-generated SIGINT/SIGTSTP remain enabled.
        input.local_modes.remove(
            LocalModes::ICANON | LocalModes::ECHO | LocalModes::ECHONL | LocalModes::IEXTEN,
        );
        input.local_modes.insert(LocalModes::ISIG);
        input
            .input_modes
            .remove(InputModes::IXON | InputModes::IXOFF);
        input.special_codes[SpecialCodeIndex::VMIN] = 1;
        input.special_codes[SpecialCodeIndex::VTIME] = 0;
        self.changed = true;
        termios::tcsetattr(&self.tty, OptionalActions::Now, &input)?;
        self.screen = true;
        execute!(&mut self.tty, EnterAlternateScreen, Hide)?;
        Ok(())
    }

    pub(super) fn restore(&mut self) -> Result<()> {
        if !termios::tcgetpgrp(&self.tty).is_ok_and(|group| group == rustix::process::getpgrp()) {
            // Abandon a stale snapshot permanently; do not apply it on later fg.
            self.changed = false;
            self.screen = false;
            return Ok(());
        }
        let attrs = if self.changed {
            let pending = termios::tcflush(&self.tty, termios::QueueSelector::IFlush)
                .map_err(io::Error::from);
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
