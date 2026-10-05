//! Five-minute stable-success reminder policy.
use cli_common::{
    process::Outcome,
    repeat::{Baseline, BaselineUpdate, Decision, Policy},
    state, Language,
};

pub struct Enough {
    pub again: bool,
}

fn recent(completed: u64, now: u64) -> bool {
    now.checked_sub(completed).is_some_and(|age| age <= 300)
}

fn reminder(count: u32, language: Language) -> &'static str {
    let en = ["It already passed.", "Nothing changed.", "enough.", "no."];
    let zh = ["已经通过了。", "没有观察到变化。", "够了。", "不必了。"];
    let index = (count.clamp(1, 4) - 1) as usize;
    language.text(en[index], zh[index])
}

impl Policy for Enough {
    fn before(&self, previous: Option<&Baseline>, _hypothesis: Option<&str>) -> Decision {
        if let Some(previous) =
            previous.filter(|b| !self.again && recent(b.completed_at, state::now()))
        {
            let count = previous.count.saturating_add(1);
            Decision::Skip {
                count,
                code: 0,
                message: reminder(count, Language::detect()),
            }
        } else {
            Decision::Run
        }
    }

    fn baseline(
        &self,
        outcome: &Outcome,
        _key: &[u8; 32],
        _invocation: &str,
        _snapshot: &str,
        _previous: Option<&Baseline>,
    ) -> Option<BaselineUpdate> {
        (outcome.exit_code == 0).then(|| BaselineUpdate {
            signature: String::new(),
            count: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_baseline_keeps_reminder_count_and_again_bypass() {
        let json = serde_json::json!({
            "run_id": "0123456789abcdef0123456789abcdef", "invocation": "invocation",
            "snapshot": "snapshot", "completed_at": state::now(), "signature": "", "count": 2
        });
        let mut previous: Baseline = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&previous).unwrap(), json);
        assert!(matches!(
            Enough { again: false }.before(Some(&previous), None),
            Decision::Skip {
                count: 3,
                code: 0,
                ..
            }
        ));
        assert!(matches!(
            Enough { again: true }.before(Some(&previous), None),
            Decision::Run
        ));
        previous.count = u32::MAX;
        assert!(matches!(
            Enough { again: false }.before(Some(&previous), None),
            Decision::Skip {
                count: u32::MAX,
                ..
            }
        ));
        previous.completed_at = state::now() - 301;
        assert!(matches!(
            Enough { again: false }.before(Some(&previous), None),
            Decision::Run
        ));
        let mut outcome = Outcome {
            category: "exited".into(),
            exit_code: 0,
            signal: None,
            stdout: None,
            stderr: None,
        };
        let update = Enough { again: true }
            .baseline(&outcome, &[0; 32], "", "", None)
            .unwrap();
        assert_eq!(update.count, 0);
        assert!(update.signature.is_empty());
        outcome.exit_code = 1;
        assert!(Enough { again: true }
            .baseline(&outcome, &[0; 32], "", "", None)
            .is_none());
    }

    #[test]
    fn success_window_includes_boundary_but_not_clock_rollback() {
        assert!(recent(100, 100));
        assert!(recent(100, 400));
        assert!(!recent(100, 401));
        assert!(!recent(100, 99));
    }

    #[test]
    fn reminders_are_localized_and_saturate() {
        for (count, en, zh) in [
            (1, "It already passed.", "已经通过了。"),
            (2, "Nothing changed.", "没有观察到变化。"),
            (3, "enough.", "够了。"),
            (4, "no.", "不必了。"),
            (u32::MAX, "no.", "不必了。"),
        ] {
            assert_eq!(reminder(count, Language::English), en);
            assert_eq!(reminder(count, Language::Chinese), zh);
        }
    }
}
