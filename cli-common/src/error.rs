//! Typed, localized errors shared by every tool.
//!
//! Diagnostics are bilingual: the locale is resolved when the error is shown,
//! so one error value serves both supported languages. Variants are grouped by
//! the subsystem that raises them (state, Git, repeat protocol, local time).

use crate::Language;
use std::borrow::Cow;
use std::fmt;

/// A bilingual diagnostic, rendered in the process locale when displayed.
#[derive(Debug, Clone)]
pub struct Message {
    en: Cow<'static, str>,
    zh: Cow<'static, str>,
}

impl Message {
    pub fn new(en: impl Into<Cow<'static, str>>, zh: impl Into<Cow<'static, str>>) -> Self {
        Self {
            en: en.into(),
            zh: zh.into(),
        }
    }

    pub fn text(&self, language: Language) -> &str {
        language.text(&self.en, &self.zh)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.text(Language::detect()))
    }
}

impl std::error::Error for Message {}

/// Static bilingual diagnostic shorthand, usable inside `#[error(...)]`.
pub const fn message(en: &'static str, zh: &'static str) -> Message {
    Message {
        en: Cow::Borrowed(en),
        zh: Cow::Borrowed(zh),
    }
}

/// Every failure cli-common can report. Tool crates wrap this in their own
/// error type and add tool-specific variants.
///
/// Tool enums also declare `Io(#[from] std::io::Error)` next to
/// `Common(#[from] Error)`. Rust never chains `From` impls, so `?` on an
/// `io::Result` inside a tool needs its own conversion; both variants render
/// transparently, so the text is identical either way. The duplication is
/// therefore intentional. Because of it, `is_not_found` below only sees
/// cli-common-created errors; a tool that needs the kind matches on its own
/// `io::Result` instead.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// A localized diagnostic that does not need its own variant.
    #[error("{}", .0)]
    Diagnostic(#[from] Message),

    // --- private state store -------------------------------------------------
    #[error(
        "{}",
        message(
            "Symlink in managed state path; check HOME/XDG_STATE_HOME for symlink components",
            "状态路径中存在符号链接；请检查 HOME/XDG_STATE_HOME 是否包含符号链接"
        )
    )]
    SymlinkInStatePath,

    #[error("{}", message("Invalid state path", "状态路径无效"))]
    InvalidStatePath,

    #[error(
        "{}",
        message(
            "State directory must be owned by you and mode 0700",
            "状态目录必须归当前用户所有且权限为 0700"
        )
    )]
    StateDirectoryPermissions,

    #[error(
        "{}",
        message("Unsafe state parent permissions", "状态父目录权限不安全")
    )]
    UnsafeStateParentPermissions,

    #[error(
        "{}",
        message("Unsafe state file permissions/type", "状态文件权限或类型不安全")
    )]
    UnsafeStateFile,

    #[error("{}", message("Unsafe state directory", "状态目录不安全"))]
    UnsafeStateDirectory,

    #[error(
        "{}",
        message(
            "State is busy or locking is unsupported",
            "状态忙碌或文件系统不支持锁"
        )
    )]
    StateBusy,

    #[error("{}", message("Unsafe replacement target", "替换目标不安全"))]
    UnsafeReplacementTarget,

    #[error(
        "{}",
        message(
            "No absolute XDG_STATE_HOME or HOME",
            "缺少绝对路径的 XDG_STATE_HOME 或 HOME"
        )
    )]
    MissingStateHome,

    #[error("{}", message("Untrusted state parent directory", "状态父目录不可信"))]
    UntrustedStateParent,

    #[error("{}", message("Invalid identity key", "身份密钥无效"))]
    InvalidIdentityKey,

    #[error(
        "{}",
        message("Operating system randomness is unavailable", "系统随机源不可用")
    )]
    EntropyUnavailable,

    #[error(
        "{}",
        message(
            "Invalid identity key; records are unavailable",
            "身份密钥损坏，记录不可用"
        )
    )]
    DamagedIdentityKey,

    #[error("{}", message("State record too large", "状态记录过大"))]
    StateRecordTooLarge,

    #[error(
        "{}",
        message("Unsupported or damaged state record", "状态记录版本不支持或记录损坏")
    )]
    DamagedStateRecord,

    #[error(
        "{}",
        message("State record path is outside the store", "状态记录路径不在存储目录内")
    )]
    RecordPathOutsideStore,

    // --- local Git probes and snapshots --------------------------------------
    #[error(
        "{}",
        message(
            "Checkout/clean filters are configured; refusing to execute repository code",
            "配置了 checkout/clean 过滤器；拒绝执行仓库相关代码"
        )
    )]
    GitFiltersConfigured,

    #[error("{}", message("Git output exceeded budget", "Git 输出超出预算"))]
    GitOutputBudget,

    #[error("{}", message("Git probe timed out", "Git 探测超时"))]
    GitTimeout,

    #[error("{}", message("Git reader failed", "Git 读取失败"))]
    GitReaderFailed,

    #[error("{}", .0)]
    GitFailed(Message),

    #[error("{}", message("Not a non-bare repository", "不是普通工作区仓库"))]
    NotARepository,

    #[error("{}", message("Invalid index", "索引无效"))]
    InvalidIndex,

    #[error(
        "{}",
        message(
            "Conflicts or submodules are not comparable",
            "冲突或子模块无法可靠比较"
        )
    )]
    IncomparableIndex,

    #[error(
        "{}",
        message("Git HEAD is not valid UTF-8", "Git HEAD 不是有效的 UTF-8")
    )]
    HeadNotUtf8,

    #[error(
        "{}",
        message("Files changed during snapshot", "文件在快照期间发生变化")
    )]
    SnapshotChanged,

    #[error("{}", message("Snapshot entry budget exceeded", "快照条目超出预算"))]
    SnapshotEntryBudget,

    #[error("{}", message("Snapshot timed out", "快照超时"))]
    SnapshotTimeout,

    #[error("{}", message("Snapshot byte budget exceeded", "快照字节超出预算"))]
    SnapshotByteBudget,

    #[error("{}", message("Snapshot file changed", "快照文件变化"))]
    SnapshotFileChanged,

    #[error("{}", message("Special file in snapshot", "快照中存在特殊文件"))]
    SpecialFileInSnapshot,

    #[error("{}", message("Unstable snapshot", "快照不稳定"))]
    UnstableSnapshot,

    #[error("{}", message("Unsafe relative path", "相对路径不安全"))]
    UnsafeRelativePath,

    #[error("{}", message("Symlinked parent", "父目录为符号链接"))]
    SymlinkedParent,

    #[error("{}", message("Not an independent regular file", "不是独立普通文件"))]
    NotIndependentFile,

    #[error("{}", message("File changed while opening", "文件在打开期间变化"))]
    FileChangedWhileOpening,

    #[error("{}", message("File hash budget exceeded", "文件指纹超出预算"))]
    FileHashBudget,

    // --- execution/repeat records -------------------------------------------
    #[error("{}", message("Invalid repeat record", "重复状态记录无效"))]
    InvalidRepeatRecord,

    #[error(
        "{}",
        message(
            "Concurrent invocation changed the repeat state",
            "并发调用改变了重复状态"
        )
    )]
    ConcurrentRepeatChange,

    #[error(
        "{}",
        message("Execution record scan exceeded budget", "执行记录扫描超出预算")
    )]
    RecordScanBudget,

    #[error("{}", message("Invalid execution record type", "执行记录类型无效"))]
    InvalidRecordType,

    #[error(
        "{}",
        message("Execution records exceed 32 MiB", "执行记录超过 32 MiB")
    )]
    RecordsTooLarge,

    #[error("{}", message("Execution record disappeared", "执行记录消失"))]
    RecordDisappeared,

    #[error("{}", message("Invalid execution record", "执行记录无效"))]
    InvalidExecutionRecord,

    #[error("{}", message("Invalid execution outcome", "执行结果无效"))]
    InvalidExecutionOutcome,

    // --- local calendar days -------------------------------------------------
    #[cfg(feature = "local-time")]
    #[error("{}", message("Local time zone is unavailable", "本地时区不可用"))]
    TimeZoneUnavailable(#[source] jiff::Error),

    #[cfg(feature = "local-time")]
    #[error(
        "{}",
        message("Local calendar day cannot be computed", "无法计算本地日历日")
    )]
    LocalDateUnavailable(#[source] jiff::Error),
}

