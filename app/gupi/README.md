# Gupi

基于 GPUI Kit 的 Pi 桌面宿主。第一阶段包含初始设置、配置恢复、主题/语言、窗口布局和 Pi 版本检查；对话/RPC 由 #219 继续。

## 运行

```sh
cargo run -p gupi
cargo run -p xtask -- bundle gupi
```

默认配置：系统 config 目录下 `gupi/config.toml`；布局：同目录 `state.toml`。
`GUPI_CONFIG_DIR` 可覆盖该目录，`GUPI_LOG_DIR` 可覆盖日志目录；相对覆盖路径在启动时按工作目录转换为绝对路径。
macOS 默认日志为 `~/Library/Logs/gupi/gupi.log`，其他平台为系统 data-local 目录下 `gupi/logs/gupi.log`。

```toml
# pi_command 缺省时从应用继承的 PATH 查找 pi。
# pi_command = "/absolute/path/to/pi"
theme = "system" # system / light / dark
# light_theme = "preset:Default Light"
# dark_theme = "preset:Default Dark"
language = "system" # system / english / chinese
```

首次缺少配置时，依次展示欢迎介绍、语言、外观和 Pi 配置；各页共享一个 `Form<AppConfig>`。语言与主题立即预览，Pi 检测通过后点击完成才创建配置。语言使用可搜索 Combobox，主题以浅色、深色两组自适应单选卡片网格展示，分别保存选择。页面使用 240ms 缓入缓出横向过渡，尊重系统减少动态效果设置；主题分组标题吸顶，悬停卡片显示完整名称。配置恢复不会读取或修改 Pi 凭据。
保存采用同目录临时文件和磁盘字节快照检测；外部修改需要重读或明确备份后覆盖。
写回已应用配置保留当前草稿；保存/重读失败保留内存与草稿。提交结果不确定时仅允许重读。
默认窗口为 960×740，最小为 800×600；旧的小尺寸记录恢复时扩至新下限，并按屏幕范围约束。窗口布局损坏须明确备份并重置，退出时的布局保存错误只记录日志。

Pi 检查仅执行指定程序的 `--version`，5 秒超时，stdout/stderr 各限制 16 KiB；不启动交互式 shell，不安装 Pi/Node，不调用模型。
已应用命令的启动检查与设置草稿检查各自持有独立的 Operation/Task；主页仅接受与当前已应用命令匹配的成功结果。草稿检测期间及成功后保留恢复设置页，保存成功后才根据新配置进入主页。
保存期间禁止修改表单；失败后保留草稿，仍用保存按钮提交当前内容。文件冲突后的备份覆盖会在确认时重新校验当前草稿，不重放旧提交。
macOS/Windows 关闭窗口隐藏，重新唤起恢复；显式退出等待已开始的配置提交、布局恢复及两种 Pi 探测收尾。所有业务 Task 均由对应 Operation 或根视图字段持有。

## 开发期边界与验证

- 使用用户指定的 Pi 官方开发期图标；来源与许可状态见 [图标记录](build-assets/icon/README.md)。
- 使用根 manifest 锁定的 GPUI Kit 0.6.0；锁文件仅新增 Gupi 包条目。
- Gupi 已通过 `cargo build -p gupi --locked --offline`、`cargo test -p gupi --locked --offline`（10 项）和 `cargo clippy -p gupi --all-targets --all-features --locked --offline -- -D warnings`；打包接入的 xtask 已通过 13 项测试及同等 Clippy 检查。
- macOS 当前开发构建已验证启动引导、语言/浅暗主题切换、分组吸顶、前进/返回及滚动位置保持、最小窗口与旧尺寸恢复、完整名称提示，以及临时 Pi fixture 的检测和完成设置；未调用模型。
- 配置错误恢复页已验证：草稿检测成功保留表单与原磁盘配置，保存后进入主页；设置页底部操作及主题列表最后一行均可滚动到达。
- Windows npm shim、Linux 实机行为、当前 release bundle 与完整发行矩阵尚未验证。
- [第一阶段计划](docs/dev/issue-218/README.md)及[运行时契约](docs/dev/issue-218/runtime.md)。
