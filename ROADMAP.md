# 实施状态与后续路线图 / Implementation status and roadmap

本文件统一记录**已实现、当前限制、待补验收、后续实现**，不是所有设想都已上线的功能清单。
首批历史基线：`c10ce88` / `0.2.0`；第二批提交：`d749d8c` / `0.3.0`；当前开发版本为 `0.4.1`（第三批及 afk TUI）。后续交付应同步更新本文件。
This document separates shipped implementations, limitations, outstanding validation and future work. First-batch historical baseline: `c10ce88` / `0.2.0`; second-batch commit: `d749d8c` / `0.3.0`; current development workspace version: `0.4.1` (batch three and afk TUI).

- 当前接口与行为：[README.md](README.md) 及各工具双语 README。
- 开发约束：[AGENTS.md](AGENTS.md)；业务参数、依赖准入和版本同步以该文件为准。
- 本地 `project.md` 保留原始构想与详细规格，当前未纳入版本控制；本路线图独立保留后续 MVP 边界，不要求读者拥有该文件。
- **“已实现”不等于全部平台和故障矩阵均已验收。**下面明确区分已有测试与待补项目。

Current contracts live in the root/per-tool READMEs; development rules live in AGENTS. The local, untracked `project.md` is supplementary, not a prerequisite for this roadmap. Implemented does not mean exhaustively validated.

## 1. 已交付 / Implemented

十四个工具均在 workspace 和安装/卸载清单中，可独立使用；共用 locale 与基础 help/version/verison 契约。
All fourteen tools are workspace members and installer entries, independently usable with shared locale and basic CLI conventions.

| 批次 / Batch | 工具 / Tool | 当前范围 / Current scope | 文档 / Docs |
|---|---|---|---|
| 原有 / Existing | `love`、`happiness`、`joy` | 各输出一句本地化祝福；没有业务参数 / One localized blessing each; no business options | [根 README](README.md) |
| 原有 / Existing | `patience` | 真实命令的假进度条；退出码透传、非 TTY 降级 / Fake progress wrapper with exit passthrough and non-TTY fallback | [中文](patience/README-zh.md) · [English](patience/README.md) |
| 第一批 1 / Batch 1.1 | `sprinkle` | 默认/worktree 创建隔离 HEAD 副本；EOF 注释、保存时 diff、校验式 undo / Isolated HEAD copy, EOF comments, saved diff, verified undo | [中文](sprinkle/README-zh.md) · [English](sprinkle/README.md) |
| 第一批 2 / Batch 1.2 | `later` | 一张最新上下文卡片；交互、`--next`、`resume` / One latest context card, interactive or explicit input, resume | [中文](later/README-zh.md) · [English](later/README.md) |
| 第一批 3 / Batch 1.3 | `enough` | 五分钟内稳定成功提醒；`--again` / Five-minute stable-success reminder with explicit rerun | [中文](enough/README-zh.md) · [English](enough/README.md) |
| 第一批 4 / Batch 1.4 | `stuck` | 三次相同失败后要求一次假设；`--hypothesis` / Hypothesis gate after three identical failures | [中文](stuck/README-zh.md) · [English](stuck/README.md) |
| 第二批 1 / Batch 2.1 | `duck` | 固定四问或显式回答，无持久化 / Four fixed questions or explicit answers; no persistence | [中文](duck/README-zh.md) · [English](duck/README.md) |
| 第二批 2 / Batch 2.2 | `one` | 全局本地列表、稳定选择至 done、隔离管道 / Global local list, stable selection until done, isolated pipes | [中文](one/README-zh.md) · [English](one/README.md) |
| 第二批 3 / Batch 2.3 | `afk` | 单调计时、Ratatui 草地/按键提示、保守降级 / Monotonic timer, Ratatui grass/key notice, conservative fallback | [中文](afk/README-zh.md) · [English](afk/README.md) |
| 第三批 1 / Batch 3.1 | `goodnight` | 只读时间、Git 与显式执行收尾摘要 / Read-only time, Git and execution summary | [中文](goodnight/README-zh.md) · [English](goodnight/README.md) |
| 第三批 2 / Batch 3.2 | `proof` | 当日本地当前 HEAD / workspace 已有证据 / Today's local current-HEAD/workspace evidence | [中文](proof/README-zh.md) · [English](proof/README.md) |
| 第三批 3 / Batch 3.3 | `poke` | 本地名单、精确去重、随机问候建议 / Local names, exact deduplication, random greeting suggestion | [中文](poke/README-zh.md) · [English](poke/README.md) |

