# goodnight

```sh
goodnight
```

只读收尾：输出带 UTC 偏移的本地时间、branch/游离 HEAD、Git 未完成路径数，
以及最近的显式命令记录。有未完成路径时可建议手动运行 `later --next <text>`，
但绝不自动调用 later、保存卡片、关闭程序、锁 shell 或评判作息。
非 Git 环境仍输出时间和结束语。

## 只说有依据的内容

Git 探测复用已有本地、有预算的接口，禁用可选索引写入。未产生首个提交/裸仓库、
Git 不可用、冲突、子模块或配置过滤器时可能显示未知。重命名算一个未完成路径。
不列出用户路径；展示的 branch/程序文本转义终端控制字符。

只读取当前 worktree 的 enough/stuck 显式记录（非 Git 按 cwd）。exit 0 只代表
那个命令成功，**不意味着测试通过或全部命令成功**。启动失败与执行失败区分，
未完成、不完整或未来结果仍标未知。根据开始时间戳的秒数判断最近；同一秒多条
最新记录不猜顺序。记录可能很旧，因此显示开始 Unix 时间戳，不声称今天运行过。
没有记录即未知。

状态来自 `${XDG_STATE_HOME:-$HOME/.local/state}/lovelaojicli/` 的共享私有校验存储。
只读入口不创建目录、身份密钥、锁文件或记录，不修复损坏、不清理旧运行记录。
复用已有共享锁与写入者协调；争用、不安全权限、符号链接、缺失密钥或坏记录使
执行来源显示未知。最多扫描 10,000 条 / 32 MiB，在记录间检查 5 秒预算。
不跨工作区扫描，不推测 shell 历史或终端活跃情况。

本地时间仅启用可选 `cli-common/local-time` 中 jiff 的系统时区/zoneinfo 支持，
遵从 `TZ`。不内置时区数据、不联网；时区不可用则时间未知，不偷偷回退 UTC。

## CLI 与验收

无业务参数，不读取 stdin。支持 `--help`、`--version`、`--verison`。
按首个非空 `LC_ALL` → `LC_MESSAGES` → `LANG` 选择中英文。
成功（含保守标未知）为 0，输出错误为 1，用法错误为 2。
Ctrl+C 保持默认终端信号行为；没有隐藏测试钩子。

目标平台为 Linux/macOS/WSL Unix；本轮只在 Linux 验证，macOS/WSL 及完整
文件系统故障注入仍待补。

```sh
cargo test -p goodnight
cargo build --locked --workspace --release
python3 tests/report_records_pty.py target/release
```
