# LoveLaoJiCLI

爱你老己命令行工具。Love you, dear myself CLI tools, powered by Rust and clap.

实施进度、当前限制、待补验收和后续批次见 [ROADMAP.md](ROADMAP.md)。本 README 描述当前已实现接口，路线图中的未来接口尚不可用。
See [ROADMAP.md](ROADMAP.md) for implementation status, limitations, outstanding validation and upcoming batches. This README documents implemented interfaces; planned interfaces are not yet available.

## 构建与运行 / Build and run

需要 Rust 1.89+ 和 Cargo。三批工具支持 Linux、macOS、WSL Linux 用户空间；Git 功能需要本机 Git。
Requires Rust 1.89+ and Cargo. All three batches target Linux, macOS and WSL Linux; Git features require local Git.

```sh
cargo build --workspace --release

./target/release/love
./target/release/happiness
./target/release/joy
./target/release/patience sleep 3

# 或者 / Or:
cargo run -p love -- --help
cargo run -p happiness -- --version
cargo run -p joy
cargo run -p patience -- sh -c 'echo hi'
```

十四个工具各有独立目录：`love/`、`happiness/`、`joy/`、`patience/`、`sprinkle/`、`later/`、`enough/`、`stuck/`、`duck/`、`one/`、`afk/`、`goodnight/`、`proof/`、`poke/`，共用 `cli-common/`。
Each tool is independently usable; `cli-common/` shares CLI, locale, private state, Git snapshots and execution plumbing.

## 交互式安装与卸载 / Interactive installation

Linux / macOS 下使用 Bash 运行，无需 sudo。安装需要 Rust、Cargo 和常用 Unix 工具。
Run with Bash on Linux/macOS, without sudo. Installation requires Rust, Cargo and standard Unix utilities.

```sh
./install.sh                 # 菜单：编译安装 / 卸载 / 退出
./install.sh install         # 直接进入安装确认 / Confirm installation
./install.sh uninstall       # 直接进入卸载确认 / Confirm uninstallation
./install.sh --help
```

- 安装前需输入 `y` 确认；空输入或 EOF 取消，不更改配置。
- 使用锁定依赖编译十四个工具的本机 release 版本，安装到 `${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli/bin`。
- 按 `$SHELL` 自动配置 PATH：Bash 使用 `.bashrc` 和生效的登录配置文件；Zsh 使用 `${ZDOTDIR:-$HOME}` 中的 `.zshrc`、`.zprofile`；Fish 使用 `${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish`。
- PATH 配置带有项目专属标记，重复安装不会重复添加。安装目录优先于原有 PATH，其他位置的同名程序不会被覆盖。
- 卸载不需要 Rust，按安装记录移除十四个工具及本项目 PATH 配置，保留其他配置、文件和源码构建产物。
- 脚本提示按 locale 切换中英文。若设置了自定义 XDG 目录，卸载时请保持相同的 `XDG_DATA_HOME`。

Installation asks for confirmation, builds all fourteen release binaries and registers the user-local directory in your Bash/Zsh/Fish startup configuration. Reinstallation is idempotent. Uninstallation removes only managed binaries and PATH blocks; Rust is not required. Keep the same `XDG_DATA_HOME` when uninstalling.

**安装后打开新终端即可使用命令**，或执行脚本最后打印的 PATH 命令，立即在当前终端生效。脚本不能直接修改父终端环境，请勿 `source install.sh`。卸载后也请打开新终端刷新 PATH 和命令缓存。
Open a new terminal after installation/uninstallation. To use the tools immediately, run the PATH command printed by the installer. Execute the installer; do not source it.

## 最小功能 / Features

- `love/happiness/joy` 直接运行：输出一句中英文祝福。These three tools print a localized blessing with no arguments.
- `--help` / `-h`：显示帮助。Show help.
- `--version` / `-V`：显示版本。Show version.
- `--verison`：兼容最初说明中的拼写。Accepted as a compatibility alias.

love、happiness、joy 不提供子命令或其他参数。No subcommands or additional arguments for love, happiness or joy.

