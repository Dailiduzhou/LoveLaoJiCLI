//! one-specific failures; shared failures come from cli-common.
use cli_common::{message, Error as Common};

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error(transparent)]
    Common(#[from] Common),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{}", message("Task limit reached (1000)", "任务数量已达上限（1000）"))]
    TaskLimit,

    #[error(
        "{}",
        message("Invalid task record; nothing changed", "任务记录无效；未作更改")
    )]
    InvalidRecord,

    #[error("{}", message("Input exceeds 1 MiB", "输入超过 1 MiB"))]
    InputTooLarge,

    #[error("{}", message("Input must be UTF-8", "输入必须为 UTF-8"))]
    InputNotUtf8,

    #[error(
        "{}",
        message(
            "Invalid task: use a single line of at most 2000 characters",
            "任务无效：请使用最多 2000 字符的单行文本"
        )
    )]
    InvalidTaskText,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
