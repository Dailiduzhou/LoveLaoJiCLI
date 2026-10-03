//! Persisted session identity and selection; field names are the on-disk schema.
use cli_common::{
    error,
    git::Repo,
    state::{self, Store},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone)]
pub(super) struct FileRecord {
    pub(super) path: Vec<u8>,
    pub(super) before: String,
    pub(super) after: String,
    pub(super) mode: u32,
    pub(super) offset: usize,
    pub(super) addition: String,
    pub(super) message: usize,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Manifest {
    pub(super) schema_version: u32,
    pub(super) record_id: String,
    pub(super) created_at: u64,
    pub(super) created_order: u128,
    pub(super) tool: String,
    pub(super) repository: Vec<u8>,
    pub(super) source: Vec<u8>,
    pub(super) target: Vec<u8>,
    pub(super) head: String,
    pub(super) branch: String,
    pub(super) phase: String,
    pub(super) git_dir: Vec<u8>,
    pub(super) git_file: String,
    pub(super) reflog: String,
    pub(super) index: String,
    pub(super) files: Vec<FileRecord>,
    pub(super) patch: String,
    pub(super) patch_hash: String,
    pub(super) scanned: usize,
    pub(super) skipped: BTreeMap<String, usize>,
}
pub(super) fn select(store: &Store, dir: &Path, repo: &Repo) -> Result<(PathBuf, Manifest)> {
    let mut sessions = Vec::new();
    for e in fs::read_dir(dir)? {
        let e = e?;
        if !e.file_type()?.is_dir() {
            continue;
        }
        let p = e.path().join("manifest.json");
        let m: Manifest = store
            .read(&p)?
            .ok_or_else(|| error("Incomplete session metadata", "会话元数据不完整"))?;
        if m.phase != "done"
            && (m.source == state::path_bytes(&repo.root)
                || m.target == state::path_bytes(&repo.root))
        {
            sessions.push((p, m));
        }
    }
    sessions.sort_by(|a, b| {
        (a.1.created_order, &a.1.record_id).cmp(&(b.1.created_order, &b.1.record_id))
    });
    sessions
        .pop()
        .ok_or_else(|| error("No managed session here", "当前工作区没有受管理的会话"))
}
pub(super) fn validate_identity(m: &Manifest, r: &Repo) -> Result<()> {
    if m.schema_version != 1
        || m.tool != "sprinkle"
        || m.repository != state::path_bytes(&r.common)
        || !m.branch.starts_with("refs/heads/sprinkle/")
        || m.source == m.target
    {
        return Err(error("Session identity mismatch", "会话身份不匹配"));
    }
    Ok(())
}
