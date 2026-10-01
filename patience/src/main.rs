//! patience: a fake progress bar for a real command.
//!
//! `patience <command> [args...]` runs the command and performs a whole show
//! around it: a rainbow marquee bar with random speed curves, repeated stalls
//! with localized comfort messages, an eternal stall near 99%, and a rapid
//! fill to 100% only if the child succeeds. The exit code passes through.

mod curve;
mod messages;
mod plan;
mod rainbow;

use std::io::{IsTerminal, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use clap::{Arg, ArgAction, Command as ClapCommand};
use rand::rngs::StdRng;
use rand::SeedableRng;

use cli_common::Language;
use messages::Comfort;
use plan::{BAR_WIDTH, OUTPUT_CAP};
use rainbow::Style;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let language = Language::detect();
    let matches = build_command(language).get_matches();
    let command: Vec<String> = matches
        .get_many::<String>("command")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if command.is_empty() {
        eprintln!(
            "{}",
            language.text(
                "Usage: patience <command> [args...] — patience without a command is just standing around",
                "用法：patience <命令> [参数...]——没命令可等的耐心不是耐心",
            )
        );
        return 2;
    }

    // Hidden test hooks, intentionally not in --help: PATIENCE_FAST compresses
    // every timing by 100x and PATIENCE_SEED fixes all randomness.
    let scale = if std::env::var("PATIENCE_FAST").ok().as_deref() == Some("1") {
        0.01
    } else {
        1.0
    };
    let seed = std::env::var("PATIENCE_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(rand::random);
    let mut rng = StdRng::seed_from_u64(seed);
    let plan = plan::generate(&mut rng, scale);
    let mut comfort = Comfort::new(language, &mut rng);

    let mut child = match Command::new(&command[0])
        .args(&command[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            println!("{}", result_line(language, false, 0.0, 127));
            return 127;
        }
    };
    let stdout_pipe = child.stdout.take().expect("child stdout is piped");
    let stderr_pipe = child.stderr.take().expect("child stderr is piped");
    let stdout_capture = std::thread::spawn(move || capture(stdout_pipe));
    let stderr_capture = std::thread::spawn(move || capture(stderr_pipe));

    // Ctrl+C: the child shares our process group, so the terminal delivers
    // SIGINT to both of us directly. We die with 128+2, the child gets its
    // signal, and no orphans are left behind.
    let show_start = Instant::now();
    let animate = std::io::stderr().is_terminal();
    let style = Style::detect();
    if animate {
        rainbow::prepare_terminal(&style);
    }
    let mut status: Option<ExitStatus> = None;
    let mut child_elapsed = 0.0;
    let mut segment = usize::MAX;
    let mut message = "";
    let mut progress;
    loop {
        if status.is_none() {
            if let Some(exit) = child.try_wait().expect("child should be waitable") {
                status = Some(exit);
                child_elapsed = show_start.elapsed().as_secs_f64();
            }
        }
        let elapsed = show_start.elapsed().as_secs_f64();
        let current = plan.segment_at(elapsed);
        if current != segment {
            segment = current;
            if plan.segments[current].is_stall() {
                message = comfort.draw(&mut rng);
            }
        }
        progress = plan.progress(elapsed);
        if animate {
            render(&style, progress, elapsed / plan.tick, message);
        }
        if status.is_some() && elapsed >= plan.min_show {
            break;
        }
        let wait = if status.is_some() {
            (plan.min_show - elapsed).clamp(0.0, plan.tick)
        } else {
            plan.tick
        };
        std::thread::sleep(Duration::from_secs_f64(wait));
    }

    let Some(status) = status else {
        unreachable!("the loop breaks only after the child exits");
    };
    let code = exit_code(&status);
    let success = status.success();

    if animate {
        if success {
            // The victory lap: sprint from wherever we are to 100%.
            let step = 100.0 / BAR_WIDTH as f64;
            while progress < 100.0 {
                progress = (progress + step).min(100.0);
                let phase = show_start.elapsed().as_secs_f64() / plan.tick;
                render(&style, progress, phase, message);
                std::thread::sleep(Duration::from_secs_f64(plan.fill_frame));
            }
        }
        // Retire the bar line: on failure it stays parked where it stalled.
        eprintln!();
    }

    let (stdout, stdout_truncated) = stdout_capture.join().expect("stdout capture");
    let (stderr, stderr_truncated) = stderr_capture.join().expect("stderr capture");
    replay(&stdout, stdout_truncated, language, true);
    replay(&stderr, stderr_truncated, language, false);
    println!("{}", result_line(language, success, child_elapsed, code));
    code
}

fn build_command(language: Language) -> ClapCommand {
    ClapCommand::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(language.text(
            "A fake progress bar for a real command",
            "给真实命令配一条假进度条",
        ))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .help_template(language.text(
            "{name} {version}\n{about}\n\nUsage: {usage}\n\nOptions:\n{options}",
            "{name} {version}\n{about}\n\n用法：{name} [选项] <命令> [参数...]\n\n选项：\n{options}",
        ))
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help(language.text("Print help", "显示帮助")),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .alias("verison")
                .action(ArgAction::Version)
                .help(language.text("Print version", "显示版本")),
        )
        .arg(
            Arg::new("command")
                .num_args(1..)
                .trailing_var_arg(true)
                .help(language.text(
                    "Command to run (and wait for)",
                    "要运行（并等待）的命令",
                )),
        )
}

