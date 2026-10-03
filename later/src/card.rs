//! Session-card schema and Git capture. The caller owns the workspace lock.
mod display;

use cli_common::{
    display, error, git,
    state::{self, Store},
    Language, Result,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub(super) struct Card {
    schema_version: u32,
    record_id: String,
    created_at: u64,
    workspace_id: String,
    tool: String,
    workspace: Vec<u8>,
    repository: Option<Vec<u8>>,
    branch: Option<String>,
    head: Option<String>,
    files: Vec<Vec<u8>>,
    file_count: usize,
    truncated: bool,
    diff_stat: Option<String>,
    latest_commit: Option<String>,
    next_action: String,
    last_failure: Option<cli_common::repeat::FailureSummary>,
}
impl Card {
    pub(super) fn validate(&self, workspace: &Path) -> Result<()> {
        if self.schema_version != 1
            || self.tool != "later"
            || self.workspace != state::path_bytes(workspace)
            || cli_common::text_input(&self.next_action, 2000).is_none()
        {
            return Err(error("Invalid session card", "上下文卡片无效"));
        }
        Ok(())
    }

    pub(super) fn capture(
        workspace: &Path,
        dir: &Path,
        repo: Option<&git::Repo>,
        next: String,
        l: Language,
    ) -> Self {
        let mut card = Card {
            schema_version: 1,
            record_id: state::id(),
            created_at: state::now(),
            workspace_id: dir.file_name().unwrap().to_string_lossy().into_owned(),
            tool: "later".into(),
            workspace: state::path_bytes(workspace),
            repository: None,
            branch: None,
            head: None,
            files: vec![],
            file_count: 0,
            truncated: false,
            diff_stat: None,
            latest_commit: None,
            next_action: next,
            last_failure: None,
        };
        if let Some(repo) = repo {
            match collect(repo, &mut card) {
                Ok(()) => (),
                Err(e) => {
                    eprintln!(
                        "{}: {}",
                        l.text("Git context unavailable", "Git 上下文不可用"),
                        display(e.to_string())
                    );
                    card.repository = None;
                    card.branch = None;
                    card.head = None;
                    card.files.clear();
                    card.file_count = 0;
                    card.truncated = false;
                    card.diff_stat = None;
                    card.latest_commit = None;
                }
            }
        }
        card
    }

    /// Caller must hold the workspace lock through failure lookup and writing.
    pub(super) fn save(&mut self, store: &Store, dir: &Path, path: &Path) -> Result<()> {
        self.last_failure = cli_common::repeat::latest_failure(store, dir)?;
        store.write(path, self)
    }
}

fn collect(repo: &git::Repo, card: &mut Card) -> Result<()> {
    let bytes = repo.status()?;
    let mut parts = bytes.split(|b| *b == 0).filter(|b| !b.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            return Err(error("Invalid Git status", "Git 状态无效"));
        }
        card.file_count += 1;
        if card.files.len() < 200 {
            card.files.push(entry.to_vec());
        }
        if entry[..2].contains(&b'R') || entry[..2].contains(&b'C') {
            let _ = parts
                .next()
                .ok_or_else(|| error("Incomplete Git rename", "Git 重命名信息不完整"))?;
        }
    }
    card.truncated = card.file_count > card.files.len();
    card.repository = Some(state::path_bytes(&repo.common));
    card.branch = repo.branch.clone();
    card.head = Some(repo.head.clone());
    card.diff_stat = Some(stat(&repo.root)?);
    card.latest_commit = Some(
        String::from_utf8_lossy(&git::run(&repo.root, ["log", "-1", "--format=%H %ct"])?)
            .trim_end()
            .to_owned(),
    );
    Ok(())
}
fn stat(root: &Path) -> Result<String> {
    let b = git::run(
        root,
        [
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--stat",
            "HEAD",
            "--",
        ],
    )?;
    Ok(String::from_utf8_lossy(&b[..b.len().min(32768)]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_git_card_roundtrip_and_validation() {
        let workspace = Path::new("/workspace");
        let card = Card::capture(
            workspace,
            Path::new("/state/workspace-id"),
            None,
            "next step".into(),
            Language::English,
        );
        let json = serde_json::to_vec(&card).unwrap();
        let mut restored: Card = serde_json::from_slice(&json).unwrap();
        restored.validate(workspace).unwrap();
        assert_eq!(restored.workspace_id, "workspace-id");
        assert_eq!(restored.next_action, "next step");
        assert!(restored.repository.is_none());
        assert!(restored.head.is_none());
        assert!(restored.files.is_empty());
        assert!(restored.validate(Path::new("/another-workspace")).is_err());
        restored.schema_version = 2;
        assert!(restored.validate(workspace).is_err());
        restored.schema_version = 1;
        restored.next_action = "invalid\nline".into();
        assert!(restored.validate(workspace).is_err());
    }
}
