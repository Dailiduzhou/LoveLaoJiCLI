//! Persisted execution schema, pruning, and read-only history views.
use super::{current, RETENTION};
use crate::{
    error,
    process::Outcome,
    state::{self, Store},
    Result,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Instant};

#[derive(Serialize, Deserialize, Clone)]
pub struct FailureSummary {
    pub program: String,
    pub created_at: u64,
    pub exit_code: i32,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Run {
    pub(super) schema_version: u32,
    pub(super) record_id: String,
    pub(super) created_at: u64,
    pub(super) workspace_id: String,
    pub(super) tool: String,
    pub(super) invocation_digest: String,
    pub(super) program: String,
    pub(super) before: Option<String>,
    pub(super) after: Option<String>,
    pub(super) started_at: Option<u64>,
    pub(super) finished_at: Option<u64>,
    pub(super) elapsed_ms: Option<u128>,
    pub(super) result: Option<Outcome>,
    pub(super) hypothesis: Option<String>,
}
fn valid_run(run: &Run, dir: &Path) -> bool {
    run.schema_version == 1
        && ["enough", "stuck"].contains(&run.tool.as_str())
        && dir
            .file_name()
            .is_some_and(|n| n == run.workspace_id.as_str())
}
pub(super) fn prune(store: &Store, dir: &Path) -> Result<()> {
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
