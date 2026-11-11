//! Child argv and streams are never interpreted by a shell. No signal handlers:
//! foreground terminal signals reach parent and child in the same process group.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

pub fn interactive() -> bool {
    io::stdin().is_terminal()
        && io::stderr().is_terminal()
        && rustix::termios::tcgetpgrp(io::stdin()).is_ok_and(|g| g == rustix::process::getpgrp())
}
/// Ask on the controlling terminal. Without the `line-input` feature there is
/// no safe reader for long answers, so prompts fail closed as "no answer".
pub fn ask(question: &str, max: usize) -> Result<Option<String>> {
    if !interactive() {
        return Ok(None);
    }
    #[cfg(feature = "line-input")]
    {
        crate::line_input::read_line(question, max)
    }
    #[cfg(not(feature = "line-input"))]
    {
        let _ = (question, max);
        Ok(None)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputDigest {
    pub digest: String,
    pub bytes: u64,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub category: String,
    pub exit_code: i32,
    pub signal: Option<i32>,
    pub stdout: Option<OutputDigest>,
    pub stderr: Option<OutputDigest>,
}
fn tee(mut pipe: impl Read, mut destination: impl Write, key: [u8; 32]) -> OutputDigest {
    let mut h = blake3::Hasher::new_keyed(&key);
    let mut bytes = 0;
    let mut complete = true;
    let mut buffer = [0u8; 16384];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                h.update(&buffer[..n]);
                bytes += n as u64;
                if destination
                    .write_all(&buffer[..n])
                    .and_then(|_| destination.flush())
                    .is_err()
                {
                    complete = false;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                complete = false;
                break;
            }
        }
    }
    OutputDigest {
        digest: h.finalize().to_hex().to_string(),
        bytes,
        complete,
    }
}
pub fn execute(argv: &[OsString], capture: bool, key: [u8; 32]) -> Outcome {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]).stdin(Stdio::inherit());
    if capture {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "{}: {}",
                crate::display(&argv[0]),
                crate::display(e.to_string())
            );
            return Outcome {
                category: "spawn-failed".into(),
                exit_code: if e.kind() == io::ErrorKind::NotFound {
                    127
                } else {
                    126
                },
                signal: None,
                stdout: None,
                stderr: None,
            };
        }
    };
    let readers = if capture {
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        Some((
            std::thread::spawn(move || tee(stdout, io::stdout(), key)),
            std::thread::spawn(move || tee(stderr, io::stderr(), key)),
        ))
    } else {
        None
    };
    let status = child.wait();
    let (stdout, stderr) = match readers {
        Some((a, b)) => (a.join().ok(), b.join().ok()),
        None => (None, None),
    };
    match status {
        Ok(s) => Outcome {
            category: if s.signal().is_some() {
                "signal"
            } else {
                "exited"
            }
            .into(),
            exit_code: s.code().unwrap_or_else(|| 128 + s.signal().unwrap_or(0)),
            signal: s.signal(),
            stdout,
            stderr,
        },
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Outcome {
                category: "wait-failed".into(),
                exit_code: 1,
                signal: None,
                stdout,
                stderr,
            }
        }
    }
}
