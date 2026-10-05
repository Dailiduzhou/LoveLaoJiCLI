//! Shared locale/CLI scaffolding and tool-independent runtime infrastructure.

use clap::{Arg, ArgAction, Command};

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

/// Run a CLI with localized help and a single message when invoked without arguments.
pub fn run(
    name: &'static str,
    version: &'static str,
    english: &'static str,
    chinese: &'static str,
) {
    let language = Language::detect();
    let message = language.text(english, chinese);
    Command::new(name)
        .version(version)
        .about(message)
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
    println!("{message}");
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

pub type Result<T> = std::io::Result<T>;
pub fn error(en: &str, zh: &str) -> std::io::Error {
    std::io::Error::other(Language::detect().text(en, zh).to_owned())
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
