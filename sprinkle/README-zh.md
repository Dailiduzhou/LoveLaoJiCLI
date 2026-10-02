# sprinkle

在隔离的 HEAD 副本中散落少量友善注释。[English](README.md)

```sh
sprinkle                 # 等同 sprinkle worktree
sprinkle diff            # 仅创建时的插入补丁
sprinkle undo            # 先回到原工作区
```

支持 `-h/--help`、`-V/--version` 和兼容拼写 `--verison`。
首版没有 branch 模式、`--force`、密度/主题/语言/seed 参数或 clean。
语言遵循 `LC_ALL → LC_MESSAGES → LANG`。

## 创建与隔离

源仓库必须有 HEAD、非 bare、可写且干净，包括没有非忽略未跟踪文件、冲突、
merge/rebase 等进行中操作。首版保守拒绝子模块，以及配置了 checkout/clean
过滤器的仓库（包括全局 LFS 配置）。每次 Git 调用单独禁用 hooks，不改用户配置；
禁用外部 diff/textconv、fsmonitor、lazy fetch 与网络协议。不执行构建、测试、
包管理脚本，不提交，不 push。

先保存私有 intent，再排他创建 ref 和源目录旁唯一的 0700 worktree。
原 checkout 的文件、索引和分支不变；common dir 必然新增分支和 worktree 登记。
这是 **HEAD 副本，不是当前磁盘的完整备份**，忽略文件不复制。
源快照受 20,000 条目、累计读取 256 MiB、5 秒预算约束；超预算拒绝。
首版不支持只有 reftable 格式 reflog 的仓库，亦不承诺不可靠权限/锁文件系统的安全性。

事务阶段持久化为 prepared → ref-created → worktree-created → planned → active。
清单保存可逆路径字节、HEAD/ref、worktree 双向关联、索引/reflog 摘要、每个文件的
模式及原始/预期指纹、插入偏移/文本/消息 ID、计数与零上下文补丁。
记录经过带密钥校验并绑定保存路径。失败保留阶段和现场并报告位置；归属不明不删除。

## 保守插入

只考虑 HEAD 跟踪的普通 UTF-8 文件：`.md`、`.go`、`.rs`、`.c/.h`、
`.cc/.cpp/.cxx/.hh/.hpp/.hxx`，单文件不超过 1 MiB。
排除目录组件 `.git`、`target`、`vendor`、`node_modules`、`dist`、`build`、
`third_party`、`generated`。跳过符号链接、含 NUL、空文件、无末尾换行、混合换行、
生成标记头部（`@generated`、`Code generated … DO NOT EDIT`）及不确定语法。

扫描识别 Rust 嵌套注释/raw string/lifetime、Go raw string、C++ raw string、
Markdown 围栏和 HTML 注释的结束状态；保守跳过大多数 C 预处理、行拼接、
trigraph/digraph 歧义，以及 Markdown front matter 和非注释内嵌 HTML。
这不是完整 parser，**不保证任意仓库仍可编译**。行号、快照、校验和和特殊工具仍可能受影响。

只在文件末尾一个独立位置添加注释。排序后的安全文件各有 25% 选中概率，
每文件最多一条，总量最多 100 条；零条也成功并保留干净副本。
保留原字节、BOM、换行风格和可执行位，写入前持久化补丁和预期指纹。
隐藏测试变量 `SPRINKLE_SEED=<u64>` 只在同版本/内容/locale/顺序下固定选择，
不固定资源 ID，不出现在帮助中。

## diff 与撤销

在受管理副本内定位该会话；在原工作区内定位它创建的最新未撤销会话。
其他 worktree 不能凭分支名字接管。撤销最新后才定位前一个。
`diff` 显示创建时保存的补丁；副本已变化时在 stderr 提示，不混入后来用户的修改。
想看全部当前修改，请进入副本运行普通 `git diff`。

undo 校验清单、仓库、规范路径、gitdir 双向关联、worktree 登记/锁、HEAD/ref/reflog、
索引、全部跟踪文件及模式，并检查**任何额外未跟踪或忽略文件**。
新提交、staged 改动、用户编辑、构建产物、路径替换、元数据不完整均拒绝。
没有 force 开关；撤销前请停止在副本中编辑或构建，本工具的锁不能约束其他进程。

所有检查通过后，只逆向删除记录中的插入字节，核对恢复为原指纹且整个副本干净，
再非强制移除 worktree，并以 expected-old-OID 删除 ref。
不使用 reset --hard、git clean，不删除未知资源，不恢复原 checkout。
restoring/removing/removed/deleting-ref 阶段允许中断后重新验证再继续；
无法确认的部分创建/移除需人工检查。已完成清单保留为小型已撤销记录，防止误接管。

状态和补丁保存在 `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`，
目录 0700、文件 0600；权限、符号链接或锁不安全即停止。
路径/补丁属于敏感本地数据，不是加密数据；无联网、遥测，卸载不删除会话和副本。
成功返回 0，业务/Git/状态失败 1，用法错误 2。
