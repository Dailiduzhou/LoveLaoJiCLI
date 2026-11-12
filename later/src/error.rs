//! later-specific failures; shared failures come from cli-common.
use cli_common::{message, Error as Common};

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error(transparent)]
    Common(#[from] Common),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{}", message("Invalid session card", "上下文卡片无效"))]
    InvalidCard,

    #[error("{}", message("Invalid Git status", "Git 状态无效"))]
    InvalidGitStatus,

    #[error(
        "{}",
        message("Incomplete Git rename information", "Git 重命名信息不完整")
    )]
    IncompleteRename,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
