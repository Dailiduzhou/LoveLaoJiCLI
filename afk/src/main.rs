use cli_common::{command, display, Language, Result};
use std::io::{self, Write};
use std::time::{Duration, Instant};

fn duration(s: &str) -> std::result::Result<Duration, String> {
    let invalid = || "Expected a positive integer followed by s/m/h, at most 24h".to_owned();
    let multiplier = match s.as_bytes().last() {
        Some(b's') => 1,
        Some(b'm') => 60,
        Some(b'h') => 3600,
        _ => return Err(invalid()),
    };
    let digits = &s[..s.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    let seconds = digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
        .filter(|n| (1..=86400).contains(n))
        .ok_or_else(invalid)?;
    Ok(Duration::from_secs(seconds))
}
fn main() {
    if let Err(e) = run() {
        eprintln!("afk: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    let matches = command(
        "afk",
        l.text(
            "Take a quiet break. Ctrl+C cancels; terminal settings are never changed.",
            "安静休息一会儿。Ctrl+C 可取消；不更改终端设置。",
        ),
    )
    .arg(
        clap::Arg::new("duration")
            .required(true)
            .value_parser(duration)
            .help(l.text(
                "Positive integer + s/m/h (up to 24h)",
                "正整数 + s/m/h（最多 24 小时）",
            )),
    )
    .get_matches();
    let mut wait = *matches.get_one::<Duration>("duration").unwrap();
    if std::env::var("AFK_FAST").as_deref() == Ok("1") {
        wait /= 100;
    }
    // Deliberate plain-text fallback in ALL environments: no raw mode, keyboard
    // reader, signal handler, alternate screen or cursor state to restore.
    // Default foreground signal handling remains intact, including Ctrl+C.
    let start = Instant::now();
    while let Some(remaining) = wait.checked_sub(start.elapsed()) {
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(remaining);
    }
    writeln!(
        io::stdout().lock(),
        "{}",
        l.text("Break complete.", "休息结束。")
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_duration_bounds() {
        for (s, n) in [
            ("1s", 1),
            ("5m", 300),
            ("15m", 900),
            ("24h", 86400),
            ("1440m", 86400),
            ("86400s", 86400),
        ] {
            assert_eq!(duration(s).unwrap(), Duration::from_secs(n));
        }
        for s in [
            "",
            "0s",
            "-1m",
            "+1s",
            "1.5h",
            "1",
            "s",
            "1S",
            "25h",
            "1441m",
            "86401s",
            " 1s",
            "1s ",
            "一s",
            "💤",
            "18446744073709551615h",
        ] {
            assert!(duration(s).is_err(), "{s}");
        }
    }
}
