# LoveLaoJiCLI

爱你老己命令行工具。Love you, dear myself CLI tools, powered by Rust and clap.

## 构建与运行 / Build and run

需要 Rust 和 Cargo（stable）。Requires stable Rust and Cargo.

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

四个工具分别位于 `love/`、`happiness/`、`joy/`、`patience/`，共用 `cli-common/` 中的参数与语言处理。
Each tool has its own directory; `cli-common/` shares argument and locale handling.

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
- 使用锁定依赖编译四个工具的本机 release 版本，安装到 `${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli/bin`。
- 按 `$SHELL` 自动配置 PATH：Bash 使用 `.bashrc` 和生效的登录配置文件；Zsh 使用 `${ZDOTDIR:-$HOME}` 中的 `.zshrc`、`.zprofile`；Fish 使用 `${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish`。
- PATH 配置带有项目专属标记，重复安装不会重复添加。安装目录优先于原有 PATH，其他位置的同名程序不会被覆盖。
- 卸载不需要 Rust，按安装记录移除四个工具及本项目 PATH 配置，保留其他配置、文件和源码构建产物。
- 脚本提示按 locale 切换中英文。若设置了自定义 XDG 目录，卸载时请保持相同的 `XDG_DATA_HOME`。

Installation asks for confirmation, builds all four release binaries and registers the user-local directory in your Bash/Zsh/Fish startup configuration. Reinstallation is idempotent. Uninstallation removes only managed binaries and PATH blocks; Rust is not required. Keep the same `XDG_DATA_HOME` when uninstalling.

**安装后打开新终端即可使用命令**，或执行脚本最后打印的 PATH 命令，立即在当前终端生效。脚本不能直接修改父终端环境，请勿 `source install.sh`。卸载后也请打开新终端刷新 PATH 和命令缓存。
Open a new terminal after installation/uninstallation. To use the tools immediately, run the PATH command printed by the installer. Execute the installer; do not source it.

## 最小功能 / Features

- 直接运行：输出一句对应的中英文祝福。No arguments: print a localized message.
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
- 非 TTY 环境不渲染动画，只输出回放与结果行。
  Not a TTY: no animation, just the replay and result line.

```sh
patience make -j4        # 假装在认真编译
patience sleep 5        # 看看它卡在 98% 时的表情
patience -- git status  # 子命令带 -- 开头的参数时用 -- 分隔
```

隐藏测试钩子（不写入 `--help`）：`PATIENCE_SEED=<u64>` 固定随机，`PATIENCE_FAST=1` 将所有时间缩至 1%。
Hidden test hooks (not in `--help`): `PATIENCE_SEED=<u64>` fixes randomness; `PATIENCE_FAST=1` scales every timing to 1%.

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

## 测试 / Tests

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# 安装脚本测试（需要 Python 3；隔离 HOME，使用模拟编译器）
# Installer tests (Python 3; isolated HOME and mock compiler)
bash -n install.sh
python3 -m unittest discover -s tests -p 'test_installer.py' -v
```
