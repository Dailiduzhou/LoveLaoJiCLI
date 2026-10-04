#!/usr/bin/env bash
# User-local installer for Linux/macOS. Run this script; do not source it.
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
: "${HOME:?HOME must be set}"
INSTALL_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/lovelaojicli"
BIN_DIR="$INSTALL_DIR/bin"
RECEIPT="$INSTALL_DIR/rc-files"
BEGIN_MARKER='# >>> LoveLaoJiCLI PATH >>>'
END_MARKER='# <<< LoveLaoJiCLI PATH <<<'
TOOLS=(love happiness joy patience sprinkle later enough stuck duck one afk goodnight proof poke)
RC_FILES=()
TEMP_FILE=''
DOWNLOAD_DIR=''
RELEASE_VERSION=${LOVELAOJI_VERSION:-v0.4.0}
REPOSITORY='https://github.com/Dailiduzhou/LoveLaoJiCLI'
cleanup() {
    if [[ -n "$TEMP_FILE" ]]; then rm -f -- "$TEMP_FILE"; fi
    if [[ -n "$DOWNLOAD_DIR" ]]; then rm -rf -- "$DOWNLOAD_DIR"; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

locale_name=${LC_ALL:-${LC_MESSAGES:-${LANG:-en_US}}}
locale_name=${locale_name%%.*}
locale_name=${locale_name%%@*}
locale_name=$(printf '%s' "$locale_name" | tr '[:upper:]-' '[:lower:]_')
CHINESE=false
case "$locale_name" in zh|zh_cn) CHINESE=true ;; esac

text() {
    if "$CHINESE"; then printf '%s\n' "$2"; else printf '%s\n' "$1"; fi
}

fail() {
    text "$1" "$2" >&2
    exit 1
}

confirm() {
    local reply
    text 'Continue? [y/N]' '是否继续？[y/N]'
    if ! IFS= read -r reply; then return 1; fi
    case "$reply" in y|Y|yes|YES|是) return 0 ;; *) return 1 ;; esac
}

# Paths are persisted one per line and must be absolute.
check_path() {
    case "$1" in
        /*) ;;
        *) fail "Expected an absolute path: $1" "路径必须为绝对路径：$1" ;;
    esac
    case "$1" in
        *$'\n'*|*$'\r'*|*:*) fail 'Paths containing line breaks or colons are unsupported.' '不支持包含换行符或冒号的路径。' ;;
    esac
}

select_shell() {
    SHELL_NAME=${SHELL##*/}
    case "$SHELL_NAME" in
        bash)
            RC_FILES=("$HOME/.bashrc")
            if [[ -e "$HOME/.bash_profile" ]]; then
                RC_FILES+=("$HOME/.bash_profile")
            elif [[ -e "$HOME/.bash_login" ]]; then
                RC_FILES+=("$HOME/.bash_login")
            else
                RC_FILES+=("$HOME/.profile")
            fi
            ;;
        zsh) RC_FILES=("${ZDOTDIR:-$HOME}/.zshrc" "${ZDOTDIR:-$HOME}/.zprofile") ;;
        fish) RC_FILES=("${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/lovelaojicli.fish") ;;
        *) fail "Unsupported shell: $SHELL_NAME (use Bash, Zsh or Fish)." "暂不支持此 shell：$SHELL_NAME（支持 Bash、Zsh、Fish）。" ;;
    esac
    local file
    for file in "${RC_FILES[@]}"; do check_path "$file"; done
}

# Remove only our exact marked block. Reject malformed blocks without editing.
strip_block() {
    awk -v begin="$BEGIN_MARKER" -v end="$END_MARKER" '
        $0 == begin { if (inside || seen) exit 1; inside = 1; seen = 1; next }
        $0 == end { if (!inside) exit 1; inside = 0; next }
        !inside { print }
        END { if (inside) exit 1 }
    ' "$1"
}

