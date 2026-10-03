mod history;
use cli_common::{command, display, git, local_time::LocalDay, repeat, Language, Result};
use std::io::{self, Write};
fn main() {
    if let Err(e) = run() {
        eprintln!("proof: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    command(
        "proof",
        l.text(
            "Show today's local recorded evidence, not a productivity score.",
            "显示今天已有的本地记录，不评价生产力。",
        ),
    )
    .get_matches();
    let mut out = io::stdout().lock();
    let unknown = l.text("unknown", "未知");
    let day = LocalDay::current().ok();
    writeln!(
        out,
        "{}: {}",
        l.text("Local date", "本地日期"),
        day.as_ref().map_or(unknown, |d| d.date.as_str())
    )?;
    let cwd = std::env::current_dir().and_then(|p| p.canonicalize());
    let context = cwd.as_ref().ok().map(|p| git::workspace(p));
    let totals = context
        .as_ref()
        .and_then(|(_, r)| r.as_ref())
        .zip(day.as_ref())
        .and_then(|(repo, day)| history::collect(repo, day).ok());
    if let Some(t) = &totals {
        writeln!(
            out,
            "{}: {}",
            l.text(
                "Authored commits (current HEAD, committer date)",
                "本人提交（当前 HEAD，按提交时间）"
            ),
            t.commits
        )?;
        writeln!(
            out,
            "{}: +{} / -{}; {}: {}; {}: {}",
            l.text("Non-merge line changes (sum)", "非合并提交行变更（累加）"),
            t.added,
            t.deleted,
            l.text("unique paths", "去重路径"),
            t.paths.len(),
            l.text("binary paths, no line count", "二进制路径，不计行数"),
            t.binary_paths.len()
        )?;
    } else {
        writeln!(
            out,
            "{}: {unknown}",
            l.text(
                "Git evidence (repository/author/history)",
                "Git 证据（仓库/作者/历史）"
            )
        )?;
    }
    let runs = context
        .as_ref()
        .and_then(|(workspace, _)| repeat::recorded_runs(workspace).ok());
    let counts = runs.as_ref().zip(day.as_ref()).map(|(runs, day)| {
        let mut completed = 0;
        let mut successful = 0;
        for run in runs {
            if run.completed_execution()
                && run
                    .finished_at
                    .and_then(|t| i64::try_from(t).ok())
                    .is_some_and(|t| day.contains(t))
            {
                completed += 1;
                if run
                    .result
                    .as_ref()
                    .is_some_and(|r| r.category == "exited" && r.exit_code == 0)
                {
                    successful += 1;
                }
            }
        }
        (completed, successful)
    });
    if let Some((completed, successful)) = counts {
        writeln!(
            out,
            "{}: {completed}",
            l.text(
                "Completed executions (current workspace, completion date)",
                "已完成执行（当前工作区，按完成时间）"
            )
        )?;
        writeln!(
            out,
            "{}: {successful}",
            l.text(
                "Successful executions (not necessarily tests)",
                "成功执行（不一定是测试）"
            )
        )?;
    } else {
        writeln!(
            out,
            "{}: {unknown}",
            l.text("Recorded executions", "执行记录")
        )?;
    }
    if totals.as_ref().is_none_or(|t| t.commits == 0) && counts.is_none_or(|(n, _)| n == 0) {
        writeln!(out, "{}", l.text("no recorded work.", "暂无工作记录。"))?;
    }
    writeln!(
        out,
        "{}",
        l.text(
            "Only available records are shown; missing evidence says nothing about your work.",
            "只显示可用记录；缺少记录不代表没有工作。"
        )
    )
}
