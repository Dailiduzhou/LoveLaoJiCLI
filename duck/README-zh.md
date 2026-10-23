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

## 已知长输入限制

交互仍使用系统 canonical 行缓冲，其**字节**容量可能小于应用的字符上限；
长粘贴可能丢失字节/换行或一直等待。macOS 已在 duck 的 600 汉字场景复现，
不能据此把其他调用者的全部场景都认定为已复现，也**不能声称已修复**。
长回答请使用显式 `--expected / --actual / --last-change / --experiment` 参数（不要填写秘密），管道不是交互回答；
提示卡住时可 Ctrl+C 取消。安全的共享输入重设计仍需终端生命周期/依赖边界审批，
不直接复用 afk 的 cbreak 代码。

已知失败回归：`cargo test -p duck long_interactive_input_contract -- --ignored --nocapture`。
常规套件显式忽略它，不计为通过；覆盖 canonical 缓冲、字符上限、ASCII/中文/混合及分批输入。
进度见 [路线图](../ROADMAP.md)。

测试：`cargo test -p duck`（终端测试需要 Python 3）。
