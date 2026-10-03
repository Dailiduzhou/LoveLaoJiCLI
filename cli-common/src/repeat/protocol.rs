//! Execution lease, overlap invalidation, and repeat-baseline transitions.
use super::{
    current,
    records::{prune, Run},
    RETENTION,
};
use crate::{
    display, error,
    git::{self, Repo},
    process::{self, Outcome},
    state::{self, Store},
    Language, Result,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::File,
    io::IsTerminal,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone)]
struct Baseline {
    run_id: String,
    invocation: String,
    snapshot: String,
    completed_at: u64,
    signature: String,
    count: u32,
}
#[derive(Serialize, Deserialize, Default)]
struct Repeat {
    schema_version: u32,
    record_id: String,
    created_at: u64,
    workspace_id: String,
    tool: String,
    generation: String,
    baseline: Option<Baseline>,
}
pub(super) struct Context {
    store: Store,
    dir: PathBuf,
    repo: Option<Repo>,
    cwd: PathBuf,
    invocation: String,
    before: Option<String>,
    previous: Option<Baseline>,
    generation: String,
    lease: Option<File>,
    run: Run,
}
pub(super) fn warning(e: impl std::fmt::Display) {
    eprintln!(
        "{}: {}",
        Language::detect().text(
            "State/comparison unavailable; running the real command",
            "状态或比较不可用；执行真实命令"
        ),
        display(e.to_string())
    );
}
fn snapshot(repo: Option<&Repo>, cwd: &Path) -> Option<String> {
    match repo {
        Some(r) => match r.snapshot(cwd) {
            Ok(s) => Some(s),
            Err(e) => {
                warning(e);
                None
            }
        },
        None => None,
    }
}
pub(super) fn prepare(
    tool: &str,
    argv: &[OsString],
    hypothesis: Option<String>,
) -> Result<Context> {
    let cwd = std::env::current_dir()?.canonicalize()?;
    let (workspace, repo) = git::workspace(&cwd);
    let store = Store::open()?;
    let dir = store.workspace(&workspace)?;
    let invocation = state::fields(
        &store.key,
        std::iter::once(cwd.as_os_str().as_bytes()).chain(argv.iter().map(|a| a.as_bytes())),
    );
    let before = snapshot(repo.as_ref(), &cwd);
    let generation = state::id();
    let run = Run {
        schema_version: 1,
        record_id: generation.clone(),
        created_at: state::now(),
        workspace_id: dir.file_name().unwrap().to_string_lossy().into_owned(),
        tool: tool.into(),
        invocation_digest: invocation.clone(),
        program: Path::new(&argv[0])
            .file_name()
            .map(display)
            .unwrap_or_else(|| "?".into()),
        before: before.clone(),
        after: None,
        started_at: None,
        finished_at: None,
        elapsed_ms: None,
        result: None,
        hypothesis,
    };
    Ok(Context {
        store,
        dir,
        repo,
        cwd,
        invocation,
        before,
        previous: None,
        generation,
        lease: None,
        run,
    })
}
impl Context {
    pub(super) fn key(&self) -> [u8; 32] {
        self.store.key
    }
    fn state_path(&self) -> PathBuf {
        self.dir.join(format!("{}.json", self.run.tool))
    }
    fn run_path(&self) -> PathBuf {
        self.dir
            .join("runs")
            .join(format!("{}.json", self.run.record_id))
    }
    fn repeat(&self, baseline: Option<Baseline>) -> Repeat {
        Repeat {
            schema_version: 1,
            record_id: self.generation.clone(),
            created_at: state::now(),
            workspace_id: self.run.workspace_id.clone(),
            tool: self.run.tool.clone(),
            generation: self.generation.clone(),
            baseline,
        }
    }
    fn read_repeat(&self) -> Result<Repeat> {
        let Some(r) = self.store.read::<Repeat>(&self.state_path())? else {
            return Ok(Repeat::default());
        };
        if r.schema_version != 1
            || r.workspace_id != self.run.workspace_id
            || r.tool != self.run.tool
            || r.record_id != r.generation
        {
            return Err(error("Invalid repeat record", "重复状态记录无效"));
        }
        Ok(r)
    }
    /// Returns an early code only for an authenticated skip/interception.
    pub(super) fn begin(
        &mut self,
        again: bool,
        mut hypothesis: Option<String>,
    ) -> Result<Option<i32>> {
        let l = Language::detect();
        let write_lock = state::lock(&self.dir.join("workspace.lock"))?;
        state::private_dir(&self.dir.join("runs"))?;
        prune(&self.store, &self.dir)?;
        self.lease = state::lock(&self.dir.join(format!("{}.execution.lock", self.run.tool))).ok();
        let mut repeat = self.read_repeat()?;
        // A prior run left generation but no baseline if it was interrupted. No hit possible.
        let comparable = std::io::stdin().is_terminal()
            && process::interactive()
            && self.lease.is_some()
            && self.before.is_some();
        let prior = repeat
            .baseline
            .clone()
            .filter(|b| current(b.completed_at, RETENTION));
        let matches = comparable
            && prior.as_ref().is_some_and(|b| {
                b.invocation == self.invocation && Some(&b.snapshot) == self.before.as_ref()
            });
        if self.run.tool == "enough"
            && matches
            && !again
            && prior.as_ref().is_some_and(|b| current(b.completed_at, 300))
        {
            let b = repeat.baseline.as_mut().unwrap();
            b.count = b.count.saturating_add(1);
            let count = b.count;
            self.store.write(&self.state_path(), &repeat)?;
            let en = ["It already passed.", "Nothing changed.", "enough.", "no."];
            let zh = ["已经通过了。", "没有观察到变化。", "够了。", "不必了。"];
            eprintln!(
                "{}",
                l.text(
                    en[(count.min(4) - 1) as usize],
                    zh[(count.min(4) - 1) as usize]
                )
            );
            return Ok(Some(0));
        }
        if self.run.tool == "stuck"
            && matches
            && prior.as_ref().is_some_and(|b| b.count >= 3)
            && hypothesis.is_none()
        {
            eprintln!(
                "{}",
                l.text(
                    "Nothing changed. Repeating the same experiment may produce the same result.",
                    "没有观察到变化。重复同一实验可能产生相同结果。"
                )
            );
            // Do not hold the write lock while asking. An execution lease prevents another
            // instance from regarding this interval as an unambiguous sequential sample.
            let observed = repeat.generation.clone();
            drop(write_lock);
            hypothesis = process::ask(
                l.text("What are you changing?", "这次你打算改变什么？"),
                512,
            )?;
            let _lock = state::lock(&self.dir.join("workspace.lock"))?;
            let latest = self.read_repeat()?;
            if latest.generation != observed {
                return Err(error(
                    "Concurrent invocation changed the repeat state",
                    "并发调用改变了重复状态",
                ));
            }
            if hypothesis.is_none() {
                eprintln!(
                    "{}",
                    l.text(
                        "Not run. Supply --hypothesis <text> (no secrets).",
                        "未执行。请提供 --hypothesis <text>（不要填写秘密）。"
                    )
                );
                return Ok(Some(125));
            }
            self.previous = None;
            self.run.hypothesis = hypothesis;
            self.start_record()?;
            return Ok(None);
        }
        self.previous = if matches && hypothesis.is_none() {
            prior
        } else {
            None
        };
        self.run.hypothesis = hypothesis;
        self.start_record()?;
        Ok(None)
    }
    fn start_record(&mut self) -> Result<()> {
        self.run.started_at = Some(state::now());
        // Invalidate BEFORE spawning, so failures/interruptions cannot retain success eligibility.
        self.store.write(&self.state_path(), &self.repeat(None))?;
        self.store.write(&self.run_path(), &self.run)
    }
    pub(super) fn finish(&mut self, outcome: Outcome, elapsed: u128) -> Result<()> {
        let completed = state::now();
        let after = snapshot(self.repo.as_ref(), &self.cwd);
        self.run.after = after.clone();
        self.run.finished_at = Some(completed);
        self.run.elapsed_ms = Some(elapsed);
        self.run.result = Some(outcome.clone());
        let _lock = state::lock(&self.dir.join("workspace.lock"))?;
        self.store.write(&self.run_path(), &self.run)?;
        let repeat = self.read_repeat()?;
        if repeat.generation != self.generation || self.lease.is_none() {
            return Ok(());
        }
        let stable = self.before.is_some()
            && self.before == after
            && std::io::stdin().is_terminal()
            && process::interactive()
            && outcome.category == "exited";
        let baseline = if stable && self.run.tool == "enough" && outcome.exit_code == 0 {
            Some(Baseline {
                run_id: self.generation.clone(),
                invocation: self.invocation.clone(),
                snapshot: after.unwrap(),
                completed_at: completed,
                signature: String::new(),
                count: 0,
            })
        } else if stable && self.run.tool == "stuck" && outcome.exit_code != 0 {
            match (&outcome.stdout, &outcome.stderr) {
                (Some(out), Some(err)) if out.complete && err.complete => {
                    let signature = state::fields(
                        &self.store.key,
                        [
                            self.invocation.as_bytes(),
                            after.as_ref().unwrap().as_bytes(),
                            &outcome.exit_code.to_le_bytes(),
                            out.digest.as_bytes(),
                            &out.bytes.to_le_bytes(),
                            err.digest.as_bytes(),
                            &err.bytes.to_le_bytes(),
                        ],
                    );
                    let count = self
                        .previous
                        .as_ref()
                        .filter(|b| b.signature == signature)
                        .map_or(1, |b| b.count.saturating_add(1));
                    Some(Baseline {
                        run_id: self.generation.clone(),
                        invocation: self.invocation.clone(),
                        snapshot: after.unwrap(),
                        completed_at: completed,
                        signature,
                        count,
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        self.store.write(&self.state_path(), &self.repeat(baseline))
    }
}
