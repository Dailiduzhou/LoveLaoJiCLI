//! sprinkle-specific failures; shared failures come from cli-common.
//!
//! Every variant is a conservative guard: creation and undo refuse to touch a
//! worktree or ref whose ownership they cannot prove.
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
            "Source checkout must be clean, including untracked files",
            "源工作区必须干净，包括未跟踪文件"
        )
    )]
    DirtySource,

    #[error("{}", message("Git operation in progress", "Git 操作尚未结束"))]
    GitOperationInProgress,

    #[error(
        "{}",
        message("Files use checkout filters", "文件使用 checkout 过滤器")
    )]
    CheckoutFilters,

    #[error("{}", message("No worktree parent", "工作区没有父目录"))]
    NoWorktreeParent,

    #[error("{}", message("Target already exists", "目标已存在"))]
    TargetExists,

    #[error("{}", message("HEAD checkout is not clean", "HEAD 副本不干净"))]
    HeadCheckoutDirty,

    #[error("{}", message("File changed before insertion", "插入前文件发生变化"))]
    FileChangedBeforeInsertion,

    #[error("{}", message("Incomplete session metadata", "会话元数据不完整"))]
    IncompleteMetadata,

    #[error("{}", message("No managed session here", "当前工作区没有受管理的会话"))]
    NoManagedSession,

    #[error("{}", message("Session identity mismatch", "会话身份不匹配"))]
    IdentityMismatch,

    #[error("{}", message("Invalid managed branch ref", "受管理的分支引用无效"))]
    InvalidRef,

    #[error(
        "{}",
        message("Insertion patch is not complete", "插入补丁尚未完整生成")
    )]
    PatchIncomplete,

    #[error(
        "{}",
        message(
            "Return to the original workspace before undo",
            "请回到原工作区后再撤销"
        )
    )]
    ReturnToSource,

    #[error("{}", message("Ref ownership changed", "分支归属变化"))]
    RefOwnershipChanged,

    #[error(
        "{}",
        message(
            "Incomplete transaction: ownership cannot be fully verified; inspect manually",
            "事务未完成，无法完整验证归属；请人工检查"
        )
    )]
    IncompleteTransaction,

    #[error(
        "{}",
        message("Unplanned user changes; refusing undo", "存在未登记修改；拒绝撤销")
    )]
    UnplannedChanges,

    #[error("{}", message("Insertion no longer matches", "插入内容不再匹配"))]
    InsertionMismatch,

    #[error("{}", message("Original fingerprint mismatch", "原始指纹不匹配"))]
    OriginalFingerprintMismatch,

    #[error(
        "{}",
        message("Restored worktree is not clean", "恢复后的工作区不干净")
    )]
    RestoredWorktreeDirty,

    #[error(
        "{}",
        message(
            "Partial worktree removal; inspect manually",
            "工作区仅部分移除；请人工检查"
        )
    )]
    PartialRemoval,

    #[error(
        "{}",
        message("Target reappeared after removal", "目标在移除后重新出现")
    )]
    TargetReappeared,

    #[error(
        "{}",
        message("Resources are still registered/in use", "资源仍被登记或使用")
    )]
    ResourcesRegistered,

    #[error("{}", message("Ref identity changed", "分支身份变化"))]
    RefIdentityChanged,

    #[error(
        "{}",
        message("Managed ref disappeared unexpectedly", "受管理的分支意外消失")
    )]
    RefDisappeared,

    #[error("{}", message("Worktree path changed", "工作区路径变化"))]
    WorktreePathChanged,

    #[error(
        "{}",
        message(
            "Worktree/ref/index identity changed; refusing undo",
            "工作区、分支或索引变化；拒绝撤销"
        )
    )]
    WorktreeIdentityChanged,

    #[error("{}", message("Worktree is locked", "工作区已锁定"))]
    WorktreeLocked,

    #[error("{}", message("Worktree backlink mismatch", "工作区反向关联不匹配"))]
    BacklinkMismatch,

    #[error("{}", message("Worktree registration mismatch", "工作区登记不匹配"))]
    RegistrationMismatch,

    #[error(
        "{}",
        message(
            "Extra untracked/ignored files; preserve your work before undo",
            "存在额外未跟踪或忽略文件；请先保存额外工作"
        )
    )]
    ExtraUntrackedFiles,

    #[error("{}", message("File mode changed", "文件模式发生变化"))]
    FileModeChanged,

    #[error(
        "{}",
        message("User edits detected; refusing undo", "检测到用户修改；拒绝撤销")
    )]
    UserEditsDetected,
}

pub(super) type Result<T> = std::result::Result<T, Error>;
