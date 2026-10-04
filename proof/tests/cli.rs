#[path = "../../tests/evidence_support.rs"]
mod support;
use cli_common::state;
use std::fs;
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_proof");
fn commit(f: &Fixture, author: (&str, &str), author_time: u64, commit_time: u64) {
    f.git(&["add", "."]);
    let mut c = std::process::Command::new("git");
    f.environment(&mut c);
    let o = c
        .env("GIT_AUTHOR_NAME", author.0)
        .env("GIT_AUTHOR_EMAIL", author.1)
        .env("GIT_AUTHOR_DATE", format!("@{author_time} +0000"))
        .env("GIT_COMMITTER_DATE", format!("@{commit_time} +0000"))
        .args(["commit", "--allow-empty", "-qm", "record"])
        .output()
        .unwrap();
    code(&o, 0);
}
#[test]
fn cli_basics() {
    basics(BIN, "proof");
}
#[test]
fn missing_records_are_not_a_claim_about_work_and_never_create_state() {
    let f = Fixture::new(BIN, false);
    let before = snapshot(&f.root);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("no recorded work."));
    assert!(s.contains("Git evidence (repository/author/history): unknown"));
    assert!(s.contains("Completed executions (current workspace, completion date): 0"));
    assert_eq!(snapshot(&f.root), before);
    let o = f
        .cmd()
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .env("TZ", "Bad/Zone")
        .output()
        .unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("Local date: unknown"));
    assert!(text(&o.stdout).contains("Recorded executions: unknown"));
}
#[test]
fn exact_author_committer_date_and_head_reachability() {
    let f = Fixture::new(BIN, true);
    let now = state::now();
    let old = now - 3 * 86400;
    // Configured author match is exact, not a regex; both name and email matter.
    f.git(&["config", "user.name", "A.*"]);
    f.git(&["config", "user.email", "a@example.test"]);
    commit(&f, ("Alice", "a@example.test"), now, now);
    commit(&f, ("A.*", "OTHER@example.test"), now, now);
    commit(&f, ("A.*", "a@example.test"), old, now); // counts by committer time
                                                     // Older child must not prune today's reachable parent.
    commit(&f, ("A.*", "a@example.test"), now, old);
    let base = text(&f.git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    f.git(&["checkout", "-qb", "unreachable"]);
    commit(&f, ("A.*", "a@example.test"), now, now);
    f.git(&["checkout", "--detach", "-q", &base]);
    let o = run(&f);
    code(&o, 0);
    assert!(
        text(&o.stdout).contains("Authored commits (current HEAD, committer date): 1"),
        "{}",
        text(&o.stdout)
    );
    f.git(&["config", "--unset", "user.email"]);
    assert!(text(&run(&f).stdout).contains("Git evidence (repository/author/history): unknown"));
}
#[test]
fn non_merge_line_sum_path_dedup_binary_and_dirty_exclusion() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, true);
    let now = state::now();
    fs::write(f.repo.join("code.rs"), "line1\nline2\n").unwrap();
    fs::write(f.repo.join("binary"), b"a\0b").unwrap();
    let odd = std::ffi::OsString::from_vec(b"odd\xff\t\n\x1b".to_vec());
    fs::write(f.repo.join(&odd), "one\n").unwrap();
    commit(&f, ("Test", "test@localhost"), now, now);
    fs::write(f.repo.join("code.rs"), "line1\nline2\nline3\n").unwrap();
    commit(&f, ("Test", "test@localhost"), now, now);
    fs::write(f.repo.join("code.rs"), "uncommitted\n").unwrap();
    let before = snapshot(&f.root);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(
        s.contains("Authored commits (current HEAD, committer date): 3"),
        "{s}"
    );
    assert!(
        s.contains("+5 / -1; unique paths: 3; binary paths, no line count: 1"),
        "{s}"
    );
    assert!(!o.stdout.contains(&27));
    assert_eq!(snapshot(&f.root), before);
}
#[test]
fn merge_counts_as_a_commit_but_not_a_second_patch() {
    let f = Fixture::new(BIN, true);
    let now = state::now();
    let base = text(&f.git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    f.git(&["checkout", "-qb", "side"]);
    fs::write(f.repo.join("side"), "one\n").unwrap();
    commit(&f, ("Test", "test@localhost"), now, now);
    f.git(&["checkout", "-qb", "mainline", &base]);
    fs::write(f.repo.join("main"), "one\n").unwrap();
    commit(&f, ("Test", "test@localhost"), now, now);
    f.git(&["merge", "--no-ff", "-qm", "merge", "side"]);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("committer date): 4"), "{s}");
    assert!(s.contains("+3 / -0; unique paths: 3"), "{s}");
}
#[test]
fn explicit_executions_exclude_missing_launch_incomplete_and_future_results() {
    let f = Fixture::new(BIN, false);
    let now = state::now();
    record(&f, &f.repo, 1, now, "exited", 0);
    record(&f, &f.repo, 2, now, "exited", 7);
    record(&f, &f.repo, 3, now, "spawn-failed", 127);
    record(&f, &f.repo, 4, now - 3 * 86400, "exited", 0);
    record(&f, &f.repo, 5, now + 3 * 86400, "exited", 0);
    let unfinished = record(&f, &f.repo, 6, now, "exited", 0);
    mutate(&f, &unfinished, |v| {
        v["result"] = serde_json::Value::Null;
        v["finished_at"] = serde_json::Value::Null;
    });
    let incomplete = record(&f, &f.repo, 7, now, "exited", 0);
    mutate(&f, &incomplete, |v| {
        v["tool"] = "stuck".into();
    }); // no complete pipe evidence
    let before = snapshot(&f.state);
    let o = run(&f);
    code(&o, 0);
    let s = text(&o.stdout);
    assert!(s.contains("completion date): 2"), "{s}");
    assert!(
        s.contains("Successful executions (not necessarily tests): 1"),
        "{s}"
    );
    assert_eq!(snapshot(&f.state), before);
    let other = f.root.join("another-workspace");
    fs::create_dir(&other).unwrap();
    let o = f
        .cmd()
        .current_dir(other)
        .env("TZ", "UTC")
        .output()
        .unwrap();
    assert!(text(&o.stdout).contains("completion date): 0"));
}
#[test]
fn corrupt_unknown_locked_symlink_and_missing_identity_are_unknown_not_zero() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new(BIN, false);
    let path = record(&f, &f.repo, 1, state::now(), "exited", 0);
    let before = fs::read(&path).unwrap();
    let lock = state::lock(&store(&f).workspace_path(&f.repo).join("workspace.lock")).unwrap();
    assert!(text(&run(&f).stdout).contains("Recorded executions: unknown"));
    drop(lock);
    mutate(&f, &path, |v| {
        v["schema_version"] = 2.into();
    });
    assert!(text(&run(&f).stdout).contains("Recorded executions: unknown"));
    fs::write(&path, "broken").unwrap();
    assert!(text(&run(&f).stdout).contains("Recorded executions: unknown"));
    fs::remove_file(&path).unwrap();
    let victim = f.root.join("victim");
    fs::write(&victim, &before).unwrap();
    symlink(&victim, &path).unwrap();
    assert!(text(&run(&f).stdout).contains("Recorded executions: unknown"));
    assert_eq!(fs::read(&victim).unwrap(), before);
    fs::remove_file(&path).unwrap();
    fs::write(&path, &before).unwrap();
    fs::remove_file(store(&f).root.join("identity.key")).unwrap();
    let before = snapshot(&f.state);
    assert!(text(&run(&f).stdout).contains("Recorded executions: unknown"));
    assert_eq!(snapshot(&f.state), before);
}
#[test]
fn working_directory_locale_and_filters_never_execute_repository_code() {
    use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
    let f = Fixture::new(BIN, true);
    let trap = f.repo.join("tripwire");
    fs::write(
        &trap,
        format!("#!/bin/sh\ntouch '{}'\n", f.root.join("EXECUTED").display()),
    )
    .unwrap();
    fs::set_permissions(&trap, fs::Permissions::from_mode(0o755)).unwrap();
    f.git(&["config", "diff.external", trap.to_str().unwrap()]);
    f.git(&["config", "diff.foo.textconv", trap.to_str().unwrap()]);
    f.git(&["config", "log.showSignature", "true"]);
    f.git(&["config", "gpg.program", trap.to_str().unwrap()]);
    fs::write(f.repo.join(".gitattributes"), "* diff=foo\n").unwrap();
    let sub = f
        .repo
        .join(std::ffi::OsString::from_vec(b"cwd\xff".to_vec()));
    fs::create_dir(&sub).unwrap();
    let o = f
        .cmd()
        .current_dir(sub)
        .env("TZ", "UTC")
        .env("LC_ALL", "")
        .env("LC_MESSAGES", "zh_CN")
        .env("LANG", "en_US")
        .output()
        .unwrap();
    code(&o, 0);
    assert!(
        text(&o.stdout).contains("本人提交（当前 HEAD，按提交时间）: 1"),
        "{}",
        text(&o.stdout)
    );
    assert!(!f.root.join("EXECUTED").exists());
}

