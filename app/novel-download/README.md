# Novel Download

Novel Download 是一个基于 GPUI、面向 `zgzl.net` 的桌面小说下载工具。输入受支持的小说 ID 或链接后，应用会从指定位置开始抓取，并把后续内容保存为本地纯文本文件。

## 支持的输入

| 输入 | 示例 | 下载范围 |
| --- | --- | --- |
| 小说 ID | `otew` | 从第一章开始，直到小说末尾 |
| 详情页 | `https://m.zgzl.net/info_otew/` | 从第一章开始，直到小说末尾 |
| 章节页 | `https://m.zgzl.net/read_otew/68hq7.html` | 从该章第一页开始，直到小说末尾 |
| 分页 | `https://m.zgzl.net/read_otew/68hq7_3.html` | 从该页开始，直到小说末尾 |

详情页也接受 `https://www.zgzl.net/info_<id>`。所有 URL 必须使用 HTTPS；章节与分页 URL 只接受 `m.zgzl.net`，且不接受端口、用户信息、查询参数或第三方地址。

## 当前功能

- 解析小说名、作者、章节列表、章节标题和分页正文。
- 同一时间运行一个下载任务，支持开始与取消，并显示当前状态、元数据、写入数量和抓取 URL。
- 对连接、超时、响应体、HTTP 408、429 和 5xx 等短暂故障最多重试 3 次，每次间隔 1 秒。
- 重定向目标必须保持为 `https://m.zgzl.net`。
- 按网络、HTTP 状态、页面解析、范围、输出与暂存清理等类别展示错误。
- 提供英语和简体中文界面。

## 输出与数据

- 成品写入操作系统的“下载”目录，文件名为 `<安全化后的小说名>by<安全化后的作者>.txt`。
- 下载期间先写入同目录的 `.part` 文件；全部成功后才同步并发布最终文件。
- 已存在同名成品或 `.part` 时不会覆盖。普通失败、取消或窗口关闭会清理本次任务拥有的暂存文件；进程被强制结束时仍可能遗留 `.part`。
- 文件名会替换控制字符和跨平台非法字符、处理 Windows 保留名，并限制标题和作者部分的长度。
- 当前没有数据库、缓存或用户配置；持久化内容只有下载文件与应用日志。
- 仅下载你有权访问和保存的内容，并遵守来源站点的使用条款。

## 当前限制

- 仅支持 `zgzl.net`，页面结构变化可能导致解析失败。
- 章节或分页链接表示下载起点，不是只下载单章或单页。
- 尚不支持任务队列、断点续传、结束范围、输出目录选择、覆盖已有文件或下载前元数据预览。

## 目录结构

```text
app/novel-download/
├─ src/main.rs                     # 日志、GPUI 初始化与主窗口
├─ src/errors.rs                   # 应用与下载领域错误
├─ src/foundation/i18n.rs          # Fluent 加载与系统语言检测
├─ src/features/workspace.rs       # 单页 UI、任务与取消协调
├─ src/features/workspace/form.rs  # 输入表单与提交时验证
├─ src/features/workspace/runtime.rs # 下载状态机
├─ src/crawler.rs                  # 下载引擎、进度与结果
├─ src/crawler/source.rs           # 输入解析与 typed range
├─ src/crawler/source/zgzl/        # 小说、章节与分页解析
├─ src/crawler/http.rs             # HTTP、重定向与重试
├─ src/crawler/output.rs           # .part 文件事务与安全发布
├─ locales/                        # en-US、zh-CN 与 macOS bundle 文案
├─ build-assets/                   # 打包图标
└─ docs/dev/                       # 实施计划与迁移记录
```

## 开发

环境基线与系统依赖见[工作区 README](../../README.md)。以下命令均从 workspace 根目录执行：

```bash
cargo run -p novel-download
cargo check -p novel-download --bin novel-download --all-features --locked
cargo test -p novel-download --bin novel-download --all-features --locked
cargo fmt --all -- --check
cargo clippy -p novel-download --all-targets --all-features --locked -- -D warnings
cargo run -p xtask -- bundle novel-download
```

开发计划与迁移记录见 [docs/dev/README.md](docs/dev/README.md)。
