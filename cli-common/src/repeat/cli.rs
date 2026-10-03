//! Enough/stuck argument parsing and fail-open child execution.
use super::protocol::{prepare, warning};
use crate::{display, process, Language};
use std::{ffi::OsString, time::Instant};

pub fn run(tool: &'static str) -> i32 {
    let l = Language::detect();
    let stuck = tool == "stuck";
    let mut cli = crate::command(
        tool,
        if stuck {
            l.text(
                "Pause after three identical failures. Hypotheses are local plaintext: no secrets.",
                "连续三次相同失败后暂停。假设保存在本地明文中：不要填写秘密。",
            )
        } else {
            l.text("Remember a recent success; not a build cache or proof that rerunning has no value.", "提醒最近一次成功；不是构建缓存，也不证明再次运行没有新信息。")
        },
    );
    if stuck {
        cli = cli.arg(
            clap::Arg::new("hypothesis")
                .long("hypothesis")
                .value_name("text")
                .value_parser(|s: &str| {
                    crate::text_input(s, 512).ok_or_else(|| {
                        "Expected a nonempty single line, at most 512 characters".to_string()
                    })
                })
                .help(l.text(
                    "One experiment hypothesis (no secrets)",
                    "仅本次实验的假设（不要填写秘密）",
                )),
        );
    } else {
        cli = cli.arg(
            clap::Arg::new("again")
                .long("again")
                .action(clap::ArgAction::SetTrue)
                .help(l.text("Always execute once", "始终真实执行一次")),
        );
    }
    let m = cli
        .arg(
            clap::Arg::new("command")
                .help_heading(l.text("Arguments", "参数"))
                .num_args(1..)
                .trailing_var_arg(true)
                .value_parser(clap::builder::OsStringValueParser::new())
                .help(l.text(
                    "Program and its unchanged arguments",
                    "程序及其原样传递的参数",
                )),
        )
        .get_matches();
    let argv: Vec<OsString> = m
        .get_many::<OsString>("command")
        .map(|v| v.cloned().collect())
        .unwrap_or_default();
    if argv.is_empty() {
        eprintln!(
            "{}: {tool} [--] <command> [args...]",
            l.text("Usage", "用法")
        );
        return 2;
    }
    let hypothesis = if stuck {
        m.get_one::<String>("hypothesis").cloned()
    } else {
        None
    };
    let again = !stuck && m.get_flag("again");
    let mut context = match prepare(tool, &argv, hypothesis.clone()) {
        Ok(mut c) => match c.begin(again, hypothesis) {
            Ok(Some(code)) => return code,
            Ok(None) => Some(c),
            Err(e) => {
                warning(e);
                None
            }
        },
        Err(e) => {
            warning(e);
            None
        }
    };
    let key = context
        .as_ref()
        .map(|c| c.key())
        .unwrap_or_else(rand::random);
    let start = Instant::now();
    let outcome = process::execute(&argv, stuck, key);
    let code = outcome.exit_code;
    if let Some(c) = context.as_mut() {
        if let Err(e) = c.finish(outcome, start.elapsed().as_millis()) {
            eprintln!(
                "{}: {}",
                l.text("Could not save execution record", "无法保存执行记录"),
                display(e.to_string())
            );
        }
    }
    code
}
