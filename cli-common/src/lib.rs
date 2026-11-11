//! Shared locale/CLI scaffolding and tool-independent runtime infrastructure.

use clap::{Arg, ArgAction, Command};
use std::io::Write;

pub mod error;
pub use error::{message, Error, Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    fn from_locales(locales: impl IntoIterator<Item = Option<String>>) -> Self {
        let locale = locales
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty())
            .unwrap_or_default();
        let base = locale.split(['.', '@']).next().unwrap_or_default();
        match base.replace('-', "_").to_ascii_lowercase().as_str() {
            "zh" | "zh_cn" => Self::Chinese,
            _ => Self::English,
        }
    }

    pub fn detect() -> Self {
        Self::from_locales(["LC_ALL", "LC_MESSAGES", "LANG"].map(|key| std::env::var(key).ok()))
    }

    /// Pick the English or Chinese variant of a text pair.
    pub fn text<'a>(self, english: &'a str, chinese: &'a str) -> &'a str {
        match self {
            Self::English => english,
            Self::Chinese => chinese,
        }
    }
}

/// Run a single-message CLI with localized help: `--help`/`--version`, or the
/// one blessing. A failed stdout write (closed pipe, full disk) is a
/// controlled exit 1 rather than a panic.
pub fn run(
    name: &'static str,
    version: &'static str,
    english: &'static str,
    chinese: &'static str,
) -> std::process::ExitCode {
    let language = Language::detect();
    let text = language.text(english, chinese);
    Command::new(name)
        .version(version)
        .about(text)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .help_template(language.text(
            "{name} {version}\n{about}\n\nUsage: {usage}\n\nOptions:\n{options}",
            "{name} {version}\n{about}\n\n用法：{name} [选项]\n\n选项：\n{options}",
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
        .get_matches();
    let mut out = std::io::stdout().lock();
    match writeln!(out, "{text}").and_then(|()| out.flush()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            report_error(name, &e);
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(unix)]
pub mod git;
#[cfg(unix)]
pub mod process;
#[cfg(unix)]
pub mod repeat;
#[cfg(unix)]
pub mod state;

/// Extensible scaffolding; the blessing tools intentionally retain their exact help layout.
pub fn command(name: &'static str, about: &'static str) -> Command {
    let l = Language::detect();
    Command::new(name)
        .disable_help_subcommand(true)
        .subcommand_help_heading(l.text("Commands", "子命令"))
        .next_help_heading(l.text("Options", "选项"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
        .disable_help_flag(true)
        .disable_version_flag(true)
        // Both locales interpolate clap's `{usage}`: it is the only source of
        // subcommands and required positionals, and it matches the usage clap
        // prints with parse errors. A hand-written "[选项]" would silently hide
        // `later <COMMAND>` or `afk <duration>`. tests/human_support.rs holds
        // both locales to the same usage body.
        .help_template(l.text(
            "{name} {version}\n{about}\n\nUsage: {usage}\n\n{all-args}",
            "{name} {version}\n{about}\n\n用法：{usage}\n\n{all-args}",
        ))
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help(l.text("Print help", "显示帮助")),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .alias("verison")
                .action(ArgAction::Version)
                .help(l.text("Print version", "显示版本")),
        )
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Map a child or synthetic status (0..=255) to a process exit code.
pub fn exit_status(code: i32) -> std::process::ExitCode {
    debug_assert!(
        (0..=255).contains(&code),
        "exit status {code} is outside the documented 0..=255 range"
    );
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// Print the one `{tool}: {error}` diagnostic on stderr. A failed stderr write
/// (closed pipe, full disk) is not a second failure: it must never panic.
pub fn report_error(tool: &str, error: &impl std::fmt::Display) {
    let mut err = std::io::stderr().lock();
    let _ = writeln!(err, "{tool}: {}", display(error.to_string()));
}

/// Escape terminal controls, including newlines in filenames and user text.
pub fn display(value: impl AsRef<std::ffi::OsStr>) -> String {
    value
        .as_ref()
        .to_string_lossy()
        .chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

pub fn text_input(value: &str, max: usize) -> Option<String> {
    let text = value.trim();
    (!text.is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
}

/// Deterministic RNG for a hidden `<TOOL>_SEED` test hook. An unset or invalid
/// value falls back to the operating system's entropy source.
pub fn seeded_rng(variable: &str) -> rand::rngs::StdRng {
    use rand::SeedableRng;
    match std::env::var(variable)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        Some(seed) => rand::rngs::StdRng::seed_from_u64(seed),
        None => rand::rngs::StdRng::from_entropy(),
    }
}

/// Clap value parser for one trimmed, nonempty, control-free single line.
pub fn bounded_text(max: usize) -> impl Fn(&str) -> std::result::Result<String, Message> + Clone {
    move |value| {
        text_input(value, max).ok_or_else(|| {
            Message::new(
                format!("Expected a nonempty single line, at most {max} characters"),
                format!("请输入非空单行文本，最多 {max} 个字符"),
            )
        })
    }
}

#[cfg(all(unix, feature = "line-input"))]
mod line_input;
#[cfg(feature = "local-time")]
pub mod local_time;

#[cfg(test)]
mod tests {
    use super::Language;

    fn language(locales: &[&str]) -> Language {
        Language::from_locales(locales.iter().map(|value| Some((*value).to_owned())))
    }

    #[test]
    fn recognizes_chinese_locale_forms() {
        for locale in [
            "zh_CN",
            "zh_CN.UTF-8",
            "zh-CN",
            "ZH-cn",
            "zh_CN@modifier",
            "zh",
        ] {
            assert_eq!(language(&[locale]), Language::Chinese, "{locale}");
        }
    }

    #[test]
    fn first_nonempty_locale_wins() {
        assert_eq!(language(&["en_US", "zh_CN", "zh_CN"]), Language::English);
        assert_eq!(language(&["", "zh_CN", "en_US"]), Language::Chinese);
        assert_eq!(language(&["", "", "zh_CN"]), Language::Chinese);
        assert_eq!(
            Language::from_locales([None, Some("zh_CN".to_owned())]),
            Language::Chinese
        );
    }

    #[test]
    fn defaults_to_english() {
        for locale in [
            "",
            "C",
            "POSIX",
            "C.UTF-8",
            "en-US",
            "en_US.UTF-8",
            "fr_FR",
            "zh_TW",
        ] {
            assert_eq!(language(&[locale]), Language::English, "{locale}");
        }
        assert_eq!(Language::from_locales([]), Language::English);
    }
}
