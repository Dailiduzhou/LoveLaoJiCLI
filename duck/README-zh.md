# duck

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

测试：`cargo test -p duck`（终端测试需要 Python 3）。