共享能力已落在 `cli-common/`，没有另建通用框架：
Shared capabilities remain in `cli-common/`, without a separate framework:

- locale 和可扩展 clap 构造 / locale and extensible clap scaffolding；
- 私有状态目录、原子 JSON、OS 锁、带密钥 BLAKE3、可逆路径 / private state, atomic JSON, OS locks, keyed digests and reversible paths；
- 本地 Git 发现与保守内容快照 / local Git discovery and conservative content snapshots；
- 真实 argv、前台交互检查、子进程执行、分流转发和重复状态 / real argv, foreground checks, child execution, stream forwarding and repeat state。

依赖已引入 clap/rand、serde/serde_json、blake3、rustix；安装器、Cargo.lock、根/工具文档和 AGENTS 状态已同步。原有四个工具的行为没有借首批交付重构。
Dependencies and installation/documentation lists are synchronized. The original four tools retain their behavior.

第二批初始交付仅复用既有依赖；rustix 用于识别 one 的 stdin 类型。当时 afk 纯文本降级；本轮经评估增加仅限 afk 的 Ratatui/signal-hook/libc，见 3.5 与工具 README。
The initial second-batch delivery reused existing dependencies with plain-text afk. The current evaluated afk-only Ratatui/signal-hook/libc addition is documented in 3.5 and the tool README.

第三批增加只读状态/执行记录读取与可选本地日历日能力；jiff 仅由 goodnight/proof 通过 `cli-common/local-time` 启用 std/tz-system/tzdb-zoneinfo，已评估并同步 AGENTS。其他工具单独构建不启用 jiff，没有内置时区数据库或新终端依赖。
Batch three adds read-only state/run inspection and optional local calendar dates. Evaluated jiff features are enabled only by goodnight/proof through `cli-common/local-time`, with AGENTS synchronized. Other tools built separately do not enable jiff; no bundled timezone database or terminal dependency was added.

## 2. 当前限制与保守降级 / Current limits and conservative fallbacks

这些是当前使用边界，不是待实现功能已经可用的承诺。
These are current boundaries, not promises that future features already exist.

| 领域 / Area | 当前边界 / Current boundary |
|---|---|
| 平台 / Platforms | 目标为 Linux、macOS、WSL Linux 用户空间；已有本轮执行证据来自 Linux，其他平台仍需验证；不承诺原生 Windows / Linux validation recorded; macOS/WSL validation pending; no native Windows promise |
| Git 快照 / Snapshots | 20,000 条目、累计读取 256 MiB、5 秒扫描预算；不是文件系统事务，不包含忽略文件、环境、外部服务或时间 / Budgeted, nontransactional, excludes ignored/external state |
| 包装器 / Wrappers | 非 Git、状态/锁/指纹故障、stdin 不可比较或无法确认前台交互时真实执行；不将未知当相同 / Fail open when comparison is unreliable |
| 流与信号 / Streams and signals | `stuck` 子命令看到管道，不是透明 PTY；共享前台进程组不约束故意 daemonize 的后代 / Pipes, not a PTY; deliberate daemonization is outside the guarantee |
| Sprinkle 创建 / Creation | 源 checkout 必须干净；当前保守拒绝子模块、配置了 checkout/clean 过滤器（含全局 LFS）的仓库、reftable-only reflog；副本是 HEAD，不是磁盘备份 / Clean HEAD copies only, with conservative repository exclusions |
| Sprinkle 插入 / Insertion | 只考虑安全 EOF，固定 25% 选择、每文件最多一条、总计最多 100 条；不保证任意仓库可编译 / Conservative EOF-only comments, not a compilation guarantee |
| Sprinkle 撤销 / Undo | 新提交、用户编辑、额外产物、身份/清单不确定时拒绝；未知中间态需人工检查；已完成清单作为已撤销记录保留 / Refuse uncertain deletion; some partial states need manual inspection; completed manifests remain as tombstones |
| One 输入 / Input | 管道/文件读取等待 EOF，上限 1 MiB UTF-8、过滤后 1,000 项；不是完整 Markdown parser。持久化列表全局最多 1,000 项 / Bounded, EOF-terminated temporary input; one global persistent list |
| Afk 终端 / Terminal | 同一前台 TTY、非空/non-dumb TERM、至少 24×10 时启用 TUI，支持分区色/彩虹/全白；其他启动环境安静降级。SIGKILL/外部 SIGSTOP/abort/终端断连无法保证恢复 / Foreground suitable-terminal TUI; conservative startup fallback; uncatchable/aborting/disconnected paths cannot guarantee cleanup |
| 只读报告 / Read-only reports | Git/时区/记录不可用时显示未知，已有共享锁争用不写状态；不是全部 shell 命令审计日志 / Unavailable sources remain unknown; existing shared locks never create state; not a complete shell audit |
| Proof 统计 / Counting | 当前 HEAD 可达、本地配置姓名与 email 精确匹配、按 committer 时间；合并计提交但不计补丁，重命名前后分路径；浅历史/超预算为未知 / Exact author/current HEAD/committer-day scope; merge patches excluded, raw rename paths, shallow/budget failures unknown |
| 本地日期 / Local dates | 系统时区或 TZ，依赖本机 zoneinfo；不假定每天 24 小时，时区缺失不默认 UTC / System zone or TZ; host zoneinfo required; calendar days, no silent UTC fallback |
| 状态与隐私 / State and privacy | 下一步/假设/任务/姓名为本地明文；摘要不是加密；锁只协调本工具；不保证网络盘或 WSL 挂载盘的权限/锁语义 / Plaintext user input and filesystem-dependent guarantees |