### patience：假进度条包装器 / Fake progress bar wrapper

`patience <command> [args...]` 像工具那样运行一个真实命令，并给它演出一条假进度条：
`patience <command> [args...]` runs a real command and performs a fake progress bar show around it:

- 每次运行随机选择一种速度曲线（匀速、先慢后快、先快后慢、S 型、阶梯式、指数逼近）。
  Each run picks one random speed curve (linear, ease-in, ease-out, sigmoid, stepped, exponential).
- 多次爬升与短卡，随后停在 97%~99.9%；只要子命令还没退出，进度条就永不再动。
  Multiple climbs and short stalls, then a hold at 97–99.9%; the bar never moves again until the child exits.
- 子命令成功退出时快速填满到 100%；失败则停在原地，绝不填满。
  On success the bar rapidly fills to 100%; on failure it stays parked and never fills.
- 卡住时展示中英文安慰语（洗牌抽取，绝不连续重复），结果行带真实耗时与退出码。
  Localized comfort messages while stalled (shuffled, never repeated back-to-back); the result line carries the real elapsed time and exit code.
- 退出码透传子命令（信号终止则 128+N）；Ctrl+C 同组直达，无孤儿进程。
  Exit codes pass through from the child (128+N on signals); Ctrl+C reaches the child via the shared process group.
- 子进程输出被捕获（每流上限 1MiB），进度条结束后回放。
  Child output is captured (1 MiB per stream) and replayed after the bar.
- 进度条是彩虹跑马灯：调色板取自 `colors.png` 的无缝循环渐变，色带界线默认斜 45°，每帧滑动一格；填充段为动画彩虹，空段仍是灰色 `░`。
  The bar is a rainbow marquee: a seamless looping palette sampled from `colors.png`, band boundaries slanted 45° by default, sliding one cell per frame; the filled part is the animated rainbow, the empty part stays gray `░`.
- 按终端能力自动降级：真彩 → 256 色 → 16 色 → 无色原样；尊重 `NO_COLOR` 与 `TERM=dumb`，非 TTY 照旧不渲染。
  Colors degrade with the terminal's capability: truecolor → 256 → 16 → plain; `NO_COLOR` and `TERM=dumb` are honored, and non-TTY runs still render nothing.
- 特判：前缀每多写一个 `patience` 记号，就同时多显示一条独立演出的进度条，真实命令仍是它们之后的那个（`patience patience sleep 3` 同时显示两条）。
  Special case: each extra leading `patience` token adds one more independently performing bar to the show; the real command is whatever follows them (`patience patience sleep 3` shows two bars at once).
- 非 TTY 环境不渲染动画，只输出回放与结果行。
  Not a TTY: no animation, just the replay and result line.

```sh
patience make -j4        # 假装在认真编译
patience sleep 5        # 看看它卡在 98% 时的表情
patience -- git status  # 子命令带 -- 开头的参数时用 -- 分隔
patience patience sleep 5  # 套两层，同时看两条进度条 / nest it to watch two bars at once
```

实现细节与设计巧思见 [`patience/README-zh.md`](patience/README-zh.md)。
Implementation notes and design decisions: [`patience/README.md`](patience/README.md).

隐藏测试钩子（不写入 `--help`）：`PATIENCE_SEED=<u64>` 固定随机，`PATIENCE_FAST=1` 将所有时间缩至 1%。
Hidden test hooks (not in `--help`): `PATIENCE_SEED=<u64>` fixes randomness; `PATIENCE_FAST=1` scales every timing to 1%.

## 首批工具 / First batch

首批 MVP 已实现，不包含原始构想中的全部未来参数；实现状态与后续验收见 [路线图](ROADMAP.md)。本地 `project.md` 保留详细设计，但当前不随仓库分发。
The first-batch MVP is implemented, not every option in the original ideas; see the [roadmap](ROADMAP.md) for status and follow-up validation. The local `project.md` retains the detailed design but is not currently distributed with the repository.

