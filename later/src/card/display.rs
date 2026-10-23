//! Read-only presentation of a saved card; never executes its contents.
use cli_common::{display, git, state, Language};
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

impl super::Card {
    pub(crate) fn show(&self, repo: Option<&git::Repo>, l: Language) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        print!("{}", self.render(now, l));
        if let Some(repo) = repo {
            if Some(&repo.head) != self.head.as_ref() || repo.branch != self.branch {
                eprintln!("{}", l.text(
                    "Note: Current branch/HEAD differs from the saved context; check it before continuing.",
                    "注意：当前分支/HEAD 与保存时不同，请先确认上下文。"
                ));
            }
        } else if self.head.is_some() {
            eprintln!(
                "{}",
                l.text(
                    "Current Git context unavailable; branch/HEAD comparison is unknown.",
                    "当前 Git 上下文不可用，无法比较分支/HEAD。"
                )
            );
        }
    }

    fn render(&self, now: Option<u64>, l: Language) -> String {
        let mut out = String::new();
        writeln!(
            out,
            "{}: {}\n",
            l.text("Next", "下一步"),
            display(&self.next_action)
        )
        .unwrap();
        writeln!(
            out,
            "{}: {} {}",
            l.text("Saved", "保存于"),
            age(self.created_at, now, l),
            l.text("(snapshot at save time)", "（以下为保存时快照）")
        )
        .unwrap();
        writeln!(
            out,
            "{}: {}",
            l.text("Workspace", "工作区"),
            display(state::path_from(&self.workspace))
        )
        .unwrap();
        let branch = match (&self.branch, &self.head) {
            (Some(branch), _) => match branch.strip_prefix("refs/heads/") {
                Some(name) => display(name),
                None => format!(
                    "{} ({})",
                    display(branch),
                    l.text("other reference", "其他引用")
                ),
            },
            (None, Some(_)) => l.text("detached HEAD", "游离 HEAD").to_owned(),
            (None, None) => l.text("unknown", "未知").to_owned(),
        };
        writeln!(out, "{}: {branch}", l.text("Branch", "分支")).unwrap();
        if self.repository.is_none() {
            writeln!(
                out,
                "{}",
                l.text("Git context unavailable.", "Git 上下文不可用。")
            )
            .unwrap();
        }
        writeln!(
            out,
            "{}: {}{}",
            l.text("Unfinished files", "未完成文件"),
            if self.repository.is_some() {
                self.file_count.to_string()
            } else {
                l.text("unknown", "未知").to_owned()
            },
            if self.truncated {
                l.text(" (truncated)", "（已截断）")
            } else {
                ""
            }
        )
        .unwrap();
        for f in &self.files {
            writeln!(out, "  {}", display(state::path_from(f))).unwrap();
        }
        let stat = match &self.diff_stat {
            Some(stat) if stat.is_empty() => l
                .text("No tracked-file differences", "无已跟踪文件差异")
                .to_owned(),
            Some(stat) => display(stat),
            None => l.text("unknown", "未知").to_owned(),
        };
        writeln!(out, "{}: {stat}", l.text("Diff stat", "差异统计")).unwrap();
        if let Some(head) = &self.head {
            writeln!(out, "HEAD: {}", display(head)).unwrap();
        }
        if let Some(commit) = &self.latest_commit {
            writeln!(
                out,
                "{}: {}",
                l.text("Latest commit (ID/time)", "最近提交（ID/时间）"),
                display(commit)
            )
            .unwrap();
        }
        if let Some(failure) = &self.last_failure {
            writeln!(
                out,
                "{}: {} (exit {}, {})",
                l.text("Last failed command", "最近失败命令"),
                display(&failure.program),
                failure.exit_code,
                failure.created_at
            )
            .unwrap();
        }
        out
    }
}

fn age(saved: u64, now: Option<u64>, l: Language) -> String {
    let Some(elapsed) = now.and_then(|now| now.checked_sub(saved)) else {
        return l
            .text("Saved time cannot be compared", "保存时间无法比较")
            .to_owned();
    };
    let (value, en, zh) = match elapsed {
        0..60 => return l.text("just now", "刚刚").to_owned(),
        60..3600 => (elapsed / 60, "minute", "分钟"),
        3600..86400 => (elapsed / 3600, "hour", "小时"),
        _ => (elapsed / 86400, "day", "天"),
    };
    let plural = if value == 1 { "" } else { "s" };
    l.text(
        &format!("about {value} {en}{plural} ago"),
        &format!("约 {value} {zh}前"),
    )
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_boundaries_and_clock_rollback() {
        for (seconds, en, zh) in [
            (0, "just now", "刚刚"),
            (59, "just now", "刚刚"),
            (60, "about 1 minute ago", "约 1 分钟前"),
            (3599, "about 59 minutes ago", "约 59 分钟前"),
            (3600, "about 1 hour ago", "约 1 小时前"),
            (86399, "about 23 hours ago", "约 23 小时前"),
            (86400, "about 1 day ago", "约 1 天前"),
            (172800, "about 2 days ago", "约 2 天前"),
        ] {
            assert_eq!(age(100, Some(100 + seconds), Language::English), en);
            assert_eq!(age(100, Some(100 + seconds), Language::Chinese), zh);
        }
        for now in [None, Some(99)] {
            assert_eq!(
                age(100, now, Language::English),
                "Saved time cannot be compared"
            );
            assert_eq!(age(100, now, Language::Chinese), "保存时间无法比较");
        }
    }

    #[test]
    fn next_first_unknown_is_not_clean_and_external_text_is_escaped() {
        let mut card = super::super::Card::capture(
            std::path::Path::new("/workspace"),
            std::path::Path::new("/state/id"),
            None,
            "start here".into(),
            Language::English,
        );
        card.created_at = 100;
        for (l, first, unknown) in [
            (
                Language::English,
                "Next: start here\n\n",
                "Diff stat: unknown",
            ),
            (
                Language::Chinese,
                "下一步: start here\n\n",
                "差异统计: 未知",
            ),
        ] {
            let out = card.render(Some(99), l);
            assert!(out.starts_with(first));
            assert!(out.contains(unknown));
        }
        card.repository = Some(b"/repo".to_vec());
        card.head = Some("a".repeat(40));
        card.branch = Some("refs/heads/main".into());
        card.diff_stat = Some(String::new());
        card.file_count = 1;
        card.files = vec![b"?? new\x1b.rs".to_vec()];
        card.truncated = true;
        let out = card.render(Some(100), Language::English);
        assert!(out.contains("Branch: main\n"));
        assert!(out.contains("Diff stat: No tracked-file differences"));
        assert!(out.contains("Unfinished files: 1 (truncated)"));
        assert!(out.contains(&format!("HEAD: {}", "a".repeat(40))));
        assert!(!out.contains('\x1b'));
        card.branch = None;
        assert!(card
            .render(Some(100), Language::English)
            .contains("Branch: detached HEAD"));
        card.branch = Some("refs/tags/v1".into());
        assert!(card
            .render(Some(100), Language::English)
            .contains("refs/tags/v1 (other reference)"));
    }
}
