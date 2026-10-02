#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
mod scan;
use cli_common::{
    command, display, error,
    git::{self, Repo},
    state::{self, Store},
    Language, Result,
};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
struct FileRecord {
    path: Vec<u8>,
    before: String,
    after: String,
    mode: u32,
    offset: usize,
    addition: String,
    message: usize,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    record_id: String,
    created_at: u64,
    created_order: u128,
    tool: String,
    repository: Vec<u8>,
    source: Vec<u8>,
    target: Vec<u8>,
    head: String,
    branch: String,
    phase: String,
    git_dir: Vec<u8>,
    git_file: String,
    reflog: String,
    index: String,
    files: Vec<FileRecord>,
    patch: String,
    patch_hash: String,
    scanned: usize,
    skipped: BTreeMap<String, usize>,
}
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
fn select(store: &Store, dir: &Path, repo: &Repo) -> Result<(PathBuf, Manifest)> {
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
fn validate_identity(m: &Manifest, r: &Repo) -> Result<()> {
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
fn create(store: &Store, dir: &Path, repo: &Repo) -> Result<()> {
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
        let seed = std::env::var("SPRINKLE_SEED")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(rand::random);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut files = copy.tracked()?;
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut count = 0;
        for (p, mode) in files {
            m.scanned += 1;
            let path = git::safe_path(&target, &p)?;
            let metadata = fs::symlink_metadata(&path)?;
            let before = if metadata.file_type().is_symlink() {
                state::digest(fs::read_link(&path)?.as_os_str().as_bytes())
            } else {
                git::fingerprint(&path)?
            };
            let mut record = FileRecord {
                path: p.clone(),
                before: before.clone(),
                after: before,
                mode: metadata.mode() & 0o177777,
                offset: 0,
                addition: String::new(),
                message: 0,
            };
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let reason = if mode == "120000" {
                Some("symlink")
            } else if p.split(|c| *c == b'/').any(|c| {
                [
                    b".git".as_slice(),
                    b"target",
                    b"vendor",
                    b"node_modules",
                    b"dist",
                    b"build",
                    b"third_party",
                    b"generated",
                ]
                .contains(&c)
            }) {
                Some("excluded-directory")
            } else if ![
                "md", "go", "rs", "c", "h", "cc", "cpp", "cxx", "hh", "hpp", "hxx",
            ]
            .contains(&ext)
            {
                Some("extension")
            } else if metadata.len() > 1024 * 1024 {
                Some("size")
            } else {
                None
            };
            if let Some(reason) = reason {
                *m.skipped.entry(reason.into()).or_default() += 1;
                m.files.push(record);
                continue;
            }
            let bytes = fs::read(&path)?;
            let text = std::str::from_utf8(&bytes).ok();
            if !text.is_some_and(|t| scan::safe(t, ext)) {
                *m.skipped.entry("syntax-or-text".into()).or_default() += 1;
                m.files.push(record);
                continue;
            }
            if rng.gen_bool(0.25) && count < 100 {
                let messages=l.text("you're doing fine|one thing at a time|this can wait|good enough is good|take your time|future-you says hi|one weird bug at a time|remember water|breathe.|tomorrow is also available", "慢一点也可以|先解决一个|今天可以到这里|能用已经很好|不着急|明天的你说你好|一次处理一个奇怪的 bug|喝口水|深呼吸|明天也可以处理");
                let messages: Vec<_> = messages.split('|').collect();
                let choice = rng.gen_range(0..messages.len());
                let newline = if text.unwrap().contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                let comment = if ext == "md" {
                    format!("<!-- {} -->", messages[choice])
                } else if ["rs", "go"].contains(&ext) {
                    format!("// {}", messages[choice])
                } else {
                    format!("/* {} */", messages[choice])
                };
                record.addition = format!("{newline}{comment}{newline}");
                record.offset = bytes.len();
                record.message = choice;
                let mut after = bytes;
                after.extend_from_slice(record.addition.as_bytes());
                record.after = state::digest(&after);
                count += 1;
            }
            m.files.push(record);
        }
        // Save the exact zero-context patch before touching files. Git-style quoting is byte-safe.
        for f in m.files.iter().filter(|f| !f.addition.is_empty()) {
            let before = fs::read(git::safe_path(&target, &f.path)?)?;
            let lines = before.iter().filter(|b| **b == b'\n').count();
            let a = quote_path(b"a/", &f.path);
            let b = quote_path(b"b/", &f.path);
            m.patch.push_str(&format!(
                "diff --git {a} {b}\n--- {a}\n+++ {b}\n@@ -{lines},0 +{},2 @@\n",
                lines + 1
            ));
            for line in f.addition.split_inclusive('\n') {
                m.patch.push('+');
                m.patch.push_str(line);
            }
        }
        m.patch_hash = state::digest(m.patch.as_bytes());
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
fn quote_path(prefix: &[u8], p: &[u8]) -> String {
    let mut s = String::from("\"");
    for b in prefix.iter().chain(p) {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            32..=126 => s.push(*b as char),
            _ => s.push_str(&format!("\\{b:03o}")),
        }
    }
    s.push('"');
    s
}
fn ref_log(m: &Manifest) -> Result<String> {
    git::fingerprint(&state::path_from(&m.repository).join("logs").join(&m.branch))
}
fn verify(m: &Manifest, restoring: bool) -> Result<Repo> {
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
fn undo(store: &Store, path: &Path, m: &mut Manifest, cwd: &Path) -> Result<()> {
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
