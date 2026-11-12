//! encourage-specific failures; shared failures come from cli-common.
use crate::{source::SourceError, GenerationError};
use cli_common::{message, Error as Common};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Common(#[from] Common),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(
        "{}: {source}",
        message("Could not build an encouragement", "无法生成鼓励语句")
    )]
    Generation {
        #[from]
        source: GenerationError,
    },

    #[error(
        "{}: {source}",
        message("Embedded replay source is invalid", "内嵌回放源码无效")
    )]
    Source {
        #[from]
        source: SourceError,
    },

    #[error(
        "{}",
        message("The terminal is no longer in the foreground", "终端已不在前台")
    )]
    NotForeground,

    #[error(
        "{}",
        message(
            "The interactive terminal became unavailable during replay",
            "回放期间交互终端不可用"
        )
    )]
    TerminalUnavailable,
}

pub type Result<T> = std::result::Result<T, Error>;

/// `rustix` reports syscall failures as `Errno`; terminal setup/restore uses `?`
/// on those results directly.
impl From<rustix::io::Errno> for Error {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Common(Common::from(error))
    }
}
