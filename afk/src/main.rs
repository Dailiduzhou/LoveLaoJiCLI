mod scene;
mod tui;
use cli_common::{command, display, Language, Result};
use std::io::{self, Write};
use std::time::Duration;

fn duration(s: &str) -> std::result::Result<Duration, String> {
    let invalid = || {
        Language::detect()
            .text(
                "Expected a positive integer followed by s/m/h, at most 24h",
                "请输入正整数加 s/m/h，最多 24 小时",
            )
            .to_owned()
    };
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
    match run() {
        Ok(Some(signal)) => {
            // All terminal guards have already run. Preserve signal termination
            // rather than presenting a cancelled break as a normal completion.
            let _ = signal_hook::low_level::emulate_default_handler(signal);
            std::process::exit(128 + signal);
        }
        Ok(None) => (),
        Err(e) => {
            let _ = writeln!(io::stderr().lock(), "afk: {}", display(e.to_string()));
            std::process::exit(1);
        }
    }
}
fn run() -> Result<Option<i32>> {
    let l = Language::detect();
    let matches = command(
        "afk",
        l.text(
            "Take a break with a small grass scene; quiet fallback without a suitable terminal.",
            "看一小片草地，休息一会儿；无合适终端时安静等待。",
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
    .arg(
        clap::Arg::new("color")
            .long("color")
            .value_name("MODE")
            .value_parser(scene::ColorMode::parse)
            .default_value("parts")
            .help(l.text(
                "Artwork colors: rainbow (moving bands), parts (fixed colors), white (all white); NO_COLOR overrides",
                "字符画配色：rainbow（彩虹跑马灯）、parts（分区固定色）、white（全白）；NO_COLOR 优先",
            )),
    )
    .get_matches();
    let color = *matches.get_one::<scene::ColorMode>("color").unwrap();
    let mut wait = *matches.get_one::<Duration>("duration").unwrap();
    if std::env::var("AFK_FAST").as_deref() == Ok("1") {
        wait /= 100;
    }
    if let Some(signal) = tui::run(wait, l, color)? {
        return Ok(Some(signal));
    }
    writeln!(
        io::stdout().lock(),
        "{}",
        l.text("Break complete.", "休息结束。")
    )?;
    Ok(None)
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
