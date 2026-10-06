use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_patience");
const NAME: &str = "patience";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn invoke(args: &[&str], locales: &[(&str, &str)], extra_env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command.args(args);
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        command.env_remove(key);
    }
    for key in [
        "PATIENCE_FAST",
        "PATIENCE_SEED",
        "PATIENCE_COLOR",
        "PATIENCE_ANGLE",
    ] {
        command.env_remove(key);
    }
    command.env("PATH", "/usr/bin:/bin");
    command.envs(locales.iter().copied());
    command.envs(extra_env.iter().copied());
    command.output().expect("patience should run")
}

fn show(arguments: &[&str], locales: &[(&str, &str)]) -> Output {
    invoke(
        arguments,
        locales,
        &[("PATIENCE_FAST", "1"), ("PATIENCE_SEED", "7")],
    )
}

#[test]
fn bare_run_quips_and_exits_2() {
    for (locale, expected) in [
        ("en_US.UTF-8", "Usage: patience <command>"),
        ("zh_CN.UTF-8", "用法：patience <命令>"),
    ] {
        let output = invoke(&[], &[("LANG", locale)], &[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(expected),
            "missing {expected:?} in {stderr:?}"
        );
    }
}

#[test]
fn rejects_unknown_options() {
    let output = invoke(&["--unknown"], &[("LANG", "en_US")], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn help_is_localized() {
    for flag in ["--help", "-h"] {
        for (locale, heading, help, version) in [
            ("en-US", "Usage:", "Print help", "Print version"),
            ("zh-CN", "用法：", "显示帮助", "显示版本"),
        ] {
            let output = invoke(&[flag], &[("LANG", locale)], &[]);
            assert!(output.status.success(), "{output:?}");
            let text = String::from_utf8(output.stdout).unwrap();
            for expected in [NAME, heading, help, version, "--help", "--version"] {
                assert!(text.contains(expected), "missing {expected:?} in {text:?}");
            }
        }
    }
}

#[test]
fn version_supports_standard_short_and_legacy_flags() {
    for locale in ["en_US", "zh_CN"] {
        for flag in ["--version", "-V", "--verison"] {
            let output = invoke(&[flag], &[("LANG", locale)], &[]);
            assert!(output.status.success(), "{output:?}");
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                format!("{NAME} {VERSION}\n")
            );
        }
    }
}

#[test]
fn success_run_replays_output_and_reports_exit_zero() {
    let output = show(&["printf", "hello"], &[("LANG", "zh_CN.UTF-8")]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("hello"), "{stdout:?}");
    assert!(stdout.contains("✓ 成了！"), "{stdout:?}");
    assert!(stdout.contains("exit 0"), "{stdout:?}");
    // Not a TTY: the animated bar must not leak into piped output.
    assert!(!stdout.contains('░'), "{stdout:?}");
    assert!(!stdout.contains('\r'), "{stdout:?}");
    assert!(
        !output.stderr.contains(&27),
        "animation is rendered on stderr"
    );
    assert!(!output.stderr.contains(&b'\r'));
}

#[test]
fn failure_run_keeps_exit_code_in_plain_output() {
    let output = show(&["sh", "-c", "exit 3"], &[("LANG", "en_US.UTF-8")]);
    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✗ It failed."), "{stdout:?}");
    assert!(stdout.contains("exit 3"), "{stdout:?}");
    assert!(!stdout.contains('█'), "{stdout:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}

#[test]
fn terminal_progress_holds_until_child_exit_and_fills_only_on_success() {
    // output() pipes stderr and disables the animation entirely. Observe real
    // PTY frames and release the child only after the final hold is visible.
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/progress_pty.py"
        ))
        .arg(BIN)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn signal_death_maps_to_128_plus_signal() {
    let output = show(&["sh", "-c", "exec kill -9 $$"], &[("LANG", "en_US.UTF-8")]);
    assert_eq!(output.status.code(), Some(137));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("exit 137"), "{stdout:?}");
}

#[test]
fn missing_child_reports_spawn_failure() {
    let output = show(
        &["definitely-not-a-command-xyz"],
        &[("LANG", "en_US.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(127));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✗"), "{stdout:?}");
    assert!(stdout.contains("exit 127"), "{stdout:?}");
}

#[test]
fn oversized_child_output_is_capped_per_stream_and_noted() {
    let output = show(
        &["python3", "-c", "import sys; sys.stdout.buffer.write(b'o'*3000000); sys.stderr.buffer.write(b'e'*3000000)"],
        &[("LANG", "en_US.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(0));
    let cap = 1024 * 1024;
    let note = "\n(child output exceeded 1 MiB and was truncated)\n";
    assert!(
        (cap..cap + 200).contains(&output.stdout.len()),
        "stdout cap was not enforced"
    );
    assert_eq!(
        output.stderr.len(),
        cap + note.len(),
        "stderr cap was not enforced"
    );
    assert_eq!(&output.stdout[..cap], vec![b'o'; cap]);
    assert_eq!(&output.stderr[..cap], vec![b'e'; cap]);
    let tail = String::from_utf8(output.stdout[cap..].to_vec()).unwrap();
    assert!(tail.starts_with(note), "{tail:?}");
    assert!(
        tail.contains("✓ Done!") && tail.contains("exit 0"),
        "{tail:?}"
    );
    assert!(tail.len() < 200, "extra child bytes escaped the cap");
    assert_eq!(&output.stderr[cap..], note.as_bytes());
}

#[test]
fn double_dash_separates_patience_flags_from_child_flags() {
    let output = show(&["--", "printf", "ok"], &[("LANG", "en_US.UTF-8")]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ok"), "{stdout:?}");
    assert!(stdout.contains("exit 0"), "{stdout:?}");
}

#[test]
fn nested_patience_runs_the_real_child_behind_extra_bars() {
    let output = show(
        &["patience", "printf", "nested"],
        &[("LANG", "en_US.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("nested"), "{stdout:?}");
    assert!(stdout.contains("✓ Done!"), "{stdout:?}");
    // Not a TTY: none of the stacked bars leak into piped output.
    assert!(!stdout.contains('\r'), "{stdout:?}");
    assert!(!stdout.contains('░'), "{stdout:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(stdout.lines().filter(|line| *line == "nested").count(), 1);
}

#[test]
fn deeply_nested_patience_keeps_one_real_child() {
    let output = show(
        &["patience", "patience", "patience", "printf", "ok"],
        &[("LANG", "zh_CN.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().filter(|line| *line == "ok").count(), 1);
    assert_eq!(stdout.matches("✓ 成了！").count(), 1);
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}

#[test]
fn nested_patience_without_a_real_command_quips() {
    let output = invoke(&["patience"], &[("LANG", "en_US.UTF-8")], &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("Usage: patience <command>"),
        "missing usage in {stderr:?}"
    );
}

#[test]
fn nested_patience_with_only_flags_reports_spawn_failure() {
    let output = show(&["patience", "--help"], &[("LANG", "en_US.UTF-8")]);
    assert_eq!(output.status.code(), Some(127));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✗ It failed."), "{stdout:?}");
    assert!(stdout.contains("exit 127"), "{stdout:?}");
}
