//! Shared enough/stuck protocol. Execution leases are distinct from workspace
//! write locks; history readers preserve the existing read-only API.
mod cli;
mod protocol;
mod records;

pub use cli::run;
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
