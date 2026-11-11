//! Persisted, resumable undo phases. Verify ownership before every removal.
use crate::error::{Error, Result};
use crate::{
    manifest::Manifest,
    verify::{ref_log, verify},
};
use cli_common::{
    git::{self, Repo},
    state::{self, Store},
    Language,
};
use std::{ffi::OsStr, fs, io::Write, os::unix::ffi::OsStrExt, path::Path};

pub(super) fn undo(store: &Store, path: &Path, m: &mut Manifest, cwd: &Path) -> Result<()> {
    let target = state::path_from(&m.target);
    let source = state::path_from(&m.source);
    if cwd.starts_with(&target) {
        return Err(Error::ReturnToSource);
    }
    if m.phase == "ref-created"
        && fs::symlink_metadata(&target).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        // A persisted successful ref creation proves ownership; an intent alone does not.
        if ref_log(m)? != m.reflog {
            return Err(Error::RefOwnershipChanged);
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
        return Err(Error::IncompleteTransaction);
    }
    if !["removing", "removed", "deleting-ref"].contains(&m.phase.as_str()) {
        verify(m, m.phase == "restoring" || m.phase == "planned")?;
        if m.phase == "worktree-created" && !Repo::discover(&target)?.status()?.is_empty() {
            return Err(Error::UnplannedChanges);
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
                return Err(Error::InsertionMismatch);
            }
            if state::digest(&bytes[..f.offset]) != f.before {
                return Err(Error::OriginalFingerprintMismatch);
            }
            state::atomic(&p, &bytes[..f.offset], f.mode & 0o777)?;
        }
        let repo = verify(m, true)?;
        if !repo.status()?.is_empty() {
            return Err(Error::RestoredWorktreeDirty);
        }
        m.phase = "removing".into();
        store.write(path, m)?;
    }
    if m.phase == "removing" {
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                let repo = verify(m, true)?;
                if !repo.status()?.is_empty() {
                    return Err(Error::RestoredWorktreeDirty);
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
                    return Err(Error::PartialRemoval);
                }
            }
            Err(e) => return Err(e.into()),
        }
        m.phase = "removed".into();
        store.write(path, m)?;
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(Error::TargetReappeared);
    }
    let registered = git::run(&source, ["worktree", "list", "--porcelain", "-z"])?;
    let branch = format!("branch {}", m.branch);
    let wt = [b"worktree ".as_slice(), target.as_os_str().as_bytes()].concat();
    if registered
        .split(|b| *b == 0)
        .any(|p| p == branch.as_bytes() || p == wt)
    {
        return Err(Error::ResourcesRegistered);
    }
    let refs = git::run(&source, ["for-each-ref", "--format=%(refname)", &m.branch])?;
    let exists = refs
        .split(|b| *b == b'\n')
        .any(|r| r == m.branch.as_bytes());
    if exists {
        if ref_log(m)? != m.reflog {
            return Err(Error::RefIdentityChanged);
        }
        m.phase = "deleting-ref".into();
        store.write(path, m)?;
        git::run(&source, ["update-ref", "-d", &m.branch, &m.head])?;
    } else if m.phase != "deleting-ref" {
        return Err(Error::RefDisappeared);
    }
    m.phase = "done".into();
    store.write(path, m)?;
    writeln!(
        std::io::stdout().lock(),
        "{}",
        Language::detect().text("Sprinkle session undone.", "Sprinkle 会话已撤销。")
    )?;
    Ok(())
}
