# later

离开前保存一条下一步，让脑内上下文落盘。[English](README.md)

```sh
later
later --next "检查 refreshToken 为什么没有调用"
later resume
```

交互只问“紧接着要做的一件事是什么？”。确认 stdin/stderr 为前台终端后，
才从 `/dev/tty` 读取；没有可用终端且未给 `--next`，返回 125。
不把管道数据当回答，EOF、空白或无效回答不覆盖旧卡片。
显式参数去掉首尾空白后必须为非空单行，最多 2,000 个 Unicode 字符；无效参数返回 2。
支持帮助、版本和 `--verison`；语言按 `LC_ALL → LC_MESSAGES → LANG` 选择。

## 只有最新一张卡片

Git 内按规范化 worktree 根目录定位，从子目录调用仍是同一张，其他 worktree 分开。
没有 Git 或 Git 不可用则按规范化 cwd 保存，并明确 Git 上下文缺失。
不是任务管理器：没有 list/show/clear、历史浏览、全文 diff、账号、联网、shell history
读取或编辑器自动恢复。

卡片保存 Unix 秒时间戳、工作区、可选仓库/分支/HEAD、最多 200 个 porcelain 文件状态
路径及总数/截断标记、有界 diff stat、最近提交的 ID/时间、下一步，及近 30 天
最近一条 enough/stuck 真实失败摘要（程序 basename、时间和退出状态）。
路径以可逆 Unix 字节保存，显示时转义终端控制字符。不保存提交正文或命令输出。
Git 采集失败降级为目录卡片；不为此改造 patience 的记录行为。

`resume` 只显示保存时的上下文，不执行下一步，不 checkout；当前分支/HEAD 不同会提示。
无卡片友好返回 0，记录损坏/版本未知返回 1，不展示半张卡片。
显式有效保存可原子替换旧卡片，完整同步写入成功后才输出保存成功提示。

## 状态与隐私

根目录 `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/`；XDG 无效回退绝对 HOME，
不写 cwd 或 `/tmp`。受管理目录 0700、文件 0600；拒绝符号链接、不安全权限和锁争用。
采用同目录原子替换及短时 OS 工作区锁。卸载保留卡片，最新卡片不会自动过期。
密钥丢失、锁与文件系统限制见根 README。

**下一步是本地明文，不要填写密码、令牌或其他秘密。**
`--next` 还可能进入 shell 历史和进程列表。路径、文件名、Git 信息、程序名本身也可能敏感；
摘要不是加密。无遥测、无后台采集，不依赖 DISPLAY、Wayland 或桌面服务。

测试：`cargo test -p later`（需要 Git，测试隔离 HOME/state）。
