//! Create the managed worktree and persist each transaction phase.
use crate::error::{Error, Result};
use crate::{
    manifest::Manifest,
    plan::plan,
    verify::{ref_log, verify},
};
use cli_common::{
    display,
    git::{self, Repo},
    state::{self, Store},
    Language,
};
use std::{collections::BTreeMap, ffi::OsStr, fs, io::Write, os::unix::ffi::OsStrExt, path::Path};

pub(super) fn create(store: &Store, dir: &Path, repo: &Repo) -> Result<()> {
    let l = Language::detect();
    if !repo.status()?.is_empty() {
        return Err(Error::DirtySource);
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
            return Err(Error::GitOperationInProgress);
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
                return Err(Error::CheckoutFilters);
            }
        }
    }
    let id = state::id();
    let time = state::now();
    let branch = format!("refs/heads/sprinkle/{time}-{}", &id[..12]);
    let parent = repo.root.parent().ok_or_else(|| Error::NoWorktreeParent)?;
    let target = parent.join(format!(
        "{}-sprinkled-{}",
        repo.root
            .file_name()
            .unwrap_or(OsStr::new("project"))
            .to_string_lossy(),
        &id[..12]
    ));
    if fs::symlink_metadata(&target).is_ok() {
        return Err(Error::TargetExists);
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
        let Some(branch) = m.branch.strip_prefix("refs/heads/") else {
            return Err(Error::InvalidRef);
        };
        let args = vec![
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--no-checkout"),
            OsStr::new("--"),
            target.as_os_str(),
            OsStr::new(branch),
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
            return Err(Error::HeadCheckoutDirty);
        }
        let count = plan(&copy, &target, &mut m, l)?;
        m.phase = "planned".into();
        store.write(&path, &m)?;
        state::atomic(&session.join("additions.patch"), m.patch.as_bytes(), 0o600)?;
        for f in m.files.iter().filter(|f| !f.addition.is_empty()) {
            let path = git::safe_path(&target, &f.path)?;
            if git::fingerprint(&path)? != f.before {
                return Err(Error::FileChangedBeforeInsertion);
            }
            let mut bytes = fs::read(&path)?;
            bytes.extend_from_slice(f.addition.as_bytes());
            state::atomic(&path, &bytes, f.mode & 0o777)?;
        }
        verify(&m, false)?;
        m.phase = "active".into();
        store.write(&path, &m)?;
        Ok(count)
    })();
    if let Ok(count) = result {
        // The session is complete; a failed report write must not claim otherwise.
        let mut out = std::io::stdout().lock();
        writeln!(out, "{}: {}", l.text("Created", "已创建"), display(&target))?;
        writeln!(
            out,
            "{}: {}; {}: {}",
            l.text("Files scanned", "扫描文件"),
            m.scanned,
            l.text("Comments added", "添加注释"),
            count
        )?;
        for (reason, count) in &m.skipped {
            writeln!(out, "{} ({reason}): {count}", l.text("Skipped", "已跳过"))?;
        }
        writeln!(out, "{}", l.text("Original checkout files and branch unchanged. This is a HEAD copy, not a full disk backup.", "原 checkout 的文件与分支未改变。这是 HEAD 副本，不是磁盘目录的完整备份。"))?;
        out.flush()?;
        return Ok(());
    }
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
    result.map(|_| ())
}
