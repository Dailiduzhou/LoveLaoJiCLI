use super::{BIN, TOOL};
#[path = "human_support.rs"]
pub mod support;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use support::*;
#[test]
fn basic_interface() {
    basics(BIN, TOOL);
    let f = Fixture::new(BIN, false);
    code(&f.run(&[]), 2);
}
#[test]
fn pass_through_exit_streams_argv_and_spawn_errors() {
    let f = Fixture::new(BIN, false);
    let o = f.run(&[
        "sh",
        "-c",
        "printf '%s' \"$1\"; printf err >&2; exit 7",
        "name",
        "a b",
    ]);
    code(&o, 7);
    assert_eq!(o.stdout, b"a b");
    assert_eq!(o.stderr, b"err");
    let o = f.run(&["sh", "-c", "printf '%s' \"$1\"", "name", "--version"]);
    code(&o, 0);
    assert_eq!(o.stdout, b"--version");
    code(&f.run(&["no-such-humanutils-command"]), 127);
    fs::write(f.repo.join("not-executable"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(
        f.repo.join("not-executable"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    code(&f.run(&["./not-executable"]), 126);
    code(&f.run(&["sh", "-c", "kill -TERM $$"]), 143);
    code(&f.run(&["--", "sh", "-c", "exit 125"]), 125);
}
#[test]
fn pipes_always_run_and_never_record_argv_or_output() {
    let f = Fixture::new(BIN, true);
    for _ in 0..4 {
        let o = f.run(&[
            "sh",
            "-c",
            "printf TOP_SECRET_OUTPUT; exit 4",
            "PASSWORD_SENTINEL",
        ]);
        code(&o, 4);
        assert_eq!(o.stdout, b"TOP_SECRET_OUTPUT");
    }
    let dirs = f.find(&format!("{TOOL}.json"));
    assert_eq!(dirs.len(), 1);
    let state = f
        .store()
        .read::<serde_json::Value>(&dirs[0])
        .unwrap()
        .unwrap();
    assert!(state["baseline"].is_null());
    let runs = dirs[0].parent().unwrap().join("runs");
    let records: Vec<_> = fs::read_dir(runs)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(records.len(), 4, "each real execution must leave a record");
    for path in records {
        let record: serde_json::Value = f.store().read(&path).unwrap().unwrap();
        assert_eq!(record["result"]["exit_code"], 4);
        assert!(record["finished_at"].as_u64().is_some());
        let data = fs::read_to_string(path).unwrap();
        assert!(!data.contains("TOP_SECRET"));
        assert!(!data.contains("PASSWORD_SENTINEL"));
    }
}
#[test]
fn state_fault_is_fail_open() {
    let f = Fixture::new(BIN, true);
    code(&f.run(&["true"]), 0);
    let path = f.find(&format!("{TOOL}.json")).pop().unwrap();
    fs::write(&path, "damaged").unwrap();
    let o = f.run(&["sh", "-c", "printf real; exit 9"]);
    code(&o, 9);
    assert_eq!(o.stdout, b"real");
    assert!(text(&o.stderr).contains("unavailable"));
    assert_eq!(fs::read_to_string(path).unwrap(), "damaged");
}
#[test]
fn forwards_large_binary_streams_without_recording_body() {
    let f = Fixture::new(BIN, false);
    // Buffered write_all equivalents avoid short os.write() results making
    // the producer itself lose bytes. Assert content, not only stream length.
    let o = f.run(&["python3", "-c", "import sys,threading; t=threading.Thread(target=lambda: sys.stderr.buffer.write(b'e'*1100000)); t.start(); sys.stdout.buffer.write(bytes(range(256))*5000); t.join()"]);
    code(&o, 0);
    assert_eq!(o.stdout, (0..=255u8).collect::<Vec<_>>().repeat(5000));
    assert_eq!(o.stderr, vec![b'e'; 1100000]);
}
#[test]
fn foreground_pty_state_machine_and_interruptions() {
    let o = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/repeat_pty.py"))
        .arg(BIN)
        .output()
        .unwrap();
    code(&o, 0);
}
