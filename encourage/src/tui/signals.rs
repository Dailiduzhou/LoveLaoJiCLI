//! Async-signal-safe flags and a UI-thread-only job-control mask.

use cli_common::Result;
use signal_hook::{consts::signal::*, SigId};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

pub(super) struct Signals {
    ids: Vec<SigId>,
    pub(super) terminate: Arc<AtomicUsize>,
    pub(super) suspend: Arc<AtomicBool>,
    pub(super) resumed: Arc<AtomicBool>,
    restored: Arc<AtomicBool>,
}

impl Signals {
    pub(super) fn new() -> Result<Self> {
        let mut result = Self {
            ids: Vec::new(),
            terminate: Arc::new(AtomicUsize::new(0)),
            suspend: Arc::new(AtomicBool::new(false)),
            resumed: Arc::new(AtomicBool::new(false)),
            restored: Arc::new(AtomicBool::new(false)),
        };
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
        result.ids.push(signal_hook::flag::register(
            SIGCONT,
            result.resumed.clone(),
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

/// Block SIGTTOU/SIGTTIN only on the UI thread while termios and terminal I/O
/// race with foreground changes. No complex work runs in a signal handler.
pub(super) struct JobControlMask {
    previous: libc::sigset_t,
}

impl JobControlMask {
    pub(super) fn new() -> Result<Self> {
        // SAFETY: sigemptyset initializes both sets; all pointers remain valid.
        // pthread_sigmask changes only this calling thread's signal mask.
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
        // SAFETY: previous was initialized on this same UI thread. The Session
        // has already restored or deliberately abandoned its terminal snapshot.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}
