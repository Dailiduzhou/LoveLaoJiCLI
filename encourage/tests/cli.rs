#[path = "../../tests/human_support.rs"]
mod support;
use encourage::{pipeline, rng, Language};
use std::{fs, io::Write, os::unix::ffi::OsStringExt, process::Stdio};
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_encourage");

fn expected(args: &[&str], language: Language) -> String {
    format!(
        "{}\n",
        pipeline::generate_plain(rng::seed_from_argv(args).unwrap(), language).unwrap()
    )
}

#[test]
fn basic_flags_and_minimal_localized_help() {
    basics(BIN, "encourage");
    let f = Fixture::new(BIN, false);
    for locale in ["C", "zh_CN"] {
        let o = f.cmd().env("LC_ALL", locale).arg("-h").output().unwrap();
        code(&o, 0);
        let help = text(&o.stdout);
        assert!(help.contains("[TEXT]..."));
        assert!(
            help.contains("--duration <SECONDS>") && help.contains("15s") && help.contains("600s")
        );
        for absent in [
            "--seed",
            "--theme",
            "--lang",
            "--speed",
            "--color",
            "--verison",
            "ENCOURAGE_",
        ] {
            assert!(!help.contains(absent), "{help}");
        }
    }
    for option in ["--seed=1", "--theme=rest", "--speed=2", "--color=white"] {
        code(&f.run(&[option]), 2);
    }
}

#[test]
fn duration_validates_without_changing_the_text_seed_or_plain_output() {
    let f = Fixture::new(BIN, false);
    for args in [
        vec!["--duration", "10s", "a", "b"],
        vec!["a", "b", "--duration=15s"],
        vec!["a", "--duration", "600s", "b"],
    ] {
        let o = f.run(&args);
        code(&o, 0);
        assert_eq!(text(&o.stdout), expected(&["a", "b"], Language::English));
        assert!(o.stderr.is_empty());
    }
    let o = f.run(&["--", "--duration", "10s"]);
    code(&o, 0);
    assert_eq!(
        text(&o.stdout),
        expected(&["--duration", "10s"], Language::English)
    );
    for value in [
        "9s",
        "601s",
        "15",
        "1m",
        "10.5s",
        "+15s",
        "bad\n\x1b[31m",
        "18446744073709551616s",
    ] {
        let o = f.run(&["--duration", value, "hello"]);
        code(&o, 2);
        assert!(o.stdout.is_empty());
        assert!(!o.stderr.contains(&27));
        assert!(!text(&o.stderr).contains("bad"));
    }
    code(&f.run(&["--duration"]), 2);
    localized_usage_error(&f, &["--duration", "9s"], "10s..600s", "10s..600s");
}

#[test]
fn explicit_argv_boundaries_empty_text_and_separator_reach_the_real_generator() {
    let f = Fixture::new(BIN, false);
    for args in [
        vec![""],
        vec!["a", "b"],
        vec!["a b"],
        vec!["42"],
        vec!["é"],
        vec!["e\u{301}"],
        vec!["a\n\x1b[31m"],
    ] {
        let o = f.run(&args);
        code(&o, 0);
        assert_eq!(text(&o.stdout), expected(&args, Language::English));
        assert!(o.stderr.is_empty());
        assert!(!o.stdout.contains(&27));
    }
    let o = f.run(&["--", "--help"]);
    code(&o, 0);
    assert_eq!(text(&o.stdout), expected(&["--help"], Language::English));
    assert_ne!(f.run(&["a", "b"]).stdout, f.run(&["a b"]).stdout);
}

#[test]
fn locale_priority_changes_only_language_and_not_seed_mapping() {
    let f = Fixture::new(BIN, false);
    for (all, messages, lang, language) in [
        ("en_US", "zh_CN", "zh_CN", Language::English),
        ("zh_CN", "en_US", "en_US", Language::Chinese),
        ("", "zh_CN", "en_US", Language::Chinese),
        ("", "", "zh-CN", Language::Chinese),
        ("", "", "fr_FR", Language::English),
    ] {
        let o = f
            .cmd()
            .env("LC_ALL", all)
            .env("LC_MESSAGES", messages)
            .env("LANG", lang)
            .arg("hello")
            .output()
            .unwrap();
        code(&o, 0);
        assert_eq!(text(&o.stdout), expected(&["hello"], language));
        assert!(o.stderr.is_empty());
    }
    localized_usage_error(&f, &["--bad\n\x1b[31m"], "Invalid arguments", "参数无效");
}

#[test]
fn invalid_utf8_is_usage_error_and_never_echoed_lossily_or_as_terminal_controls() {
    let f = Fixture::new(BIN, false);
    for bytes in [b"bad-\xff".to_vec(), b"--bad\n\x1b[31m".to_vec()] {
        let o = f
            .cmd()
            .arg(std::ffi::OsString::from_vec(bytes))
            .output()
            .unwrap();
        code(&o, 2);
        assert!(o.stdout.is_empty());
        assert!(std::str::from_utf8(&o.stderr).is_ok());
        assert!(!o.stderr.contains(&27));
        assert!(!text(&o.stderr).contains("bad"));
    }
}

#[test]
fn no_arguments_use_entropy_but_never_stdin_external_commands_or_state() {
    let f = Fixture::new(BIN, false);
    let mut child = f
        .cmd()
        .env("PATH", "")
        .env("TERM", "xterm")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"not seed material\n")
        .unwrap();
    let o = child.wait_with_output().unwrap();
    code(&o, 0);
    assert!(o.stderr.is_empty());
    let sentence = text(&o.stdout);
    assert_eq!(sentence.lines().count(), 1);
    assert!(sentence.ends_with(".\n"));
    assert!(
        !sentence.contains('\x1b')
            && !sentence.contains("seed")
            && !sentence.contains("not seed material")
    );
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&f.repo).unwrap().count(), 0);
    for term in ["", "dumb", "xterm"] {
        let o = f
            .cmd()
            .env("TERM", term)
            .env("NO_COLOR", "1")
            .env("PATH", "")
            .arg("hello")
            .output()
            .unwrap();
        code(&o, 0);
        assert_eq!(text(&o.stdout), expected(&["hello"], Language::English));
    }
}

#[test]
fn cwd_terminal_color_and_output_failures_do_not_change_mapping_or_panic() {
    let f = Fixture::new(BIN, false);
    let cwd = f.root.join(filename_for_filesystem(&f.root, b"cwd-\xff"));
    fs::create_dir(&cwd).unwrap();
    let o = f
        .cmd()
        .current_dir(cwd)
        .env_remove("NO_COLOR")
        .arg("hello")
        .output()
        .unwrap();
    code(&o, 0);
    assert_eq!(text(&o.stdout), expected(&["hello"], Language::English));
    for args in [vec!["hello"], vec!["--help"], vec!["--version"]] {
        let mut cmd = f.cmd();
        cmd.args(args);
        assert_closed_stdout_fails(cmd, "encourage");
    }
}

#[test]
fn real_cli_terminal_lifecycle_and_plain_fallbacks() {
    let status = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/replay_pty.py"))
        .arg(BIN)
        .status()
        .unwrap();
    assert!(status.success());
    let status = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/ownership_pty.py"
        ))
        .arg(BIN)
        .status()
        .unwrap();
    assert!(status.success());
}
