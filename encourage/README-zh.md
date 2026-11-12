# encourage

[English](README.md)

用超短字词片段生成一句温柔鼓励。显式文本**只作种子材料**：encourage 不解读情绪、不回答问题，也不按输入含义选择主题。离线运行，不访问 Git、不把 stdin 当种子，不保存输入、种子、历史或轨迹。

## 用法

在仓库根目录构建（Rust 1.89+、Cargo；Linux/macOS/WSL Unix）：

```sh
cargo build --locked --release -p encourage
./target/release/encourage
./target/release/encourage hello
./target/release/encourage --duration 30s hello
./target/release/encourage --duration 600s hello | cat
./target/release/encourage "one small step"
./target/release/encourage one small step
./target/release/encourage ""
./target/release/encourage -- --help
./target/release/encourage --help
./target/release/encourage --version
LC_ALL=zh_CN.UTF-8 ./target/release/encourage hello
```

`encourage [--duration SECONDS] [TEXT ...]` 接受可选的 UTF-8 种子文本、`--duration`、`-h/--help`、`-V/--version` 和隐藏兼容别名 `--verison`。默认播放 **15s**；`--duration` 接受 **10s 到 600s** 的整数秒（含两端），必须带小写 `s`，不接受小数、分钟或小时。这是显示选项，不进入种子；`encourage -- --duration 30s` 则把后面两个参数当作字面种子文本。不增加种子、主题、语言、速度或配色选项，也没有生产环境测试钩子。无效 UTF-8、时长值和未知选项返回本地化参数错误，不回显输入。

语言沿用共享规则，由首个非空的 `LC_ALL → LC_MESSAGES → LANG` 决定，支持简体中文和英文。显式空参数也是确定性文本，不等于随机运行。保留空白、大小写、Unicode 原始编码和参数边界：`"one small step"` 与 `one small step` 是不同种子输入。`42` 是文本，不是直接指定数字种子 42。不同输入仍可能生成同一句话。

## 普通输出与阶段回放

- 没有合适终端时立即输出最终句子及一个换行，不带 ANSI、不等待、不输出种子、JSON 或阶段统计。普通生成不采集回放预览。
- TUI 启动要求 stdin/stdout/stderr 为同一前台终端、`TERM` 非空且非 `dumb`、可用的控制终端 `/dev/tty`，并至少 **80 列 × 24 行**。重定向、管道、后台启动和初始小窗口直接普通输出，不修改 termios。
- 合适终端在指定总预算内（默认 15s）均匀回放真实且已经算完的数据流阶段，不累积逐阶段取整误差。手动暂停、小窗口暂停、挂起、后台时间和最终确认等待不消耗预算。这**不是** Rust 指令执行顺序：惰性迭代器的各操作按元素交错执行。
- **空格**暂停/继续；**Enter** 可在十秒前跳到最后阶段，再按一次确认。自动播放完成后一直等待确认，不到时自动退出；**q 始终取消**，完成页也不输出最终句子。
- 上部框内**只有文本**，保留顺序和重复，不显示 `Word`、`Atom`、`Ok(...)`、类型、ID、调试标签或计数。在首个文本输出之前，按声明顺序展示实际当前语言词库的固定词、候选变体和标点，状态栏明确标为**候选原料**。这是与种子无关的开场上下文，不冒充当前阶段输出或已拼接句子。进入文本阶段后切换为**当前输出**，仅展示该阶段的结构化文本载荷；后续无文本输出仍留空，不回退到候选池。文本自动换行，裁剪/省略提示在框外。候选池沿用最多 256 片段 / 4,096 字节的限制，不填入轨迹或拼接预览。
- 独立的**当前拼接预览**仅拼接该阶段实际已有的选定原料，可以为空、不完整、临时打乱、尚未大写或等待词语修订。真实插词/删词时同步变化，不是提前算好最终句子再打字机揭示；多个备选不伪装成一句话。仅完成后标题变为**最终句子**。
- 确认后先恢复原屏幕，再仅输出一次相同的普通句子。会话中 resize 到小于最小尺寸会暂停，不突然降级输出。`--duration` 即使为 `600s` 也不延迟普通输出。
- 非空 `NO_COLOR` 关闭高亮，不影响生成、播放或交互资格。
- 终端写入只使用本工具独立打开的非阻塞 `/dev/tty`。输出缓冲写满（终端慢、SSH 或 PTY 背压）时按单次写入最多两秒的预算重试，不因此中断当前帧；终端始终不排空才报告输出失败，并仍恢复 termios、光标和屏幕。

执行和展示使用同一份编译期嵌入的 `src/pipeline.rs`，运行时不需要源码文件、编译器、解析器、LSP 或外部命令。**34 个操作包括 28 个内容操作和 6 个明确标记的可逆过渡**，不是 34 个语义内容变换。三组过渡分别是附加/丢弃序号、单元素包装/展开、片段洗牌/按原位置复原；之后才执行真实词语插入与删除。开场候选池直接读取实际当前语言词库，不按种子预选；进入文本阶段后，原料框和预览使用实际输出的结构化观测，不解析 Debug 字符串、不借用后续阶段的词。原有带标签的诊断观测仍有界保存在内存，但不再作为原料展示；缺失的全局来源 ID 和无法确认的数量保持未知，不补造。

