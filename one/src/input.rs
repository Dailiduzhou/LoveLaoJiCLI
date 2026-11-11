//! Isolated, bounded stdin selection; never opens persistent state.
use crate::error::{Error, Result};
use crate::tasks::{MAX_TASKS, MAX_TEXT};
use cli_common::text_input;
use std::io::{self, Read};

const MAX_PIPE: u64 = 1024 * 1024;

pub(super) fn temporary_input() -> Result<bool> {
    // /dev/null (common in headless invocations) is not a task list. Pipes,
    // sockets and redirected regular files are; no /dev/tty fallback is used.
    // cli_common::Error converts rustix::io::Errno directly, but this crate's
    // error enum has no such impl and From never chains, so name the step.
    let stat = rustix::fs::fstat(io::stdin()).map_err(std::io::Error::from)?;
    Ok(matches!(
        rustix::fs::FileType::from_raw_mode(stat.st_mode),
        rustix::fs::FileType::Fifo
            | rustix::fs::FileType::RegularFile
            | rustix::fs::FileType::Socket
    ))
}
pub(super) fn read_tasks(input: impl Read) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    input.take(MAX_PIPE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PIPE {
        return Err(Error::InputTooLarge);
    }
    let input = std::str::from_utf8(&bytes).map_err(|_| Error::InputNotUtf8)?;
    let mut tasks = Vec::new();
    for line in input.lines() {
        let Some(line) = task_line(line) else {
            continue;
        };
        let task = text_input(line, MAX_TEXT).ok_or_else(|| Error::InvalidTaskText)?;
        if tasks.len() >= MAX_TASKS {
            return Err(Error::TaskLimit);
        }
        tasks.push(task);
    }
    Ok(tasks)
}
fn task_line(line: &str) -> Option<&str> {
    let mut s = line.trim();
    let heading = s.trim_start_matches('#');
    let count = s.len() - heading.len();
    if (1..=6).contains(&count) && (heading.is_empty() || heading.starts_with(char::is_whitespace))
    {
        return None;
    }
    if let Some(rest) = s.strip_prefix(['-', '*', '+']) {
        if rest.starts_with(char::is_whitespace) {
            s = rest.trim_start();
        }
    } else {
        let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
        if rest.len() < s.len() {
            if let Some(rest) = rest.strip_prefix(['.', ')']) {
                if rest.starts_with(char::is_whitespace) {
                    s = rest.trim_start();
                }
            }
        }
    }
    for mark in ["[x]", "[X]", "[ ]"] {
        if let Some(rest) = s.strip_prefix(mark) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                if mark != "[ ]" {
                    return None;
                }
                s = rest.trim_start();
                break;
            }
        }
    }
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conservative_line_filtering() {
        let lines = "# TODO\n\n- [x] done\n* [X] done too\n- [ ] first\n2. second\n3) third\n+ fourth\nplain\n#hashtag\n";
        assert_eq!(
            read_tasks(lines.as_bytes()).unwrap(),
            ["first", "second", "third", "fourth", "plain", "#hashtag"]
        );
        assert!(read_tasks(b"\xff".as_slice()).is_err());
        assert!(read_tasks(b"- bad\x1btext".as_slice()).is_err());
        assert!(read_tasks("x".repeat(2001).as_bytes()).is_err());
        assert!(read_tasks("x\n".repeat(1001).as_bytes()).is_err());
        assert!(read_tasks(vec![b' '; MAX_PIPE as usize + 1].as_slice()).is_err());
    }
}
