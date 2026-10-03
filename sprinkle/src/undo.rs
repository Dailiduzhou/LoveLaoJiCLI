//! Persisted, resumable undo phases. Verify ownership before every removal.
use crate::{
    manifest::Manifest,
    verify::{ref_log, verify},
};
use cli_common::{
    error,
    git::{self, Repo},
    state::{self, Store},
    Language, Result,
};
use std::{ffi::OsStr, fs, os::unix::ffi::OsStrExt, path::Path};

pub(super) fn undo(store: &Store, path: &Path, m: &mut Manifest, cwd: &Path) -> Result<()> {
    let target = state::path_from(&m.target);
    let source = state::path_from(&m.source);
    if cwd.starts_with(&target) {
        return Err(error(
            "Return to the original workspace before undo",
            "请回到原工作区后再撤销",
        ));
    }
    if m.phase == "ref-created"
        && fs::symlink_metadata(&target).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        // A persisted successful ref creation proves ownership; an intent alone does not.
        if ref_log(m)? != m.reflog {
            return Err(error("Ref ownership changed", "分支归属变化"));
        }
        m.phase = "removed".into();
        store.write(path, m)?;
    }
    if ![
        "active",
        "planned",
        "worktree-created",
        "restoring",
        "removing",
        "removed",
        "deleting-ref",
    ]
    .contains(&m.phase.as_str())
    {
        return Err(error(
            "Incomplete transaction: ownership cannot be fully verified; inspect manually",
            "事务未完成，无法完整验证归属；请人工检查",
        ));
    }
    if !["removing", "removed", "deleting-ref"].contains(&m.phase.as_str()) {
        verify(m, m.phase == "restoring" || m.phase == "planned")?;
        if m.phase == "worktree-created" && !Repo::discover(&target)?.status()?.is_empty() {
            return Err(error(
                "Unplanned user changes; refusing undo",
                "存在未登记修改；拒绝撤销",
            ));
        }
        m.phase = "restoring".into();
        store.write(path, m)?;
        for f in m.files.iter().filter(|f| !f.addition.is_empty()) {
            let p = git::safe_path(&target, &f.path)?;
            let bytes = fs::read(&p)?;
            if state::digest(&bytes) == f.before {
                continue;
            }
            if state::digest(&bytes) != f.after
                || bytes.get(f.offset..) != Some(f.addition.as_bytes())
            {
                return Err(error("Insertion no longer matches", "插入内容不再匹配"));
            }
            if state::digest(&bytes[..f.offset]) != f.before {
                return Err(error("Original fingerprint mismatch", "原始指纹不匹配"));
            }
            state::atomic(&p, &bytes[..f.offset], f.mode & 0o777)?;
        }
        let repo = verify(m, true)?;
        if !repo.status()?.is_empty() {
            return Err(error(
                "Restored worktree is not clean",
                "恢复后的工作区不干净",
            ));
        }
        m.phase = "removing".into();
        store.write(path, m)?;
    }
    if m.phase == "removing" {
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                let repo = verify(m, true)?;
                if !repo.status()?.is_empty() {
                    return Err(error(
                        "Restored worktree is not clean",
                        "恢复后的工作区不干净",
                    ));
                }
                git::run(
                    &source,
                    [
                        OsStr::new("worktree"),
                        OsStr::new("remove"),
                        OsStr::new("--"),
                        target.as_os_str(),
                    ],
                )?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if fs::symlink_metadata(state::path_from(&m.git_dir)).is_ok() {
                    return Err(error(
                        "Partial worktree removal; inspect manually",
                        "工作区仅部分移除；请人工检查",
                    ));
                }
            }
            Err(e) => return Err(e),
        }
        m.phase = "removed".into();
        store.write(path, m)?;
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(error(
            "Target reappeared after removal",
            "目标在移除后重新出现",
        ));
    }
    let registered = git::run(&source, ["worktree", "list", "--porcelain", "-z"])?;
    let branch = format!("branch {}", m.branch);
    let wt = [b"worktree ".as_slice(), target.as_os_str().as_bytes()].concat();
    if registered
        .split(|b| *b == 0)
        .any(|p| p == branch.as_bytes() || p == wt)
    {
        return Err(error(
            "Resources are still registered/in use",
            "资源仍被登记或使用",
        ));
    }
    let refs = git::run(&source, ["for-each-ref", "--format=%(refname)", &m.branch])?;
    let exists = refs
        .split(|b| *b == b'\n')
        .any(|r| r == m.branch.as_bytes());
    if exists {
        if ref_log(m)? != m.reflog {
            return Err(error("Ref identity changed", "分支身份变化"));
        }
        m.phase = "deleting-ref".into();
        store.write(path, m)?;
        git::run(&source, ["update-ref", "-d", &m.branch, &m.head])?;
    } else if m.phase != "deleting-ref" {
        return Err(error(
            "Managed ref disappeared unexpectedly",
            "受管理的分支意外消失",
        ));
    }
    m.phase = "done".into();
    store.write(path, m)?;
    println!(
        "{}",
        Language::detect().text("Sprinkle session undone.", "Sprinkle 会话已撤销。")
    );
    Ok(())
}
