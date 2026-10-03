//! Ownership checks shared by creation, diff, and destructive undo.
use crate::manifest::Manifest;
use cli_common::{
    error,
    git::{self, Repo},
    state, Result,
};
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
};

pub(super) fn ref_log(m: &Manifest) -> Result<String> {
    git::fingerprint(&state::path_from(&m.repository).join("logs").join(&m.branch))
}
pub(super) fn verify(m: &Manifest, restoring: bool) -> Result<Repo> {
    let target = state::path_from(&m.target);
    if fs::symlink_metadata(&target)?.file_type().is_symlink() || target.canonicalize()? != target {
        return Err(error("Worktree path changed", "工作区路径变化"));
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
        return Err(error(
            "Worktree/ref/index identity changed; refusing undo",
            "工作区、分支或索引变化；拒绝撤销",
        ));
    }
    if repo.git_dir.join("locked").exists() {
        return Err(error("Worktree is locked", "工作区已锁定"));
    }
    let back = fs::read(repo.git_dir.join("gitdir"))?;
    let back = back.strip_suffix(b"\n").unwrap_or(&back);
    if state::path_from(back) != target.join(".git") {
        return Err(error("Worktree backlink mismatch", "工作区反向关联不匹配"));
    }
    let registered = git::run(&target, ["worktree", "list", "--porcelain", "-z"])?;
    let expected = [b"worktree ".as_slice(), target.as_os_str().as_bytes()].concat();
    if registered
        .split(|b| *b == 0)
        .filter(|p| *p == expected)
        .count()
        != 1
    {
        return Err(error("Worktree registration mismatch", "工作区登记不匹配"));
    }
    if !git::run(&target, ["ls-files", "--others", "-z"])?.is_empty() {
        return Err(error(
            "Extra untracked/ignored files; preserve your work before undo",
            "存在额外未跟踪或忽略文件；请先保存额外工作",
        ));
    }
    for f in &m.files {
        let path = git::safe_path(&target, &f.path)?;
        let md = fs::symlink_metadata(&path)?;
        if md.mode() & 0o177777 != f.mode {
            return Err(error("File mode changed", "文件模式发生变化"));
        }
        let hash = if md.file_type().is_symlink() {
            state::digest(fs::read_link(&path)?.as_os_str().as_bytes())
        } else {
            git::fingerprint(&path)?
        };
        if hash != f.after && !(restoring && hash == f.before) {
            return Err(error(
                "User edits detected; refusing undo",
                "检测到用户修改；拒绝撤销",
            ));
        }
    }
    Ok(repo)
}
