use cli_common::{
    command, display, error,
    state::{self, Store},
    text_input, Language, Result,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, Read, Write};

const MAX_TASKS: usize = 1000;
const MAX_TEXT: usize = 2000;
const MAX_PIPE: u64 = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    id: String,
    text: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tasks {
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
    fn validate(&self) -> Result<()> {
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
fn choose(len: usize) -> usize {
    let mut rng = match std::env::var("ONE_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    };
    rng.gen_range(0..len)
}
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
                .value_parser(|s: &str| {
                    text_input(s, MAX_TEXT).ok_or_else(|| {
                        "Expected a nonempty single line, at most 2000 characters".to_owned()
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
            if record.tasks.len() >= MAX_TASKS {
                return Err(error(
                    "Task limit reached (1000)",
                    "任务数量已达上限（1000）",
                ));
            }
            let mut id = state::id();
            while record.tasks.iter().any(|t| t.id == id) {
                id = state::id();
            }
            record.tasks.push(Task {
                id,
                text: args.get_one::<String>("text").unwrap().clone(),
            });
            store.write(&path, &record)?;
            writeln!(out, "{}", l.text("Added.", "已添加。"))?;
        }
        Some(("done", _)) => {
            if let Some(id) = record.selected_task_id.take() {
                record.tasks.retain(|t| t.id != id);
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
            if record.selected_task_id.is_none() && !record.tasks.is_empty() {
                record.selected_task_id = Some(record.tasks[choose(record.tasks.len())].id.clone());
                store.write(&path, &record)?;
            }
            let task = record
                .tasks
                .iter()
                .find(|t| Some(&t.id) == record.selected_task_id.as_ref());
            show(&mut out, task.map(|t| t.text.as_str()), l)?;
        }
    }
    Ok(())
}
fn show(out: &mut impl Write, task: Option<&str>, l: Language) -> Result<()> {
    writeln!(
        out,
        "{}",
        task.unwrap_or(l.text("Nothing to choose from.", "没有待选事项。"))
    )
}
fn temporary_input() -> Result<bool> {
    // /dev/null (common in headless invocations) is not a task list. Pipes,
    // sockets and redirected regular files are; no /dev/tty fallback is used.
    let stat = rustix::fs::fstat(io::stdin())?;
    Ok(matches!(
        rustix::fs::FileType::from_raw_mode(stat.st_mode),
        rustix::fs::FileType::Fifo
            | rustix::fs::FileType::RegularFile
            | rustix::fs::FileType::Socket
    ))
}
fn read_tasks(input: impl Read) -> Result<Vec<String>> {
    let mut bytes = Vec::new();
    input.take(MAX_PIPE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PIPE {
        return Err(error("Input exceeds 1 MiB", "输入超过 1 MiB"));
    }
    let input = std::str::from_utf8(&bytes)
        .map_err(|_| error("Input must be UTF-8", "输入必须为 UTF-8"))?;
    let mut tasks = Vec::new();
    for line in input.lines() {
        let Some(line) = task_line(line) else {
            continue;
        };
        let task = text_input(line, MAX_TEXT).ok_or_else(|| {
            error(
                "Invalid task: use a single line of at most 2000 characters",
                "任务无效：请使用最多 2000 字符的单行文本",
            )
        })?;
        if tasks.len() >= MAX_TASKS {
            return Err(error(
                "Task limit reached (1000)",
                "任务数量已达上限（1000）",
            ));
        }
        tasks.push(task);
    }
    Ok(tasks)
}
fn task_line(line: &str) -> Option<&str> {
    let mut s = line.trim();
    let heading = s.trim_start_matches('#');
    let count = s.len() - heading.len();
    if (1..=6).contains(&count) && (heading.is_empty() || heading.starts_with(char::is_whitespace))
    {
        return None;
    }
    if let Some(rest) = s.strip_prefix(['-', '*', '+']) {
        if rest.starts_with(char::is_whitespace) {
            s = rest.trim_start();
        }
    } else {
        let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
        if rest.len() < s.len() {
            if let Some(rest) = rest.strip_prefix(['.', ')']) {
                if rest.starts_with(char::is_whitespace) {
                    s = rest.trim_start();
                }
            }
        }
    }
    for mark in ["[x]", "[X]", "[ ]"] {
        if let Some(rest) = s.strip_prefix(mark) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                if mark != "[ ]" {
                    return None;
                }
                s = rest.trim_start();
                break;
            }
        }
    }
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conservative_line_filtering() {
        let lines = "# TODO\n\n- [x] done\n* [X] done too\n- [ ] first\n2. second\n3) third\n+ fourth\nplain\n#hashtag\n";
        assert_eq!(
            read_tasks(lines.as_bytes()).unwrap(),
            ["first", "second", "third", "fourth", "plain", "#hashtag"]
        );
        assert!(read_tasks(b"\xff".as_slice()).is_err());
        assert!(read_tasks(b"- bad\x1btext".as_slice()).is_err());
        assert!(read_tasks("x".repeat(2001).as_bytes()).is_err());
        assert!(read_tasks("x\n".repeat(1001).as_bytes()).is_err());
        assert!(read_tasks(vec![b' '; MAX_PIPE as usize + 1].as_slice()).is_err());
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
