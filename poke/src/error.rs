//! poke-specific failures; shared failures come from cli-common.
use cli_common::{message, Error as Common};

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error(transparent)]
    Common(#[from] Common),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(
        "{}",
        message("Invalid contact list; nothing changed", "名单无效；未作更改")
    )]
    InvalidList,

    #[error("{}", message("Name limit reached (1000)", "名单已达上限（1000）"))]
    NameLimit,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