#[test]
fn repeat_baselines_are_not_execution_records_and_worktrees_are_isolated() {
    let f = Fixture::new(BIN, true);
    let path = record(&f, &f.repo, 1, state::now(), "exited", 0);
    let store = store(&f);
    let dir = store.workspace_path(&f.repo);
    // Reminder/gate counts in repeat state are not evidence of execution.
    for tool in ["enough", "stuck"] {
        store
            .write(
                &dir.join(format!("{tool}.json")),
                &serde_json::json!({"schema_version":1,"baseline":{"count":999}}),
            )
            .unwrap();
    }
    assert!(text(&run(&f).stdout).contains("completion date): 1"));
    let copy = f.root.join("other");
    f.git(&["worktree", "add", "-qb", "other", copy.to_str().unwrap()]);
    let o = f.cmd().current_dir(copy).env("TZ", "UTC").output().unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("completion date): 0"));
    fs::remove_file(path).unwrap();
    assert!(text(&run(&f).stdout).contains("completion date): 0"));
}

#[test]
fn renames_count_raw_paths_and_shallow_history_is_unknown() {
    let f = Fixture::new(BIN, true);
    f.git(&["mv", "code.rs", "renamed.rs"]);
    commit(&f, ("Test", "test@localhost"), state::now(), state::now());
    let o = run(&f);
    code(&o, 0);
    assert!(
        text(&o.stdout).contains("+2 / -1; unique paths: 2"),
        "{}",
        text(&o.stdout)
    );
    let head = text(&f.git(&["rev-parse", "HEAD"]).stdout);
    fs::write(f.repo.join(".git/shallow"), head).unwrap();
    let o = run(&f);
    code(&o, 0);
    assert!(text(&o.stdout).contains("Git evidence (repository/author/history): unknown"));
}

