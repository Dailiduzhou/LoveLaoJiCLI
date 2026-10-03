//! Shared records and execution protocol for enough/stuck. The execution lease is
//! distinct from the short-held workspace write lock. Overlaps invalidate both baselines.
use crate::{
    display, error,
    git::{self, Repo},
    process::{self, Outcome},
    state::{self, Store},
    Language, Result,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::IsTerminal;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Instant;
const RETENTION: u64 = 30 * 24 * 60 * 60;
#[derive(Serialize, Deserialize, Clone)]
pub struct FailureSummary {
    pub program: String,
    pub created_at: u64,
    pub exit_code: i32,
}
#[derive(Serialize, Deserialize)]
struct Run {
    schema_version: u32,
    record_id: String,
    created_at: u64,
    workspace_id: String,
    tool: String,
    invocation_digest: String,
    program: String,
    before: Option<String>,
    after: Option<String>,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    elapsed_ms: Option<u128>,
    result: Option<Outcome>,
    hypothesis: Option<String>,
}
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
struct Context {
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
fn warning(e: impl std::fmt::Display) {
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
fn current(completed: u64, window: u64) -> bool {
    state::now()
        .checked_sub(completed)
        .is_some_and(|age| age <= window)
}
fn prepare(tool: &str, argv: &[OsString], hypothesis: Option<String>) -> Result<Context> {
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
    fn begin(&mut self, again: bool, mut hypothesis: Option<String>) -> Result<Option<i32>> {
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
    fn finish(&mut self, outcome: Outcome, elapsed: u128) -> Result<()> {
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
pub fn run(tool: &'static str) -> i32 {
    let l = Language::detect();
    let stuck = tool == "stuck";
    let mut cli = crate::command(
        tool,
        if stuck {
            l.text(
                "Pause after three identical failures. Hypotheses are local plaintext: no secrets.",
                "连续三次相同失败后暂停。假设保存在本地明文中：不要填写秘密。",
            )
        } else {
            l.text("Remember a recent success; not a build cache or proof that rerunning has no value.", "提醒最近一次成功；不是构建缓存，也不证明再次运行没有新信息。")
        },
    );
    if stuck {
        cli = cli.arg(
            clap::Arg::new("hypothesis")
                .long("hypothesis")
                .value_name("text")
                .value_parser(|s: &str| {
                    crate::text_input(s, 512).ok_or_else(|| {
                        "Expected a nonempty single line, at most 512 characters".to_string()
                    })
                })
                .help(l.text(
                    "One experiment hypothesis (no secrets)",
                    "仅本次实验的假设（不要填写秘密）",
                )),
        );
    } else {
        cli = cli.arg(
            clap::Arg::new("again")
                .long("again")
                .action(clap::ArgAction::SetTrue)
                .help(l.text("Always execute once", "始终真实执行一次")),
        );
    }
    let m = cli
        .arg(
            clap::Arg::new("command")
                .help_heading(l.text("Arguments", "参数"))
                .num_args(1..)
                .trailing_var_arg(true)
                .value_parser(clap::builder::OsStringValueParser::new())
                .help(l.text(
                    "Program and its unchanged arguments",
                    "程序及其原样传递的参数",
                )),
        )
        .get_matches();
    let argv: Vec<OsString> = m
        .get_many::<OsString>("command")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    if argv.is_empty() {
        eprintln!(
            "{}: {tool} [--] <command> [args...]",
            l.text("Usage", "用法")
        );
        return 2;
    }
    let hypothesis = if stuck {
        m.get_one::<String>("hypothesis").cloned()
    } else {
        None
    };
    let again = !stuck && m.get_flag("again");
    let mut context = match prepare(tool, &argv, hypothesis.clone()) {
        Ok(mut c) => match c.begin(again, hypothesis) {
            Ok(Some(code)) => return code,
            Ok(None) => Some(c),
            Err(e) => {
                warning(e);
                None
            }
        },
        Err(e) => {
            warning(e);
            None
        }
    };
    let key = context
        .as_ref()
        .map(|c| c.store.key)
        .unwrap_or_else(rand::random);
    let start = Instant::now();
    let outcome = process::execute(&argv, stuck, key);
    let code = outcome.exit_code;
    if let Some(c) = context.as_mut() {
        if let Err(e) = c.finish(outcome, start.elapsed().as_millis()) {
            eprintln!(
                "{}: {}",
                l.text("Could not save execution record", "无法保存执行记录"),
                display(e.to_string())
            );
        }
    }
    code
}
fn valid_run(run: &Run, dir: &Path) -> bool {
    run.schema_version == 1
        && ["enough", "stuck"].contains(&run.tool.as_str())
        && dir
            .file_name()
            .is_some_and(|n| n == run.workspace_id.as_str())
}
fn prune(store: &Store, dir: &Path) -> Result<()> {
    let runs = dir.join("runs");
    if !runs.exists() {
        return Ok(());
    }
    for e in fs::read_dir(runs)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        if let Ok(Some(run)) = store.read::<Run>(&e.path()) {
            if valid_run(&run, dir)
                && state::now()
                    .checked_sub(run.finished_at.unwrap_or(run.created_at))
                    .is_some_and(|age| age > RETENTION)
                && e.file_name() == format!("{}.json", run.record_id).as_str()
            {
                // Interrupted runs/hypotheses can expire, but never while an execution
                // lease for that tool is live. Unknown/corrupt records are not removed.
                let lease = state::lock(&dir.join(format!("{}.execution.lock", run.tool)));
                if run.finished_at.is_some() || lease.is_ok() {
                    fs::remove_file(e.path())?;
                }
            }
        }
    }
    Ok(())
}
pub fn latest_failure(store: &Store, dir: &Path) -> Result<Option<FailureSummary>> {
    prune(store, dir)?;
    let runs = dir.join("runs");
    if !runs.exists() {
        return Ok(None);
    }
    let mut last: Option<FailureSummary> = None;
    for e in fs::read_dir(runs)? {
        let e = e?;
        if !e.file_type()?.is_file() {
            continue;
        }
        // An unrelated damaged event does not invalidate an otherwise complete card.
        if let Ok(Some(run)) = store.read::<Run>(&e.path()) {
            if !valid_run(&run, dir) {
                continue;
            }
            if let (Some(time), Some(outcome)) = (run.finished_at, run.result) {
                if current(time, RETENTION)
                    && ["exited", "signal"].contains(&outcome.category.as_str())
                    && outcome.exit_code != 0
                    && last.as_ref().is_none_or(|old| old.created_at <= time)
                {
                    last = Some(FailureSummary {
                        program: run.program,
                        created_at: time,
                        exit_code: outcome.exit_code,
                    });
                }
            }
        }
    }
    Ok(last)
}
/// A validated, existing explicit wrapper record. Reading never prunes or writes.
pub struct RecordedRun {
    pub tool: String,
    pub program: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub result: Option<Outcome>,
}
impl RecordedRun {
    pub fn completed_execution(&self) -> bool {
        self.finished_at.is_some()
            && self.result.as_ref().is_some_and(|r| {
                matches!(r.category.as_str(), "exited" | "signal")
                    && r.stdout.as_ref().is_none_or(|s| s.complete)
                    && r.stderr.as_ref().is_none_or(|s| s.complete)
                    && (self.tool != "stuck" || (r.stdout.is_some() && r.stderr.is_some()))
            })
    }
}
/// Read only this exact workspace. An unavailable/corrupt source is NOT an empty
/// history. Shared locks use existing lock files and conflict with active writes.
pub fn recorded_runs(workspace: &Path) -> Result<Vec<RecordedRun>> {
    let Some(store) = Store::open_readonly()? else {
        return Ok(vec![]);
    };
    let dir = store.workspace_path(workspace);
    if !state::existing_private_dir(&dir)? {
        return Ok(vec![]);
    }
    let _lock = state::lock_readonly(&dir.join("workspace.lock"))?;
    let runs = dir.join("runs");
    if !state::existing_private_dir(&runs)? {
        return Ok(vec![]);
    }
    let mut result = Vec::new();
    let mut bytes = 0u64;
    let start = Instant::now();
    for (n, entry) in fs::read_dir(runs)?.enumerate() {
        if n >= 10_000 || start.elapsed().as_secs() >= 5 {
            return Err(error(
                "Execution record scan exceeded budget",
                "执行记录扫描超出预算",
            ));
        }
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(error("Invalid execution record type", "执行记录类型无效"));
        }
        bytes = bytes.saturating_add(entry.metadata()?.len());
        if bytes > 32 * 1024 * 1024 {
            return Err(error(
                "Execution records exceed 32 MiB",
                "执行记录超过 32 MiB",
            ));
        }
        let run: Run = store
            .read(&entry.path())?
            .ok_or_else(|| error("Execution record disappeared", "执行记录消失"))?;
        if !valid_run(&run, &dir)
            || run.record_id.len() != 32
            || !run.record_id.bytes().all(|b| b.is_ascii_hexdigit())
            || entry.file_name() != format!("{}.json", run.record_id).as_str()
            || run.program.is_empty()
            || run.program.len() > 16384
            || run.started_at.is_none()
            || run.result.is_some() != run.finished_at.is_some()
            || run
                .finished_at
                .zip(run.started_at)
                .is_some_and(|(f, s)| f < s)
        {
            return Err(error("Invalid execution record", "执行记录无效"));
        }
        if let Some(o) = &run.result {
            let valid = match o.category.as_str() {
                "exited" => o.signal.is_none() && (0..=255).contains(&o.exit_code),
                "signal" => o
                    .signal
                    .is_some_and(|s| (1..=127).contains(&s) && o.exit_code == 128 + s),
                "spawn-failed" => o.signal.is_none() && [126, 127].contains(&o.exit_code),
                "wait-failed" => o.signal.is_none() && o.exit_code != 0,
                _ => false,
            };
            if !valid {
                return Err(error("Invalid execution outcome", "执行结果无效"));
            }
        }
        result.push(RecordedRun {
            tool: run.tool,
            program: run.program,
            started_at: run.started_at.unwrap(),
            finished_at: run.finished_at,
            result: run.result,
        });
    }
    result.sort_by(|a, b| {
        (a.started_at, a.finished_at, &a.tool, &a.program).cmp(&(
            b.started_at,
            b.finished_at,
            &b.tool,
            &b.program,
        ))
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clock_rollback_invalidates() {
        assert!(!super::current(crate::state::now() + 100, 300));
    }
}