```sh
sprinkle                       # HEAD 的隔离副本 / isolated HEAD copy
sprinkle worktree              # 同上 / same as the default
sprinkle diff                  # 创建时的插入补丁 / original insertion patch
sprinkle undo                  # 验证无额外工作才撤销 / verified undo only
later --next "check token expiry"  # headless 保存 / save
later                          # 交互只问一个问题 / one interactive question
later resume                   # 显示，不执行 / display, never execute
enough cargo test             # 最近真实成功后 5 分钟内提醒 / recent-success reminder
enough --again cargo test     # 始终执行 / always execute
stuck cargo test               # 三次相同失败后要求假设 / hypothesis after three equal failures
stuck --hypothesis "increase timeout" cargo test
```

- **sprinkle**：源 checkout 必须干净；仅在旁边新建的受管理 worktree 中插入注释。支持 Markdown、Go、Rust、C/C++，仅考虑可确认安全的文件末尾；固定 25% 文件概率，每文件一条，最多 100 条，零条也成功。原 checkout 文件与分支不变，Git common dir 会新增登记。副本是 HEAD，不是完整备份；不保证任意仓库都能编译，不执行脚本验证。
  Requires a clean source; adds conservative EOF comments only in its new worktree. Fixed 25% selection, one per file, 100 total; zero additions is valid. The original checkout stays unchanged, but shared Git metadata gains a branch/worktree. This is a HEAD copy, not a backup or compilation guarantee.
- **later**：每个 worktree 一张最新卡片；非 Git 按当前目录保存。`--next` 为非空单行，最多 2,000 字符。没有交互终端也没有 `--next`，立即返回 125，不读取管道、不覆盖旧卡片。
  One latest card per worktree (per cwd without Git). `--next` is a nonempty single line, at most 2,000 characters. Without a foreground terminal or explicit answer it returns 125, without consuming pipes or replacing the old card.
- **enough**：同 cwd/argv、稳定代码快照、最近真实成功完成后 5 分钟内才可能跳过，提示写 stderr，stdout 留空。`--again` 总是执行，失败会清除旧成功资格；跳过不刷新窗口。不是构建缓存，不能证明外部服务、忽略文件或 stdin 没变。
  Can skip only a matching invocation with a stable snapshot within five minutes of real success. Notices use stderr; skips have empty stdout and do not extend the window. `--again` always runs and invalidates old success on failure. This is not a build cache or proof about external state.
- **stuck**：连续三次相同 invocation/代码/退出码/两流原始字节失败后，第四次要求假设。假设为非空单行、最多 512 字符，仅放行一次；成功、变化、并发歧义或不完整输出重置链。实时分别转发 stdout/stderr，固定内存增量摘要，不保存输出；管道使子命令看不到 TTY，不是 PTY 包装器。
  Three equal failures gate the fourth call. A nonempty single-line hypothesis (512 characters maximum) grants one execution, not permanent bypass. Success, changes, overlap or incomplete output break the chain. Streams are forwarded live and hashed separately with bounded memory; no output bodies are stored. The child sees pipes, not a PTY.

业务选项必须在子命令前；子命令之后的参数原样传递，必要时用 `--` 分隔。包装器直接执行程序，不拼接 shell；stdin 继承。
Put wrapper options before the child command. Child arguments retain their boundaries; use `--` for disambiguation. No implicit shell is involved, and child stdin is inherited.

包装器在非 Git、Git/状态/锁/指纹故障、管道或重定向 stdin、无法确认前台交互时放行真实命令，不凭不可靠信息跳过或拦截。未找到子程序返回 127，不可执行返回 126；真实退出码透传，信号映射 128+N。工具业务/持久化失败为 1，用法错误为 2，无法取得回答/主动拦截为 125。Ctrl+C 通过共享前台进程组送达；不约束自行 daemonize 的程序。
Wrappers fail open when Git/state/locks/snapshots are unavailable or stdin is not comparable (including pipelines and redirected input). Launch errors use 127/126; real child codes pass through, signals map to 128+N. Tool errors use 1, usage errors 2, and unavailable answers/interception 125. Foreground signals reach the shared process group; deliberately daemonizing commands are outside this guarantee.

