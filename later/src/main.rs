#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
use cli_common::{
    command, display, error, git, process,
    state::{self, Store},
    Language, Result,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct Card {
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
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("later: {}", display(e.to_string()));
            1
        }
    };
    std::process::exit(code);
}
fn run() -> Result<i32> {
    let l = Language::detect();
    let matches = command(
        "later",
        l.text(
            "Save one next action locally. Do not enter secrets.",
            "在本地保存一条下一步。请勿填写秘密。",
        ),
    )
    .args_conflicts_with_subcommands(true)
    .arg(
        clap::Arg::new("next")
            .long("next")
            .value_name("text")
            .value_parser(|s: &str| {
                cli_common::text_input(s, 2000).ok_or_else(|| {
                    "Expected a nonempty single line, at most 2000 characters".to_string()
                })
            })
            .help(l.text(
                "Next action (local plaintext; no secrets)",
                "下一步（本地明文；不要填写秘密）",
            )),
    )
    .subcommand(command(
        "resume",
        l.text(
            "Show the saved context; never execute it",
            "显示已保存的上下文，不执行命令",
        ),
    ))
    .get_matches();
    let cwd = std::env::current_dir()?.canonicalize()?;
    let (workspace, repo) = git::workspace(&cwd);
    let store = Store::open()?;
    let dir = store.workspace(&workspace)?;
    let path = dir.join("later.json");
    if matches.subcommand_name() == Some("resume") {
        let _lock = state::lock(&dir.join("workspace.lock"))?;
        let Some(card) = store.read::<Card>(&path)? else {
            println!("{}", l.text("No saved context yet.", "暂无保存的上下文。"));
            return Ok(0);
        };
        if card.schema_version != 1
            || card.tool != "later"
            || card.workspace != state::path_bytes(&workspace)
            || cli_common::text_input(&card.next_action, 2000).is_none()
        {
            return Err(error("Invalid session card", "上下文卡片无效"));
        }
        println!(
            "{}: {}",
            l.text(
                "Saved context (Unix timestamp)",
                "保存时的上下文（Unix 时间戳）"
            ),
            card.created_at
        );
        println!(
            "{}: {}",
            l.text("Workspace", "工作区"),
            display(state::path_from(&card.workspace))
        );
        if let Some(head) = &card.head {
            println!("HEAD: {}", display(head));
        }
        if let Some(branch) = &card.branch {
            println!("{}: {}", l.text("Branch", "分支"), display(branch));
        }
        if card.repository.is_none() {
            println!(
                "{}",
                l.text("Git context unavailable.", "Git 上下文不可用。")
            );
        }
        println!(
            "{}: {}{}",
            l.text("Unfinished files", "未完成文件"),
            card.file_count,
            if card.truncated {
                l.text(" (truncated)", "（已截断）")
            } else {
                ""
            }
        );
        for f in &card.files {
            println!("  {}", display(state::path_from(f)));
        }
        if let Some(stat) = card.diff_stat {
            println!("{}: {}", l.text("Diff stat", "差异统计"), display(stat));
        }
        if let Some(commit) = card.latest_commit {
            println!(
                "{}: {}",
                l.text("Latest commit (ID/time)", "最近提交（ID/时间）"),
                display(commit)
            );
        }
        if let Some(failure) = card.last_failure {
            println!(
                "{}: {} (exit {}, {})",
                l.text("Last failed command", "最近失败命令"),
                display(failure.program),
                failure.exit_code,
                failure.created_at
            );
        }
        println!(
            "{}: {}",
            l.text("Next", "下一步"),
            display(card.next_action)
        );
        if repo.as_ref().map(|r| &r.head) != card.head.as_ref()
            || repo.as_ref().and_then(|r| r.branch.as_ref()) != card.branch.as_ref()
        {
            eprintln!(
                "{}",
                l.text(
                    "Current branch/HEAD differs from the saved context.",
                    "当前分支/HEAD 与保存时不同。"
                )
            );
        }
        return Ok(0);
    }
    let next = if let Some(next) = matches.get_one::<String>("next") {
        Some(next.clone())
    } else {
        process::ask(
            l.text("What is the very next thing?", "紧接着要做的一件事是什么？"),
            2000,
        )?
    };
    let Some(next) = next else {
        eprintln!(
            "{}",
            l.text(
                "No answer available; use --next <text>. Nothing saved.",
                "无法取得回答；请使用 --next <text>。未保存任何内容。"
            )
        );
        return Ok(125);
    };
    let mut card = Card {
        schema_version: 1,
        record_id: state::id(),
        created_at: state::now(),
        workspace_id: dir.file_name().unwrap().to_string_lossy().into_owned(),
        tool: "later".into(),
        workspace: state::path_bytes(&workspace),
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
        match collect(&repo, &mut card) {
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
    let _lock = state::lock(&dir.join("workspace.lock"))?;
    card.last_failure = cli_common::repeat::latest_failure(&store, &dir)?;
    store.write(&path, &card)?;
    println!(
        "{}",
        l.text(
            "saved for tomorrow\nYou are allowed to forget this now.",
            "已为明天保存\n现在可以放心忘掉它了。"
        )
    );
    Ok(0)
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
