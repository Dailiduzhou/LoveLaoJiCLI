//! Child argv and streams are never interpreted by a shell. No signal handlers:
//! foreground terminal signals reach parent and child in the same process group.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

pub fn interactive() -> bool {
    io::stdin().is_terminal()
        && io::stderr().is_terminal()
        && rustix::termios::tcgetpgrp(io::stdin()).is_ok_and(|g| g == rustix::process::getpgrp())
}
pub fn ask(question: &str, max: usize) -> Result<Option<String>> {
    if !interactive() {
        return Ok(None);
    }
    let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        return Ok(None);
    };
    if !rustix::termios::tcgetpgrp(&tty).is_ok_and(|g| g == rustix::process::getpgrp()) {
        return Ok(None);
    }
    if writeln!(tty, "{question}")
        .and_then(|_| tty.flush())
        .is_err()
    {
        return Ok(None);
    }
    // Read exactly one line without BufReader read-ahead into the child's future input.
    let mut bytes = Vec::new();
    while bytes.len() <= max * 4 {
        let mut byte = [0u8; 1];
        match tty.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => {
                return Ok(std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| crate::text_input(s, max)));
            }
            Ok(1) => bytes.push(byte[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            _ => return Ok(None),
        }
    }
    Ok(None)
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
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
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