所有首批工具不需要桌面、不联网、不上传、不读取 shell history、不装 shell hook；工具自身不输出 ANSI，适用于 SSH/headless、`NO_COLOR` 和 `TERM=dumb`。子命令原始输出不翻译或删改。
All first-batch tools are desktop-free, offline and local-only, with no telemetry, history scanning or shell hooks. Tool-generated output has no ANSI escapes; child output stays untranslated and unchanged.

详解 / Deep dives: [sprinkle](sprinkle/README.md) · [中文](sprinkle/README-zh.md)；[later](later/README.md) · [中文](later/README-zh.md)；[enough](enough/README.md) · [中文](enough/README-zh.md)；[stuck](stuck/README.md) · [中文](stuck/README-zh.md)。

### 本地状态、隐私与故障 / State, privacy and failures

状态根目录为 `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`。空/非绝对 XDG 路径回退到绝对 HOME；两者都不可用时不写 cwd 或 `/tmp`。受管理目录 0700、文件 0600；不跟随状态符号链接，不接受不可信权限。采用同目录原子替换、同步、非阻塞 OS 文件锁、版本化 JSON 与带密钥 BLAKE3 校验。锁只协调本工具，不能阻止编辑器或其他 Git 进程；权限/锁语义不可靠的网络盘或 WSL 挂载盘会拒绝记录或保守降级。
State lives under the path above. Invalid XDG paths fall back to absolute HOME, never cwd or `/tmp`. Managed directories/files use 0700/0600, symlinks and unsafe permissions are rejected. Records use same-directory atomic replacement, sync, nonblocking OS locks, versioned JSON and keyed BLAKE3 checksums. Locks coordinate these tools only; filesystems without reliable permission/lock semantics are not promised equivalent safety.

包装器命令参数只保留按边界编码的本地带密钥摘要和程序 basename；不保存 argv 原文、完整 stdout/stderr 或环境变量。**下一步和假设是本地明文，勿填写密码、令牌或其他秘密**；命令行参数还可能进入 shell 历史或进程列表。仓库路径、文件名、Git 元数据和 sprinkle 补丁也可能敏感。密钥丢失/损坏会使旧记录失效，校验摘要不等于加密。
Wrapper arguments are retained only as a keyed, boundary-preserving digest plus program basename, not raw argv, output bodies or environment. **Next actions and hypotheses are plaintext: never enter secrets.** CLI values can also appear in shell history/process lists. Paths, filenames, Git metadata and sprinkle patches can be sensitive. Lost/corrupt identity keys invalidate old records; hashing is not encryption.

真实执行记录和假设在相关调用时惰性清理超过 30 天的可识别、非活动记录；损坏/未知版本不盲删。latest 卡片保留到成功覆盖，活动 sprinkle 元数据不按时间删除。卸载仅删除程序和 PATH 配置，保留卡片、执行记录及 worktree。
Recognized inactive runs/hypotheses older than 30 days are lazily pruned; unknown/corrupt records are left alone. Cards remain until successfully replaced; active sprinkle sessions never expire. Uninstallation preserves all user state and worktrees.

代码快照包含 HEAD、索引条目、文件内容/删除/类型/可执行位、符号链接文本、非忽略未跟踪文件与 cwd；两次扫描核对一致性，并在执行前后比较。最多 20,000 条目、累计读取 256 MiB、5 秒扫描预算；超预算、冲突、子模块、过滤器或竞争都放弃重复优化。快照不涵盖忽略文件、网络、数据库、环境和时间，不是文件系统事务。
Snapshots include HEAD, index entries, contents/deletions/types/modes, symlink text, nonignored untracked files and cwd. Two scans detect changes, and execution has pre/post snapshots. Budgets are 20,000 entries, 256 MiB total reads and five seconds; overflow, conflicts, submodules, configured filters or races disable optimization. This is not a filesystem transaction or evidence about ignored/external state.

