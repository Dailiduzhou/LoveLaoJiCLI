use clap::{builder::StringValueParser, Arg};
use cli_common::{command, display, Language};
use encourage::{duration::PlaybackDuration, pipeline, rng, tui};
use std::io::{self, Write};

fn main() {
    match run() {
        Ok(Exit::Success) => {}
        Ok(Exit::Usage) => std::process::exit(2),
        Ok(Exit::Signal(signal)) => {
            // The TUI has already restored its terminal and dropped all guards.
            let _ = signal_hook::low_level::emulate_default_handler(signal);
            std::process::exit(128 + signal);
        }
        Err(error) => {
            let language = Language::detect();
            let _ = writeln!(
                io::stderr().lock(),
                "encourage: {}: {}",
                language.text("Could not complete encouragement", "未能完成鼓励"),
                display(error.to_string())
            );
            std::process::exit(1);
        }
    }
}

enum Exit {
    Success,
    Usage,
    Signal(i32),
}

fn run() -> io::Result<Exit> {
    let language = Language::detect();
    let matches = command(
        "encourage",
        language.text(
            "Build a gentle sentence from word fragments; text is seed material, not a question.",
            "用短词片段生成一句温柔鼓励；文本只作种子材料，不作语义解读。",
        ),
    )
    .arg(
        Arg::new("text")
            .value_name("TEXT")
            .num_args(1..)
            .value_parser(StringValueParser::new())
            .help(language.text(
                "Optional UTF-8 seed text (argument boundaries are preserved); absent: system randomness",
                "可选 UTF-8 种子文本（保留参数边界）；不传时使用系统随机源",
            )),
    )
    .arg(
        Arg::new("duration")
            .long("duration")
            .value_name("SECONDS")
            .default_value("15s")
            .value_parser(PlaybackDuration::parse)
            .help(language.text(
                "Interactive playback: integer seconds 10s..600s; pauses excluded, plain output never waits",
                "交互播放时长：整数秒 10s..600s；暂停不计时，普通输出不等待",
            )),
    )
    .try_get_matches();
    let matches = match matches {
        Ok(matches) => matches,
        Err(error) if !error.use_stderr() => {
            // Includes help/version. Neither path samples entropy or opens a TTY.
            error.print()?;
            return Ok(Exit::Success);
        }
        Err(_) => {
            // Do not echo untrusted seed text, terminal controls or invalid UTF-8
            // from Clap's detailed diagnostics. Keep the usage error localized.
            writeln!(
                io::stderr().lock(),
                "encourage: {}",
                language.text(
                    "Invalid arguments; use UTF-8 text, --duration 10s..600s, or '--' before option-like text. See 'encourage --help'.",
                    "参数无效；请使用 UTF-8 文本、--duration 10s..600s；选项形式的文本前请加 '--'。详见 'encourage --help'。",
                )
            )?;
            return Ok(Exit::Usage);
        }
    };
    let duration = *matches
        .get_one::<PlaybackDuration>("duration")
        .expect("default duration");
    let args: Vec<_> = matches
        .get_many::<String>("text")
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let seed = rng::seed(&args).map_err(|_| {
        io::Error::other(language.text("System randomness unavailable", "系统随机源不可用"))
    })?;

    let sentence = if tui::is_interactive() {
        let generation = pipeline::generate(seed, language).map_err(io::Error::other)?;
        match tui::run(&generation, language, duration)? {
            tui::Outcome::Plain | tui::Outcome::Confirmed => generation.sentence,
            tui::Outcome::Cancelled => return Ok(Exit::Success),
            tui::Outcome::Signal(signal) => return Ok(Exit::Signal(signal)),
        }
    } else {
        pipeline::generate_plain(seed, language).map_err(io::Error::other)?
    };
    let mut out = io::stdout().lock();
    writeln!(out, "{sentence}")?;
    out.flush()?;
    Ok(Exit::Success)
}