更完整的限制与故障行为见各工具 README；不能为了增加命中率或自动清理而降低安全检查。
See per-tool READMEs for details. Do not weaken safety checks to improve hit rates or automate cleanup.

## 3. 验收状态 / Validation status

### 3.1 已有执行记录 / Previously completed checks

首批交付时，在当前 Linux 开发环境执行通过：
The first-batch delivery passed these checks in the Linux development environment:

- `cargo fmt --all -- --check`
- `cargo test --workspace`：92 项 Rust 测试通过，包含调用 Python 的真实前台 PTY 场景 / 92 Rust tests, including Python-backed foreground PTY scenarios。
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace --release`
- `bash -n install.sh`
- `python3 -m unittest discover -s tests -p 'test_installer.py' -v`：14 项安装器测试通过 / 14 installer tests。

这份记录是交付时的验证，不代表本次文档修改重新运行了全部测试，也不代表已经完成 macOS/WSL 或全部故障注入验收。
This is historical delivery evidence, not a claim of rerunning tests for this documentation change or completing all platform/fault-injection coverage.

现有测试入口 / Existing test entry points:

- [sprinkle/tests/cli.rs](sprinkle/tests/cli.rs)、[扫描器单元测试](sprinkle/src/scan.rs)：隔离、dirty 拒绝、固定种子、零插入、用户改动/产物/锁保护、hook/filter、多会话及部分恢复场景。
- [later/tests/cli.rs](later/tests/cli.rs)：headless 保存、无回答不覆盖、worktree 隔离、非 Git、损坏/符号链接、权限、控制字符路径、XDG 和锁争用。
- [tests/repeat_cases.rs](tests/repeat_cases.rs)、[tests/repeat_pty.py](tests/repeat_pty.py)：退出码、argv/流透传、大输出、管道放行、状态损坏、成功跳过、假设门槛、并发与 Ctrl+C。
- [tests/test_installer.py](tests/test_installer.py)：隔离 HOME 的安装/升级/卸载和状态保留，不运行真实安装修改用户环境。

### 3.2 下一步优先补齐 / Priority validation follow-up

以下为**待补充或待确认的覆盖**，不等同于已发现相应功能故障；在宣称完整验收前应逐项保留测试证据。
These are coverage gaps to close or verify, not assertions that each behavior is broken.

- [ ] macOS 自动测试；WSL 原生 Linux 文件系统与 `/mnt/*` 权限/锁不足时的拒绝或降级验证。
- [ ] `sprinkle` 每个创建/恢复/移除/ref 删除阶段的故障注入，而不只修改一次 `restoring` 阶段；验证原 checkout 和未知资源不受损。
- [ ] `sprinkle` ref 移动/删除重建、worktree 移动/路径替换、清单缺失、并发创建/撤销的完整安全矩阵。
- [ ] 状态未知 schema、密钥丢失/损坏、磁盘满、flush/sync/rename 失败及中断写入的覆盖。
- [ ] 可控时钟验证：五分钟边界、跳过不延长窗口、时间倒退、30 天清理及活动记录不被误删；不新增公开测试参数。
- [ ] 快照变化矩阵：HEAD/index/文件内容/未跟踪文件、执行期间变化、冲突/子模块、Git 超时与扫描预算。
- [ ] 补全 `stuck` 成功/退出码变化重置、异常转发不计数、并发乱序不形成伪失败链；补全 `later` 交互取消及失败记录摘要关联。

原建议先补首批加固；本轮按请求交付第二批，不将以上首批缺口标为完成。继续保留这些加固项；不在未完成工具上预建安装入口或占位二进制。
First-batch hardening was the recommended prerequisite. This delivery implements the requested second batch without claiming those earlier gaps are closed. Keep them outstanding; do not pre-register unimplemented binaries.

### 3.3 第二批交付验证 / Second-batch delivery validation

在当前 Linux 环境，workspace `0.3.0` 执行通过：
Passed in the current Linux environment, workspace `0.3.0`:

- `cargo fmt --all -- --check`
- `cargo test --workspace`：114 项 Rust 测试（首批 92 + 第二批 22），含 Python 驱动真实前台 PTY / 114 Rust tests including Python-backed foreground PTYs。
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace --release`
- `bash -n install.sh`
- `python3 -m unittest discover -s tests -p 'test_installer.py' -v`：14 项通过，安装/卸载覆盖全部 11 个二进制 / 14 passed, covering installation/removal of all 11 binaries。

首次运行有本地 Cargo 增量 dep-graph 缓存读取警告；禁用增量缓存（`CARGO_INCREMENTAL=0`）后重新运行测试和 clippy 通过，无源码诊断。未删除用户缓存或修改构建配置。
The initial run reported local Cargo incremental dep-graph cache warnings. Tests and clippy were rerun with `CARGO_INCREMENTAL=0`, passing without source diagnostics; no user cache deletion or build-configuration change was needed.

测试入口 / Test entry points:

- [duck/tests/cli.rs](duck/tests/cli.rs)：四项顺序、完整/部分显式输入、缺项/管道/空白/EOF、输出分离、locale、非 UTF-8 cwd、无状态目录。
- [one/tests/cli.rs](one/tests/cli.rs) 及源码单元测试：选择保持/完成/添加、全局范围、管道/文件隔离、确定性、大小/编码、未知 schema、权限/符号链接/密钥丢失、持锁拒绝、并发写入/选择、非 UTF-8 状态路径。
- [afk/tests/cli.rs](afk/tests/cli.rs) 及时长单元测试：严格边界、真实非 TTY 等待、快速钩子、locale/非 UTF-8 cwd。
- [tests/batch_two_pty.py](tests/batch_two_pty.py)：duck 真实前台提问/取消；afk 普通/TERM=dumb、输入不延时、正常/参数错误/输出错误/Ctrl+C/SIGTERM/SIGHUP 后终端属性不变。

**当时待验证**：macOS/WSL、第一批 3.2 节故障矩阵、one 磁盘满/同步失败注入；当时 afk 场景/按键尚未实现。后续 TUI 实现与独立恢复验收见 3.5，不用原纯文本测试替代。
**Pending at that delivery:** macOS/WSL, first-batch hardening and one disk-full/sync-failure injection; afk scenery/key capture were then deferred. Later TUI recovery validation is separately recorded in 3.5, not inferred from the old plain-text tests.

### 3.4 第三批实现验证 / Third-batch implementation validation

当前 Linux 环境、workspace `0.4.0` 执行通过：
Passed in the current Linux environment, workspace `0.4.0`:

- `cargo fmt --all -- --check`
- `CARGO_INCREMENTAL=0 cargo test --workspace`：143 项 Rust 测试通过（原有 114 + 本轮 29，含时区单元测试） / 143 Rust tests, including timezone units。
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace --release`
- `bash -n install.sh`
- `python3 -m unittest discover -s tests -p 'test_installer.py' -v`：14 项通过，安装/卸载清单包含全部 14 个二进制 / 14 passed, all 14 binaries covered。
- `python3 tests/report_records_pty.py target/release`：真实前台 enough 执行/跳过、stuck 三次失败/第四次拦截后，proof 仅计 4 次真实执行、1 次成功；两种报告保持状态文件不变 / Real foreground execution/skip/gate acceptance; 4 actual runs, 1 success, no report state writes。

新增验收入口 / New validation entry points:

- [goodnight/tests/cli.rs](goodnight/tests/cli.rs)：非 Git/无 HOME、时区未知、dirty 摘要但索引/文件不写、工作树隔离、游离/unborn HEAD、同秒顺序/未来结果不猜测、只读旧记录、locale/非 UTF-8 cwd。
- [proof/tests/cli.rs](proof/tests/cli.rs) 及 [统计单元测试](proof/src/history.rs)：精确作者、committer 日期、旧子提交不剪枝、HEAD 可达性、合并/根提交/二进制/重命名/去重、工作树隔离、浅历史未知、记录分类与损坏/未知 schema/锁/符号链接/密钥缺失、无外部 diff/textconv 执行。
- [cli-common/src/local_time.rs](cli-common/src/local_time.rs)：23/25 小时 DST 日期、本地跨日、午夜跳跃、起点/终点/未来边界。
- [poke/tests/cli.rs](poke/tests/cli.rs)：精确去重、空列表/不存在移除、容量、固定随机允许重复且不写历史、权限/锁/损坏/schema/符号链接/并发、locale/非 UTF-8 状态路径。
- [tests/report_records_pty.py](tests/report_records_pty.py)：跨工具真实记录兼容与只读验收；不是仅靠模拟 JSON 宣称正确。

仍保留：macOS/WSL、首批故障矩阵、完整磁盘满/sync 故障注入、Git 超时/全部扫描预算注入及全部历史时区异常。此时 afk 仍为纯文本；后续 TUI 验证见 3.5。没有将其他遗留矩阵勾为完成。
Still pending: macOS/WSL, first-batch hardening, comprehensive disk-full/sync failures, Git timeout/all-budget injection and every historical timezone anomaly. Afk was still plain text at this point; later TUI validation is in 3.5. Other pending matrices are not silently marked complete.

### 3.5 afk Ratatui 评估与本轮验证 / Afk Ratatui evaluation and current validation

按本轮请求，解除 afk 的“暂不使用 Ratatui”限制，但不扩大时长参数或加入完整番茄钟。
Ratatui 对单段动画偏重，采用它的收益是差分绘制、布局/裁剪与 TestBackend 测试；恢复仍由
本工具自己实现，不把第三方初始化函数当作终端安全保证。
The requested Ratatui addition supersedes afk's prior deferral, not the minimal business interface. It costs more than direct Crossterm but supplies rendering/layout and testability; terminal recovery remains our responsibility.

- 仅 afk 使用 Ratatui 0.30.2（默认 features 关闭、crossterm_0_29）、signal-hook 原子信号标记与 libc 作业控制信号掩码；rustix 保存/恢复 termios。AGENTS/Cargo.lock 已同步，其他工具独立构建不引入这些 TUI 依赖。
- Ratatui 声明最低 Rust 1.88；workspace 仍 1.89，已核查依赖元数据，但未实测 Rust 1.89 编译。新增解析 67 个 lockfile 条目，含不在当前平台编译的可选/平台依赖。
- 不调用 Crossterm raw mode/event reader；使用保留 ISIG 的非规范/无回显输入。避开会查询光标并临时切换 raw mode 的 Terminal::clear；固定、有上限的视口在缩放时重建，不走可能执行 tput 的尺寸回退。
- RAII + 常规循环处理 SIGINT/SIGTERM/SIGHUP/SIGQUIT；先恢复再重新触发信号。Ctrl+Z 先恢复再停止，fg 重查前台，bg 不读取或绘制；原单调截止时刻不变。

本轮 Linux 检查通过 / Passed on Linux:

- `cargo fmt --all -- --check`
- `CARGO_INCREMENTAL=0 cargo test --workspace`：146 项 Rust 测试（原 143 + 场景 2 + 真 TUI PTY 入口 1）。
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace --release`
- `bash -n install.sh`；14 项安装器测试 / 14 installer tests。
- `python3 tests/report_records_pty.py target/release` 跨工具验收 / cross-tool acceptance。

新增 [scene 单元测试](afk/src/scene.rs) 与 [真实 TUI PTY](tests/afk_tui_pty.py)，由 `cargo test -p afk` 调用。
覆盖：正常完成、按键/粘贴洪泛不延时或泄漏给 shell、缩小/极大尺寸、中文、Ctrl+C 与四种终止信号、
进入后错误/真实后端写错误/panic 展开、Ctrl+Z/fg/bg、挂起超过期限，以及 dumb/小终端/重定向降级。
比较完整 termios，验证备用屏幕退出和光标显示，拒绝意外光标位置查询；未用假 TTY 开关绕过前台检查。
New scene units and real controlling PTYs validate cleanup and timer behavior, including setup/write/panic faults and job control. No fake-interactivity hook bypasses foreground checks.

新增隐藏钩子 `AFK_TEST_FAILURE=enter|draw|panic` 仅用于隔离 TUI 故障注入；既有 `AFK_FAST=1` 保持不变。
macOS/WSL、真实终端模拟器目视验收与全部 I/O 故障时序仍待补；SIGKILL/外部 SIGSTOP/abort、
硬件故障与不可写/消失终端不属于可靠清理保证，不宣称完成所有信号/平台矩阵。
The failure hook is documented but absent from help. macOS/WSL, emulator visual checks and exhaustive I/O timing remain pending; uncatchable/aborting/disconnected paths are explicitly outside cleanup guarantees.

## 4. 第二批：本轮交付 / Batch two: delivered this round

**已按 `duck → one → afk` 实现独立 crate、测试、文档和安装入口；afk 后续加入经评估的最小 TUI，保留降级。**
**Independent crates, tests, docs and installer entries delivered in order; afk subsequently gained the evaluated minimal TUI with fallback retained.**

### 4.1 duck — 把问题说清楚 / Explain the problem

- 接口 / Interface：`duck`；headless 使用 `duck --expected <text> --actual <text> --last-change <text> --experiment <text>`。
- 依次询问 Expected、Actual、Last change、Next experiment；四项参数齐全不提问，部分提供则只问缺项。完整结果固定格式输出 stdout，以 `quack.` 结束；提示走控制终端。
- 回答 trim 后非空单行、最多 2,000 字符且无控制字符。非交互缺项、EOF/空白/无效交互回答返回 125，stdout 留空；不把意外管道当回答。显式无效参数为 2。
- 默认不落盘，用户可重定向 stdout；不回答技术问题，不调用 AI 或搜索。复用 locale、clap 和前台边界，没有持久化模块或测试开关。

Four fixed questions, formatted output, no advice/AI/search or default persistence. Missing headless answers cannot hang. Explicit answers skip their prompts; invalid/unavailable interactive answers produce no partial summary.

### 4.2 one — 只展示一个下一步 / Show exactly one thing

- 接口 / Interface：`one add <text>`、`one`、`one done`；以及 `cat TODO.md | one` / `one < TODO.md`。
- 全局本地列表随机选一个并持久化 `selected_task_id`；添加不改变选中项，done 删除选中项，下次 one 再选。只显示一项；无选中项的 done 不会代选。
- 最多 1,000 个未完成任务，每项 trim 后非空单行、最多 2,000 字符且无控制字符；明文私有原子状态与非阻塞锁，损坏/未知版本/锁争用 fail closed。
- 管道/文件是临时选择：最多 1 MiB UTF-8、1,000 个候选，按行过滤空白、ATX Markdown 标题、已勾选项，去除常见列表前缀；不是完整 Markdown parser，读取等待 EOF。
- 临时选择不访问状态、不添加任务、不覆盖持久化选中项；add/done 忽略 stdin，`/dev/null` 使用持久化列表。空列表友好成功。
- 无任务大小/优先级推断、项目、截止时间、标签、看板或分数。隐藏钩子 `ONE_SEED=<u64>` 已实现并记入 README，仅固定选择、不影响 ID 或已有选中项。

Persist one selected task until completion. Temporary input never opens or alters state. No prioritization or project-management features; bounded plaintext tasks use the shared private store.

### 4.3 afk — 不带惩罚的休息 / A break without penalties

- 接口为 `afk <duration> [--color rainbow|parts|white]`，时长为正整数加小写 s/m/h、最多 24h。按明确请求加入显示配色选项，默认 parts；NO_COLOR 优先于所有配色，降级输出不着色。
  Color is an explicit display-only extension: default fixed component colors, animated rainbow bands, or all white; NO_COLOR wins.
- 合适前台终端显示可选配色的 ASCII 四阶段草地、太阳和剩余时间，最多 10 帧/秒、240×100 逻辑视口。普通按键显示 `the grass noticed.`（中文本地化）两秒；不计分、不延时、不重置。q/Esc 不作为退出键。
- stdin/stdout/stderr 必须为同一前台 TTY、TERM 非空且非 dumb、启动尺寸至少 24×10；否则安静等待，不监听键盘、不改变终端模式。运行中缩小时显示紧凑文本。
- Ctrl+C 或 SIGINT/SIGTERM/SIGHUP/SIGQUIT 恢复后取消，无完成行；Ctrl+Z 恢复后挂起，原截止时刻继续推进。fg 可重入，bg 不读取或绘图。仍在前台时退出/挂起前丢弃排队 TUI 输入，不回放给 shell。
- 保留信号的 cbreak-style 输入、完整 termios 快照、独立非阻塞 tty 读写、已有信号恢复测试；对不可捕捉信号/abort/终端失联只保留明确限制，不虚称绝对恢复。
- 正常结束恢复屏幕后打印一行；用法错误 2、I/O 错误 1。无状态、网络、外部辅助程序、桌面提醒、锁屏、后台常驻、--until、完整番茄钟或 streak。
- 隐藏钩子 `AFK_FAST=1` 与 `AFK_TEST_FAILURE=enter|draw|panic` 见 README，不进入帮助。

A bounded monochrome Ratatui scene, nonpunitive key notice, monotonic deadline and tested terminal lifecycle, with conservative headless/dumb/small/background startup fallback. Details and non-guarantees are explicit in the tool READMEs.

## 5. 第三批：本轮交付 / Batch three: delivered this round

**已按 `goodnight → proof → poke` 实现独立 crate、测试、双语文档和安装入口。**
**Independent crates, tests, bilingual docs and installation delivered in `goodnight → proof → poke` order.**

### 5.1 goodnight — 只读收尾 / Read-only session ending

- 接口只有 `goodnight` 及基础 help/version；不读取 stdin。
- 显示本地时间/UTC 偏移、branch/游离 HEAD、Git 未完成路径数、当前 workspace 最近显式命令状态；缺失/损坏/忙碌/同秒顺序不明时未知。
- 任意命令成功不称“测试通过”；记录打印开始 Unix 时间戳，不假定今天运行过。
- 有未完成路径时可提示手动运行 later，不自动调用。非 Git 仍可输出时间/结束语；无锁 shell、杀进程、关机或作息评判。

Read-only time, Git and recorded command status, with unknowns preserved and no enforcement or invented test results.

### 5.2 proof — 只展示已有证据 / Existing evidence only

- 接口只有 `proof` 及基础 help/version；不读取 stdin，不扫其他仓库或 shell 活动。
- 当前 HEAD 可达历史，精确匹配有效配置 `user.name` 和 `user.email`；按 committer 时间落入本地当天且不晚于采集时刻。不用时间剪枝遗漏日期乱序的祖先。
- 提交数包含合并；增删行只累加非合并 numstat，含初始提交。路径按字节去重，关闭 rename 检测、旧新路径分别计；二进制只计路径。未提交变化不计入。
- 缺失作者、浅历史、Git 错误/HEAD 变化/预算不足时整项未知。每探测 5 秒/32 MiB；历史最多 20,000、当天本人最多 1,000、路径最多 20,000、numstat 累计 32 MiB，探测间检查 10 秒采集预算。
- 当前 worktree（非 Git 按 cwd）的 enough/stuck 显式完整真实执行按完成日期统计；成功不称测试通过。跳过/拦截、启动/等待失败、未完成/转发不完整、未来结果不计入。
- 没有可用证据输出 `no recorded work.`（中文本地化），仍标明未知来源，不推断没工作。无分数、排名、跨仓库发现或终端活跃估计。

Current-HEAD authored commits by local committer date plus current-workspace completed real executions by completion date; explicit statistics, unknowns and no productivity claims.

### 5.3 poke — 一个轻量问候建议 / A small greeting suggestion

- 接口为 `poke add <name>`、`poke remove <name>`、`poke`。
- trim 后非空单行、最多 200 字符、无控制字符；最多 1,000 人。按原文精确去重，大小写/Unicode 表示不同则不同，不要求真实姓名。
- 全局本地名单均匀随机，允许连续选到同一人；空名单、重复添加、移除不存在姓名友好成功。
- `poke/names.json` 为本地明文，私有原子状态/校验/非阻塞锁；错误 fail closed，不重置。无联系历史、时间戳、负债，不读通讯录、不发送消息或接 API。
- 隐藏测试钩子 `POKE_SEED=<u64>` 已实现并记入 README；无效值回退熵，不在帮助中显示。

A private global name list and uniform random suggestion, with exact deduplication, benign empty/missing cases and no social integrations/history/debt.

### 5.4 共享边界 / Shared boundaries

- 只读报告不会创建目录、身份密钥、锁文件或记录，不修复/清理；已有共享锁争用或状态不可信时为未知。记录扫描最多 10,000 条/32 MiB，记录间检查 5 秒。
- `jiff` 已完成本轮依赖评估，仅可选启用系统时区/zoneinfo；不提前给其他工具启用、不内置数据库。“今天”按日历日，DST 不固定 24h；时区不可用不回退 UTC。没有时钟测试环境变量，单元测试直接注入时间戳。
- 成功或保守报告未知为 0；I/O/持久化错误为 1，用法错误为 2；默认 Ctrl+C，headless 不提问。目标仍为 Linux/macOS/WSL Unix，本轮执行证据只来自 Linux。

Reports are genuinely read-only; optional calendar support is narrowly scoped. Errors, privacy and platform boundaries remain conservative.

## 6. 每个工具的交付清单 / Per-tool delivery checklist

- [ ] 明确本轮最小接口、输入长度/空值处理、headless 行为、退出码和隐私边界。
- [ ] 新增自己的 crate、源码与集成测试；只抽取本轮真正复用的公共能力。
- [ ] 覆盖 locale 优先级、help/version/verison、非 UTF-8 路径、无 TTY、失败/取消及适用的状态/并发场景。
- [ ] 同步 workspace members、统一版本、Cargo.lock、安装/卸载清单及安装测试。
- [ ] 同步根 README、工具英文 README.md/中文 README-zh.md、AGENTS 与本路线图；未来参数不能写进已支持接口。
- [ ] 执行第 3.1 节检查命令，记录平台和结果；未验证项明确保留，不用“已实现”替代测试证据。

Deliver one independently usable tool at a time, updating code, tests, installer, versioned dependencies and bilingual documentation together.

## 7. 不进入当前交付 / Explicitly deferred

- 项目改名或迁移 `crates/`、另建 `humanutils-core`、无实际需求的框架化重构。
- 原生 Windows、桌面通知、shell hook、后台监控、远端同步、全局配置和插件系统。
- Sprinkle 原地 branch、dirty override、force undo、clean、主题/密度/公开 seed、mixed 语言及任意语句间插入。
- Later 历史卡片管理、编辑器恢复；One 复杂任务系统；Afk 完整番茄钟/多页面 TUI。
- Proof 跨仓库自动发现、推测终端活跃时间、自动识别所有测试工具。

No renaming/framework migration, platform expansion, background monitoring or expanded tool interfaces is implied. Throughout all batches: no diagnosis, productivity scores, streaks, punitive prompts, forced rest/sleep or automated social contact.
