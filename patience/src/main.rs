//! patience: a fake progress bar for a real command.
//!
//! `patience <command> [args...]` runs the command and performs a whole show
//! around it: a rainbow marquee bar with random speed curves, repeated stalls
//! with localized comfort messages, an eternal stall near 99%, and a rapid
//! fill to 100% only if the child succeeds. Each extra leading `patience`
//! token adds one more independently performing bar to the same show. The
//! exit code passes through.

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
use rainbow::{Style, PERIOD};

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let language = Language::detect();
    let matches = build_command(language).get_matches();
    let tokens: Vec<String> = matches
        .get_many::<String>("command")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    // Leading `patience` tokens never become the child: each one adds another
    // bar to the show instead, and the real command starts after them.
    let (bars, command) = split_bars(&tokens);
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
    // Every bar performs its own independent show, reproducible from the one
    // seed. Timings are shared because generate() derives them from `scale`.
    let mut shows: Vec<Show> = (0..bars)
        .map(|index| Show::new(language, derive_seed(seed, index), scale))
        .collect();
    let tick = shows[0].plan.tick;
    let fill_frame = shows[0].plan.fill_frame;
    // The show runs at least until the slowest bar's script allows an exit.
    let min_show = shows
        .iter()
        .map(|show| show.plan.min_show)
        .fold(0.0, f64::max);

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
    let mut redrawn = false;
    loop {
        if status.is_none() {
            if let Some(exit) = child.try_wait().expect("child should be waitable") {
                status = Some(exit);
                child_elapsed = show_start.elapsed().as_secs_f64();
            }
        }
        let elapsed = show_start.elapsed().as_secs_f64();
        for show in &mut shows {
            show.advance(elapsed);
        }
        if animate {
            render(&style, &shows, elapsed / tick, redrawn);
            redrawn = true;
        }
        if status.is_some() && elapsed >= min_show {
            break;
        }
        let wait = if status.is_some() {
            (min_show - elapsed).clamp(0.0, tick)
        } else {
            tick
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
            // The victory lap: every bar sprints from wherever it is to 100%.
            let step = 100.0 / BAR_WIDTH as f64;
            while shows.iter().any(|show| show.progress < 100.0) {
                for show in &mut shows {
                    show.progress = (show.progress + step).min(100.0);
                }
                let phase = show_start.elapsed().as_secs_f64() / tick;
                render(&style, &shows, phase, true);
                std::thread::sleep(Duration::from_secs_f64(fill_frame));
            }
        }
        // Retire the bars: on failure they stay parked where they stalled.
        eprintln!();
    }

    let (stdout, stdout_truncated) = stdout_capture.join().expect("stdout capture");
    let (stderr, stderr_truncated) = stderr_capture.join().expect("stderr capture");
    replay(&stdout, stdout_truncated, language, true);
    replay(&stderr, stderr_truncated, language, false);
    println!("{}", result_line(language, success, child_elapsed, code));
    code
}

/// One bar's whole independent performance, bound to the shared child: its
/// own random script, its own comfort deck, its own message and progress.
struct Show {
    plan: plan::Plan,
    comfort: Comfort,
    rng: StdRng,
    message: &'static str,
    segment: usize,
    progress: f64,
}

impl Show {
    fn new(language: Language, seed: u64, scale: f64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let plan = plan::generate(&mut rng, scale);
        let comfort = Comfort::new(language, &mut rng);
        Self {
            plan,
            comfort,
            rng,
            message: "",
            segment: usize::MAX,
            progress: 0.0,
        }
    }

    /// Play the show's script to `elapsed` seconds, drawing a new comfort
    /// message whenever it enters a stall.
    fn advance(&mut self, elapsed: f64) {
        let current = self.plan.segment_at(elapsed);
        if current != self.segment {
            self.segment = current;
            if self.plan.segments[current].is_stall() {
                self.message = self.comfort.draw(&mut self.rng);
            }
        }
        self.progress = self.plan.progress(elapsed);
    }
}

/// Special case for nesting: leading `patience` tokens add one bar each
/// instead of becoming the child. Returns the bar count and the real command
/// that follows them (an empty rest means there is nothing to wait for).
fn split_bars(tokens: &[String]) -> (usize, &[String]) {
    let nested = tokens
        .iter()
        .take_while(|token| token.as_str() == "patience")
        .count();
    (nested + 1, &tokens[nested..])
}

/// Mix the run seed with a bar's index (splitmix64 finalizer) so every bar
/// gets an independent yet reproducible random stream from the one seed.
fn derive_seed(seed: u64, index: usize) -> u64 {
    let mut z = seed ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1AE4_1BC9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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

/// One frame of every bar, stacked and refreshed in place: the first frame
/// paints all the lines, later frames climb back up with a cursor escape and
/// repaint. Each bar's rainbow phase is spread across the palette so stacked
/// bars never look like copies.
fn render(style: &Style, shows: &[Show], phase: f64, redrawn: bool) {
    let bars = shows.len();
    let mut frame = String::new();
    if redrawn && bars > 1 {
        frame.push_str(&format!("\u{1b}[{}A", bars - 1));
    }
    for (index, show) in shows.iter().enumerate() {
        if index > 0 {
            frame.push('\n');
        }
        let offset = index as f64 * PERIOD / bars as f64;
        frame.push_str(&rainbow::render_line(
            style,
            show.progress,
            phase + offset,
            show.message,
        ));
    }
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(frame.as_bytes()).ok();
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

#[cfg(test)]
mod tests {
    use super::{derive_seed, split_bars};

    #[test]
    fn leading_patience_tokens_add_bars_not_children() {
        let cases: &[(&[&str], usize, &[&str])] = &[
            (&[], 1, &[]),
            (&["sleep", "3"], 1, &["sleep", "3"]),
            (&["patience", "sleep", "3"], 2, &["sleep", "3"]),
            (&["patience", "patience", "ls", "-la"], 3, &["ls", "-la"]),
            (&["patience", "patience", "patience"], 4, &[]),
            (&["patience", "--help"], 2, &["--help"]),
            (&["./patience", "sleep"], 1, &["./patience", "sleep"]),
            (&["Patience", "sleep"], 1, &["Patience", "sleep"]),
        ];
        for &(input, bars, rest) in cases {
            let tokens: Vec<String> = input.iter().map(|token| token.to_string()).collect();
            let (got_bars, got_rest) = split_bars(&tokens);
            assert_eq!(got_bars, bars, "input {input:?}");
            let expected: Vec<String> = rest.iter().map(|token| token.to_string()).collect();
            assert_eq!(got_rest, expected.as_slice(), "input {input:?}");
        }
    }

    #[test]
    fn bar_seeds_are_stable_and_distinct() {
        // Pinned seed-mixer vectors, not a function compared with itself.
        for (index, expected) in [
            0xb5c5_555b_b913_a4ed,
            0x67ac_3b30_0ad4_f69b,
            0x63f9_40b8_3b47_865b,
            0x81d7_d769_a4d6_9a96,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(derive_seed(7, index), expected, "bar {index}");
        }
        let seeds: std::collections::HashSet<_> =
            (0..8).map(|index| derive_seed(7, index)).collect();
        assert_eq!(seeds.len(), 8, "every pair of bars needs a distinct stream");
        assert_ne!(derive_seed(7, 1), derive_seed(8, 1));
    }
}
