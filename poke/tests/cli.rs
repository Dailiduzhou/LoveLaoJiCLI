#[path = "../../tests/human_support.rs"]
mod support;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::Stdio;
use support::*;
const BIN: &str = env!("CARGO_BIN_EXE_poke");
#[test]
fn cli_basics_and_input_bounds() {
    basics(BIN, "poke");
    let f = Fixture::new(BIN, false);
    assert!(!text(&f.run(&["--help"]).stdout).contains("POKE_SEED"));
    for args in [
        vec!["add"],
        vec!["remove"],
        vec!["add", " "],
        vec!["remove", "bad\nname"],
        vec!["add", "\x1b"],
    ] {
        code(&f.run(&args), 2);
    }
    code(&f.run(&["add", &"x".repeat(201)]), 2);
}
#[test]
fn trim_exact_dedup_remove_and_empty_success() {
    let f = Fixture::new(BIN, false);
    let o = f.run(&[]);
    code(&o, 0);
    assert!(text(&o.stdout).contains("No names yet"));
    code(&f.run(&["add", "  Alice  "]), 0);
    let path = f.find("names.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    assert!(text(&f.run(&["add", "Alice"]).stdout).contains("Already on"));
    assert_eq!(fs::read(&path).unwrap(), before);
    code(&f.run(&["add", "alice"]), 0);
    let names: serde_json::Value = f.store().read(&path).unwrap().unwrap();
    assert_eq!(names["names"], serde_json::json!(["Alice", "alice"]));
    let before = fs::read(&path).unwrap();
    assert!(text(&f.run(&["remove", "absent"]).stdout).contains("not on"));
    assert_eq!(fs::read(&path).unwrap(), before);
    code(&f.run(&["remove", " Alice "]), 0);
    assert_eq!(
        text(&f.run(&[]).stdout),
        "If you like, say hello to: alice\n"
    );
    code(&f.run(&["remove", "alice"]), 0);
    assert!(text(&f.run(&[]).stdout).contains("No names yet"));
}
#[test]
fn deterministic_selection_allows_repeats_and_keeps_no_history() {
    let f = Fixture::new(BIN, false);
    for name in ["A", "B", "C"] {
        code(&f.run(&["add", name]), 0);
    }
    let path = f.find("names.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    let run = || f.cmd().env("POKE_SEED", "42").output().unwrap();
    let first = run();
    code(&first, 0);
    assert!(["A", "B", "C"]
        .iter()
        .any(|name| { text(&first.stdout) == format!("If you like, say hello to: {name}\n") }));
    for _ in 0..4 {
        let o = run();
        code(&o, 0);
        assert_eq!(o.stdout, first.stdout);
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 2); // list and lock only
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    code(&f.cmd().env("POKE_SEED", "invalid").output().unwrap(), 0);
}
#[test]
fn corrupt_unknown_duplicate_unsafe_and_locked_state_fail_closed() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "before"]), 0);
    let path = f.find("names.json").pop().unwrap();
    let before = fs::read(&path).unwrap();
    {
        let _lock = cli_common::state::lock(&path.parent().unwrap().join("names.lock")).unwrap();
        for args in [&[][..], &["add", "after"], &["remove", "before"]] {
            code(&f.run(args), 1);
        }
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    for value in [
        serde_json::json!({"schema_version":2,"names":[]}),
        serde_json::json!({"schema_version":1,"names":["a","a"]}),
    ] {
        f.store().write(&path, &value).unwrap();
        let before = fs::read(&path).unwrap();
        code(&f.run(&["add", "after"]), 1);
        code(&f.run(&[]), 1);
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    fs::write(&path, &before).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    code(&f.run(&["remove", "before"]), 1);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, "broken").unwrap();
    code(&f.run(&[]), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    fs::remove_file(&path).unwrap();
    let victim = f.root.join("victim");
    fs::write(&victim, "not yours").unwrap();
    symlink(&victim, &path).unwrap();
    code(&f.run(&["add", "after"]), 1);
    assert_eq!(fs::read_to_string(victim).unwrap(), "not yours");
}
#[test]
fn concurrent_adds_preserve_every_success() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "initial"]), 0);
    let children: Vec<_> = (0..12)
        .map(|n| {
            f.cmd()
                .args(["add", &format!("name-{n}")])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut expected = vec!["initial".to_owned()];
    for (n, child) in children.into_iter().enumerate() {
        let o = child.wait_with_output().unwrap();
        if o.status.success() {
            expected.push(format!("name-{n}"));
        } else {
            code(&o, 1);
            assert!(text(&o.stderr).contains("busy"), "{o:?}");
        }
    }
    let path = f.find("names.json").pop().unwrap();
    let value: serde_json::Value = f.store().read(&path).unwrap().unwrap();
    let mut actual: Vec<_> = value["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "every acknowledged name must survive exactly once"
    );
}
#[test]
fn capacity_is_bounded_but_duplicate_add_still_succeeds() {
    let f = Fixture::new(BIN, false);
    code(&f.run(&["add", "initial"]), 0);
    let path = f.find("names.json").pop().unwrap();
    let names: Vec<_> = (0..1000).map(|n| format!("name-{n}")).collect();
    f.store()
        .write(
            &path,
            &serde_json::json!({"schema_version":1,"names":names}),
        )
        .unwrap();
    let before = fs::read(&path).unwrap();
    code(&f.run(&["add", "new"]), 1);
    code(&f.run(&["add", "name-0"]), 0);
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn global_scope_locale_priority_and_non_utf8_state_path() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(BIN, false);
    let state = f
        .root
        .join(std::ffi::OsString::from_vec(b"state\xff".to_vec()));
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
    assert_eq!(text(&run(&["add", "朋友"]).stdout), "已添加。\n");
    assert_eq!(text(&run(&[]).stdout), "如果愿意，向这位朋友问个好: 朋友\n");
    let o = f
        .cmd()
        .current_dir(&f.root)
        .env("XDG_STATE_HOME", state)
        .env("LC_ALL", "en_US")
        .env("LC_MESSAGES", "zh_CN")
        .output()
        .unwrap();
    code(&o, 0);
    assert_eq!(text(&o.stdout), "If you like, say hello to: 朋友\n");
}

#[test]
fn invalid_arguments_follow_locale_priority() {
    let f = Fixture::new(BIN, false);
    localized_usage_error(
        &f,
        &["add", ""],
        "Expected a nonempty single line, at most 200 characters",
        "请输入非空单行文本，最多 200 个字符",
    );
}
