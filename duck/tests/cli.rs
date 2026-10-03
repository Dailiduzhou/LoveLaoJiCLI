#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::io::Write;
use std::process::Stdio;
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_duck");
const ANSWERS: [&str; 8] = [
    "--expected",
    " success ",
    "--actual",
    "failure",
    "--last-change",
    "timeout",
    "--experiment",
    "increase it",
];
#[test]
fn cli_basics() {
    basics(BIN, "duck");
}
#[test]
fn explicit_answers_are_ordered_and_never_saved() {
    let f = Fixture::new(BIN, false);
    let o = f.run(&ANSWERS);
    code(&o, 0);
    assert_eq!(text(&o.stdout), "Expected: success\nActual: failure\nLast change: timeout\nNext experiment: increase it\nquack.\n");
    assert!(o.stderr.is_empty());
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&f.repo).unwrap().count(), 0);
    // Even unusable state/home cannot affect this stateless tool.
    let o = f
        .cmd()
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .args(ANSWERS)
        .output()
        .unwrap();
    code(&o, 0);
}
#[test]
fn unavailable_and_invalid_answers() {
    let f = Fixture::new(BIN, false);
    for n in [0, 2, 4, 6] {
        let o = f.run(&ANSWERS[..n]);
        code(&o, 125);
        assert!(o.stdout.is_empty());
    }
    for answer in [
        " ".to_owned(),
        "a\nb".to_owned(),
        "x".repeat(2001),
        "\x1b[31m".into(),
    ] {
        code(&f.run(&["--expected", &answer]), 2);
    }
    let mut child = f
        .cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .take()
        .unwrap()
        .write_all(b"expected\nactual\nchange\nexperiment\n");
    let o = child.wait_with_output().unwrap();
    code(&o, 125);
    assert!(o.stdout.is_empty());
}
#[test]
fn locale_precedence_and_non_utf8_cwd() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, false);
    let cwd = f
        .root
        .join(std::ffi::OsString::from_vec(b"cwd-\xff".to_vec()));
    fs::create_dir(&cwd).unwrap();
    for (all, messages, lang, chinese) in [
        ("en_US", "zh_CN", "zh_CN", false),
        ("", "zh_CN.UTF-8", "en_US", true),
        ("", "", "zh-CN", true),
    ] {
        let o = f
            .cmd()
            .current_dir(&cwd)
            .env("LC_ALL", all)
            .env("LC_MESSAGES", messages)
            .env("LANG", lang)
            .args(ANSWERS)
            .output()
            .unwrap();
        code(&o, 0);
        assert!(text(&o.stdout).starts_with(if chinese {
            "预期结果: success"
        } else {
            "Expected: success"
        }));
    }
}
#[test]
fn foreground_questions_and_cancellation() {
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/batch_two_pty.py"
        ))
        .args([BIN, "duck"])
        .status()
        .unwrap();
    assert!(status.success());
}
