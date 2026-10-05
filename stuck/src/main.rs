#[cfg(not(unix))]
compile_error!("The first batch supports Linux/macOS/WSL Unix only");
mod policy;

use clap::Arg;
use cli_common::Language;
use std::ffi::OsString;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
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
            .value_parser(|s: &str| {
                cli_common::text_input(s, 512).ok_or_else(|| {
                    "Expected a nonempty single line, at most 512 characters".to_string()
                })
            })
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
        return 2;
    }
    cli_common::repeat::execute(
        "stuck",
        &argv,
        true,
        m.get_one::<String>("hypothesis").cloned(),
        &policy::Stuck,
    )
}
