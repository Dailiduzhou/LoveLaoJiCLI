use std::process::{Command, Output};

fn invoke(args: &[&str], locales: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command.args(args);
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        command.env_remove(key);
    }
    command.env("NO_COLOR", "1");
    command.envs(locales.iter().copied());
    command.output().expect("CLI should run")
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn bare_command_is_localized() {
    for (locale, expected) in [("en_US.UTF-8", ENGLISH), ("zh_CN.UTF-8", CHINESE)] {
        assert_eq!(
            success(invoke(&[], &[("LANG", locale)])),
            format!("{expected}\n")
        );
    }
}

#[test]
fn help_is_localized() {
    for flag in ["--help", "-h"] {
        for (locale, heading, help, version, message) in [
            ("en-US", "Usage:", "Print help", "Print version", ENGLISH),
            ("zh-CN", "用法：", "显示帮助", "显示版本", CHINESE),
        ] {
            let text = success(invoke(&[flag], &[("LANG", locale)]));
            for expected in [NAME, heading, help, version, message, "--help", "--version"] {
                assert!(text.contains(expected), "missing {expected:?} in {text:?}");
            }
        }
    }
}

#[test]
fn version_supports_standard_short_and_legacy_flags() {
    for locale in ["en_US", "zh_CN"] {
        for flag in ["--version", "-V", "--verison"] {
            let text = success(invoke(&[flag], &[("LANG", locale)]));
            assert_eq!(text, format!("{NAME} {}\n", env!("CARGO_PKG_VERSION")));
        }
    }
}

#[test]
fn locale_precedence_and_fallback() {
    for (locales, expected) in [
        (vec![], ENGLISH),
        (vec![("LANG", "fr_FR.UTF-8")], ENGLISH),
        (vec![("LC_ALL", "C"), ("LANG", "zh_CN")], ENGLISH),
        (vec![("LC_ALL", "zh_CN"), ("LC_MESSAGES", "en_US")], CHINESE),
        (vec![("LC_MESSAGES", "zh_CN"), ("LANG", "en_US")], CHINESE),
        (
            vec![("LC_ALL", ""), ("LC_MESSAGES", ""), ("LANG", "zh_CN")],
            CHINESE,
        ),
    ] {
        assert_eq!(success(invoke(&[], &locales)), format!("{expected}\n"));
    }
}

#[test]
fn rejects_unknown_options_and_positional_arguments() {
    for argument in ["--unknown", "unexpected"] {
        let output = invoke(&[argument], &[("LANG", "en_US")]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}
