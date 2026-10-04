use cli_common::{
    command, display, error,
    state::{self, Store},
    text_input, Language, Result,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, Write};
const MAX_NAMES: usize = 1000;
const MAX_NAME: usize = 200;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Names {
    schema_version: u32,
    names: Vec<String>,
}
impl Default for Names {
    fn default() -> Self {
        Self {
            schema_version: 1,
            names: vec![],
        }
    }
}
impl Names {
    fn validate(&self) -> Result<()> {
        let mut unique = HashSet::new();
        if self.schema_version != 1
            || self.names.len() > MAX_NAMES
            || self.names.iter().any(|n| {
                text_input(n, MAX_NAME).as_deref() != Some(n.as_str()) || !unique.insert(n)
            })
        {
            return Err(error(
                "Invalid contact list; nothing changed",
                "名单无效；未作更改",
            ));
        }
        Ok(())
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("poke: {}", display(e.to_string()));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let l = Language::detect();
    let mut cli = command(
        "poke",
        l.text(
            "A small local reminder to say hello; never sends messages.",
            "小小的本地问候提醒；绝不自动发送消息。",
        ),
    );
    for (name, en, zh) in [
        ("add", "Add a name or nickname", "添加姓名或昵称"),
        ("remove", "Remove a name or nickname", "移除姓名或昵称"),
    ] {
        cli = cli.subcommand(
            command(name, l.text(en, zh)).arg(
                clap::Arg::new("name")
                    .required(true)
                    .value_parser(move |s: &str| {
                        text_input(s, MAX_NAME).ok_or_else(|| {
                            l.text(
                                "Expected a nonempty single line, at most 200 characters",
                                "请输入非空单行文本，最多 200 个字符",
                            )
                            .to_owned()
                        })
                    })
                    .help(l.text(
                        "Name (local plaintext; no secrets)",
                        "姓名（本地明文，不要填写秘密）",
                    )),
            ),
        );
    }
    let args = cli.get_matches();
    let store = Store::open()?;
    let dir = store.dir("poke")?;
    let _lock = state::lock(&dir.join("names.lock"))?;
    let path = dir.join("names.json");
    let mut list = store.read::<Names>(&path)?.unwrap_or_default();
    list.validate()?;
    let mut out = io::stdout().lock();
    match args.subcommand() {
        Some(("add", args)) => {
            let name = args.get_one::<String>("name").unwrap();
            if list.names.contains(name) {
                return writeln!(out, "{}", l.text("Already on the list.", "已在名单中。"));
            }
            if list.names.len() >= MAX_NAMES {
                return Err(error("Name limit reached (1000)", "名单已达上限（1000）"));
            }
            list.names.push(name.clone());
            store.write(&path, &list)?;
            writeln!(out, "{}", l.text("Added.", "已添加。"))
        }
        Some(("remove", args)) => {
            let name = args.get_one::<String>("name").unwrap();
            if !list.names.contains(name) {
                return writeln!(
                    out,
                    "{}",
                    l.text("That name is not on the list.", "该姓名不在名单中。")
                );
            }
            list.names.retain(|n| n != name);
            store.write(&path, &list)?;
            writeln!(out, "{}", l.text("Removed.", "已移除。"))
        }
        _ => {
            if list.names.is_empty() {
                return writeln!(
                    out,
                    "{}",
                    l.text(
                        "No names yet. Use poke add <name>.",
                        "暂无姓名。请使用 poke add <name>。"
                    )
                );
            }
            let mut rng = match std::env::var("POKE_SEED")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
            {
                Some(seed) => StdRng::seed_from_u64(seed),
                None => StdRng::from_entropy(),
            };
            let name = &list.names[rng.gen_range(0..list.names.len())];
            writeln!(
                out,
                "{}: {}",
                l.text("If you like, say hello to", "如果愿意，向这位朋友问个好"),
                name
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validate_schema_names_and_exact_duplicates() {
        Names::default().validate().unwrap();
        for names in [
            vec!["x".into(), "x".into()],
            vec![" ".into()],
            vec!["bad\x1b".into()],
            vec!["x".repeat(201)],
        ] {
            assert!(Names {
                schema_version: 1,
                names
            }
            .validate()
            .is_err());
        }
        Names {
            schema_version: 1,
            names: vec!["Alice".into(), "alice".into()],
        }
        .validate()
        .unwrap();
        assert!(Names {
            schema_version: 2,
            names: vec![]
        }
        .validate()
        .is_err());
    }
}
