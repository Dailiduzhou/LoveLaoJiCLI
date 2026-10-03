# 实施状态与后续路线图 / Implementation status and roadmap

本文件统一记录**已实现、当前限制、待补验收、后续实现**，不是所有设想都已上线的功能清单。
首批历史基线：`c10ce88` / `0.2.0`；本轮第二批交付后 workspace 版本为 `0.3.0`。后续交付应同步更新本文件。
This document separates shipped implementations, limitations, outstanding validation and future work. First-batch historical baseline: `c10ce88` / `0.2.0`; current second-batch workspace version: `0.3.0`.

- 当前接口与行为：[README.md](README.md) 及各工具双语 README。
- 开发约束：[AGENTS.md](AGENTS.md)；业务参数、依赖准入和版本同步以该文件为准。
- 本地 `project.md` 保留原始构想与详细规格，当前未纳入版本控制；本路线图独立保留后续 MVP 边界，不要求读者拥有该文件。
- **“已实现”不等于全部平台和故障矩阵均已验收。**下面明确区分已有测试与待补项目。

Current contracts live in the root/per-tool READMEs; development rules live in AGENTS. The local, untracked `project.md` is supplementary, not a prerequisite for this roadmap. Implemented does not mean exhaustively validated.

## 1. 已交付 / Implemented

十一个工具均在 workspace 和安装/卸载清单中，可独立使用；共用 locale 与基础 help/version/verison 契约。
All eleven tools are workspace members and installer entries, independently usable with shared locale and basic CLI conventions.

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
| 第二批 3 / Batch 2.3 | `afk` | 单调休息计时，所有终端纯文本降级 / Monotonic break timer; plain-text fallback on all terminals | [中文](afk/README-zh.md) · [English](afk/README.md) |

共享能力已落在 `cli-common/`，没有另建通用框架：
Shared capabilities remain in `cli-common/`, without a separate framework:

- locale 和可扩展 clap 构造 / locale and extensible clap scaffolding；
- 私有状态目录、原子 JSON、OS 锁、带密钥 BLAKE3、可逆路径 / private state, atomic JSON, OS locks, keyed digests and reversible paths；
- 本地 Git 发现与保守内容快照 / local Git discovery and conservative content snapshots；
- 真实 argv、前台交互检查、子进程执行、分流转发和重复状态 / real argv, foreground checks, child execution, stream forwarding and repeat state。

依赖已引入 clap/rand、serde/serde_json、blake3、rustix；安装器、Cargo.lock、根/工具文档和 AGENTS 状态已同步。原有四个工具的行为没有借首批交付重构。
Dependencies and installation/documentation lists are synchronized. The original four tools retain their behavior.

第二批仅复用既有依赖和共享能力；rustix 额外用于识别 one 的 stdin 文件类型，没有新增终端依赖。
Batch two reuses existing dependencies and shared capabilities; rustix also identifies one stdin types. No terminal dependency was added.

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
| Afk 终端 / Terminal | 本版所有终端均纯文本，不修改模式、不监听按键；ASCII 场景与按键提示暂缓 / Plain text everywhere, no mode changes or key capture; scenery/key notices deferred |
| 状态与隐私 / State and privacy | 下一步/假设/任务为本地明文；摘要不是加密；锁只协调本工具；不保证网络盘或 WSL 挂载盘的权限/锁语义 / Plaintext user input and filesystem-dependent guarantees |

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

### 3.3 第二批本轮验证 / Current second-batch validation

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

**仍待验证**：macOS/WSL、第一批 3.2 节故障矩阵、one 磁盘满/同步失败注入。afk 未启用 raw mode，不宣称 raw mode 恢复矩阵已验证；场景/按键能力保留为后续工作。
**Still pending:** macOS/WSL, the first-batch fault matrix in 3.2, and one disk-full/sync-failure injection. Afk never enables raw mode, so this is not a raw-mode recovery claim; scenery/key capture remain deferred.

## 4. 第二批：本轮交付 / Batch two: delivered this round

**已按 `duck → one → afk` 实现独立 crate、测试、文档和安装入口。afk 使用纯文本降级。**
**Independent crates, tests, documentation and installer entries delivered in `duck → one → afk` order. Afk uses the plain-text fallback.**

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

- 接口 / Interface：`afk <duration>`；ASCII 正整数加小写 `s/m/h`，不超过 24 小时；用法错误为 2。
- 使用单调时钟安静等待，正常结束打印一行。Ctrl+C 使用默认前台信号取消，无完成行。
- **本轮所有 TTY、非 TTY 和 TERM=dumb 均使用允许的纯文本降级**，不启用 raw mode、不捕捉键盘、不改变终端属性，无需在信号退出时恢复。正常终端行规程仍有效。
- ASCII 生长场景及 `the grass noticed.` 按键提示**未实现**；终端生命周期/信号恢复需要另行设计验收，不以本轮无 raw mode 的测试替代。无新增终端依赖。
- 隐藏钩子 `AFK_FAST=1` 将等待缩至 1%，已记入 README；不改变时长校验。
- 无 Ratatui、`--until`、番茄钟循环、锁屏、桌面提醒、后台常驻或 streak。

Monotonic quiet timer with the explicitly permitted plain-text fallback everywhere. No keyboard capture or terminal mutation. Scenery/key notices are deferred, not silently advertised as implemented. No desktop services, scoring or enforced breaks.

## 5. 第三批 / Batch three

**均未实现，顺序为 `goodnight → proof → poke`，在第二批之后推进。**
**Not implemented; follow batch two in this order.**

| 工具 / Tool | MVP 范围 / Planned scope | 交付前重点 / Key validation |
|---|---|---|
| `goodnight` | 只读展示本地时间、branch、工作区摘要及已记录执行状态，缺失写未知；可提示手动运行 later，不自动调用。非 Git 仍可输出时间/结束语 / Read-only session ending; missing evidence stays unknown | 不把任意命令成功称为测试通过；无锁 shell、杀进程、关机或作息评判 / No invented test status or enforcement |
| `proof` | 仅当前仓库、当前 HEAD 可达的当天本地作者提交及当前 workspace 显式执行记录；无记录输出 `no recorded work.` / Current-repository evidence only | 本地时区/DST 日界线、作者配置缺失、按提交时间过滤；非 merge 增删行累加、路径去重、二进制不硬算行数；只计真实执行，不计跳过/拦截 / Explicit counting rules, no productivity scores |
| `poke` | `poke add <name>`、`poke remove <name>`、`poke`；trim 后非空精确去重、均匀随机，可连续选同一人；空名单/删除不存在姓名友好处理 / Small local contact reminder | 状态/并发安全及未来 `POKE_SEED`；不读取通讯录、不保存联系历史/负债、不自动发消息 / No integrations or guilt metrics |

需要本地日期/时区时再评估 `jiff`，不提前为所有工具引入；现行依赖白名单不会因本路线图自动放宽。
Evaluate time-zone dependencies such as `jiff` only when needed. This roadmap does not itself authorize new dependencies.

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
- Later 历史卡片管理、编辑器恢复；One 复杂任务系统；Afk 完整番茄钟/TUI。
- Proof 跨仓库自动发现、推测终端活跃时间、自动识别所有测试工具。

No renaming/framework migration, platform expansion, background monitoring or expanded tool interfaces is implied. Throughout all batches: no diagnosis, productivity scores, streaks, punitive prompts, forced rest/sleep or automated social contact.
