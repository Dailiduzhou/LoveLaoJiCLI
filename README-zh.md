# LoveLaoJiCLI

[English](README.md)

爱你老己命令行工具，使用 Rust 和 clap 构建。

实施进度、当前限制、待补验收和后续批次见 [ROADMAP.md](ROADMAP.md)。本 README 描述当前已实现接口，路线图中的未来接口尚不可用。

## 构建与运行

需要 Rust 1.89+ 和 Cargo。三批工具支持 Linux、macOS、WSL Linux 用户空间；Git 功能需要本机 Git。

```sh
cargo build --workspace --release

./target/release/love
./target/release/happiness
./target/release/joy
./target/release/patience sleep 3

# 或者
cargo run -p love -- --help
cargo run -p happiness -- --version
cargo run -p joy
cargo run -p patience -- sh -c 'echo hi'
```

十四个工具各有独立目录：`love/`、`happiness/`、`joy/`、`patience/`、`sprinkle/`、`later/`、`enough/`、`stuck/`、`duck/`、`one/`、`afk/`、`goodnight/`、`proof/`、`poke/`，共用 `cli-common/`。

## 交互式安装与卸载

Linux / macOS / WSL 下使用 Bash，无需 sudo 或 Rust。需要 `curl`、`tar`、`sha256sum`（Linux）或 `shasum`（macOS）和常用 Unix 工具。

从 `raw.githubusercontent.com` 下载脚本，检查后执行；二进制来自 GitHub Releases，而不是 raw 源码目录。不要直接 `curl | bash`：交互确认需要保留标准输入。

```sh
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  https://raw.githubusercontent.com/Dailiduzhou/LoveLaoJiCLI/main/install.sh \
  --output install.sh
# 检查脚本后
bash install.sh install
```

```sh
bash install.sh                 # 菜单：发布版安装 / 卸载 / 本地编译 / 退出
bash install.sh install         # 下载脚本对应版本
bash install.sh uninstall       # 直接进入卸载确认
bash install.sh --help
LOVELAOJI_VERSION=v0.5.0 bash install.sh install  # 指定版本

# 在源码 checkout 内，保留本地构建安装（需要 Rust/Cargo）
./install.sh install-local
```

脚本当前默认 `v0.5.0`，不会静默追踪 latest。只有对应 Release 发布后才能下载；404 或网络失败会报错，不自动回退编译。需要固定脚本本身时，将 raw URL 的 `main` 换成对应标签（例如 `v0.5.0`）。

| 平台 | 发布目标 | 系统基线 |
| --- | --- | --- |
| Linux / WSL x86_64 | `x86_64-unknown-linux-gnu` | glibc 2.35+ (Ubuntu 22.04+) |
| Linux / WSL ARM64 | `aarch64-unknown-linux-gnu` | glibc 2.39+ (Ubuntu 24.04+) |
| macOS Intel | `x86_64-apple-darwin` | macOS 13+ |
| macOS Apple Silicon | `aarch64-apple-darwin` | macOS 13+ |

基于 `uname` 自动选择平台；Linux 包不是 musl 静态包，不支持 Alpine/musl 或 Windows 原生终端。旧系统可尝试源码构建；Git 功能仍需要本机 Git，日期功能需要系统时区数据。macOS 包未经 Apple 签名或公证。

- 安装前需输入 `y` 确认；空输入或 EOF 取消，不下载、不更改配置。
- 下载十四个工具的对应平台压缩包和 SHA-256 文件，校验摘要、成员列表及版本可执行性后才更新安装。校验失败保留原安装；SHA-256 用于完整性校验，不是独立签名，仍需信任仓库和 HTTPS。
- 安装到 `${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli/bin`。`install-local` 使用锁定依赖构建本机 release，产物位于 `target/installer/<target>/release/`。
- 按 `$SHELL` 自动配置 PATH：Bash 使用 `.bashrc` 和生效的登录配置文件；Zsh 使用 `${ZDOTDIR:-$HOME}` 中的 `.zshrc`、`.zprofile`；Fish 使用 `${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish`。
- PATH 配置带有项目专属标记，重复安装不会重复添加。安装目录优先于原有 PATH，其他位置的同名程序不会被覆盖。
- 卸载不需要 Rust 或联网，按安装记录移除十四个工具及本项目 PATH 配置，保留其他配置、文件、用户状态和源码构建产物。
- 脚本提示按 locale 切换中英文。若设置了自定义 XDG 目录，卸载时请保持相同的 `XDG_DATA_HOME`。

