#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
mod create;
mod manifest;
mod plan;
mod scan;
mod undo;
mod verify;

use crate::{
    create::create,
    manifest::{select, validate_identity},
    undo::undo,
    verify::verify,
};
use cli_common::{
    command, display, error,
    git::Repo,
    state::{self, Store},
    Language, Result,
};
use std::os::unix::ffi::OsStrExt;

fn main() {
    if let Err(e) = run() {
        eprintln!("sprinkle: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    let m = command(
        "sprinkle",
        l.text(
            "Kind comments in a managed HEAD worktree",
            "在受管理的 HEAD 副本中散落友善注释",
        ),
    )
    .subcommand(command(
        "worktree",
        l.text("Create an isolated copy", "创建隔离副本"),
    ))
    .subcommand(command(
        "diff",
        l.text("Show the original insertion patch", "显示创建时的插入补丁"),
    ))
    .subcommand(command(
        "undo",
        l.text(
            "Undo only a verified, unmodified session",
            "仅撤销验证通过且无额外修改的会话",
        ),
    ))
    .get_matches();
    let cwd = std::env::current_dir()?.canonicalize()?;
    let repo = Repo::discover(&cwd)?;
    let store = Store::open()?;
    let rid = state::fields(&store.key, [repo.common.as_os_str().as_bytes()]);
    let dir = store.dir(&format!("sprinkle/{rid}"))?;
    let _lock = state::lock(&dir.join("repository.lock"))?;
    match m.subcommand_name() {
        None | Some("worktree") => create(&store, &dir, &repo),
        Some(action) => {
            let (path, mut manifest) = select(&store, &dir, &repo)?;
            validate_identity(&manifest, &repo)?;
            if action == "diff" {
                if !["active", "restoring", "removed", "done"].contains(&manifest.phase.as_str())
                    || state::digest(manifest.patch.as_bytes()) != manifest.patch_hash
                {
                    return Err(error(
                        "Insertion patch is not complete",
                        "插入补丁尚未完整生成",
                    ));
                }
                if verify(&manifest, false).is_err() {
                    eprintln!("{}",l.text("This is the creation-time patch; the copy has changed or is unavailable.", "以下是创建时的补丁；副本已变化或不可用。"));
                }
                print!("{}", manifest.patch);
                Ok(())
            } else {
                undo(&store, &path, &mut manifest, &cwd)
            }
        }
    }
}
