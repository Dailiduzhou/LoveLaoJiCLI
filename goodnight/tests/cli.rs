#[path = "../../tests/evidence_support.rs"]
mod support;
use cli_common::state;
use std::fs;
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_goodnight");
#[test]
fn cli_basics() {
    basics(BIN, "goodnight");
}
#[test]
fn non_git_is_readonly_even_without_home_or_time_zone() {
    let f = Fixture::new(BIN, false);
    let before = snapshot(&f.root);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("Local time: ") && s.contains("+00:00"));
    assert!(s.contains("Branch: unknown") && s.contains("Good night."));
    assert!(s.contains("Latest recorded command (not necessarily tests): unknown"));
    assert_eq!(snapshot(&f.root), before);
    let o = f
        .cmd()
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .env("TZ", "No/SuchZone")
        .output()
        .unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("Local time: unknown"));
    assert_eq!(snapshot(&f.root), before);
}
#[test]
fn git_summary_does_not_update_index_or_invoke_later() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, true);
    fs::write(f.repo.join("code.rs"), "changed\n").unwrap();
    fs::write(
        f.repo
            .join(std::ffi::OsString::from_vec(b"\xff\n\x1b.txt".to_vec())),
        "new",
    )
    .unwrap();
    let before = snapshot(&f.root);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("Unfinished paths (Git status): 2"), "{s}");
    assert!(s.contains("later --next <text>"));
    assert!(!o.stdout.contains(&27));
    assert_eq!(snapshot(&f.root), before);
}
#[test]
fn records_are_commands_not_test_results_and_never_pruned() {
    let f = Fixture::new(BIN, true);
    let now = state::now();
    record(&f, &f.repo, 1, now - 40 * 86400, "exited", 1);
    let path = record(&f, &f.repo, 2, now, "exited", 0);
    let before = snapshot(&f.state);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("enough / build — exit 0"), "{s}");
    assert!(!s.contains("tests passed"));
    assert_eq!(snapshot(&f.state), before);
    mutate(&f, &path, |v| {
        v["finished_at"] = serde_json::Value::Null;
        v["result"] = serde_json::Value::Null;
    });
    assert!(text(&run(&f).stdout).contains("unfinished or incomplete"));
    let store = store(&f);
    let lock = state::lock(&store.workspace_path(&f.repo).join("workspace.lock")).unwrap();
    let before = snapshot(&f.state);
    assert!(text(&run(&f).stdout).contains("tests): unknown"));
    assert_eq!(snapshot(&f.state), before);
    drop(lock);
    fs::write(&path, "broken").unwrap();
    assert!(text(&run(&f).stdout).contains("tests): unknown"));
    assert_eq!(fs::read_to_string(path).unwrap(), "broken");
}
#[test]
fn worktrees_detached_and_unborn_are_safe() {
    let f = Fixture::new(BIN, true);
    record(&f, &f.repo, 1, state::now(), "exited", 0);
    let other = f.root.join("other");
    f.git(&["worktree", "add", "-qb", "other", other.to_str().unwrap()]);
    let o = f.cmd().current_dir(&other).output().unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("tests): unknown"));
    f.git(&["checkout", "--detach", "-q"]);
    assert!(text(&run(&f).stdout).contains("detached HEAD"));
    let empty = Fixture::new(BIN, false);
    empty.git(&["init", "-q"]);
    code(&run(&empty), 0);
}
#[test]
fn locale_priority_and_non_utf8_cwd() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, false);
    let path = f
        .root
        .join(std::ffi::OsString::from_vec(b"cwd\xff".to_vec()));
    fs::create_dir(&path).unwrap();
    let o = f
        .cmd()
        .current_dir(path)
        .env("LC_ALL", "")
        .env("LC_MESSAGES", "zh_CN")
        .env("LANG", "en_US")
        .output()
        .unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("晚安。"));
    let o = f
        .cmd()
        .env("LC_ALL", "en_US")
        .env("LC_MESSAGES", "zh_CN")
        .output()
        .unwrap();
    assert!(text(&o.stdout).contains("Good night."));
}

#[test]
fn same_second_order_and_future_completion_are_not_guessed() {
    let f = Fixture::new(BIN, false);
    let now = state::now();
    let path = record(&f, &f.repo, 1, now, "exited", 0);
    mutate(&f, &path, |v| {
        v["finished_at"] = (now + 3600).into();
    });
    assert!(text(&run(&f).stdout).contains("outcome unknown"));
    record(&f, &f.repo, 2, now, "exited", 7);
    assert!(text(&run(&f).stdout).contains("multiple commands started in the same second"));
}
