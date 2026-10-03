//! Signal flags and the UI-thread job-control mask; no terminal mutation.
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
pub(super) struct JobControlMask {
    previous: libc::sigset_t,
}
impl JobControlMask {
    pub(super) fn new() -> Result<Self> {
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
