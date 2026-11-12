use cli_common::{bounded_text, command, process, Language, Result};
use std::io::{self, Write};
use std::process::ExitCode;

const QUESTIONS: [(&str, &str, &str); 4] = [
    ("expected", "Expected", "预期结果"),
    ("actual", "Actual", "实际结果"),
    ("last-change", "Last change", "最近改动"),
    ("experiment", "Next experiment", "下一次实验"),
];

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            cli_common::report_error("duck", &e);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let l = Language::detect();
    let mut cli = command(
        "duck",
        l.text(
            "Explain the problem in four answers; nothing is saved.",
            "用四个回答说清问题；不保存内容。",
        ),
    );
    for (name, en, zh) in QUESTIONS {
        cli = cli.arg(
            clap::Arg::new(name)
                .long(name)
                .value_name("text")
                .value_parser(bounded_text(2000))
                .help(l.text(en, zh)),
        );
    }
    let matches = cli.get_matches();
    let mut answers = Vec::new();
    for (name, en, zh) in QUESTIONS {
        let answer = match matches.get_one::<String>(name) {
            Some(answer) => Some(answer.clone()),
            None => process::ask(&format!("{}?", l.text(en, zh)), 2000)?,
        };
        let Some(answer) = answer else {
            eprintln!("{}", l.text(
                "No valid answer available; provide --expected, --actual, --last-change and --experiment.",
                "无法取得有效回答；请提供 --expected、--actual、--last-change 和 --experiment。"
            ));
            return Ok(ExitCode::from(125));
        };
        answers.push(answer);
    }
    // No partial summary if an answer is unavailable; prompts never go to stdout.
    let mut out = io::stdout().lock();
    for ((_, en, zh), answer) in QUESTIONS.into_iter().zip(answers) {
        writeln!(out, "{}: {}", l.text(en, zh), answer)?;
    }
    writeln!(out, "quack.")?;
    Ok(ExitCode::SUCCESS)
}
