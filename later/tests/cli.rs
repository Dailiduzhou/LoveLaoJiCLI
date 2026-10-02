#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_later");
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
    assert!(text(&o.stdout).contains("Next: check refreshToken"));
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
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, true);
    let weird = std::ffi::OsString::from_vec(b"x\xff\x1b\n.rs".to_vec());
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
