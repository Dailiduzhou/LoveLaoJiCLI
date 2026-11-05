//! Termios guard and the bounded byte-at-a-time prompt loop.
use super::editor::{Editor, Keys, Step};
use super::signals::{JobControlMask, Signals};
use crate::Result;
use rustix::event::{poll, PollFd, PollFlags, Timespec};
use rustix::termios::{self, LocalModes, OptionalActions, SpecialCodeIndex, Termios};
use signal_hook::consts::signal::SIGTSTP;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    sync::atomic::Ordering,
    time::Duration,
};

const TICK: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 100_000_000,
};

fn foreground(tty: &File) -> bool {
    termios::tcgetpgrp(tty).is_ok_and(|g| g == rustix::process::getpgrp())
}

/// Saved settings for one foreground episode. A fresh snapshot is taken on
/// every (re)entry; after ownership loss the old one is never applied.
struct Mode<'a> {
    tty: &'a File,
    saved: Termios,
    changed: bool,
}
impl<'a> Mode<'a> {
    fn enter(tty: &'a File) -> Result<Self> {
        let saved = termios::tcgetattr(tty)?;
        let mut mode = Self {
            tty,
            saved: saved.clone(),
            changed: false,
        };
        let mut input = saved;
        // Cbreak, not raw: ISIG keeps terminal signals for the whole group,
        // output processing and input CR/NL mapping stay as the user set them.
        input.local_modes.remove(
            LocalModes::ICANON | LocalModes::ECHO | LocalModes::ECHONL | LocalModes::IEXTEN,
        );
        input.local_modes.insert(LocalModes::ISIG);
        input.special_codes[SpecialCodeIndex::VMIN] = 1;
        input.special_codes[SpecialCodeIndex::VTIME] = 0;
        mode.changed = true; // Restore even if tcsetattr partially applied.
        termios::tcsetattr(tty, OptionalActions::Now, &input)?;
        Ok(mode)
    }
    fn keys(&self) -> Keys {
        let c = &self.saved.special_codes;
        Keys {
            erase: c[SpecialCodeIndex::VERASE],
            kill: c[SpecialCodeIndex::VKILL],
            werase: c[SpecialCodeIndex::VWERASE],
            eof: c[SpecialCodeIndex::VEOF],
        }
    }
    /// Flush queued input so a rejected/cancelled paste never reaches the shell
    /// or a wrapped child. A valid submit must not call this: the next line
    /// belongs to the child.
    fn discard_input(&self) {
        let _ = termios::tcflush(self.tty, termios::QueueSelector::IFlush);
    }
    fn restore(&mut self) {
        if !self.changed {
            return;
        }
        if !foreground(self.tty) {
            // A new foreground owner may already have its own settings. Drop
            // this snapshot, including the Drop retry.
            self.changed = false;
            return;
        }
        if termios::tcsetattr(self.tty, OptionalActions::Now, &self.saved).is_ok() {
            self.changed = false;
        }
    }
}
impl Drop for Mode<'_> {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Ask one question and read one line without the kernel canonical buffer.
/// Returns the validated answer, or `None` for EOF/blank/invalid/unavailable.
/// `Err` is reserved for I/O failures the caller reports as exit 1.
pub(crate) fn read_line(question: &str, max: usize) -> Result<Option<String>> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open("/dev/tty")?;
    if !foreground(&tty) {
        return Ok(None);
    }
    // Registration and mask precede any terminal mutation; reverse drop order
    // lets a panicking Mode restore termios before they go away.
    let signals = Signals::new()?;
    let _job = JobControlMask::new()?;
    let mut out = tty.try_clone()?;
    if writeln!(out, "{question}")
        .and_then(|_| out.flush())
        .is_err()
    {
        return Ok(None);
    }
    let mut mode = Mode::enter(&tty)?;
    let mut editor = Editor::new(max, mode.keys());
    let mut input = tty.try_clone()?;
    loop {
        check_signals(&signals, &mut mode)?;
        if !foreground(&tty) {
            // Relinquish without touching the new owner's terminal.
            mode.restore();
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        if !mode.changed {
            // Back in the foreground (fg): fresh snapshot, then show the
            // question and the partial answer again.
            mode = Mode::enter(&tty)?;
            let mut screen = format!("{question}\n").into_bytes();
            editor.redraw(&mut screen);
            let _ = out.write_all(&screen).and_then(|_| out.flush());
        }
        let mut fds = [PollFd::new(&input, PollFlags::IN)];
        match poll(&mut fds, Some(&TICK)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => continue,
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
        let mut byte = [0u8; 1];
        match input.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                let mut echo = Vec::new();
                let step = editor.feed(byte[0], &mut echo);
                if !echo.is_empty() {
                    let _ = out.write_all(&echo).and_then(|_| out.flush());
                }
                match step {
                    Step::More => {}
                    Step::Eof | Step::Submit(None) => {
                        // Rejected or cancelled: drop the rest of the paste.
                        mode.discard_input();
                        return Ok(None);
                    }
                    Step::Submit(answer) => return Ok(answer),
                }
            }
            Err(_) if !foreground(&tty) => mode.restore(),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
}

/// Apply a pending terminate/suspend with the terminal already restored.
fn check_signals(signals: &Signals, mode: &mut Mode) -> Result<()> {
    let signal = signals.terminate.load(Ordering::SeqCst) as i32;
    if signal != 0 {
        mode.discard_input();
        mode.restore();
        // Default action, so the exit status reflects the signal exactly like
        // a canonical-mode Ctrl+C did (shells report 130 for SIGINT).
        signal_hook::low_level::emulate_default_handler(signal)?;
        std::process::exit(128 + signal);
    }
    if signals.suspend.swap(false, Ordering::SeqCst) {
        mode.restore();
        // Stopped with original settings live; the loop re-enters only once
        // foreground ownership is confirmed again.
        signal_hook::low_level::emulate_default_handler(SIGTSTP)?;
    }
    Ok(())
}
