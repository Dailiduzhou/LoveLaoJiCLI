//! Non-canonical single-line prompts on the controlling terminal.
//!
//! The kernel canonical line buffer is byte-limited (1024 on macOS, about 4096
//! on Linux) and can drop input before the application sees a newline. This
//! reader instead receives bytes as they arrive, so long valid answers work and
//! oversized ones are rejected instead of hanging.
//!
//! - cbreak-style: ICANON/ECHO off, ISIG kept, so Ctrl+C/Ctrl+Z/Ctrl+\ remain
//!   real terminal signals for the whole foreground group (wrapped children too).
//! - One byte per read(2): nothing after the submitted newline is consumed, so a
//!   wrapped child still receives its own queued input.
//! - Termios is restored before any return, stop or re-raised termination. After
//!   foreground loss the snapshot is relinquished, never written to a new owner.
//! - Rejected/cancelled input: queued remainder is flushed, so a refused paste
//!   cannot leak into the shell or child. A valid submit never flushes.
//! - Not recoverable: SIGKILL, external SIGSTOP, abort and terminal disconnect.
mod editor;
mod signals;
mod terminal;

pub(crate) use terminal::read_line;
