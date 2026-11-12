//! Validated interactive playback budget; never part of the generation seed.

use cli_common::Language;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackDuration(Duration);

impl PlaybackDuration {
    pub const DEFAULT: Self = Self(Duration::from_secs(15));

    pub fn parse(value: &str) -> Result<Self, String> {
        let seconds = value.strip_suffix('s').filter(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        });
        seconds
            .and_then(|digits| digits.parse::<u64>().ok())
            .filter(|seconds| (10..=600).contains(seconds))
            .map(|seconds| Self(Duration::from_secs(seconds)))
            .ok_or_else(|| {
                Language::detect()
                    .text(
                        "Expected integer seconds from 10s to 600s",
                        "请输入 10s 到 600s 的整数秒数",
                    )
                    .to_owned()
            })
    }

    pub const fn get(self) -> Duration {
        self.0
    }
}

impl Default for PlaybackDuration {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bounded_integer_seconds_are_accepted() {
        assert_eq!(PlaybackDuration::default().get(), Duration::from_secs(15));
        for (value, seconds) in [("10s", 10), ("15s", 15), ("30s", 30), ("600s", 600)] {
            assert_eq!(
                PlaybackDuration::parse(value).unwrap().get(),
                Duration::from_secs(seconds)
            );
        }
        for value in [
            "",
            "s",
            "0s",
            "9s",
            "601s",
            "15",
            "15S",
            "1m",
            "1h",
            "10.5s",
            "+15s",
            "-15s",
            " 15s",
            "15s ",
            "十s",
            "💤",
            "18446744073709551616s",
        ] {
            assert!(PlaybackDuration::parse(value).is_err(), "{value}");
        }
    }
}
