//! Independent `/dev/tty`, cbreak input, bounded viewport and RAII restoration.

use crate::error::{Error, Result};
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
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::termios::{self, InputModes, LocalModes, OptionalActions, SpecialCodeIndex, Termios};
use std::{
    fs::{File, OpenOptions},
    io::{self, IsTerminal},
    os::fd::AsFd,
    os::unix::fs::OpenOptionsExt,
    time::{Duration, Instant},
};

const MIN_COLUMNS: u16 = 80;
const MIN_ROWS: u16 = 24;
const MAX_COLUMNS: u16 = 240;
const MAX_ROWS: u16 = 100;

/// A full PTY, SSH channel or terminal buffer is transient backpressure, not a
/// broken terminal: retrying keeps the replay alive instead of aborting a live
/// frame. The budget is bounded so a terminal that never drains still reaches
/// the restoration path instead of hanging the UI thread.
const STALL_BUDGET: Duration = Duration::from_secs(2);
const STALL_TICK: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 20_000_000,
};

/// Write one frame piece, waiting for a full output buffer to drain within
/// `budget`. Our own description stays nonblocking, so writes report
/// `WouldBlock` instead of blocking a frame; inherited descriptors are untouched.
fn write_drained<W>(writer: &mut W, bytes: &[u8], budget: Duration) -> io::Result<usize>
where
    W: io::Write + AsFd,
{
    let deadline = Instant::now() + budget;
    loop {
        match writer.write(bytes) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(error);
                }
                let mut fds = [PollFd::new(writer, PollFlags::OUT)];
                match poll(&mut fds, Some(&STALL_TICK)) {
                    Ok(_) | Err(rustix::io::Errno::INTR) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            result => return result,
        }
    }
}

/// Same retry for the screen/termios escape sequences owned by `ModeGuard`.
struct Drained<'a>(&'a File);

impl io::Write for Drained<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        write_drained(&mut &*self.0, bytes, STALL_BUDGET)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

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
        .map_err(Error::from)
    }

    pub(super) fn new() -> Result<Self> {
        // Separate opens keep O_NONBLOCK off the shell's inherited descriptions.
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open("/dev/tty")?;
        if !termios::tcgetpgrp(&tty).is_ok_and(|group| group == rustix::process::getpgrp()) {
            return Err(Error::NotForeground);
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
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.owned() {
            // Ratatui Drop may still try to show the cursor. Discard all stale
            // output rather than disturbing the new foreground job.
            return Ok(bytes.len());
        }
        write_drained(&mut self.file, bytes, STALL_BUDGET)
    }

    fn flush(&mut self) -> io::Result<()> {
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
        let mut screen = Drained(&self.tty);
        execute!(&mut screen, EnterAlternateScreen, Hide)?;
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
            let pending = termios::tcflush(&self.tty, termios::QueueSelector::IFlush);
            let result = termios::tcsetattr(&self.tty, OptionalActions::Now, &self.saved);
            if result.is_ok() {
                self.changed = false;
            }
            result.and(pending).map_err(Error::from)
        } else {
            Ok(())
        };
        let screen = if self.screen {
            let mut screen = Drained(&self.tty);
            let color = execute!(&mut screen, ResetColor);
            let cursor = execute!(&mut screen, Show);
            let alternate = execute!(&mut screen, LeaveAlternateScreen);
            let result = color.and(cursor).and(alternate);
            if result.is_ok() {
                self.screen = false;
            }
            result.map_err(Error::from)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    /// Fill a nonblocking stream until the kernel reports backpressure.
    fn stalled() -> (UnixStream, UnixStream) {
        let (writer, reader) = UnixStream::pair().unwrap();
        writer.set_nonblocking(true).unwrap();
        let chunk = [0_u8; 4096];
        let mut filled = 0;
        loop {
            match io::Write::write(&mut &writer, &chunk) {
                Ok(0) => panic!("stream accepted no bytes"),
                Ok(size) => filled += size,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("{error}"),
            }
            assert!(filled < 8 * 1024 * 1024, "stream buffer did not fill");
        }
        (writer, reader)
    }

    #[test]
    fn full_output_buffers_are_retried_until_the_reader_drains_them() {
        let (mut writer, reader) = stalled();
        let (drained, was_drained) = std::sync::mpsc::channel();
        // Hold the reading end open until the write under test has returned:
        // closing it first would make the retried write report EPIPE instead.
        let (release, held) = std::sync::mpsc::channel();
        let drain = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            let mut buffer = [0_u8; 8192];
            assert!(io::Read::read(&mut &reader, &mut buffer).unwrap() > 0);
            drained.send(()).unwrap();
            assert!(held.recv_timeout(Duration::from_secs(5)).is_ok());
        });
        assert_eq!(
            write_drained(&mut writer, b"frame", STALL_BUDGET).unwrap(),
            5
        );
        release.send(()).unwrap();
        was_drained.recv_timeout(Duration::from_secs(5)).unwrap();
        drain.join().unwrap();
    }

    #[test]
    fn a_terminal_that_never_drains_stops_at_the_budget_instead_of_hanging() {
        let (mut writer, _reader) = stalled();
        let began = Instant::now();
        let error = write_drained(&mut writer, b"frame", Duration::from_millis(150)).unwrap_err();
        let elapsed = began.elapsed();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(elapsed >= Duration::from_millis(140), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    }
}
