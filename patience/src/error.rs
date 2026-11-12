//! patience-specific failures only. The child's own outcome is never an error:
//! its exit code and streams pass through unchanged.

use cli_common::{message, Error as Common};

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error(transparent)]
    Common(#[from] Common),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(
        "{}",
        message(
            "Child output reader stopped unexpectedly",
            "子命令输出读取线程异常退出"
        )
    )]
    OutputReader,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
