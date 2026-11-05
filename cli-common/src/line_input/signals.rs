//! Signal flags and the prompt-thread job-control mask, mirroring afk/tui.
//! No terminal mutation here: the reader restores termios before acting.
use crate::Result;
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
    restored: Arc<AtomicBool>,
}
impl Signals {
    pub(super) fn new() -> Result<Self> {
        let mut result = Self {
            ids: vec![],
            terminate: Arc::new(AtomicUsize::new(0)),
            suspend: Arc::new(AtomicBool::new(false)),
            restored: Arc::new(AtomicBool::new(false)),
        };
        // Keep a conditional default action alive until process exit: a signal
        // arriving during the final restore/flush still terminates correctly.
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

/// tcsetattr can raise SIGTTOU if foreground ownership changes mid-call. Block
/// job-control I/O signals on this single prompt thread until restoration is
/// done; a racing background read then fails with EIO instead of stopping.
pub(super) struct JobControlMask {
    previous: libc::sigset_t,
}
impl JobControlMask {
    pub(super) fn new() -> Result<Self> {
        // SAFETY: sigset_t is zeroed then populated with sigemptyset/sigaddset.
        // All pointers are live; pthread_sigmask writes `previous` on success and
        // changes only the calling thread's mask, never signal dispositions.
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
        // SAFETY: `previous` was captured on this same thread; the terminal has
        // already been restored by the reader before this drop runs.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}
