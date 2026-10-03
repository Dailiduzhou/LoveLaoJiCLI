//! Create the managed worktree and persist each transaction phase.
use crate::{
    manifest::Manifest,
    plan::plan,
    verify::{ref_log, verify},
};
use cli_common::{
    display, error,
    git::{self, Repo},
    state::{self, Store},
    Language, Result,
};
use std::{collections::BTreeMap, ffi::OsStr, fs, os::unix::ffi::OsStrExt, path::Path};

pub(super) fn create(store: &Store, dir: &Path, repo: &Repo) -> Result<()> {
    let l = Language::detect();
    if !repo.status()?.is_empty() {
        return Err(error(
            "Source checkout must be clean, including untracked files",
            "源工作区必须干净，包括未跟踪文件",
        ));
    }
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "BISECT_LOG",
    ] {
        if repo.git_dir.join(marker).exists() {
            return Err(error("Git operation in progress", "Git 操作尚未结束"));
        }
    }
    let tracked = repo.tracked()?;
    let _ = repo.snapshot(&repo.root)?; // same conservative file/count budget as wrappers

    git::ensure_no_filters(&repo.root)?;
    // Also catch attributes whose filter driver is not currently configured.
    for chunk in tracked.chunks(100) {
        let mut args = git::args(&["check-attr", "-z", "filter", "--"]);
        args.extend(chunk.iter().map(|(p, _)| OsStr::from_bytes(p).to_owned()));
        let out = git::run(&repo.root, args)?;
        for triple in out.split(|b| *b == 0).collect::<Vec<_>>().chunks(3) {
            if triple.len() == 3
                && ![b"unspecified".as_slice(), b"unset".as_slice()].contains(&triple[2])
            {
                return Err(error(
                    "Files use checkout filters",
                    "文件使用 checkout 过滤器",
                ));
            }
        }
    }
    let id = state::id();
    let time = state::now();
    let branch = format!("refs/heads/sprinkle/{time}-{}", &id[..12]);
    let parent = repo
        .root
        .parent()
        .ok_or_else(|| error("No worktree parent", "工作区没有父目录"))?;
    let target = parent.join(format!(
        "{}-sprinkled-{}",
        repo.root
            .file_name()
            .unwrap_or(OsStr::new("project"))
            .to_string_lossy(),
        &id[..12]
    ));
    if fs::symlink_metadata(&target).is_ok() {
        return Err(error("Target already exists", "目标已存在"));
    }
    let session = dir.join(&id);
    state::private_dir(&session)?;
    let path = session.join("manifest.json");
    let mut m = Manifest {
        schema_version: 1,
        record_id: id,
        created_at: time,
        created_order: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        tool: "sprinkle".into(),
        repository: state::path_bytes(&repo.common),
        source: state::path_bytes(&repo.root),
        target: state::path_bytes(&target),
        head: repo.head.clone(),
        branch,
        phase: "prepared".into(),
        git_dir: vec![],
        git_file: String::new(),
        reflog: String::new(),
        index: String::new(),
        files: vec![],
        patch: String::new(),
        patch_hash: String::new(),
        scanned: 0,
        skipped: BTreeMap::new(),
    };
    store.write(&path, &m)?;
    let result = (|| {
        git::run(
            &repo.root,
            [
                "-c",
                "core.logAllRefUpdates=true",
                "update-ref",
                &m.branch,
                &m.head,
                &"0".repeat(m.head.len()),
            ],
        )?;
        m.phase = "ref-created".into();
        m.reflog = ref_log(&m)?;
        store.write(&path, &m)?;
        state::private_dir(&target)?;
        let args = vec![
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--no-checkout"),
            OsStr::new("--"),
            target.as_os_str(),
            OsStr::new(m.branch.strip_prefix("refs/heads/").unwrap()),
        ];
        git::run(&repo.root, args)?;
        git::ensure_no_filters(&target)?;
        git::run(&target, ["checkout", "HEAD", "--", "."])?;
        let copy = Repo::discover(&target)?;
        m.git_dir = state::path_bytes(&copy.git_dir);
        m.git_file = git::fingerprint(&target.join(".git"))?;
        m.index = state::digest(&copy.index()?);
        m.phase = "worktree-created".into();
        store.write(&path, &m)?;
        if !copy.status()?.is_empty() {
            return Err(error("HEAD checkout is not clean", "HEAD 副本不干净"));
        }
        let count = plan(&copy, &target, &mut m, l)?;
        m.phase = "planned".into();
        store.write(&path, &m)?;
        state::atomic(&session.join("additions.patch"), m.patch.as_bytes(), 0o600)?;
        for f in m.files.iter().filter(|f| !f.addition.is_empty()) {
            let path = git::safe_path(&target, &f.path)?;
            if git::fingerprint(&path)? != f.before {
                return Err(error("File changed before insertion", "插入前文件发生变化"));
            }
            let mut bytes = fs::read(&path)?;
            bytes.extend_from_slice(f.addition.as_bytes());
            state::atomic(&path, &bytes, f.mode & 0o777)?;
        }
        verify(&m, false)?;
        m.phase = "active".into();
        store.write(&path, &m)?;
        println!("{}: {}", l.text("Created", "已创建"), display(&target));
        println!(
            "{}: {}; {}: {}",
            l.text("Files scanned", "扫描文件"),
            m.scanned,
            l.text("Comments added", "添加注释"),
            count
        );
        for (reason, count) in &m.skipped {
            println!("{} ({reason}): {count}", l.text("Skipped", "已跳过"));
        }
        println!("{}",l.text("Original checkout files and branch unchanged. This is a HEAD copy, not a full disk backup.", "原 checkout 的文件与分支未改变。这是 HEAD 副本，不是磁盘目录的完整备份。"));
        Ok(())
    })();
    if result.is_err() {
        eprintln!(
            "{}: {} ({})",
            l.text(
                "Incomplete session retained for verified undo/manual inspection",
                "未完成会话已保留，请验证撤销或人工检查"
            ),
            display(&path),
            m.phase
        );
    }
    result
}