**安装后打开新终端即可使用命令**，或执行脚本最后打印的 PATH 命令，立即在当前终端生效。脚本不能直接修改父终端环境，请勿 `source install.sh`。卸载后也请打开新终端刷新 PATH 和命令缓存。

## 发布

[Release workflow](.github/workflows/release.yml) 在推送稳定版标签 `vX.Y.Z` 时触发，也可通过 Actions 手动指定一个已存在的标签。先在 Linux 上执行完整本地验证，再用四个平台的原生 runner 构建、检查中英文 `--help`/`--version`、打包。每个平台上传一个包含全部十四个工具的 `.tar.gz` 及对应 `.sha256`。所有平台成功后才创建/补全 draft 并正式发布；重跑不会覆盖已公开的版本。

发布前运行 `bash scripts/bump-version.sh vX.Y.Z`，它会同步 workspace 版本、`Cargo.lock`、`install.sh` 默认版本、`README.md`/`README-zh.md` 的版本示例和 ROADMAP 开发版本，并拒绝残留旧版本或未继承 workspace 版本的 crate；审阅 diff 后提交并打标签。打包脚本会拒绝标签/工作区/安装器版本不一致，或安装器工具列表遗漏二进制的情况。仓库需启用 Actions 并允许发布 job 使用 `contents: write`（其余 job 只读）。

```sh
# 确保版本尚未发布
git tag vX.Y.Z
git push origin vX.Y.Z

# 可选：本机复现打包，不创建 GitHub Release
bash scripts/package-release.sh vX.Y.Z "$(rustc -vV | awk '/^host: / { print $2 }')"
# 输出：dist/lovelaojicli-vX.Y.Z-<target>.tar.gz[.sha256]
```

## 最小功能

- `love/happiness/joy` 直接运行：输出一句中英文祝福。
- `--help` / `-h`：显示帮助。
- `--version` / `-V`：显示版本。
- `--verison`：兼容最初说明中的拼写。

love、happiness、joy 不提供子命令或其他参数。

### patience：假进度条包装器

`patience <command> [args...]` 像工具那样运行一个真实命令，并给它演出一条假进度条：

- 每次运行随机选择一种速度曲线（匀速、先慢后快、先快后慢、S 型、阶梯式、指数逼近、平方根）。
- 多次爬升与短卡，随后停在 97%~99.9%；只要子命令还没退出，进度条就永不再动。
- 子命令成功退出时快速填满到 100%；失败则停在原地，绝不填满。
- 卡住时展示中英文安慰语（洗牌抽取，绝不连续重复），结果行带真实耗时与退出码。
- 退出码透传子命令（信号终止则 128+N）；Ctrl+C 同组直达，无孤儿进程。
- 子进程输出被捕获（每流上限 1MiB），进度条结束后回放。
- 进度条是彩虹跑马灯：调色板是 13 个锚点组成的无缝循环渐变，色带界线默认斜 45°，每帧滑动一格；填充段为动画彩虹，空段仍是灰色 `░`。
- 按终端能力自动降级：真彩 → 256 色 → 16 色 → 无色原样；尊重 `NO_COLOR` 与 `TERM=dumb`，非 TTY 照旧不渲染。
- 特判：前缀每多写一个 `patience` 记号，就同时多显示一条独立演出的进度条，真实命令仍是它们之后的那个（`patience patience sleep 3` 同时显示两条）。
- 动画按 stderr 尺寸预算，预留一列/一行，先省略文案再缩短条宽；过窄、尺寸/能力未知或条数过多时无动画。检测到缩放后本次永久停止重绘，真实命令仍只运行一次。`NO_COLOR` 只禁颜色。
- 非 TTY 和保守降级不输出动画控制码，只输出回放与结果行。

```sh
patience make -j4        # 假装在认真编译
patience sleep 5        # 看看它卡在 98% 时的表情
patience -- git status  # 子命令带 -- 开头的参数时用 -- 分隔
patience patience sleep 5  # 套两层，同时看两条进度条
```

实现细节与设计巧思见 [`patience/README-zh.md`](patience/README-zh.md)。

隐藏测试钩子（不写入 `--help`）：`PATIENCE_SEED=<u64>` 固定随机，`PATIENCE_FAST=1` 将所有时间缩至 1%。

## 首批工具

首批 MVP 已实现，不包含原始构想中的全部未来参数；实现状态与后续验收见 [路线图](ROADMAP.md)。本地 `project.md` 保留详细设计，但当前不随仓库分发。

