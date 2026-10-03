use cli_common::{command, display, git, local_time::LocalDay, repeat, Language, Result};
use std::io::{self, Write};

fn main() {
    if let Err(e) = run() {
        eprintln!("goodnight: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    command(
        "goodnight",
        l.text(
            "Read-only session summary. Nothing is saved or enforced.",
            "只读收尾摘要。不保存内容，不强制作息。",
        ),
    )
    .get_matches();
    let mut out = io::stdout().lock();
    let unknown = l.text("unknown", "未知");
    let time = LocalDay::current().ok();
    writeln!(
        out,
        "{}: {}",
        l.text("Local time", "本地时间"),
        time.as_ref().map_or(unknown, |t| t.local_time.as_str())
    )?;
    let cwd = std::env::current_dir().and_then(|p| p.canonicalize());
    let context = cwd.as_ref().ok().map(|cwd| git::workspace(cwd));
    let repo = context.as_ref().and_then(|(_, repo)| repo.as_ref());
    writeln!(
        out,
        "{}: {}",
        l.text("Branch", "分支"),
        repo.map_or_else(
            || unknown.to_owned(),
            |r| r.branch.as_ref().map(display).unwrap_or_else(|| format!(
                "{} ({})",
                l.text("detached HEAD", "游离 HEAD"),
                r.head
            ))
        )
    )?;
    let count = repo
        .and_then(|r| r.status().ok())
        .and_then(|s| status_count(&s));
    writeln!(
        out,
        "{}: {}",
        l.text("Unfinished paths (Git status)", "未完成路径（Git 状态）"),
        count.map_or_else(|| unknown.to_owned(), |n| n.to_string())
    )?;
    let records = context
        .as_ref()
        .map(|(workspace, _)| repeat::recorded_runs(workspace));
    write_latest(&mut out, records.as_ref().and_then(|r| r.as_ref().ok()), l)?;
    if count.is_some_and(|n| n > 0) {
        writeln!(
            out,
            "{}",
            l.text(
                "If useful, save a next step with later --next <text>.",
                "如有需要，可手动运行 later --next <text> 保存下一步。"
            )
        )?;
    }
    writeln!(
        out,
        "{}",
        l.text(
            "Good night. You can leave this here.",
            "晚安。可以先停在这里。"
        )
    )
}
fn status_count(bytes: &[u8]) -> Option<usize> {
    let mut fields = bytes.split(|b| *b == 0).filter(|s| !s.is_empty());
    let mut n = 0;
    while let Some(entry) = fields.next() {
        if entry.len() < 4 {
            return None;
        }
        n += 1;
        if entry[..2].contains(&b'R') || entry[..2].contains(&b'C') {
            fields.next()?;
        }
    }
    Some(n)
}
fn write_latest(
    out: &mut impl Write,
    records: Option<&Vec<repeat::RecordedRun>>,
    l: Language,
) -> Result<()> {
    let label = l.text(
        "Latest recorded command (not necessarily tests)",
        "最近记录的命令（不一定是测试）",
    );
    let latest = records.and_then(|r| {
        r.iter()
            .rev()
            .find(|r| r.started_at <= cli_common::state::now())
    });
    let Some(run) = latest else {
        return writeln!(out, "{label}: {}", l.text("unknown", "未知"));
    };
    if records.is_some_and(|records| {
        records
            .iter()
            .filter(|r| r.started_at == run.started_at)
            .count()
            > 1
    }) {
        return writeln!(
            out,
            "{label}: {}",
            l.text(
                "unknown (multiple commands started in the same second)",
                "未知（多个命令在同一秒开始）"
            )
        );
    }
    let status = match &run.result {
        Some(o)
            if run.completed_execution()
                && run
                    .finished_at
                    .is_some_and(|t| t <= cli_common::state::now()) =>
        {
            format!("{} {}", l.text("exit", "退出码"), o.exit_code)
        }
        Some(o) if o.category == "spawn-failed" => l.text("launch failed", "启动失败").to_owned(),
        _ => l
            .text(
                "unfinished or incomplete; outcome unknown",
                "未完成或记录不完整；结果未知",
            )
            .to_owned(),
    };
    writeln!(
        out,
        "{label}: {} / {} — {} ({}: {})",
        run.tool,
        display(&run.program),
        status,
        l.text("started, Unix time", "开始时间，Unix 时间戳"),
        run.started_at
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn rename_is_one_changed_path() {
        assert_eq!(super::status_count(b"R  new\0old\0?? extra\0"), Some(2));
        assert_eq!(super::status_count(b"R  missing\0"), None);
    }
}