#[test]
fn attributes_are_pinned_per_commit_not_worktree_index_or_global_files() {
    let f = Fixture::new(BIN, true);
    let clean = run(&f);
    code(&clean, 0);
    assert!(
        text(&clean.stdout).contains("+1 / -0; unique paths: 1; binary paths, no line count: 0")
    );
    // Untracked, then staged, then dirty attributes must not reinterpret HEAD.
    let attrs = f.repo.join(".gitattributes");
    fs::write(&attrs, "*.rs -diff\n").unwrap();
    assert_eq!(run(&f).stdout, clean.stdout);
    f.git(&["add", ".gitattributes"]);
    assert_eq!(run(&f).stdout, clean.stdout);
    fs::write(&attrs, "* -diff\n").unwrap();
    let global = f.root.join("attributes");
    fs::write(&global, "* -diff\n").unwrap();
    f.git(&["config", "core.attributesFile", global.to_str().unwrap()]);
    let before = snapshot(&f.root);
    assert_eq!(run(&f).stdout, clean.stdout);
    assert_eq!(snapshot(&f.root), before);

    // Committed attributes ARE respected, but only for that commit: the first
    // commit's text line must still count after the next commit marks it binary.
    fs::write(&attrs, "*.rs -diff\n").unwrap();
    fs::write(f.repo.join("code.rs"), "changed\nanother line\n").unwrap();
    commit(&f, ("Test", "test@localhost"), state::now(), state::now());
    let before = snapshot(&f.root);
    let o = f
        .cmd()
        .env("TZ", "UTC")
        .env("GIT_ATTR_SOURCE", "HEAD~1")
        .output()
        .unwrap();
    code(&o, 0);
    assert!(
        text(&o.stdout).contains("+2 / -0; unique paths: 2; binary paths, no line count: 1"),
        "{}",
        text(&o.stdout)
    );
    assert_eq!(snapshot(&f.root), before);
}

#[test]
fn local_attribute_overrides_are_unknown_in_main_and_linked_worktrees() {
    let f = Fixture::new(BIN, true);
    let other = f.root.join("other");
    f.git(&["worktree", "add", "-qb", "other", other.to_str().unwrap()]);
    fs::write(f.repo.join(".git/info/attributes"), "* -diff\n").unwrap();
    let before = snapshot(&f.root);
    for cwd in [&f.repo, &other] {
        let o = f.cmd().current_dir(cwd).env("TZ", "UTC").output().unwrap();
        code(&o, 0);
        assert!(text(&o.stdout).contains("Git evidence (repository/author/history): unknown"));
    }
    assert_eq!(snapshot(&f.root), before);
}

#[test]
fn unsupported_attribute_source_never_falls_back_to_mutable_attributes() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(BIN, true);
    let bin = f.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let git = bin.join("git");
    fs::write(&git, b"#!/bin/sh\nfor arg do\n case \"$arg\" in --attr-source=*) exit 129;; esac\ndone\nPATH=/usr/bin:/bin exec git \"$@\"\n").unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let before = snapshot(&f.root);
    let o = f.cmd().env("PATH", bin).env("TZ", "UTC").output().unwrap();
    code(&o, 0);
    assert!(text(&o.stdout).contains("Git evidence (repository/author/history): unknown"));
    assert_eq!(snapshot(&f.root), before);
}