```sh
sprinkle                       # HEAD 的隔离副本
sprinkle worktree              # 同上
sprinkle diff                  # 创建时的插入补丁
sprinkle undo                  # 验证无额外工作才撤销
later --next "check token expiry"  # headless 保存
later                          # 交互只问一个问题
later resume                   # 显示，不执行
enough cargo test             # 最近真实成功后 5 分钟内提醒
enough --again cargo test     # 始终执行
stuck cargo test               # 三次相同失败后要求假设
stuck --hypothesis "increase timeout" cargo test
```

- **sprinkle**：源 checkout 必须干净；仅在旁边新建的受管理 worktree 中插入注释。支持 Markdown、Go、Rust、C/C++，仅考虑可确认安全的文件末尾；固定 25% 文件概率，每文件一条，最多 100 条，零条也成功。原 checkout 文件与分支不变，Git common dir 会新增登记。副本是 HEAD，不是完整备份；不保证任意仓库都能编译，不执行脚本验证。
- **later**：每个 worktree 一张最新卡片；非 Git 按当前目录保存。`--next` 为非空单行，最多 2,000 字符。没有交互终端也没有 `--next`，立即返回 125，不读取管道、不覆盖旧卡片。
- **enough**：同 cwd/argv、稳定代码快照、最近真实成功完成后 5 分钟内才可能跳过，提示写 stderr，stdout 留空。`--again` 总是执行，失败会清除旧成功资格；跳过不刷新窗口。不是构建缓存，不能证明外部服务、忽略文件或 stdin 没变。
- **stuck**：连续三次相同 invocation/代码/退出码/两流原始字节失败后，第四次要求假设。假设为非空单行、最多 512 字符，仅放行一次；成功、变化、并发歧义或不完整输出重置链。实时分别转发 stdout/stderr，固定内存增量摘要，不保存输出；管道使子命令看不到 TTY，不是 PTY 包装器。

`later resume` 首先显示下一步，再显示粗粒度经过时间和保存时快照；空 diff 明确为“无已跟踪文件差异”，未知数据仍为未知，保留完整 HEAD、文件/截断和偏离提示。`enough` 每次跳过均明确“本次未执行”并提示 `enough --again <命令> [参数...]`，不回显真实 argv。

