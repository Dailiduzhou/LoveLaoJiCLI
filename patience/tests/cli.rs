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
}

#[test]
fn failure_run_keeps_exit_code_and_never_fills() {
    let output = show(&["sh", "-c", "exit 3"], &[("LANG", "en_US.UTF-8")]);
    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✗ It failed."), "{stdout:?}");
    assert!(stdout.contains("exit 3"), "{stdout:?}");
    assert!(!stdout.contains('█'), "{stdout:?}");
}

#[test]
fn slow_children_hold_the_final_stall_then_fill() {
    // The show script is fully scripted with PATIENCE_FAST=1, so the child
    // outlives it; the bar must park and the run still succeed.
    let output = invoke(
        &["sleep", "0.3"],
        &[("LANG", "en_US.UTF-8")],
        &[("PATIENCE_FAST", "1"), ("PATIENCE_SEED", "42")],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ Done!"), "{stdout:?}");
    assert!(stdout.contains("exit 0"), "{stdout:?}");
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
fn oversized_child_output_is_capped_and_noted() {
    let output = show(
        &["head", "-c", "3000000", "/dev/zero"],
        &[("LANG", "en_US.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.len() >= 1024 * 1024);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("truncated"), "{stdout:?}");
    assert!(stdout.contains("✓ Done!"), "{stdout:?}");
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
}

#[test]
fn deeply_nested_patience_keeps_one_real_child() {
    let output = show(
        &["patience", "patience", "patience", "printf", "ok"],
        &[("LANG", "zh_CN.UTF-8")],
    );
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ok"), "{stdout:?}");
    assert!(stdout.contains("✓ 成了！"), "{stdout:?}");
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