隐藏测试钩子：`SPRINKLE_SEED=<u64>` 固定注释选择；只保证同版本、Git 内容、locale 与候选顺序的可复现性，不控制随机资源 ID，不显示于帮助。包装器测试使用真实 PTY，不提供绕过输入安全判断的测试开关。
Hidden test hook: `SPRINKLE_SEED=<u64>` fixes comment selection for the same version/content/locale/candidate order, not resource IDs. Wrapper tests use real PTYs, not safety-bypass flags.

## 第二批工具 / Second batch

```sh
duck                          # 前台交互四问 / four foreground questions
duck --expected "success" --actual "timeout" --last-change "retry logic" --experiment "disable retries"
one add "check token expiry"
one                           # 选中一件，保持到 done / stable selection
one done                      # 完成持久化选中项 / complete persistent selection
cat TODO.md | one             # 临时选择，不改任务库 / temporary selection
afk 5m                        # 安静休息；Ctrl+C 取消 / quiet break; Ctrl+C cancels
```

- **duck**：依次输出 Expected、Actual、Last change、Next experiment，最后 `quack.`。显式参数齐全时不提问；仅对缺项询问，提示走控制终端，完整结果走 stdout。回答 trim 后须为非空单行、最多 2,000 字符，不含控制字符。缺少前台交互、EOF 或无效交互回答返回 125，stdout 留空；不把管道当回答。无落盘、AI、搜索或技术建议。
  Four fixed answers in order, ending with `quack.`. Complete explicit arguments skip prompts; missing answers require a foreground terminal. Prompts use the controlling terminal, the complete summary uses stdout. Answers are trimmed, nonempty, single-line, at most 2,000 characters, without control characters. Missing interaction, EOF or invalid interactive input returns 125 with empty stdout; pipes are not answers. No persistence, AI, search or advice.
- **one**：所有目录共用一个本地任务列表（最多 1,000 项，每项最多 2,000 字符，trim 后非空单行且无控制字符）。默认随机选中并保存 ID，添加任务不改变选中项；`done` 删除选中任务，下次 `one` 才重新选择。管道或文件重定向按行临时选择，过滤空行、ATX Markdown 标题、已勾选项，去除常见列表前缀；最多读取 1 MiB UTF-8、1,000 项，不是完整 Markdown parser。临时输入完全不访问状态；`add`/`done` 忽略 stdin，`/dev/null` 使用持久化列表。空列表成功；无选中项的 `done` 不会代选。状态损坏、版本未知或锁争用返回 1，不重置列表。
  One global local list (1,000 tasks maximum; each a trimmed, nonempty single line of up to 2,000 characters, no controls). Selection is persisted until `done` removes it; adding tasks does not change it. The next bare invocation selects again. Piped/redirected UTF-8 input is temporary (1 MiB / 1,000 tasks maximum), filtering blanks, ATX headings and checked items and stripping common list prefixes; not a full Markdown parser. Temporary input never opens state. `add`/`done` ignore stdin; `/dev/null` uses persistent tasks. Empty lists succeed; `done` without selection does not select one. Corruption, unknown schemas or lock contention fail closed with exit 1.
- **afk**：正整数加 `s/m/h`，最多 24h；单调时钟计时。合适的同一前台终端显示 Ratatui 彩色 ASCII 草地、剩余时间（非空 `NO_COLOR` 切换为单色）；普通按键提示 `the grass noticed.`（中文“小草注意到了。”），不惩罚、不重置或延长时长。Ctrl+C 取消；Ctrl+Z 先恢复终端再挂起，时间继续流逝，fg 可重入、bg 不读键盘。非 TTY、输出重定向、TERM 缺失/dumb、后台或启动时小于 24×10 则安静等待，结束仅一行。不使用 Crossterm raw mode，而是保留信号的无回显/非规范输入；RAII 恢复原始 termios、光标和备用屏幕。仅 afk 增加 Ratatui/signal-hook/libc 依赖；SIGKILL、外部 SIGSTOP、abort 或终端断连不可保证清理。详见工具 README 的评估与恢复边界。
  Positive integer plus `s/m/h`, up to 24h, timed monotonically. A suitable shared foreground terminal gets a colored Ratatui ASCII garden and remaining time (nonempty `NO_COLOR` selects monochrome). Keys show `the grass noticed.` without penalties or timer changes. Ctrl+C cancels; Ctrl+Z restores before stopping, time keeps advancing, fg can resume and bg never reads keys. Non-TTY, redirected output, missing/dumb TERM, background startup or terminals smaller than 24×10 wait quietly and emit one completion line. Cbreak-style input retains signals, with exact termios and screen/cursor RAII restoration—not Crossterm raw mode. Ratatui/signal-hook/libc are scoped to afk. SIGKILL, external SIGSTOP, abort or terminal loss cannot guarantee cleanup; see the tool README for evaluation and boundaries.

