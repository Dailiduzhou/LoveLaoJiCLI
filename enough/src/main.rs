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
        "enough",
        l.text(
            "Remember a recent success; not a build cache or proof that rerunning has no value.",
            "提醒最近一次成功；不是构建缓存，也不证明再次运行没有新信息。",
        ),
    )
    .arg(
        Arg::new("again")
            .long("again")
            .action(clap::ArgAction::SetTrue)
            .help(l.text("Always execute once", "始终真实执行一次")),
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
            "{}: enough [--] <command> [args...]",
            l.text("Usage", "用法")
        );
        return 2;
    }
    cli_common::repeat::execute(
        "enough",
        &argv,
        false,
        None,
        &policy::Enough {
            again: m.get_flag("again"),
        },
    )
}
