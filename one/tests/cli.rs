#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::{Output, Stdio};
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_one");
fn pipe(f: &Fixture, args: &[&str], input: &[u8]) -> Output {
    let mut c = f
        .cmd()
        .env("ONE_SEED", "42")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = c.stdin.take().unwrap().write_all(input);
    c.wait_with_output().unwrap()
}
#[test]
fn cli_basics() {
    basics(BIN, "one");
    let f = Fixture::new(BIN, false);
    assert!(!text(&f.run(&["--help"]).stdout).contains("ONE_SEED"));
    for args in [
        &["add"][..],
        &["add", " "],
        &["add", "a\nb"],
        &["done", "extra"],
    ] {
        code(&f.run(args), 2);
    }
}
#[test]
fn stable_selection_until_done_and_global_scope() {
    let f = Fixture::new(BIN, false);
    assert!(text(&f.run(&[]).stdout).contains("Nothing to choose"));
    code(&f.run(&["add", "first"]), 0);
    code(&f.run(&["add", "second"]), 0);
    assert!(text(&f.run(&["done"]).stdout).contains("No selected"));
    let selected = f.cmd().env("ONE_SEED", "42").output().unwrap();
    code(&selected, 0);
    assert!([b"first\n".as_slice(), b"second\n"].contains(&selected.stdout.as_slice()));
    for seed in ["1", "7", "99"] {
        assert_eq!(
            f.cmd().env("ONE_SEED", seed).output().unwrap().stdout,
            selected.stdout
        );
    }
    // Task list is global, not keyed by cwd or Git identity.
    assert_eq!(
        f.cmd().current_dir(&f.root).output().unwrap().stdout,
        selected.stdout
    );
    let path = f.find("tasks.json").pop().unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    code(&f.run(&["done"]), 0);
    let next = f.run(&[]);
    code(&next, 0);
    assert_ne!(next.stdout, selected.stdout);
    code(&f.run(&["done"]), 0);
    assert!(text(&f.run(&[]).stdout).contains("Nothing to choose"));
}
#[test]
fn temporary_lists_are_isolated_even_with_broken_state() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "persistent"]), 0);
    code(&f.run(&[]), 0);
    let path = f.find("tasks.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    let o = pipe(
        &f,
        &[],
        b"# heading\n\n- [x] done\n* [X] also done\n1. [ ] temporary\n",
    );
    code(&o, 0);
    assert_eq!(text(&o.stdout), "temporary\n");
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(text(&f.run(&[]).stdout), "persistent\n");
    for input in [b"".as_slice(), b"# heading\n- [x] done\n"] {
        let o = pipe(&f, &[], input);
        code(&o, 0);
        assert!(text(&o.stdout).contains("Nothing to choose"));
    }
    fs::write(&path, "broken").unwrap();
    assert_eq!(text(&pipe(&f, &[], b"temporary\n").stdout), "temporary\n");
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    fs::remove_file(&path).unwrap();
    // Piped done still operates on persisted selection, never the pipe.
    code(&f.run(&["add", "persistent"]), 0);
    code(&f.run(&[]), 0);
    code(&pipe(&f, &["done"], b"unrelated\n"), 0);
    assert!(text(&f.run(&[]).stdout).contains("Nothing to choose"));
}
#[test]
fn piped_selection_is_repeatable_and_creates_no_state() {
    let f = Fixture::new(BIN, false);
    let input = b"- a\n* b\n3) c\n";
    let a = pipe(&f, &[], input);
    code(&a, 0);
    assert_eq!(a.stdout, pipe(&f, &[], input).stdout);
    assert_eq!(fs::read_dir(&f.state).unwrap().count(), 0);
    code(&pipe(&f, &[], b"\xff"), 1);
    let file = f.root.join("TODO.md");
    fs::write(&file, "- [ ] file task\n").unwrap();
    let o = f
        .cmd()
        .stdin(fs::File::open(file).unwrap())
        .output()
        .unwrap();
    code(&o, 0);
    assert_eq!(text(&o.stdout), "file task\n");
}
#[test]
fn locks_corruption_and_unknown_schema_fail_closed() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "before"]), 0);
    let path = f.find("tasks.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    {
        let _lock = cli_common::state::lock(&path.parent().unwrap().join("tasks.lock")).unwrap();
        for args in [&[][..], &["add", "after"], &["done"]] {
            code(&f.run(args), 1);
        }
        assert_eq!(fs::read(&path).unwrap(), before);
        code(&pipe(&f, &[], b"temporary\n"), 0);
    }
    // Valid envelope, invalid business schema or selected ID must never be reset.
    for value in [
        serde_json::json!({"schema_version": 2, "tasks": [], "selected_task_id": null}),
        serde_json::json!({"schema_version": 1, "tasks": [], "selected_task_id": "missing"}),
    ] {
        f.store().write(&path, &value).unwrap();
        let before = fs::read(&path).unwrap();
        code(&f.run(&["add", "after"]), 1);
        code(&f.run(&["done"]), 1);
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    fs::write(&path, "broken").unwrap();
    code(&f.run(&[]), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    fs::remove_file(&path).unwrap();
    let victim = f.root.join("victim");
    fs::write(&victim, "untouched").unwrap();
    symlink(&victim, &path).unwrap();
    code(&f.run(&["add", "after"]), 1);
    assert_eq!(fs::read_to_string(victim).unwrap(), "untouched");
}
#[test]
fn concurrent_adds_do_not_lose_successful_writes() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "initial"]), 0);
    let children: Vec<_> = (0..12)
        .map(|n| {
            f.cmd()
                .args(["add", &format!("task-{n}")])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut successes = 1;
    for child in children {
        let o = child.wait_with_output().unwrap();
        match o.status.code() {
            Some(0) => successes += 1,
            Some(1) => (),
            _ => panic!("unexpected: {o:?}"),
        }
    }
    let path = f.find("tasks.json").pop().unwrap();
    let record: serde_json::Value = f.store().read(&path).unwrap().unwrap();
    assert_eq!(record["tasks"].as_array().unwrap().len(), successes);
}
#[test]
fn non_utf8_state_and_locale_priority() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, false);
    let state = f
        .root
        .join(std::ffi::OsString::from_vec(b"state-\xff".to_vec()));
    let run = |args: &[&str]| {
        f.cmd()
            .env("XDG_STATE_HOME", &state)
            .env("LC_ALL", "")
            .env("LC_MESSAGES", "zh_CN")
            .env("LANG", "en_US")
            .args(args)
            .output()
            .unwrap()
    };
    let o = run(&["add", "下一步"]);
    code(&o, 0);
    assert_eq!(text(&o.stdout), "已添加。\n");
    assert_eq!(text(&run(&[]).stdout), "下一步\n");
    code(&run(&["done"]), 0);
    assert_eq!(text(&run(&[]).stdout), "没有待选事项。\n");
}