任务明文保存在状态根目录下 `one/tasks.json`，复用私有权限、原子写入、版本校验和非阻塞锁；完成即从列表删除，不保留完成历史，卸载保留状态。不要输入秘密；参数也可能进入 shell 历史/进程列表。duck/afk 不创建状态，不联网。第二批用法错误为 2，I/O/状态错误为 1，成功为 0；duck 无回答为 125。
Tasks are local plaintext in `one/tasks.json` under the state root, with shared private permissions, atomic writes, version validation and nonblocking locks. Completion removes the task without history; uninstall preserves state. Do not enter secrets; CLI arguments can enter shell history/process lists. Duck/afk never create state; all three are offline. Usage errors return 2, I/O/state errors 1, success 0; duck without an answer returns 125.

隐藏测试钩子（不在帮助中）：`ONE_SEED=<u64>` 固定同版本/候选顺序的随机选择，不影响任务 ID 或已有选中项；无效值忽略。`AFK_FAST=1` 将等待时长缩至 1%，不放宽时长校验；`AFK_TEST_FAILURE=enter|draw|panic` 仅在 TUI 模式注入初始化后失败/后端写错误/panic，未知值忽略，只用于隔离测试。
Hidden test hooks (not in help): `ONE_SEED=<u64>` fixes random selection for the same version/candidate order, not task IDs or existing selections; invalid values are ignored. `AFK_FAST=1` scales waits to 1% without relaxing duration validation. `AFK_TEST_FAILURE=enter|draw|panic` injects post-entry/backend-write/panic failures in TUI mode only; unknown values are ignored, for isolated tests only.

详解 / Deep dives: [duck](duck/README.md) · [中文](duck/README-zh.md)；[one](one/README.md) · [中文](one/README-zh.md)；[afk](afk/README.md) · [中文](afk/README-zh.md)。

## 第三批工具 / Third batch

```sh
goodnight                     # 只读收尾，不强制休息 / read-only session ending
proof                         # 今天可用的本地证据 / available local evidence today
poke add "小明"
poke                          # 只建议问候，不发消息 / suggests, never sends
poke remove "小明"
```

- **goodnight**：显示本地时间（含 UTC 偏移）、当前 branch/游离 HEAD、Git 未完成路径数，以及当前工作区最近记录的命令状态。数据缺失、损坏、锁争用、并发时间顺序不明时显示未知；不是测试结果检查器。工作区有未完成路径时可提示手动运行 later，绝不自动调用，不保存卡片、不锁 shell、不关闭程序或强制作息。非 Git 仍有时间和结束语。
  Local time with UTC offset, branch/detached HEAD, Git unfinished-path count and the latest explicitly recorded command in the current workspace. Missing/corrupt/busy or ambiguously ordered data stays unknown. It does not identify test results. Unfinished paths can prompt a manual later invocation, never an automatic call. No saved card, shell lock, shutdown or enforced schedule; non-Git still gets time and a closing line.
