#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::time::{Duration, Instant};
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_afk");
#[test]
fn cli_basics() {
    basics(BIN, "afk");
    let f = Fixture::new(BIN, false);
    let help = text(&f.run(&["--help"]).stdout);
    assert!(!help.contains("AFK_FAST"));
    assert!(!help.contains("AFK_TEST_FAILURE"));
    code(&f.run(&[]), 2);
    for s in ["0s", "25h", "2.5m", "tomorrow", "18446744073709551615h"] {
        code(&f.run(&[s]), 2);
    }
}
#[test]
fn non_tty_waits_and_emits_only_a_completion_line() {
    let f = Fixture::new(BIN, false);
    let start = Instant::now();
    let o = f
        .cmd()
        .env_remove("AFK_FAST")
        .env("TERM", "dumb")
        .arg("1s")
        .output()
        .unwrap();
    assert!(start.elapsed() >= Duration::from_secs(1));
    code(&o, 0);
    assert_eq!(text(&o.stdout), "Break complete.\n");
    assert!(o.stderr.is_empty());
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
}
#[test]
fn fast_hook_and_localized_completion() {
    let f = Fixture::new(BIN, false);
    let o = f
        .cmd()
        .env("AFK_FAST", "1")
        .env("LC_ALL", "")
        .env("LC_MESSAGES", "zh_CN")
        .env("LANG", "en_US")
        .arg("1s")
        .output()
        .unwrap();
    code(&o, 0);
    assert_eq!(text(&o.stdout), "休息结束。\n");
    assert!(o.stderr.is_empty());
}
#[test]
fn fallback_terminal_settings_unchanged_on_completion_error_and_signals() {
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/batch_two_pty.py"
        ))
        .args([BIN, "afk"])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn real_tui_lifecycle_and_job_control() {
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/afk_tui_pty.py"
        ))
        .arg(BIN)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn non_utf8_cwd_and_english_locale_override() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, false);
    let cwd = f
        .root
        .join(std::ffi::OsString::from_vec(b"cwd-\xff".to_vec()));
    fs::create_dir(&cwd).unwrap();
    let o = f
        .cmd()
        .current_dir(cwd)
        .env("AFK_FAST", "1")
        .env("LC_ALL", "en_US")
        .env("LC_MESSAGES", "zh_CN")
        .env("LANG", "zh_CN")
        .arg("1s")
        .output()
        .unwrap();
    code(&o, 0);
    assert_eq!(text(&o.stdout), "Break complete.\n");
}

#[test]
fn invalid_arguments_follow_locale_priority() {
    let f = Fixture::new(BIN, false);
    localized_usage_error(
        &f,
        &["nope"],
        "Expected a positive integer followed by s/m/h, at most 24h",
        "请输入正整数加 s/m/h，最多 24 小时",
    );
}

#[test]
fn ownership_loss_preserves_new_foreground_settings_and_screen() {
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/afk_ownership_pty.py"
        ))
        .arg(BIN)
        .status()
        .unwrap();
    assert!(status.success());
}
