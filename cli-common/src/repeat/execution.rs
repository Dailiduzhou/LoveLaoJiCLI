//! Fail-open execution around the shared lease/record protocol. No CLI parsing.
use super::{
    protocol::{prepare, warning},
    Policy,
};
use crate::{display, process, Language};
use std::{ffi::OsString, time::Instant};

/// Execute already-parsed argv (which must contain a program). Tool frontends
/// own usage errors, capture selection, and the supplied business policy.
pub fn execute(
    tool: &str,
    argv: &[OsString],
    capture: bool,
    hypothesis: Option<String>,
    policy: &impl Policy,
) -> i32 {
    let l = Language::detect();
    let mut context = match prepare(tool, argv, hypothesis.clone()) {
        Ok(mut c) => match c.begin(policy, hypothesis) {
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
        .map(|c| c.key())
        .unwrap_or_else(rand::random);
    let start = Instant::now();
    let outcome = process::execute(argv, capture, key);
    let code = outcome.exit_code;
    if let Some(c) = context.as_mut() {
        if let Err(e) = c.finish(policy, outcome, start.elapsed().as_millis()) {
            eprintln!(
                "{}: {}",
                l.text("Could not save execution record", "无法保存执行记录"),
                display(e.to_string())
            );
        }
    }
    code
}