impl Error {
    /// True when the failure is a missing filesystem entry.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
    }
}

impl From<rustix::io::Errno> for Error {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Io(std::io::Error::from(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One value per bilingual variant, so a missing or copy-pasted translation
    /// fails here instead of reaching a user.
    fn bilingual() -> Vec<Error> {
        let mut all = vec![
            Error::Diagnostic(message("localized", "已本地化")),
            Error::SymlinkInStatePath,
            Error::InvalidStatePath,
            Error::StateDirectoryPermissions,
            Error::UnsafeStateParentPermissions,
            Error::UnsafeStateFile,
            Error::UnsafeStateDirectory,
            Error::StateBusy,
            Error::UnsafeReplacementTarget,
            Error::MissingStateHome,
            Error::UntrustedStateParent,
            Error::InvalidIdentityKey,
            Error::EntropyUnavailable,
            Error::DamagedIdentityKey,
            Error::StateRecordTooLarge,
            Error::DamagedStateRecord,
            Error::RecordPathOutsideStore,
            Error::GitFiltersConfigured,
            Error::GitOutputBudget,
            Error::GitTimeout,
            Error::GitReaderFailed,
            Error::GitFailed(Message::new("git failed", "git 失败")),
            Error::NotARepository,
            Error::InvalidIndex,
            Error::IncomparableIndex,
            Error::HeadNotUtf8,
            Error::SnapshotChanged,
            Error::SnapshotEntryBudget,
            Error::SnapshotTimeout,
            Error::SnapshotByteBudget,
            Error::SnapshotFileChanged,
            Error::SpecialFileInSnapshot,
            Error::UnstableSnapshot,
            Error::UnsafeRelativePath,
            Error::SymlinkedParent,
            Error::NotIndependentFile,
            Error::FileChangedWhileOpening,
            Error::FileHashBudget,
            Error::InvalidRepeatRecord,
            Error::ConcurrentRepeatChange,
            Error::RecordScanBudget,
            Error::InvalidRecordType,
            Error::RecordsTooLarge,
            Error::RecordDisappeared,
            Error::InvalidExecutionRecord,
            Error::InvalidExecutionOutcome,
        ];
        #[cfg(feature = "local-time")]
        {
            let broken = "not a timestamp".parse::<jiff::Timestamp>().unwrap_err();
            all.push(Error::TimeZoneUnavailable(broken.clone()));
            all.push(Error::LocalDateUnavailable(broken));
        }
        all
    }

    /// `Message` resolves the locale when it is displayed, so both readings come
    /// from the same values. No other test in this crate reads the locale, and
    /// the variable is restored so `cargo test` under a Chinese shell is unaffected.
    #[test]
    fn every_variant_is_translated() {
        let all = bilingual();
        let previous = std::env::var_os("LC_ALL");
        std::env::set_var("LC_ALL", "C");
        let english: Vec<String> = all.iter().map(ToString::to_string).collect();
        std::env::set_var("LC_ALL", "zh_CN.UTF-8");
        let chinese: Vec<String> = all.iter().map(ToString::to_string).collect();
        match previous {
            Some(value) => std::env::set_var("LC_ALL", value),
            None => std::env::remove_var("LC_ALL"),
        }
        for ((en, zh), error) in english.iter().zip(&chinese).zip(&all) {
            assert!(!en.is_empty() && !zh.is_empty(), "{error:?}");
            assert_ne!(en, zh, "{error:?} has no Chinese translation");
        }
    }
}
