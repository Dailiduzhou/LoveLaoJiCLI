#![allow(dead_code)]
#[path = "human_support.rs"]
mod human;
use cli_common::state::{self, Store};
pub use human::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn store(f: &Fixture) -> Store {
    let root = f.state.join("lovelaojicli");
    if !root.exists() {
        state::private_dir(&root).unwrap();
        state::atomic(&root.join("identity.key"), &[7; 32], 0o600).unwrap();
        state::atomic(&root.join("identity.lock"), b"", 0o600).unwrap();
    }
    f.store()
}
pub fn record(
    f: &Fixture,
    workspace: &Path,
    id: u32,
    time: u64,
    category: &str,
    exit: i32,
) -> PathBuf {
    let store = store(f);
    let dir = store.workspace(workspace).unwrap();
    state::private_dir(&dir.join("runs")).unwrap();
    // Each fixture is private to one test. Create the authentic lock file, but
    // don't hold an OS lock across other test threads' concurrent fork/exec.
    // A fork could briefly inherit the lock and make the next setup step busy.
    if !dir.join("workspace.lock").exists() {
        state::atomic(&dir.join("workspace.lock"), b"", 0o600).unwrap();
    }
    let id = format!("{id:032x}");
    let value = json!({
        "schema_version": 1, "record_id": id, "created_at": time,
        "workspace_id": dir.file_name().unwrap().to_str().unwrap(),
        "tool": "enough", "invocation_digest": "digest", "program": "build",
        "before": null, "after": null, "started_at": time,
        "finished_at": time, "elapsed_ms": 1, "hypothesis": null,
        "result": {"category": category, "exit_code": exit, "signal": null, "stdout": null, "stderr": null}
    });
    let path = dir.join("runs").join(format!("{id}.json"));
    store.write(&path, &value).unwrap();
    path
}
pub fn mutate(f: &Fixture, path: &Path, change: impl FnOnce(&mut Value)) {
    let store = store(f);
    let mut value: Value = store.read(path).unwrap().unwrap();
    change(&mut value);
    store.write(path, &value).unwrap();
}
pub fn snapshot(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for e in fs::read_dir(path).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                visit(root, &e.path(), out);
            } else if e.file_type().unwrap().is_file() {
                out.push((
                    e.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(e.path()).unwrap(),
                ));
            }
        }
    }
    let mut out = vec![];
    visit(path, path, &mut out);
    out.sort();
    out
}
pub fn run(f: &Fixture) -> std::process::Output {
    f.cmd().env("TZ", "UTC").output().unwrap()
}
