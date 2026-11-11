//! Ownership checks shared by creation, diff, and destructive undo.
use crate::error::{Error, Result};
use crate::manifest::Manifest;
use cli_common::{
    git::{self, Repo},
    state,
};
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
};

pub(super) fn ref_log(m: &Manifest) -> Result<String> {
    Ok(git::fingerprint(
        &state::path_from(&m.repository).join("logs").join(&m.branch),
    )?)
}
pub(super) fn verify(m: &Manifest, restoring: bool) -> Result<Repo> {
    let target = state::path_from(&m.target);
    if fs::symlink_metadata(&target)?.file_type().is_symlink() || target.canonicalize()? != target {
        return Err(Error::WorktreePathChanged);
    }
    let repo = Repo::discover(&target)?;
    if state::path_bytes(&repo.root) != m.target
        || state::path_bytes(&repo.common) != m.repository
        || state::path_bytes(&repo.git_dir) != m.git_dir
        || repo.head != m.head
        || repo.branch.as_deref() != Some(&m.branch)
        || git::fingerprint(&target.join(".git"))? != m.git_file
        || ref_log(m)? != m.reflog
        || state::digest(&repo.index()?) != m.index
    {
        return Err(Error::WorktreeIdentityChanged);
    }
    if repo.git_dir.join("locked").exists() {
        return Err(Error::WorktreeLocked);
    }
    let back = fs::read(repo.git_dir.join("gitdir"))?;
    let back = back.strip_suffix(b"\n").unwrap_or(&back);
    if state::path_from(back) != target.join(".git") {
        return Err(Error::BacklinkMismatch);
    }
    let registered = git::run(&target, ["worktree", "list", "--porcelain", "-z"])?;
    let expected = [b"worktree ".as_slice(), target.as_os_str().as_bytes()].concat();
    if registered
        .split(|b| *b == 0)
        .filter(|p| *p == expected)
        .count()
        != 1
    {
        return Err(Error::RegistrationMismatch);
    }
    if !git::run(&target, ["ls-files", "--others", "-z"])?.is_empty() {
        return Err(Error::ExtraUntrackedFiles);
    }
    for f in &m.files {
        let path = git::safe_path(&target, &f.path)?;
        let md = fs::symlink_metadata(&path)?;
        if md.mode() & 0o177777 != f.mode {
            return Err(Error::FileModeChanged);
        }
        let hash = if md.file_type().is_symlink() {
            state::digest(fs::read_link(&path)?.as_os_str().as_bytes())
        } else {
            git::fingerprint(&path)?
        };
        if hash != f.after && !(restoring && hash == f.before) {
            return Err(Error::UserEditsDetected);
        }
    }
    Ok(repo)
}