quote_path() {
    local value=$1
    if [[ "$SHELL_NAME" == fish ]]; then
        value=${value//\\/\\\\}
        value=${value//\'/\\\'}
    else
        value=${value//\'/\'\\\'\'}
    fi
    printf "'%s'" "$value"
}

path_block() {
    local quoted
    quoted=$(quote_path "$BIN_DIR")
    printf '%s\n' "$BEGIN_MARKER"
    if [[ "$SHELL_NAME" == fish ]]; then
        printf 'if not contains -- %s $PATH\n    set -gx PATH %s $PATH\nend\n' "$quoted" "$quoted"
    else
        printf 'case ":${PATH-}:" in\n    *:%s:*) ;;\n    *) export PATH=%s"${PATH:+:$PATH}" ;;\nesac\n' "$quoted" "$quoted"
    fi
    printf '%s\n' "$END_MARKER"
}

register_path() {
    local file=$1
    mkdir -p -- "$(dirname -- "$file")"
    TEMP_FILE=$(mktemp)
    if [[ -e "$file" ]]; then
        strip_block "$file" > "$TEMP_FILE" || fail "Malformed PATH block: $file" "PATH 配置块损坏：$file"
    fi
    path_block >> "$TEMP_FILE"
    # Preserve permissions and follow existing user-owned dotfile symlinks.
    cat "$TEMP_FILE" > "$file"
    rm -f -- "$TEMP_FILE"
    TEMP_FILE=''
}

unregister_path() {
    local file=$1
    [[ -e "$file" ]] || return 0
    TEMP_FILE=$(mktemp)
    strip_block "$file" > "$TEMP_FILE" || fail "Malformed PATH block: $file" "PATH 配置块损坏：$file"
    cat "$TEMP_FILE" > "$file"
    rm -f -- "$TEMP_FILE"
    TEMP_FILE=''
}

# Release archives contain exactly the fourteen regular binaries at their root.
prepare_release() {
    local platform arch target asset base digest actual tool entries
    [[ "$RELEASE_VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail 'Only stable vX.Y.Z release tags are supported.' '仅支持稳定版 vX.Y.Z 标签。'
    platform=$(uname -s)
    arch=$(uname -m)
    case "$platform/$arch" in
        Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
        Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-gnu ;;
        Darwin/x86_64) target=x86_64-apple-darwin ;;
        Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
        *) fail "Unsupported platform: $platform/$arch; try install-local in a checkout." "不支持的平台：$platform/$arch；可在源码目录尝试 install-local。" ;;
    esac
    for tool in curl tar; do
        command -v "$tool" >/dev/null 2>&1 || fail "Required command: $tool" "需要命令：$tool"
    done
    if command -v sha256sum >/dev/null 2>&1; then
        HASH_COMMAND=(sha256sum)
    elif command -v shasum >/dev/null 2>&1; then
        HASH_COMMAND=(shasum -a 256)
    else
        fail 'SHA-256 verification requires sha256sum or shasum.' 'SHA-256 校验需要 sha256sum 或 shasum。'
    fi
    asset="lovelaojicli-$RELEASE_VERSION-$target.tar.gz"
    base="$REPOSITORY/releases/download/$RELEASE_VERSION"
    DOWNLOAD_DIR=$(mktemp -d)
    text "Downloading $RELEASE_VERSION ($target)…" "正在下载 $RELEASE_VERSION（$target）…"
    for tool in "$asset" "$asset.sha256"; do
        curl --fail --show-error --silent --location --proto '=https' --proto-redir '=https' \
            --connect-timeout 20 --max-time 300 --retry 3 \
            "$base/$tool" --output "$DOWNLOAD_DIR/$tool" || fail 'Release download failed; no installation changes made.' 'Release 下载失败；未更改安装。'
    done
    actual=$(cd "$DOWNLOAD_DIR" && "${HASH_COMMAND[@]}" "$asset")
    digest=${actual%% *}
    [[ $(cat "$DOWNLOAD_DIR/$asset.sha256") == "$digest  $asset" ]] || fail 'Release checksum mismatch.' 'Release 校验和不匹配。'
    # Reject unexpected/duplicate paths. Stream members instead of extracting paths,
    # so archive links and path traversal can never write outside the staging area.
    entries=$(tar -tzf "$DOWNLOAD_DIR/$asset" | LC_ALL=C sort)
    [[ "$entries" == "$(printf '%s\n' "${TOOLS[@]}" | LC_ALL=C sort)" ]] || fail 'Unexpected release archive contents.' 'Release 压缩包内容不符合预期。'
    SOURCE_BIN="$DOWNLOAD_DIR/bin"
    mkdir "$SOURCE_BIN"
    for tool in "${TOOLS[@]}"; do
        tar -xOzf "$DOWNLOAD_DIR/$asset" "$tool" > "$SOURCE_BIN/$tool"
        [[ -s "$SOURCE_BIN/$tool" ]] || fail "Empty release binary: $tool" "Release 可执行文件为空：$tool"
        chmod 755 "$SOURCE_BIN/$tool"
        actual=$("$SOURCE_BIN/$tool" --version) || fail "Cannot run $tool; check OS compatibility or use install-local." "无法运行 $tool；请检查系统兼容性或使用 install-local。"
        [[ "$actual" == "$tool ${RELEASE_VERSION#v}" ]] || fail "Unexpected binary version: $tool" "可执行文件版本不符合预期：$tool"
    done
}

prepare_local() {
    command -v cargo >/dev/null 2>&1 || fail 'Cargo is required. Install Rust first: https://rustup.rs' '需要 Cargo，请先安装 Rust：https://rustup.rs'
    command -v rustc >/dev/null 2>&1 || fail 'rustc is required.' '需要 rustc。'
    [[ -f "$ROOT/Cargo.toml" ]] || fail 'install-local must run from a source checkout.' 'install-local 必须从源码仓库运行。'
    local host
    host=$(rustc -vV | awk '/^host: / { print $2 }')
    [[ -n "$host" ]] || fail 'Cannot detect the native Rust target.' '无法检测 Rust 本机编译目标。'
    # Explicit paths and native target avoid CARGO_TARGET_DIR/build.target ambiguity.
    (cd "$ROOT" && cargo build --locked --release --workspace --target "$host" --target-dir "$ROOT/target/installer")
    SOURCE_BIN="$ROOT/target/installer/$host/release"
}

install_tools() {
    local mode=$1
    select_shell
    if [[ -L "$INSTALL_DIR" || -L "$BIN_DIR" || -L "$RECEIPT" ]]; then
        fail 'Refusing a symlinked installation directory or receipt.' '拒绝使用符号链接形式的安装目录或安装记录。'
    fi
    if [[ -e "$INSTALL_DIR" && ! -f "$RECEIPT" ]]; then
        fail "Unmanaged directory already exists: $INSTALL_DIR" "目录已存在且不属于本脚本管理：$INSTALL_DIR"
    fi
    if [[ "$mode" == install-local ]]; then
        text "Build and install: ${TOOLS[*]} → $BIN_DIR" "编译并安装：${TOOLS[*]} → $BIN_DIR"
    else
        text "Install release $RELEASE_VERSION: ${TOOLS[*]} → $BIN_DIR" "安装发布版 $RELEASE_VERSION：${TOOLS[*]} → $BIN_DIR"
    fi
    text 'PATH configuration files:' '将配置以下文件中的 PATH：'
    printf '  %s\n' "${RC_FILES[@]}"
    confirm || { text 'Cancelled.' '已取消。'; return; }
    local tool file
    for tool in "${TOOLS[@]}"; do
        [[ ! -d "$BIN_DIR/$tool" ]] || fail "Binary destination is a directory: $BIN_DIR/$tool" "可执行文件目标是目录：$BIN_DIR/$tool"
    done
    for file in "${RC_FILES[@]}"; do
        if [[ -e "$file" ]]; then
            [[ -f "$file" ]] || fail "Not a regular shell configuration: $file" "Shell 配置不是普通文件：$file"
            strip_block "$file" >/dev/null || fail "Malformed PATH block: $file" "PATH 配置块损坏：$file"
        fi
    done
    if [[ "$mode" == install-local ]]; then prepare_local; else prepare_release; fi
    for tool in "${TOOLS[@]}"; do
        [[ -x "$SOURCE_BIN/$tool" ]] || fail "Missing binary: $tool" "缺少可执行文件：$tool"
    done
    mkdir -p -- "$BIN_DIR"
    touch "$RECEIPT"
    for tool in "${TOOLS[@]}"; do
        TEMP_FILE=$(mktemp "$BIN_DIR/.install.XXXXXX")
        install -m 755 "$SOURCE_BIN/$tool" "$TEMP_FILE"
        mv -f -- "$TEMP_FILE" "$BIN_DIR/$tool"
        TEMP_FILE=''
    done
    for file in "${RC_FILES[@]}"; do
        # Record before editing so a partially completed install can be removed.
        if ! grep -Fqx -- "$file" "$RECEIPT"; then printf '%s\n' "$file" >> "$RECEIPT"; fi
        register_path "$file"
    done
    text "Installed. Open a new terminal to use: ${TOOLS[*]}" "安装完成。打开新终端即可使用：${TOOLS[*]}"
    text 'For the current terminal, run:' '当前终端可执行：'
    if [[ "$SHELL_NAME" == fish ]]; then
        printf '  set -gx PATH %s $PATH\n' "$(quote_path "$BIN_DIR")"
    else
        printf '  export PATH=%s"${PATH:+:$PATH}"\n' "$(quote_path "$BIN_DIR")"
    fi
}

uninstall_tools() {
    if [[ -L "$INSTALL_DIR" || -L "$BIN_DIR" || -L "$RECEIPT" ]]; then
        fail 'Refusing a symlinked installation directory or receipt.' '拒绝使用符号链接形式的安装目录或安装记录。'
    fi
    if [[ ! -f "$RECEIPT" ]]; then text 'No managed installation found.' '未找到本脚本管理的安装。'; return; fi
    text "Uninstall ${TOOLS[*]} from $BIN_DIR and remove managed PATH blocks?" "从 $BIN_DIR 卸载 ${TOOLS[*]} 并移除本项目的 PATH 配置？"
    confirm || { text 'Cancelled.' '已取消。'; return; }
    local file tool
    while IFS= read -r file; do
        check_path "$file"
        unregister_path "$file"
    done < "$RECEIPT"
    for tool in "${TOOLS[@]}"; do rm -f -- "$BIN_DIR/$tool"; done
    rm -f -- "$RECEIPT"
    # Do not remove unrelated files or build artifacts.
    rmdir -- "$BIN_DIR" 2>/dev/null || true
    rmdir -- "$INSTALL_DIR" 2>/dev/null || true
    text 'Uninstalled. Open a new terminal to refresh PATH and command caches.' '卸载完成。请打开新终端以刷新 PATH 和命令缓存。'
}

main() {
    local action=${1:-}
    if [[ $# -gt 1 ]]; then fail 'Expected at most one action.' '最多指定一个操作。'; fi
    case "$action" in
        -h|--help)
            text 'Usage: ./install.sh [install|install-local|uninstall] (interactive confirmation; default: menu)' '用法：./install.sh [install|install-local|uninstall]（交互确认；默认显示菜单）'
            return ;;
        ''|install|install-local|uninstall) ;;
        *) fail "Unknown action: $action" "未知操作：$action" ;;
    esac
    check_path "$HOME"
    check_path "$INSTALL_DIR"
    if [[ -z "$action" ]]; then
        text '1) Install release  2) Uninstall  3) Build and install locally  0) Exit' '1) 安装发布版  2) 卸载  3) 本地编译并安装  0) 退出'
        text 'Choose [0]:' '请选择 [0]：'
        if ! IFS= read -r action; then return; fi
        case "$action" in
            1) action=install ;;
            2) action=uninstall ;;
            3) action=install-local ;;
            ''|0) return ;;
            *) fail 'Invalid choice.' '无效选项。' ;;
        esac
    fi
    case "$action" in
        install|install-local) SHELL=${SHELL:-/bin/bash}; install_tools "$action" ;;
        uninstall) uninstall_tools ;;
    esac
}

main "$@"
