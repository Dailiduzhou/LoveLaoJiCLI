#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_later");

fn input_pty(long: bool) {
    let mut command = std::process::Command::new("python3");
    command
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/input_pty.py"))
        .arg(BIN);
    if long {
        command.arg("--long");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[test]
fn interactive_input_preserves_tool_boundaries() {
    input_pty(false);
}

#[test]
#[ignore = "known canonical-buffer overflow; shared terminal-input design approval required"]
fn long_interactive_input_contract() {
    input_pty(true);
}

#[test]
fn cli_basics() {
    basics(BIN, "later");
}
#[test]
fn headless_card_and_resume() {
    let f = Fixture::new(BIN, true);
    code(&f.run(&["resume"]), 0);
    code(&f.run(&["--next", "  check refreshToken  "]), 0);
    let o = f.run(&["resume"]);
    code(&o, 0);
    assert!(text(&o.stdout)
        .starts_with("Next: check refreshToken\n\nSaved: just now (snapshot at save time)\n"));
    assert!(text(&o.stdout).contains("Diff stat: No tracked-file differences"));
    fs::create_dir(f.repo.join("sub")).unwrap();
    let o = f
        .cmd()
        .current_dir(f.repo.join("sub"))
        .arg("resume")
        .output()
        .unwrap();
    assert!(text(&o.stdout).contains("check refreshToken"));
    let file = f.find("later.json").pop().unwrap();
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before = fs::read(&file).unwrap();
    code(&f.run(&[]), 125);
    code(&f.run(&["--next", "  "]), 2);
    code(&f.run(&["--next", "a\nb"]), 2);
    code(&f.run(&["--next", &"x".repeat(2001)]), 2);
    assert_eq!(fs::read(file).unwrap(), before);
}
#[test]
fn non_git_degrades_and_corruption_is_not_displayed() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["--next", "start here"]), 0);
    let o = f.run(&["resume"]);
    assert!(text(&o.stdout).contains("Git context unavailable"));
    let path = f.find("later.json").pop().unwrap();
    fs::write(&path, "corrupted").unwrap();
    let o = f.run(&["resume"]);
    code(&o, 1);
    assert!(o.stdout.is_empty());
    assert_eq!(fs::read_to_string(path).unwrap(), "corrupted");
}
#[test]
fn saved_schema_is_unchanged_and_context_changes_are_explicit() {
    let f = Fixture::new(BIN, true);
    fs::write(f.repo.join("untracked"), "new").unwrap();
    code(&f.run(&["--next", "continue here"]), 0);
    let path = f.find("later.json").pop().unwrap();
    let store = f.store();
    let mut old: serde_json::Value = store.read(&path).unwrap().unwrap();
    assert_eq!(old["schema_version"], 1);
    old["created_at"] = u64::MAX.into();
    store.write(&path, &old).unwrap();
    let before = fs::read(&path).unwrap();
    for (locale, first, future, empty) in [
        (
            "C",
            "Next: continue here\n\n",
            "Saved time cannot be compared",
            "No tracked-file differences",
        ),
        (
            "zh_CN",
            "下一步: continue here\n\n",
            "保存时间无法比较",
            "无已跟踪文件差异",
        ),
    ] {
        let o = f
            .cmd()
            .env("LC_ALL", locale)
            .arg("resume")
            .output()
            .unwrap();
        code(&o, 0);
        let out = text(&o.stdout);
        assert!(
            out.starts_with(first) && out.contains(future) && out.contains(empty),
            "{out}"
        );
        assert!(out.contains(": 1\n")); // untracked file is not mistaken for a clean workspace
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    f.git(&["checkout", "-qb", "changed"]);
    let o = f.run(&["resume"]);
    assert!(text(&o.stderr).contains("Current branch/HEAD differs"));
    assert_eq!(fs::read(&path).unwrap(), before);
    old["diff_stat"] = serde_json::Value::Null;
    store.write(&path, &old).unwrap();
    assert!(text(&f.run(&["resume"]).stdout).contains("Diff stat: unknown"));
    f.git(&["checkout", "--detach", "-q"]);
    code(&f.run(&["--next", "detached step"]), 0);
    assert!(text(&f.run(&["resume"]).stdout).contains("Branch: detached HEAD"));
}

#[test]
fn never_follows_state_symlinks() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["--next", "before"]), 0);
    let path = f.find("later.json").pop().unwrap();
    fs::remove_file(&path).unwrap();
    let victim = f.root.join("victim");
    fs::write(&victim, "not yours").unwrap();
    symlink(&victim, &path).unwrap();
    code(&f.run(&["--next", "after"]), 1);
    code(&f.run(&["resume"]), 1);
    assert_eq!(fs::read_to_string(victim).unwrap(), "not yours");
}
#[test]
fn workspace_and_worktree_separation() {
    let f = Fixture::new(BIN, true);
    code(&f.run(&["--next", "original"]), 0);
    let copy = f.root.join("other");
    f.git(&["worktree", "add", "-qb", "other", copy.to_str().unwrap()]);
    let o = f.cmd().current_dir(&copy).arg("resume").output().unwrap();
    assert!(text(&o.stdout).contains("No saved context"));
    let o = f
        .cmd()
        .current_dir(&copy)
        .args(["--next", "other"])
        .output()
        .unwrap();
    code(&o, 0);
    assert!(text(&f.run(&["resume"]).stdout).contains("Next: original"));
}
#[test]
fn non_utf8_and_control_paths() {
    let f = Fixture::new(BIN, true);
    let weird = filename_for_filesystem(&f.root, b"x\xff\x1b\n.rs");
    fs::write(f.repo.join(weird), "x").unwrap();
    code(&f.run(&["--next", "test paths"]), 0);
    let o = f.run(&["resume"]);
    code(&o, 0);
    assert!(!o.stdout.contains(&27));
    assert!(text(&o.stdout).contains("\\u{1b}"));
}
#[test]
fn invalid_xdg_falls_back_and_localizes() {
    let f = Fixture::new(BIN, false);
    let o = f
        .cmd()
        .env("XDG_STATE_HOME", "relative")
        .env("LC_ALL", "zh_CN")
        .args(["--next", "明天再看"])
        .output()
        .unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("已为明天保存"));
    assert!(f.root.join(".local/state/lovelaojicli").exists());
}
#[test]
fn locks_fail_without_overwriting() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["--next", "before"]), 0);
    let path = f.find("later.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    let _lock = cli_common::state::lock(&path.parent().unwrap().join("workspace.lock")).unwrap();
    code(&f.run(&["--next", "after"]), 1);
    assert_eq!(fs::read(path).unwrap(), before);
}