## 生成协议：encourage-gen-v2

此标识独立于 Cargo 包版本。V2 增加结构化原料快照并升级观测契约，**保留 v1 的 argv 种子、随机消费顺序、词库/候选排序和最终句子**，由全部 68 个有序 v1 组合及固定向量验证。修改这些映射或语义观测必须升级生成版本；纯绘图修复不必升级。播放时长不属于种子或生成协议。

- 显式 argv 编码为 `b"encourage/argv/v1\0"`、`u64_le(argc)`，再逐项追加 `u64_le(UTF-8 字节数)` 和原始字节；不合并、trim、归一化，也不混入 locale。
- BLAKE3 keyed hash 的公开 key 为 ASCII `encourage/argv-seed/v1` 右侧补零到 32 字节，摘要前八字节按小端解释为种子。该指纹**不是输入匿名化**；不要输入秘密，参数可能进入 shell 历史和进程列表。
- 仅没有文本时通过可失败的 `rand::rngs::OsRng` 接口获取八字节，失败返回 1，不退回时间、PID 或固定种子。内部保留的随机种子只供诊断，不是位置参数重放接口。
- 显式实现 wrapping SplitMix64，依次选择四个主题之一、其中两个结构之一，再按语法槽顺序选择候选。拒绝采样避免取模偏差，被拒绝的随机字在同槽重试。这是分层均匀，不保证所有句子全局等概率；生成器不用于密码学。
- 同种子的中英文共享主题/结构意图，不要求逐字翻译或后续耗数相同。每语言八个结构、34 个不同组合，由词和标点组装，不存整句候选。
- 预算固定为 4,096 个选择随机字、64 个最终词/标点原子、512 个 UTF-8 字节。超预算或不变量错误直接失败，不截断、不换种子重试、不选备用整句。
- 可逆 Fisher–Yates 过渡对每个片段使用独立的 SplitMix64 种子 2，单独限制 4,096 次取数，不推进内容选择 RNG；通过原位置精确恢复重复词和句末标记。
- 每阶段原料快照保留最多 256 个文本片段 / 4,096 个 UTF-8 字节的前缀，并明确计数被省略片段。句末边界不伪造文本；fold 快照是实际标量输出，不是所有累积前缀串在一起。普通模式不采集原料快照。

## 终端生命周期与退出

Ratatui 通过独立打开的 `/dev/tty` 绘制。cbreak 关闭规范输入和回显，但保留 `ISIG`；不使用 Crossterm raw mode、事件循环或光标位置查询。信号标记与限定于 UI 线程的作业控制信号掩码协调清理；会话守卫恢复 termios、光标和备用屏幕，不修改继承的非阻塞 I/O 模式。

Ctrl+Z 先恢复再停止；`fg` 重新检查归属并读取新快照，暂停/后台时间不推进播放。失去前台时丢弃旧快照，禁止陈旧 termios 写入、绘制和清理转义序列；后台不读键盘。SIGINT、SIGTERM、SIGHUP 和 SIGQUIT 先恢复再按默认信号动作终止（shell 中为 `128 + signal`）。

成功和 `q` 返回 0；参数错误返回 2；熵源、生成、终端和输出失败返回 1。尝试进入 TUI 后的初始化/绘制失败是错误，不伪装成普通输出成功。SIGKILL、无法执行恢复动作时的外部 SIGSTOP、abort 和终端断连无法保证清理。

## 验证

```sh
cargo fmt --all -- --check
cargo test --locked -p encourage --all-targets
cargo clippy --locked -p encourage --all-targets -- -D warnings
cargo build --locked -p encourage
python3 encourage/tests/replay_pty.py target/debug/encourage
python3 encourage/tests/ownership_pty.py target/debug/encourage
```

测试覆盖固定 argv/RNG 向量、全部 68 个 v1 兼容双语组合、10,000 个种子的双语生成与独立参考比较、预算/拒绝采样、结构化输出/阶段预览真实性、轨迹与普通输出一致性、源码元数据、纯原料绘制/裁剪、与种子无关的开场候选池、精确切换当前输出且不回退/修改轨迹、时长校验与有理数调度、输出缓冲写满时的有界重试、CLI/locale/无效 UTF-8/关闭输出，以及真实 CLI PTY。PTY 检查双语开场候选及状态标签、切换真实输出，并实测默认 15s 和指定 10s/17s 的自动播放，检查完成后等待 Enter、完成页 q 取消、600s 可跳过且不延迟重定向输出，并覆盖暂停、终止信号、resize、Ctrl+Z/恢复、前台归属替换和普通降级，检查精确 termios 恢复、备用屏幕和光标清理。熵源错误通过纯测试接口注入，不增加生产环境变量。

实际平台结果和剩余验收见[仓库路线图](../ROADMAP.md)。不能从 PTY 推断 WSL 或真实终端模拟器视觉验收已通过；完整初始化/绘制故障注入和 panic 展开 PTY 覆盖仍留待后续。

[隔离实验区](experiments/README-zh.md) 保留为历史参考，不加入 workspace 或安装器。`examples/expanded_replay.rs` 是固定种子 0 的开发驱动；CLI 验证直接针对真实的 `encourage` 二进制。
