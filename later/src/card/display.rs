//! Read-only presentation of a saved card; never executes its contents.
use cli_common::{display, git, state, Language};

impl super::Card {
    pub(crate) fn show(&self, repo: Option<&git::Repo>, l: Language) {
        println!(
            "{}: {}",
            l.text(
                "Saved context (Unix timestamp)",
                "保存时的上下文（Unix 时间戳）"
            ),
            self.created_at
        );
        println!(
            "{}: {}",
            l.text("Workspace", "工作区"),
            display(state::path_from(&self.workspace))
        );
        if let Some(head) = &self.head {
            println!("HEAD: {}", display(head));
        }
        if let Some(branch) = &self.branch {
            println!("{}: {}", l.text("Branch", "分支"), display(branch));
        }
        if self.repository.is_none() {
            println!(
                "{}",
                l.text("Git context unavailable.", "Git 上下文不可用。")
            );
        }
        println!(
            "{}: {}{}",
            l.text("Unfinished files", "未完成文件"),
            self.file_count,
            if self.truncated {
                l.text(" (truncated)", "（已截断）")
            } else {
                ""
            }
        );
        for f in &self.files {
            println!("  {}", display(state::path_from(f)));
        }
        if let Some(stat) = &self.diff_stat {
            println!("{}: {}", l.text("Diff stat", "差异统计"), display(stat));
        }
        if let Some(commit) = &self.latest_commit {
            println!(
                "{}: {}",
                l.text("Latest commit (ID/time)", "最近提交（ID/时间）"),
                display(commit)
            );
        }
        if let Some(failure) = &self.last_failure {
            println!(
                "{}: {} (exit {}, {})",
                l.text("Last failed command", "最近失败命令"),
                display(&failure.program),
                failure.exit_code,
                failure.created_at
            );
        }
        println!(
            "{}: {}",
            l.text("Next", "下一步"),
            display(&self.next_action)
        );
        if repo.as_ref().map(|r| &r.head) != self.head.as_ref()
            || repo.as_ref().and_then(|r| r.branch.as_ref()) != self.branch.as_ref()
        {
            eprintln!(
                "{}",
                l.text(
                    "Current branch/HEAD differs from the saved context.",
                    "当前分支/HEAD 与保存时不同。"
                )
            );
        }
    }
}
