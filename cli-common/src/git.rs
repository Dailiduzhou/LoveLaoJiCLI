//! Bounded local Git probes and conservative content snapshots; no shell or external diff.
use crate::state::{digest, path_from};
use crate::{Error, Message, Result};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn ensure_no_filters(cwd: &Path) -> Result<()> {
    let config = run(cwd, ["config", "--null", "--list"])?;
    if config.split(|b| *b == 0).any(|entry| {
        entry
            .split(|b| *b == b'\n')
            .next()
            .is_some_and(|key| key.starts_with(b"filter."))
    }) {
        return Err(Error::GitFiltersConfigured);
    }
    Ok(())
}
pub fn command(cwd: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(cwd)
        .args([
            "--no-pager",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "core.sparseCheckout=false",
        ])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PAGER", "cat")
        .stdin(Stdio::null());
    for k in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_EXTERNAL_DIFF",
    ] {
        c.env_remove(k);
    }
    c
}
fn collect(mut r: impl Read) -> Result<Vec<u8>> {
    let mut b = Vec::new();
    r.by_ref().take(32 * 1024 * 1024 + 1).read_to_end(&mut b)?;
    if b.len() > 32 * 1024 * 1024 {
        std::io::copy(&mut r, &mut std::io::sink())?;
        return Err(Error::GitOutputBudget);
    }
    Ok(b)
}
pub fn run(cwd: &Path, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<Vec<u8>> {
    output(command(cwd).args(args))
}
/// Collect a configured local Git probe with the same stream/time budgets as
/// `run`. Callers start with `command` to retain its offline safety settings.
pub fn output(command: &mut Command) -> Result<Vec<u8>> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let out = child.stdout.take().expect("stdout is piped");
    let err = child.stderr.take().expect("stderr is piped");
    let a = std::thread::spawn(move || collect(out));
    let b = std::thread::spawn(move || collect(err));
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::GitTimeout);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let out = a.join().map_err(|_| Error::GitReaderFailed)??;
    let err = b.join().map_err(|_| Error::GitReaderFailed)??;
    if !status.success() {
        let detail = crate::display(OsStr::from_bytes(&err));
        return Err(Error::GitFailed(Message::new(
            format!("Git failed: {detail}"),
            format!("Git 失败：{detail}"),
        )));
    }
    Ok(out)
}
fn line(mut b: Vec<u8>) -> Vec<u8> {
    if b.last() == Some(&b'\n') {
        b.pop();
    }
    b
}
#[derive(Clone, Debug)]
pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common: PathBuf,
    pub head: String,
    pub branch: Option<String>,
}
impl Repo {
    pub fn discover(cwd: &Path) -> Result<Self> {
        if run(cwd, ["rev-parse", "--is-bare-repository"])? != b"false\n" {
            return Err(Error::NotARepository);
        }
        let root = path_from(&line(run(cwd, ["rev-parse", "--show-toplevel"])?)).canonicalize()?;
        let git_dir =
            path_from(&line(run(cwd, ["rev-parse", "--absolute-git-dir"])?)).canonicalize()?;
        let common = path_from(&line(run(
            cwd,
            ["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))
        .canonicalize()?;
        let head = String::from_utf8(line(run(cwd, ["rev-parse", "--verify", "HEAD"])?))
            .map_err(|_| Error::HeadNotUtf8)?;
        let branch = run(cwd, ["symbolic-ref", "--quiet", "HEAD"])
            .ok()
            .map(|b| String::from_utf8_lossy(&line(b)).into_owned());
        Ok(Self {
            root,
            git_dir,
            common,
            head,
            branch,
        })
    }
    pub fn status(&self) -> Result<Vec<u8>> {
        // Reject submodules before status can recurse into another repository.
        let _ = self.tracked()?;
        ensure_no_filters(&self.root)?;
        run(
            &self.root,
            [
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=none",
            ],
        )
    }
    pub fn index(&self) -> Result<Vec<u8>> {
        run(&self.root, ["ls-files", "--stage", "-z"])
    }
    pub fn tracked(&self) -> Result<Vec<(Vec<u8>, String)>> {
        let mut files = Vec::new();
        for entry in self.index()?.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            let tab = entry
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| Error::InvalidIndex)?;
            let header = String::from_utf8_lossy(&entry[..tab]);
            let bits: Vec<_> = header.split(' ').collect();
            if bits.len() != 3 || bits[2] != "0" || bits[0] == "160000" {
                return Err(Error::IncomparableIndex);
            }
            files.push((entry[tab + 1..].to_vec(), bits[0].to_owned()));
        }
        Ok(files)
    }
    pub fn snapshot(&self, cwd: &Path) -> Result<String> {
        let start = Instant::now();
        let mut budget = 256 * 1024 * 1024u64;
        let a = self.scan(cwd, &start, &mut budget)?;
        let b = self.scan(cwd, &start, &mut budget)?;
        if a != b {
            return Err(Error::SnapshotChanged);
        }
        Ok(a)
    }
    fn scan(&self, cwd: &Path, start: &Instant, budget: &mut u64) -> Result<String> {
        let head = run(&self.root, ["rev-parse", "--verify", "HEAD"])?;
        let index = self.index()?;
        let status = self.status()?;
        let mut files = self
            .tracked()?
            .into_iter()
            .map(|(p, _)| p)
            .collect::<Vec<_>>();
        files.extend(
            run(
                &self.root,
                ["ls-files", "--others", "--exclude-standard", "-z"],
            )?
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| p.to_vec()),
        );
        files.sort();
        files.dedup();
        if files.len() > 20_000 {
            return Err(Error::SnapshotEntryBudget);
        }
        let mut h = blake3::Hasher::new();
        for b in [
            self.common.as_os_str().as_bytes(),
            self.root.as_os_str().as_bytes(),
            cwd.as_os_str().as_bytes(),
            &head,
            &index,
            &status,
        ] {
            add(&mut h, b);
        }
        for p in files {
            if start.elapsed() > Duration::from_secs(5) {
                return Err(Error::SnapshotTimeout);
            }
            let path = safe_path(&self.root, &p)?;
            add(&mut h, &p);
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    add(&mut h, b"deleted");
                }
                Err(e) => return Err(e.into()),
                Ok(m) => {
                    add(&mut h, &(m.mode() & 0o170777).to_le_bytes());
                    let bytes = if m.file_type().is_symlink() {
                        fs::read_link(&path)?.as_os_str().as_bytes().to_vec()
                    } else if m.is_file() {
                        if m.len() > *budget {
                            return Err(Error::SnapshotByteBudget);
                        }
                        let mut f = std::fs::OpenOptions::new()
                            .read(true)
                            .custom_flags(
                                rustix::fs::OFlags::NOFOLLOW.bits() as i32
                                    | rustix::fs::OFlags::NONBLOCK.bits() as i32,
                            )
                            .open(&path)?;
                        let fm = f.metadata()?;
                        if !fm.is_file() || fm.ino() != m.ino() || fm.dev() != m.dev() {
                            return Err(Error::SnapshotFileChanged);
                        }
                        let mut b = Vec::new();
                        f.by_ref().take(*budget + 1).read_to_end(&mut b)?;
                        b
                    } else {
                        return Err(Error::SpecialFileInSnapshot);
                    };
                    *budget = budget
                        .checked_sub(bytes.len() as u64)
                        .ok_or_else(|| Error::SnapshotByteBudget)?;
                    add(&mut h, &bytes);
                }
            }
        }
        if head != run(&self.root, ["rev-parse", "--verify", "HEAD"])?
            || index != self.index()?
            || status != self.status()?
            || start.elapsed() > Duration::from_secs(5)
        {
            return Err(Error::UnstableSnapshot);
        }
        Ok(h.finalize().to_hex().to_string())
    }
}
use std::os::unix::fs::OpenOptionsExt;
fn add(h: &mut blake3::Hasher, b: &[u8]) {
    h.update(&(b.len() as u64).to_le_bytes());
    h.update(b);
}
pub fn safe_path(root: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let rel = path_from(bytes);
    if rel
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || rel.as_os_str().is_empty()
    {
        return Err(Error::UnsafeRelativePath);
    }
    let path = root.join(rel);
    let mut parent = path.parent();
    while let Some(p) = parent {
        if p == root {
            break;
        }
        if fs::symlink_metadata(p)?.file_type().is_symlink() {
            return Err(Error::SymlinkedParent);
        }
        parent = p.parent();
    }
    Ok(path)
}
pub fn workspace(cwd: &Path) -> (PathBuf, Option<Repo>) {
    match Repo::discover(cwd) {
        Ok(r) => (r.root.clone(), Some(r)),
        Err(_) => (cwd.to_owned(), None),
    }
}
pub fn fingerprint(path: &Path) -> Result<String> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.file_type().is_symlink() || m.nlink() != 1 {
        return Err(Error::NotIndependentFile);
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file()
        || opened.nlink() != 1
        || opened.ino() != m.ino()
        || opened.dev() != m.dev()
    {
        return Err(Error::FileChangedWhileOpening);
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(Error::FileHashBudget);
    }
    Ok(digest(&bytes))
}
pub fn args(parts: &[&str]) -> Vec<OsString> {
    parts.iter().map(OsString::from).collect()
}
