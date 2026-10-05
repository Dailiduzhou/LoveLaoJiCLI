//! Three identical failures require one explicit experiment hypothesis.
use cli_common::{
    process::Outcome,
    repeat::{Baseline, BaselineUpdate, Decision, Policy},
    state, Language,
};

pub struct Stuck;

impl Policy for Stuck {
    fn before(&self, previous: Option<&Baseline>, hypothesis: Option<&str>) -> Decision {
        if previous.is_some_and(|b| b.count >= 3) && hypothesis.is_none() {
            let l = Language::detect();
            Decision::Prompt {
                notice: l.text(
                    "Nothing changed. Repeating the same experiment may produce the same result.",
                    "没有观察到变化。重复同一实验可能产生相同结果。",
                ),
                question: l.text("What are you changing?", "这次你打算改变什么？"),
                max: 512,
                refusal: l.text(
                    "Not run. Supply --hypothesis <text> (no secrets).",
                    "未执行。请提供 --hypothesis <text>（不要填写秘密）。",
                ),
                code: 125,
            }
        } else {
            Decision::Run
        }
    }

    fn baseline(
        &self,
        outcome: &Outcome,
        key: &[u8; 32],
        invocation: &str,
        snapshot: &str,
        previous: Option<&Baseline>,
    ) -> Option<BaselineUpdate> {
        if outcome.exit_code == 0 {
            return None;
        }
        match (&outcome.stdout, &outcome.stderr) {
            (Some(out), Some(err)) if out.complete && err.complete => {
                // Field order/encoding is part of the existing on-disk baseline protocol.
                let signature = state::fields(
                    key,
                    [
                        invocation.as_bytes(),
                        snapshot.as_bytes(),
                        &outcome.exit_code.to_le_bytes(),
                        out.digest.as_bytes(),
                        &out.bytes.to_le_bytes(),
                        err.digest.as_bytes(),
                        &err.bytes.to_le_bytes(),
                    ],
                );
                let count = previous
                    .filter(|b| b.signature == signature)
                    .map_or(1, |b| b.count.saturating_add(1));
                Some(BaselineUpdate { signature, count })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli_common::process::OutputDigest;

    #[test]
    fn legacy_signature_keeps_failure_chain_and_hypothesis_bypasses_gate() {
        // Fixed vector from the pre-extraction keyed length-prefixed field order.
        let signature = "cd0d56e3fe562241f77ed8780cd6cdcac30d655ad336d183ae33a30f06bebc78";
        let json = serde_json::json!({
            "run_id": "0123456789abcdef0123456789abcdef", "invocation": "invocation",
            "snapshot": "snapshot", "completed_at": 123, "signature": signature, "count": 2
        });
        let mut previous: Baseline = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&previous).unwrap(), json);
        assert!(matches!(Stuck.before(Some(&previous), None), Decision::Run));
        let stream = OutputDigest {
            digest: "digest".into(),
            bytes: 12,
            complete: true,
        };
        let outcome = Outcome {
            category: "exited".into(),
            exit_code: 7,
            signal: None,
            stdout: Some(stream.clone()),
            stderr: Some(stream),
        };
        let update = Stuck
            .baseline(
                &outcome,
                &[1; 32],
                "invocation",
                "snapshot",
                Some(&previous),
            )
            .unwrap();
        assert_eq!(update.signature, signature);
        assert_eq!(update.count, 3);
        previous.count = update.count;
        assert!(matches!(
            Stuck.before(Some(&previous), None),
            Decision::Prompt {
                max: 512,
                code: 125,
                ..
            }
        ));
        assert!(matches!(
            Stuck.before(Some(&previous), Some("change timeout")),
            Decision::Run
        ));
        previous.count = u32::MAX;
        assert_eq!(
            Stuck
                .baseline(
                    &outcome,
                    &[1; 32],
                    "invocation",
                    "snapshot",
                    Some(&previous)
                )
                .unwrap()
                .count,
            u32::MAX
        );
        assert_eq!(
            Stuck
                .baseline(&outcome, &[1; 32], "invocation", "changed", Some(&previous))
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn only_failures_with_two_complete_streams_form_a_baseline() {
        let stream = OutputDigest {
            digest: "digest".into(),
            bytes: 12,
            complete: true,
        };
        let mut outcome = Outcome {
            category: "exited".into(),
            exit_code: 7,
            signal: None,
            stdout: Some(stream.clone()),
            stderr: Some(stream),
        };
        let baseline = Stuck
            .baseline(&outcome, &[1; 32], "invocation", "snapshot", None)
            .unwrap();
        assert_eq!(baseline.count, 1);
        outcome.exit_code = 0;
        assert!(Stuck
            .baseline(&outcome, &[1; 32], "invocation", "snapshot", None)
            .is_none());
        outcome.exit_code = 7;
        outcome.stderr.as_mut().unwrap().complete = false;
        assert!(Stuck
            .baseline(&outcome, &[1; 32], "invocation", "snapshot", None)
            .is_none());
        outcome.stderr = None;
        assert!(Stuck
            .baseline(&outcome, &[1; 32], "invocation", "snapshot", None)
            .is_none());
    }
}