**已知长输入限制：** `duck/later/stuck` 交互仍受系统 canonical 行缓冲的字节上限影响，长粘贴可能丢失换行或挂住（macOS 的 duck 600 汉字已复现）。这不是已修复功能；长文本使用各自显式参数，不输入秘密，不用管道作答，卡住可 Ctrl+C。安全输入重设计仍待独立审批；常规测试显式忽略的长输入回归不计为通过。详见 [ROADMAP.md §3.6](ROADMAP.md#36-实测修复方案实施--improvement-plan-implementation)。

业务选项必须在子命令前；子命令之后的参数原样传递，必要时用 `--` 分隔。包装器直接执行程序，不拼接 shell；stdin 继承。

包装器在非 Git、Git/状态/锁/指纹故障、管道或重定向 stdin、无法确认前台交互时放行真实命令，不凭不可靠信息跳过或拦截。未找到子程序返回 127，不可执行返回 126；真实退出码透传，信号映射 128+N。工具业务/持久化失败为 1，用法错误为 2，无法取得回答/主动拦截为 125。Ctrl+C 通过共享前台进程组送达；不约束自行 daemonize 的程序。

所有首批工具不需要桌面、不联网、不上传、不读取 shell history、不装 shell hook；工具自身不输出 ANSI，适用于 SSH/headless、`NO_COLOR` 和 `TERM=dumb`。子命令原始输出不翻译或删改。

详解：[sprinkle](sprinkle/README-zh.md) · [later](later/README-zh.md) · [enough](enough/README-zh.md) · [stuck](stuck/README-zh.md)。

持久化路径被符号链接拒绝时，请检查 HOME/XDG_STATE_HOME 的路径组成（例如 macOS `/tmp → /private/tmp`）；不会自动 canonicalize 不可信路径后放行。可信测试临时目录先解析真实路径，能力探测仅对文件系统明确拒绝的非 UTF-8 名称用例报告跳过，其他 I/O 错误仍失败。

### 本地状态、隐私与故障

状态根目录为 `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`。空/非绝对 XDG 路径回退到绝对 HOME；两者都不可用时不写 cwd 或 `/tmp`。受管理目录 0700、文件 0600；不跟随状态符号链接，不接受不可信权限。采用同目录原子替换、同步、非阻塞 OS 文件锁、版本化 JSON 与带密钥 BLAKE3 校验。锁只协调本工具，不能阻止编辑器或其他 Git 进程；权限/锁语义不可靠的网络盘或 WSL 挂载盘会拒绝记录或保守降级。

包装器命令参数只保留按边界编码的本地带密钥摘要和程序 basename；不保存 argv 原文、完整 stdout/stderr 或环境变量。**下一步和假设是本地明文，勿填写密码、令牌或其他秘密**；命令行参数还可能进入 shell 历史或进程列表。仓库路径、文件名、Git 元数据和 sprinkle 补丁也可能敏感。密钥丢失/损坏会使旧记录失效，校验摘要不等于加密。

真实执行记录和假设在相关调用时惰性清理超过 30 天的可识别、非活动记录；损坏/未知版本不盲删。latest 卡片保留到成功覆盖，活动 sprinkle 元数据不按时间删除。卸载仅删除程序和 PATH 配置，保留卡片、执行记录及 worktree。

代码快照包含 HEAD、索引条目、文件内容/删除/类型/可执行位、符号链接文本、非忽略未跟踪文件与 cwd；两次扫描核对一致性，并在执行前后比较。最多 20,000 条目、累计读取 256 MiB、5 秒扫描预算；超预算、冲突、子模块、过滤器或竞争都放弃重复优化。快照不涵盖忽略文件、网络、数据库、环境和时间，不是文件系统事务。

隐藏测试钩子：`SPRINKLE_SEED=<u64>` 固定注释选择；只保证同版本、Git 内容、locale 与候选顺序的可复现性，不控制随机资源 ID，不显示于帮助。包装器测试使用真实 PTY，不提供绕过输入安全判断的测试开关。

## 第二批工具

```sh
duck                          # 前台交互四问
duck --expected "success" --actual "timeout" --last-change "retry logic" --experiment "disable retries"
one add "check token expiry"
one                           # 选中一件，保持到 done
one done                      # 完成持久化选中项
cat TODO.md | one             # 临时选择，不改任务库
afk 5m                        # 安静休息；Ctrl+C 取消
afk 5m --color rainbow        # 彩虹跑马灯
afk 5m --color parts          # 默认分区固定色
afk 5m --color white          # 全白
```

- **duck**：依次输出 Expected、Actual、Last change、Next experiment，最后 `quack.`。显式参数齐全时不提问；仅对缺项询问，提示走控制终端，完整结果走 stdout。回答 trim 后须为非空单行、最多 2,000 字符，不含控制字符。缺少前台交互、EOF 或无效交互回答返回 125，stdout 留空；不把管道当回答。无落盘、AI、搜索或技术建议。
- **one**：所有目录共用一个本地任务列表（最多 1,000 项，每项最多 2,000 字符，trim 后非空单行且无控制字符）。默认随机选中并保存 ID，添加任务不改变选中项；`done` 删除选中任务，下次 `one` 才重新选择。管道或文件重定向按行临时选择，过滤空行、ATX Markdown 标题、已勾选项，去除常见列表前缀；最多读取 1 MiB UTF-8、1,000 项，不是完整 Markdown parser。临时输入完全不访问状态；`add`/`done` 忽略 stdin，`/dev/null` 使用持久化列表。空列表成功；无选中项的 `done` 不会代选。状态损坏、版本未知或锁争用返回 1，不重置列表。
- **afk**：正整数加 `s/m/h`，最多 24h；单调时钟计时。合适的同一前台终端显示 Ratatui 彩色 ASCII 草地、剩余时间（`--color parts` 默认分区固定色、`rainbow` 彩虹跑马灯、`white` 全白；非空 `NO_COLOR` 优先，切换为终端默认前景色）；普通按键提示 `the grass noticed.`（中文“小草注意到了。”），不惩罚、不重置或延长时长。Ctrl+C 取消；Ctrl+Z 先恢复终端再挂起，时间继续流逝，fg 可重入、bg 不读键盘。非 TTY、输出重定向、TERM 缺失/dumb、后台或启动时小于 24×10 则安静等待，结束仅一行。不使用 Crossterm raw mode，而是保留信号的无回显/非规范输入；RAII 恢复原始 termios、光标和备用屏幕。失去前台归属时放弃旧快照，不覆盖新前台程序的设置或发送清理转义序列。仅 afk 增加 Ratatui/signal-hook/libc 依赖；SIGKILL、外部 SIGSTOP、abort 或终端断连不可保证清理。详见工具 README 的评估与恢复边界。

`one` 的持久化空列表提示 `one add "下一小步"`；临时空输入/过滤后无候选只说“输入中没有可选事项。”，不访问全局状态。无选中项的 `done` 仍不代选。非空输出仍只有任务正文。

任务明文保存在状态根目录下 `one/tasks.json`，复用私有权限、原子写入、版本校验和非阻塞锁；完成即从列表删除，不保留完成历史，卸载保留状态。不要输入秘密；参数也可能进入 shell 历史/进程列表。duck/afk 不创建状态，不联网。第二批用法错误为 2，I/O/状态错误为 1，成功为 0；duck 无回答为 125。

隐藏测试钩子（不在帮助中）：`ONE_SEED=<u64>` 固定同版本/候选顺序的随机选择，不影响任务 ID 或已有选中项；无效值忽略。`AFK_FAST=1` 将等待时长缩至 1%，不放宽时长校验；`AFK_TEST_FAILURE=enter|draw|panic` 在 TUI 模式注入初始化后失败/后端写错误/panic；`read-stop` 在前台检查后、首次读取输入前发送一次 SIGSTOP，由 PTY 控制器转移归属并继续进程以复现竞态。仅 TUI 模式有效，未知值忽略，只用于隔离测试。

详解：[duck](duck/README-zh.md) · [one](one/README-zh.md) · [afk](afk/README-zh.md)。

## 第三批工具

```sh
goodnight                     # 只读收尾，不强制休息
proof                         # 今天可用的本地证据
poke add "小明"
poke                          # 只建议问候，不发消息
poke remove "小明"
```

- **goodnight**：显示本地时间（含 UTC 偏移）、当前 branch/游离 HEAD、Git 未完成路径数，以及当前工作区最近记录的命令状态。数据缺失、损坏、锁争用、并发时间顺序不明时显示未知；不是测试结果检查器。工作区有未完成路径时可提示手动运行 later，绝不自动调用，不保存卡片、不锁 shell、不关闭程序或强制作息。非 Git 仍有时间和结束语。
- **proof**：仅遍历当前 HEAD 可达的本地提交，以当前有效 Git 配置 `user.name` **和** `user.email` 原文精确匹配作者，以 committer 时间落入本地“今天”且不晚于采集开始时刻为准。提交数含合并提交；增删行只累加非合并提交（含初始提交），路径按原始字节去重；关闭 rename 检测，重命名前后算不同路径；二进制仅计路径、不编造行数。每个补丁只使用该提交的属性文件，隔离工作树/索引及全局/系统属性；存在本地 `info/attributes` 或 Git 不支持所需 `--attr-source` 时显示未知。不扫描其他分支或仓库。作者配置缺失、浅克隆、探测错误或预算不足时 Git 证据显示未知。
- **执行证据**：goodnight/proof 只读 enough/stuck 已有显式记录。proof 按完成时间统计当天当前 worktree（非 Git 按 cwd）的完整真实执行；成功执行不称“测试通过”，跳过/拦截、启动失败、未完成、转发不完整及未来记录不计入。没有可用证据时显示 `no recorded work.`，同时保留未知来源说明；不推断没有工作，不打分。读取不会创建目录/密钥/锁文件，不修复、不清理；使用已有共享锁，争用则显示未知。
- **poke**：全局本地名单；姓名/昵称 trim 后为非空单行、最多 200 字符且无控制字符，最多 1,000 人。大小写与 Unicode 原文精确去重；空名单、重复添加、移除不存在姓名均友好成功。均匀随机选择，可连续选同一人；只输出行动建议，不记录选择/联系历史或负债，不读取系统通讯录、不接 API、不发消息。

本地时间仅 goodnight/proof 使用可选 `jiff` 依赖（系统时区与 zoneinfo，无内置时区数据库）；遵从系统时区/`TZ`，缺失或无效时标未知，不偷偷回退 UTC。“今天”用日历日边界，不假定 DST 日期为 24 小时。Git 探测沿用每次 5 秒/32 MiB；proof 最多 20,000 条历史、1,000 条当天本人提交、20,000 个变更路径、32 MiB 累计 numstat，并在探测间检查 10 秒采集预算；状态扫描最多 10,000 条/32 MiB、探测间检查 5 秒。超预算不输出部分数字。

poke 明文名单保存在状态根目录的 `poke/names.json`，使用既有私有权限、原子写入、校验和非阻塞锁；损坏/未知版本/不安全权限/争用返回 1，不重置。不要输入秘密；卸载保留名单。隐藏测试钩子 `POKE_SEED=<u64>` 固定同版本和名单顺序的选择，无效值忽略，不显示于帮助。

第三批不读取 stdin，适用于 headless；成功/保守报告未知返回 0，输出/写状态错误为 1，CLI 用法错误为 2；Ctrl+C 使用默认信号行为。

详解：[goodnight](goodnight/README-zh.md) · [proof](proof/README-zh.md) · [poke](poke/README-zh.md)。

## 语言

按 `LC_ALL` → `LC_MESSAGES` → `LANG` 的顺序，采用首个非空值。

- `zh_CN`、`zh-CN`（也接受 `zh`）：简体中文。
- `en_US`、`en-US`：英文。
- 缺省、`C`、`POSIX` 及其他 locale：回退到英文。

接受 `.UTF-8`、`@modifier` 等后缀，locale 名称不区分大小写。

```sh
LANG=zh_CN.UTF-8 ./target/release/love
# 爱你老己。
LC_ALL=en_US.UTF-8 ./target/release/love
# Love yourself, dear me.
LC_ALL=zh_CN.UTF-8 ./target/release/joy --help
```

帮助与默认输出已本地化；版本号不随语言变化，参数错误保留 clap 的英文诊断。

## 代码结构

按职责拆分，CLI 参数与本地记录格式不受模块划分影响。

- `afk/src/tui.rs`：计时与事件循环；`tui/terminal.rs` 管理终端恢复，`tui/signals.rs` 管理信号与 job-control mask。循环保持守卫的创建/析构顺序。
- `sprinkle/src/`：`manifest` 管理会话元数据，`create` 编排创建事务，`plan` 生成插入计划，`scan` 检查语法边界，`verify` 校验归属，`undo` 执行可恢复撤销。
- `one/src/`：`input` 处理独立管道输入，`tasks` 封装记录校验与稳定选择，`main` 保留 CLI 和加锁读写。
- `later/src/`：`main` 编排 CLI 与锁；`card` 管理记录和 Git 上下文采集；`card/display` 只负责展示。
- `enough/src/`、`stuck/src/`：各自的 `main` 定义参数和帮助，`policy` 定义成功提醒/失败拦截、提示文案和基线判断。
- `cli-common/src/repeat/`：仅共享 `execution` 失败放行执行、`protocol` 租约/锁/快照/基线持久化，以及 `records` 跨工具历史格式与查询；不按工具名分派业务逻辑。
- 工具专属源码、文案、参数、测试场景和资源放在 `<tool>/`。根目录 `tests/` 只放复用的测试支撑/参数化契约、跨工具验收及安装器测试；`scripts/` 保留工作区自动化。
- `cli-common/` 只依赖通用基础设施，不反向依赖工具。多个工具读写的历史记录格式（含旧记录的生产者校验和完整性规则）是共享兼容性契约；模块移动不改变磁盘格式。

## 测试

保留独立本地验证脚本，不安装、不编辑 shell 配置。需要 Rust（含 rustfmt/clippy）、Cargo、Git、Python 3；检查 README 链接/双语一致性和版本同步，再执行安装器隔离测试、fmt、workspace 测试、clippy、锁定依赖 release 构建和跨工具 PTY 验收。产物位于 `target/verify/<target>/release/`。

本次 macOS 自动化结果见 [ROADMAP.md §3.6](ROADMAP.md#36-实测修复方案实施--improvement-plan-implementation)：191 项 Rust 测试通过、3 项已知长输入回归明确忽略，另有 25 项安装器测试通过。忽略和文件系统能力跳过不等于通过；Linux/WSL 本次重验、GUI 目视及长输入安全重设计仍待完成。

```sh
./scripts/verify.sh

# 或逐项执行
cargo fmt --all -- --check
cargo test --locked --workspace    # 新增集成测试需要 Git 和 Python 3
cargo clippy --locked --workspace --all-targets -- -D warnings

# 文档链接、示例及隔离版本同步测试
python3 -m unittest discover -s tests -p 'test_documentation.py' -v

# 安装脚本测试（隔离 HOME；模拟编译器/下载，无网络）
bash -n install.sh
python3 -m unittest discover -s tests -p 'test_installer.py' -v

# 跨工具真实执行/跳过/拦截验收
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```