- **proof**：仅遍历当前 HEAD 可达的本地提交，以当前有效 Git 配置 `user.name` **和** `user.email` 原文精确匹配作者，以 committer 时间落入本地“今天”且不晚于采集开始时刻为准。提交数含合并提交；增删行只累加非合并提交（含初始提交），路径按原始字节去重；关闭 rename 检测，重命名前后算不同路径；二进制仅计路径、不编造行数。不扫描其他分支或仓库。作者配置缺失、浅克隆、探测错误或预算不足时 Git 证据显示未知。
  Only locally reachable history from current HEAD, matching **both** configured author name and email exactly. Commits are filtered by committer timestamp within today's local calendar boundaries, excluding future times. Commit count includes merges; line totals sum non-merge patches including root commits, with byte-exact path deduplication. Rename detection is off (old/new paths count separately); binaries contribute paths but no line counts. No scanning of other refs/repositories. Missing identity, shallow history, failed probes or budget exhaustion makes Git evidence unknown.
- **执行证据 / Execution evidence**：goodnight/proof 只读 enough/stuck 已有显式记录。proof 按完成时间统计当天当前 worktree（非 Git 按 cwd）的完整真实执行；成功执行不称“测试通过”，跳过/拦截、启动失败、未完成、转发不完整及未来记录不计入。没有可用证据时显示 `no recorded work.`，同时保留未知来源说明；不推断没有工作，不打分。读取不会创建目录/密钥/锁文件，不修复、不清理；使用已有共享锁，争用则显示未知。
  Both reports read existing enough/stuck records only. Proof counts today's complete real executions by completion time in this worktree (cwd outside Git). Successful executions are not called passed tests. Skips, gates, spawn failures, unfinished/incompletely forwarded runs and future records do not count. Without available evidence it says `no recorded work.`, retaining unknown-source labels—not a claim of no work or a score. Reads never create directories/keys/lock files, repair state or prune history; existing shared locks fail conservatively on contention.
- **poke**：全局本地名单；姓名/昵称 trim 后为非空单行、最多 200 字符且无控制字符，最多 1,000 人。大小写与 Unicode 原文精确去重；空名单、重复添加、移除不存在姓名均友好成功。均匀随机选择，可连续选同一人；只输出行动建议，不记录选择/联系历史或负债，不读取系统通讯录、不接 API、不发消息。
  Global local list: trimmed nonempty single-line names/nicknames, at most 200 characters without controls, up to 1,000 names. Exact case/Unicode deduplication; empty lists, duplicate adds and missing removes succeed kindly. Uniform random selection permits repeats. Suggestions only: no selection/contact history, reminder debt, address-book access, API or messaging.

本地时间仅 goodnight/proof 使用可选 `jiff` 依赖（系统时区与 zoneinfo，无内置时区数据库）；遵从系统时区/`TZ`，缺失或无效时标未知，不偷偷回退 UTC。“今天”用日历日边界，不假定 DST 日期为 24 小时。Git 探测沿用每次 5 秒/32 MiB；proof 最多 20,000 条历史、1,000 条当天本人提交、20,000 个变更路径、32 MiB 累计 numstat，并在探测间检查 10 秒采集预算；状态扫描最多 10,000 条/32 MiB、探测间检查 5 秒。超预算不输出部分数字。
Only goodnight/proof enable optional `jiff` (system timezone and zoneinfo, no bundled database). System timezone/`TZ` failures stay unknown, never silently UTC. Local days use calendar boundaries, not fixed 24-hour DST assumptions. Git probes retain 5-second/32-MiB limits; proof caps history at 20,000 commits, today's matching commits at 1,000, changed paths at 20,000 and cumulative numstat at 32 MiB, checking a 10-second collection budget between probes. Record scans cap at 10,000 entries/32 MiB with a 5-second inter-record budget. Budget failure suppresses that source's partial numbers.

poke 明文名单保存在状态根目录的 `poke/names.json`，使用既有私有权限、原子写入、校验和非阻塞锁；损坏/未知版本/不安全权限/争用返回 1，不重置。不要输入秘密；卸载保留名单。隐藏测试钩子 `POKE_SEED=<u64>` 固定同版本和名单顺序的选择，无效值忽略，不显示于帮助。
Poke stores plaintext names in `poke/names.json` under the state root, using existing private permissions, atomic writes, validation and nonblocking locks. Corruption/unknown schema/unsafe permissions/contention returns 1 without reset. Do not enter secrets; uninstall preserves the list. Hidden `POKE_SEED=<u64>` fixes choices for the same version/list order; invalid values are ignored, and the hook is absent from help.

