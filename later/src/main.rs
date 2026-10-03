#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
mod card;

use card::Card;
use cli_common::{
    command, display, git, process,
    state::{self, Store},
    Language, Result,
};

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
        card.validate(&workspace)?;
        card.show(repo.as_ref(), l);
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
    let mut card = Card::capture(&workspace, &dir, repo.as_ref(), next, l);
    let _lock = state::lock(&dir.join("workspace.lock"))?;
    card.save(&store, &dir, &path)?;
    println!(
        "{}",
        l.text(
            "saved for tomorrow\nYou are allowed to forget this now.",
            "已为明天保存\n现在可以放心忘掉它了。"
        )
    );
    Ok(0)
}
