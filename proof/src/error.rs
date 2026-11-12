//! proof-specific failures; shared failures come from cli-common.
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
            "Git evidence is invalid or exceeds its budget",
            "Git 证据无效或超出预算"
        )
    )]
    InvalidEvidence,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