/// Read a pipe to the end, keeping at most `OUTPUT_CAP` bytes.
fn capture(mut pipe: impl Read) -> (Vec<u8>, bool) {
    let mut out: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let remaining = OUTPUT_CAP - out.len();
                out.extend_from_slice(&buffer[..read.min(remaining)]);
                if out.len() >= OUTPUT_CAP {
                    // Drain the rest so the child never blocks on a full pipe.
                    while matches!(pipe.read(&mut buffer), Ok(n) if n > 0) {}
                    return (out, true);
                }
            }
        }
    }
    let truncated = out.len() >= OUTPUT_CAP;
    (out, truncated)
}

/// Replay captured child output after the show, with a note if truncated.
fn replay(bytes: &[u8], truncated: bool, language: Language, to_stdout: bool) {
    if !bytes.is_empty() {
        let written = if to_stdout {
            std::io::stdout().write_all(bytes)
        } else {
            std::io::stderr().write_all(bytes)
        };
        written.ok();
        if !bytes.ends_with(b"\n") {
            if to_stdout {
                println!();
            } else {
                eprintln!();
            }
        }
    }
    if truncated {
        let note = language.text(
            "(child output exceeded 1 MiB and was truncated)",
            "（子命令输出超过 1MiB，已截断）",
        );
        if to_stdout {
            println!("{note}");
        } else {
            eprintln!("{note}");
        }
    }
}

fn render(style: &Style, progress: f64, phase: f64, message: &str) {
    let line = rainbow::render_line(style, progress, phase, message);
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(line.as_bytes()).ok();
    stderr.flush().ok();
}

fn result_line(language: Language, success: bool, elapsed: f64, code: i32) -> String {
    if success {
        language
            .text(
                &format!(
                    "✓ Done! Every second you waited was worth it ({elapsed:.1}s, exit {code})"
                ),
                &format!("✓ 成了！你等的每一秒都值得（{elapsed:.1}s，exit {code}）"),
            )
            .to_owned()
    } else {
        language
            .text(
                &format!("✗ It failed. You tried, and that counts. ({elapsed:.1}s, exit {code})"),
                &format!("✗ 没成。但尽力了就好，结果不定义你（{elapsed:.1}s，exit {code}）"),
            )
            .to_owned()
    }
}

#[cfg(unix)]
fn exit_code(status: &ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

#[cfg(not(unix))]
fn exit_code(status: &ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}
