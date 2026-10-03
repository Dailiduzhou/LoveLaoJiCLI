//! Persistent task schema, validation, and selection.
use cli_common::{error, state, text_input, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub(super) const MAX_TASKS: usize = 1000;
pub(super) const MAX_TEXT: usize = 2000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    id: String,
    text: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tasks {
    schema_version: u32,
    tasks: Vec<Task>,
    selected_task_id: Option<String>,
}
impl Default for Tasks {
    fn default() -> Self {
        Self {
            schema_version: 1,
            tasks: vec![],
            selected_task_id: None,
        }
    }
}
impl Tasks {
    /// Add a CLI-validated task without replacing the current selection.
    pub(super) fn add(&mut self, text: String) -> Result<()> {
        if self.tasks.len() >= MAX_TASKS {
            return Err(error(
                "Task limit reached (1000)",
                "任务数量已达上限（1000）",
            ));
        }
        let mut id = state::id();
        while self.tasks.iter().any(|t| t.id == id) {
            id = state::id();
        }
        self.tasks.push(Task { id, text });
        Ok(())
    }

    /// Return whether persistent state changed and needs to be written.
    pub(super) fn complete(&mut self) -> bool {
        if let Some(id) = self.selected_task_id.take() {
            self.tasks.retain(|t| t.id != id);
            true
        } else {
            false
        }
    }

    /// Select only once, until completion. Report whether a write is needed.
    pub(super) fn select(&mut self) -> bool {
        if self.selected_task_id.is_none() && !self.tasks.is_empty() {
            self.selected_task_id = Some(self.tasks[choose(self.tasks.len())].id.clone());
            true
        } else {
            false
        }
    }

    pub(super) fn selected(&self) -> Option<&str> {
        self.tasks
            .iter()
            .find(|t| Some(&t.id) == self.selected_task_id.as_ref())
            .map(|t| t.text.as_str())
    }

    pub(super) fn validate(&self) -> Result<()> {
        let mut ids = HashSet::new();
        if self.schema_version != 1
            || self.tasks.len() > MAX_TASKS
            || self.tasks.iter().any(|t| {
                t.id.len() != 32
                    || !t.id.bytes().all(|b| b.is_ascii_hexdigit())
                    || !ids.insert(&t.id)
                    || text_input(&t.text, MAX_TEXT).as_deref() != Some(t.text.as_str())
            })
            || self
                .selected_task_id
                .as_ref()
                .is_some_and(|id| !ids.contains(id))
        {
            return Err(error(
                "Invalid task record; nothing changed",
                "任务记录无效；未作更改",
            ));
        }
        Ok(())
    }
}
pub(super) fn choose(len: usize) -> usize {
    let mut rng = match std::env::var("ONE_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };
    rng.gen_range(0..len)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_survives_add_and_roundtrip_until_completed() {
        let mut tasks = Tasks::default();
        assert!(!tasks.select());
        assert!(!tasks.complete());
        assert_eq!(tasks.selected(), None);
        tasks.add("first".into()).unwrap();
        assert!(tasks.select());
        tasks.add("second".into()).unwrap();
        assert!(!tasks.select());
        assert_eq!(tasks.selected(), Some("first"));

        let json = serde_json::to_vec(&tasks).unwrap();
        let mut restored: Tasks = serde_json::from_slice(&json).unwrap();
        restored.validate().unwrap();
        assert!(!restored.select());
        assert_eq!(restored.selected(), Some("first"));
        assert!(restored.complete());
        assert_eq!(restored.selected(), None);
        assert!(!restored.complete());
        assert!(restored.select());
        assert_eq!(restored.selected(), Some("second"));
        restored.validate().unwrap();
    }

    #[test]
    fn rejects_invalid_records() {
        let mut r = Tasks::default();
        r.validate().unwrap();
        r.selected_task_id = Some("missing".into());
        assert!(r.validate().is_err());
        r.selected_task_id = None;
        r.tasks.push(Task {
            id: "0".repeat(32),
            text: "valid".into(),
        });
        r.validate().unwrap();
        r.tasks.push(Task {
            id: "0".repeat(32),
            text: "duplicate".into(),
        });
        assert!(r.validate().is_err());
        r.tasks.pop();
        r.schema_version = 2;
        assert!(r.validate().is_err());
    }
}
