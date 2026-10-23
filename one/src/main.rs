mod input;
mod tasks;

use cli_common::{
    command, display,
    state::{self, Store},
    text_input, Language, Result,
};
use input::{read_tasks, temporary_input};
use std::io::{self, Write};
use tasks::{choose, Tasks, MAX_TEXT};

fn main() {
    if let Err(e) = run() {
        eprintln!("one: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    let matches = command(
        "one",
        l.text(
            "Show just one next task. Tasks are local plaintext.",
            "只显示一件下一步任务。任务为本地明文。",
        ),
    )
    .subcommand(
        command("add", l.text("Add a task", "添加任务")).arg(
            clap::Arg::new("text")
                .required(true)
                .value_parser(move |s: &str| {
                    text_input(s, MAX_TEXT).ok_or_else(|| {
                        l.text(
                            "Expected a nonempty single line, at most 2000 characters",
                            "请输入非空单行文本，最多 2000 个字符",
                        )
                        .to_owned()
                    })
                })
                .help(l.text("Task (no secrets)", "任务（不要填写秘密）")),
        ),
    )
    .subcommand(command(
        "done",
        l.text("Complete the persistent selection", "完成持久化选中的任务"),
    ))
    .get_matches();
    let mut out = io::stdout().lock();
    if matches.subcommand_name().is_none() && temporary_input()? {
        let tasks = read_tasks(io::stdin().lock())?;
        return show(
            &mut out,
            tasks
                .get(if tasks.is_empty() {
                    0
                } else {
                    choose(tasks.len())
                })
                .map(String::as_str),
            l,
            Source::Temporary,
        );
    }
    let store = Store::open()?;
    let dir = store.dir("one")?;
    let _lock = state::lock(&dir.join("tasks.lock"))?;
    let path = dir.join("tasks.json");
    let mut record = store.read::<Tasks>(&path)?.unwrap_or_default();
    record.validate()?;
    match matches.subcommand() {
        Some(("add", args)) => {
            record.add(args.get_one::<String>("text").unwrap().clone())?;
            store.write(&path, &record)?;
            writeln!(out, "{}", l.text("Added.", "已添加。"))?;
        }
        Some(("done", _)) => {
            if record.complete() {
                store.write(&path, &record)?;
                writeln!(out, "{}", l.text("Done.", "已完成。"))?;
            } else {
                writeln!(
                    out,
                    "{}",
                    l.text("No selected task yet.", "暂无选中的任务。")
                )?;
            }
        }
        _ => {
            if record.select() {
                store.write(&path, &record)?;
            }
            show(&mut out, record.selected(), l, Source::Persistent)?;
        }
    }
    Ok(())
}
enum Source {
    Persistent,
    Temporary,
}
fn show(out: &mut impl Write, task: Option<&str>, l: Language, source: Source) -> Result<()> {
    writeln!(
        out,
        "{}",
        task.unwrap_or(match source {
            Source::Persistent => l.text(
                "Nothing to choose from. Use one add \"next small step\" to add a task.",
                "没有待选事项。用 one add \"下一小步\" 添加一件事。",
            ),
            Source::Temporary =>
                l.text("No selectable tasks in the input.", "输入中没有可选事项。"),
        })
    )
}
