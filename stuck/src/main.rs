#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
mod policy;

use clap::Arg;
use cli_common::{exit_status, Language};
use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    run()
}

fn run() -> ExitCode {
    let l = Language::detect();
    let m = cli_common::command(
        "stuck",
        l.text(
            "Pause after three identical failures. Hypotheses are local plaintext: no secrets.",
            "连续三次相同失败后暂停。假设保存在本地明文中：不要填写秘密。",
        ),
    )
    .arg(
        Arg::new("hypothesis")
            .long("hypothesis")
            .value_name("text")
            .value_parser(cli_common::bounded_text(512))
            .help(l.text(
                "One experiment hypothesis (no secrets)",
                "仅本次实验的假设（不要填写秘密）",
            )),
    )
    .arg(
        Arg::new("command")
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
            "{}: stuck [--] <command> [args...]",
            l.text("Usage", "用法")
        );
        return ExitCode::from(2);
    }
    exit_status(cli_common::repeat::execute(
        "stuck",
        &argv,
        true,
        m.get_one::<String>("hypothesis").cloned(),
        &policy::Stuck,
    ))
}
