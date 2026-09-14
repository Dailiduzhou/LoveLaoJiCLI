# LoveLaoJiCLI

爱你老己命令行工具。Love you, dear myself CLI tools, powered by Rust and clap.

## 构建与运行 / Build and run

需要 Rust 和 Cargo（stable）。Requires stable Rust and Cargo.

```sh
cargo build --workspace --release

./target/release/love
./target/release/happiness
./target/release/joy

# 或者 / Or:
cargo run -p love -- --help
cargo run -p happiness -- --version
cargo run -p joy
```

三个工具分别位于 `love/`、`happiness/`、`joy/`，共用 `cli-common/` 中的参数与语言处理。
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
- 使用锁定依赖编译三个工具的本机 release 版本，安装到 `${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli/bin`。
- 按 `$SHELL` 自动配置 PATH：Bash 使用 `.bashrc` 和生效的登录配置文件；Zsh 使用 `${ZDOTDIR:-$HOME}` 中的 `.zshrc`、`.zprofile`；Fish 使用 `${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish`。
- PATH 配置带有项目专属标记，重复安装不会重复添加。安装目录优先于原有 PATH，其他位置的同名程序不会被覆盖。
- 卸载不需要 Rust，按安装记录移除三个工具及本项目 PATH 配置，保留其他配置、文件和源码构建产物。
- 脚本提示按 locale 切换中英文。若设置了自定义 XDG 目录，卸载时请保持相同的 `XDG_DATA_HOME`。

Installation asks for confirmation, builds all three release binaries and registers the user-local directory in your Bash/Zsh/Fish startup configuration. Reinstallation is idempotent. Uninstallation removes only managed binaries and PATH blocks; Rust is not required. Keep the same `XDG_DATA_HOME` when uninstalling.

**安装后打开新终端即可使用命令**，或执行脚本最后打印的 PATH 命令，立即在当前终端生效。脚本不能直接修改父终端环境，请勿 `source install.sh`。卸载后也请打开新终端刷新 PATH 和命令缓存。
Open a new terminal after installation/uninstallation. To use the tools immediately, run the PATH command printed by the installer. Execute the installer; do not source it.

## 最小功能 / Features

- 直接运行：输出一句对应的中英文祝福。No arguments: print a localized message.
- `--help` / `-h`：显示帮助。Show help.
- `--version` / `-V`：显示版本。Show version.
- `--verison`：兼容最初说明中的拼写。Accepted as a compatibility alias.

不提供子命令或其他参数。No subcommands or additional arguments.

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