#[test]
fn unsafe_permissions_and_lost_identity_do_not_reset_tasks() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "keep me"]), 0);
    let path = f.find("tasks.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    code(&f.run(&["add", "after"]), 1);
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(f.state.join("lovelaojicli/identity.key")).unwrap();
    code(&f.run(&[]), 1);
    code(&f.run(&["add", "after"]), 1);
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn adding_preserves_selection_and_concurrent_readers_agree() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "original"]), 0);
    assert_eq!(text(&f.run(&[]).stdout), "original\n");
    code(&f.run(&["add", "new task"]), 0);
    assert_eq!(text(&f.run(&[]).stdout), "original\n");
    code(&f.run(&["done"]), 0);
    code(&f.run(&["add", "another"]), 0);
    let children: Vec<_> = (0..12)
        .map(|n| {
            f.cmd()
                .env("ONE_SEED", n.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut selections = Vec::new();
    for child in children {
        let o = child.wait_with_output().unwrap();
        if o.status.success() {
            selections.push(o.stdout);
        } else {
            code(&o, 1);
        }
    }
    assert!(!selections.is_empty());
    let saved = f.run(&[]);
    code(&saved, 0);
    assert!(selections.iter().all(|s| *s == saved.stdout));
}

#[test]
fn invalid_arguments_follow_locale_priority() {
    let f = Fixture::new(BIN, false);
    localized_usage_error(
        &f,
        &["add", ""],
        "Expected a nonempty single line, at most 2000 characters",
        "请输入非空单行文本，最多 2000 个字符",
    );
}
