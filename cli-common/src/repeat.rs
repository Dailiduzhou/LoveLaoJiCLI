//! Shared execution protocol. Execution leases are distinct from workspace
//! write locks. Report readers never mutate state; latest-failure lookup may prune.
mod execution;
mod protocol;
mod records;

pub use execution::execute;
pub use protocol::Baseline;

/// Tool-owned decisions are evaluated only after shared comparability checks.
/// Skip/Prompt require a matched baseline; prompts run outside the write lock.
pub enum Decision {
    Run,
    Skip {
        count: u32,
        code: i32,
        message: &'static str,
    },
    Prompt {
        notice: &'static str,
        question: &'static str,
        max: usize,
        refusal: &'static str,
        code: i32,
    },
}

pub struct BaselineUpdate {
    pub signature: String,
    pub count: u32,
}

/// Product policy stays in the tool; locks, snapshots and records stay here.
pub trait Policy {
    fn before(&self, previous: Option<&Baseline>, hypothesis: Option<&str>) -> Decision;
    /// Called only for stable, foreground, normally exited executions.
    fn baseline(
        &self,
        outcome: &crate::process::Outcome,
        key: &[u8; 32],
        invocation: &str,
        snapshot: &str,
        previous: Option<&Baseline>,
    ) -> Option<BaselineUpdate>;
}
pub use records::{latest_failure, recorded_runs, FailureSummary, RecordedRun};

const RETENTION: u64 = 30 * 24 * 60 * 60;

fn current(completed: u64, window: u64) -> bool {
    crate::state::now()
        .checked_sub(completed)
        .is_some_and(|age| age <= window)
}

#[cfg(test)]
mod tests {
    #[test]
    fn clock_rollback_invalidates() {
        assert!(!super::current(crate::state::now() + 100, 300));
    }
}