第三批不读取 stdin，适用于 headless；成功/保守报告未知返回 0，输出/写状态错误为 1，CLI 用法错误为 2；Ctrl+C 使用默认信号行为。
Third-batch tools never read stdin and work headlessly. Success/conservative unknown reports return 0, output/state-write errors 1, usage errors 2; Ctrl+C uses default signal handling.

详解 / Deep dives: [goodnight](goodnight/README.md) · [中文](goodnight/README-zh.md)；[proof](proof/README.md) · [中文](proof/README-zh.md)；[poke](poke/README.md) · [中文](poke/README-zh.md)。

## 语言 / Language

按 `LC_ALL` → `LC_MESSAGES` → `LANG` 的顺序，采用首个非空值。
The first nonempty value in `LC_ALL` → `LC_MESSAGES` → `LANG` selects the language.

- `zh_CN`、`zh-CN`（也接受 `zh`）：简体中文。
- `en_US`、`en-US`：English.
- 缺省、`C`、`POSIX` 及其他 locale：回退到英文。Missing or unsupported locales fall back to English.

接受 `.UTF-8`、`@modifier` 等后缀，locale 名称不区分大小写。
Encoding/modifier suffixes are accepted; locale names are case-insensitive.

```sh
LANG=zh_CN.UTF-8 ./target/release/love
# 爱你老己。
LC_ALL=en_US.UTF-8 ./target/release/love
# Love yourself, dear me.
LC_ALL=zh_CN.UTF-8 ./target/release/joy --help
```

帮助与默认输出已本地化；版本号不随语言变化，参数错误保留 clap 的英文诊断。
Help and default messages are localized; versions are language-neutral, and argument errors use clap's English diagnostics.

## 代码结构 / Code structure

按职责拆分，CLI 参数与本地记录格式不受模块划分影响。
Modules separate responsibilities without changing CLI arguments or persisted record formats.

- `afk/src/tui.rs`：计时与事件循环；`tui/terminal.rs` 管理终端恢复，`tui/signals.rs` 管理信号与 job-control mask。循环保持守卫的创建/析构顺序。
  Timer/event loop, terminal restoration, and signal/job-control guards; guard lifetime ordering stays in the loop.
- `sprinkle/src/`：`manifest` 管理会话元数据，`create` 编排创建事务，`plan` 生成插入计划，`scan` 检查语法边界，`verify` 校验归属，`undo` 执行可恢复撤销。
  Session metadata, creation transaction, insertion planning, syntax checks, ownership verification, and resumable undo.
- `one/src/`：`input` 处理独立管道输入，`tasks` 封装记录校验与稳定选择，`main` 保留 CLI 和加锁读写。
  Isolated stdin parsing, validated task model/stable selection, and CLI/locked persistence.
- `later/src/`：`main` 编排 CLI 与锁；`card` 管理记录和 Git 上下文采集；`card/display` 只负责展示。
  CLI/lock orchestration, card schema/Git capture, and read-only presentation.
- `cli-common/src/repeat/`：enough/stuck 共用 `cli` 调度、`protocol` 执行租约与基线状态机、`records` 历史记录与只读查询；`repeat.rs` 保持公共接口。
  Shared enough/stuck CLI dispatch, execution lease/baseline protocol, and record/history readers; `repeat.rs` preserves the public API.

## 测试 / Tests

```sh
cargo fmt --all -- --check
cargo test --workspace             # 新增集成测试需要 Git 和 Python 3 / Git + Python 3 required
cargo clippy --workspace --all-targets -- -D warnings

# 安装脚本测试（需要 Python 3；隔离 HOME，使用模拟编译器）
# Installer tests (Python 3; isolated HOME and mock compiler)
bash -n install.sh
python3 -m unittest discover -s tests -p 'test_installer.py' -v

# 跨工具真实执行/跳过/拦截验收 / Cross-tool real execution, skip and gate checks
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```
