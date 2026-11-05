# duck

[English](README.md)

把问题说清楚；不提供建议、不调用 AI 或搜索、不持久化。

```sh
duck
duck --expected "成功" --actual "超时" --last-change "重试逻辑" --experiment "关闭重试"
duck --expected "成功"       # 只询问其他三项
```

问题和结果按固定顺序显示：

```text
预期结果: 成功
实际结果: 超时
最近改动: 重试逻辑
下一次实验: 关闭重试
quack.
```

回答 trim 后须为非空单行、最多 2,000 个 Unicode 字符，不含控制字符。
显式参数无效返回 2。四项齐全则不提问，否则按顺序询问缺项；复用前台检查：
stdin/stderr 必须为终端且 stdin 属于当前前台进程组。提示走 `/dev/tty`，不写 stdout。
EOF、空白/无效交互回答或无法交互时返回 125，不输出不完整结果。绝不把管道当回答。
Ctrl+C 使用终端默认信号行为，不改变终端设置。

完整结果走 stdout，错误走 stderr；需要保存时自行重定向。工具不打开状态目录，
不调用任何服务。参数仍可能进入 shell 历史和进程列表，请勿填写秘密。
I/O 错误为 1，成功为 0；没有隐藏测试钩子。

支持 `--help`、`--version`、兼容拼写 `--verison`。
语言按首个非空 `LC_ALL` → `LC_MESSAGES` → `LANG` 选择，zh-CN/zh_CN/zh 为中文，
其余回退英文。目标平台为 Linux/macOS/WSL Unix；本轮在 Linux 验证，包含真实前台 PTY。

## 交互长输入

提问以非 canonical（cbreak）模式读取控制终端，每次读一个字节，不再依赖内核
canonical 行缓冲。该缓冲有字节上限（macOS 为 1024），过去会让长粘贴丢失或挂住。
现在 2,000 字符以内的回答可以完整成功，更长或无效的回答明确拒绝（返回 125），
不会挂住；只读取到提交的换行为止。

- 编辑：退格、Ctrl+U（删除整行）、Ctrl+W（删除一个词）使用终端配置的按键；
  方向键/功能键序列被忽略；空行上的 Ctrl+D 为 EOF。
- 终端信号保持有效（ISIG）：Ctrl+C/Ctrl+\ 以及 SIGTERM/SIGHUP 先恢复 termios
  再结束；Ctrl+Z 先恢复再挂起，`fg` 后重新显示问题和已输入的部分；后台时不读取也不绘制。
- 被拒绝或取消的粘贴会被丢弃，剩余内容不会流入 shell 或被包装的子命令。
- SIGKILL、外部 SIGSTOP、abort 和终端断连无法保证恢复。

显式 `--expected / --actual / --last-change / --experiment` 参数会完全跳过提问（不要填写秘密；管道不是交互回答）。
`cargo test -p duck long_interactive_input_contract` 覆盖 canonical 缓冲与字符边界、
ASCII/中文/混合粘贴及分批输入。进度见 [路线图](../ROADMAP.md)。

测试：`cargo test -p duck`（终端测试需要 Python 3）。
