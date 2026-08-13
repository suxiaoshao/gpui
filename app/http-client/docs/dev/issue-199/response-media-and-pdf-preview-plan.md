# Issue #199：Response 音频、视频与 PDF 只读预览实施计划（历史基线）

## 状态、范围与执行边界

- 状态：`Superseded`
- 子任务：`HTTP-199-04`
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 后继 issue：[#200](https://github.com/suxiaoshao/gpui/issues/200)，其
  [root hub](../../../../../docs/dev/issue-200/README.md) 是未完成 native runtime、许可、fixture、打包与
  三平台验证的唯一执行入口。
- 目标分支：`codex/199-adopt-gpui-store-form-operation`
- primary owner：`app/http-client`；secondary owner：`crates/xtask`、`script` 与 CI 的 app-specific
  GStreamer 安装/打包钩子；[子任务索引](README.md)
- 产品决定权威：[HTTP Client 产品与迁移草稿](http-client-product-and-migration-draft.md)
- 前置：`HTTP-199-03` 已由提交 `24e4a9f` 交付并推送；本计划只消费其中的
  `RequestRuntime::Ready { response: Arc<ResponseData> }`、`ResponseReadLease`、50 MiB 完整
  Response 上限和现有 Response pane。
- 文档语言：中文；类型、API、crate、协议、命令与上游项目名保留源码拼写。
- 待确认问题：无。用户已确认 PDF 采用 app-local 极简 viewer，其余媒体问题按本文推荐方案执行。
- `Ready` 表示产品选择、owner 边界、producer/consumer gate、consumer 目标 API、状态、文件动作和验收已经
  闭环。fork 的最终 façade/commit、native artifact checksum 与 plugin/license manifest 是 `WP-1700`
  必须先产出的工程证据；它们不是允许 consumer 猜测的占位值，也不是新的用户决策。
- 当前已进入实施阶段；按 `WP-1700` producer gate 开始修改依赖、fork、runtime、代码与打包配置。
- 实施证据（截至 2026-08-11）：视频 fork 已固定为
  [`suxiaoshao/gpui-video-player@4f1a6cc49ddab9d0afc73404afc259bba73d6407`](https://github.com/suxiaoshao/gpui-video-player/commit/4f1a6cc49ddab9d0afc73404afc259bba73d6407)；
  `http-client` 已实现 Response asset、媒体/PDF 私有运行态、音频/视频/PDF viewer、局部错误投影和双语
  文案。该应用工作树仍在实施中，尚无本轮 app 提交。
- 已执行的自动化证据：fork 的 `cargo fmt --all -- --check`、`cargo test --all-features`、
  `cargo clippy --all-targets --all-features -- -D warnings` 均通过；app 的完整测试 154 项、媒体定向测试
  19 项、PDF 定向测试 12 项、i18n 定向测试 4 项通过，且 app Clippy、全 workspace 格式检查与
  `git diff --check` 通过。实际桌面 UI、真实音视频播放、PDF 手工翻页、安装包和三平台验证均**未执行**。
- 移交时的 release gate：`C-1701` 尚未闭环。GStreamer runtime manifest、第三方 notices、受许可审计的
  fixture corpus 以及 macOS/Windows/Linux 的发行包/安装后 plugin 验证均未完成；不得将本计划或任何
  Audio/Video 发行能力标为 `Done`，也不得以 Cargo 测试替代这些证据。
  当前审计已确认：macOS 官方 1.28.5 payload 可得到包含目标 codec 的 54-file dylib closure，但尚缺
  runtime data、loader/rpath/codesign 与逐组件许可文本验证；Windows 官方 1.28.5 MSVC x86_64 installer
  已按官方 SHA-256 校验，但其 Inno Setup 6.7.0 payload 无法由当前 macOS 上的 `innoextract` 1.9 或
  `7zz` 解包，因而仍不能列出递归 DLL/plugin/许可闭包。为避免用未审计的 release manifest 反向阻塞首次
  盘点，新增仅手动触发的 Windows discovery workflow：它以已校验的官方 URL/SHA 安装到隔离 runner，保存
  文件 SHA-256、`gst-inspect` 原始输出、实际 element→plugin metadata、候选许可文件与 PE 静态 import
  闭包为 artifact。该 artifact 是待人工审计的输入，不是 manifest/notice，也不自动证明许可证、source
  offer、codec 专利或 delay-load/`LoadLibrary` 完整性；人工冻结 manifest 后，release/bundle gate 仍只
  读取该 manifest，普通源码 CI 只验证独立的开发 SDK contract。
  Linux 仅有目标系统包契约，尚未在 runner 验证解析版本、元素与 `.deb` depends。三者都不能用占位 manifest
  代替。

### 目标

在不改变单请求 Send runtime 的前提下，为已经完整接收的 `Ready` Response 增加可销毁的音频、视频与
PDF 只读预览：按 `Content-Type` 自动选择，也允许用户手动尝试；媒体默认不自动播放；任何解析、解码、
播放或渲染失败都只影响 viewer，不改变成功的 HTTP Response。

### 明确范围

1. `ViewerMode` 增加 `Audio`、`Video`、`Pdf`，Auto 识别 `audio/*`、`video/*` 与
   `application/pdf`。
2. 音视频共用 app-local GStreamer `MediaSession`，以私有
   `Transition<MediaMessage>` 管理准备、播放、暂停、结束、失败与停止；不使用预定义
   `refresh::Operation` 或 `repair::Operation`。
3. 视频消费窄范围、精确 pin 的 `gpui-video-player` fork；音频直接使用同一 GStreamer runtime，UI
   组合现有 `gpui-component` 控件。
4. PDF 直接使用 `hayro 0.7.1`，由 app 自己实现当前页、总页数、上一页与下一页的单页 CPU 光栅化
   viewer。
5. 补齐 Response asset 生命周期、三平台 native runtime/plugin、许可证/notice、CI、打包、样本与资源预算。

### 非目标

- 不修改 `RequestRuntime::{Idle,Sending,Receiving,Ready,Failed}`、transport、redirect、collector、Save、
  Form 或 `PreparedRequest`；不为媒体再请求原 URL。
- 不在 `Receiving` 展示或播放部分 body，不提高 50 MiB capture cap，不实现 streaming media 或
  `Send and Download`。
- 不加入 History、multi-tab、Store、Cookie、Environment、repair、自动 retry 或持久化。
- 不支持 H.265、AV1、4K、字幕、倍速、全屏、画中画、循环或媒体编辑。
- PDF 不支持缩放、连续多页滚动、目录、搜索、文本选择、链接、批注、表单、密码输入或编辑。
- 不依赖或 fork `gpui-pdf`，不引入 `zpdf` reader、WebView、Rodio、Symphonia 或直接 FFmpeg API。

## 系统面适用性

| ID | 系统面 | 本计划结论 | 工作包 |
| --- | --- | --- | --- |
| `S-1700` | 入口与生命周期 | `RequestView` 继续是唯一页面 owner；新 Send、Clear、mode 切换与页面 drop 统一停止 preview | `WP-1701`–`WP-1706` |
| `S-1701` | action / command / keybinding | 只增加 viewer 内播放、暂停、seek、音量、静音与 PDF 翻页；不加全局快捷键 | `WP-1703`–`WP-1706` |
| `S-1702` | UI 结构与交互 | 复用现有 Response Body toolbar；视频/页面占满 body 余下区域，音频显示紧凑控制条 | `WP-1703`–`WP-1706` |
| `S-1703` | focus / native identity | 控制 Entity 在同一 Response/mode 下稳定；进度事件不得重建 slider 或 request controls | `WP-1703`、`WP-1704`、`WP-1706` |
| `S-1704` | 异步与 Task owner | materialize、media driver、bus/event bridge 与 PDF render 都由 Response pane/session 持有，禁止 detach | `WP-1701`–`WP-1706` |
| `S-1705` | Operation / 状态机 | 媒体与 PDF 使用 app 私有 `Transition<Message>`；不改 HTTP runtime，不套预定义 family | `WP-1702`、`WP-1705` |
| `S-1706` | Form | 不适用；viewer 不读写 `Form<RequestDraft>` | — |
| `S-1707` | Store | 单页面派生 viewer，不引入 Store、Global 或第二份 Response authority | — |
| `S-1708` | native / 外部协议 | GStreamer、平台 decoder/audio sink、`gpui-video-player` fork 与 `hayro` 适用 | `WP-1700`、`WP-1702`–`WP-1705` |
| `S-1709` | 错误与恢复 | 全部是 viewer-local problem；音视频只有一次实例级 software fallback，没有 retry/repair | `WP-1702`–`WP-1706` |
| `S-1710` | 数据库与持久化 | 不适用；派生临时文件随媒体 asset owner 回收 | `WP-1701` |
| `S-1711` | generated / synchronized 内容 | `Cargo.lock` 由 Cargo 更新；native runtime manifest/notice 由审计脚本生成并验证 | `WP-1700`、`WP-1707` |
| `S-1712` | assets / fixtures | 增加有来源与许可证记录的小型 codec/PDF fixture；不把 runtime binary 提交进 Git | `WP-1700`、`WP-1707` |
| `S-1713` | Fluent i18n | 两 locale 同步增加 mode、控制、加载、错误与 PDF 页码文案 | `WP-1706` |
| `S-1714` | 安全与隐私 | 不执行响应内容、不记录 URL/header/body/path；PDF/media 有尺寸、像素、队列和次数预算 | `WP-1701`–`WP-1707` |
| `S-1715` | tracing | 只记录安全的 mode、phase、decoder factory、caps 摘要、尺寸、计数和 problem kind | `WP-1702`、`WP-1707` |
| `S-1716` | packaging / CI | macOS/Windows 私有携带审计 runtime；Linux `.deb` 声明系统包；三平台验证 plugin contract | `WP-1700`、`WP-1707` |
| `S-1717` | 依赖与许可 | 精确 pin Rust crate/fork；增加 GPL 许可选项但保留 MIT；bundle 记录实际 native 许可 | `WP-1700` |
| `S-1718` | owner 文档 | 本文件是执行权威；README 只记录状态和入口，草稿保留未实施产品决定 | `WP-1707` |
| `S-1719` | 验证证据 | pure、GPUI、codec/PDF fixtures、资源预算、三平台 plugin/package smoke 与手工播放 | `WP-1707` |

## 当前证据、复用审计与固定决定

| ID | 分类 | 已核实事实或决定 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| `E-1700` | 当前事实 | `HTTP-199-03` 已由 `24e4a9f` 交付，`Ready` 持有 `Arc<ResponseData>`。 | `request/runtime.rs`、Git HEAD | 只扩展 Ready viewer，不修改网络状态机。 |
| `E-1701` | 当前事实 | `ResponseData` 私有持有 Memory/`TempPath`，`ResponseReadLease` 可读 prefix 或精确复制；8 MiB spill、50 MiB cap 已验证。 | `response/data.rs`、`collector.rs` | 派生 asset 只能经 read lease 建立，不暴露现有裸路径。 |
| `E-1702` | 当前事实 | `ResponsePane` 已持有 mode、projection、preview/save Task，并以 `Arc::ptr_eq + ViewerMode` 拒绝迟到投影。 | `response.rs`、`request.rs` | 延用相同 identity，PDF 另加 page generation。 |
| `E-1703` | 当前事实 | 现有 Auto 只分 Text/JSON/XML/Image/Bytes；PDF、audio、video 都落入 Bytes。 | `response/decoding.rs`、`viewer.rs` | 扩展 `ContentKind` 与 mode，不改变既有安全文本/图片路径。 |
| `E-1704` | 上游事实 | GStreamer 是 pipeline/plugin 框架；decoder 由 registry/caps/rank 自动选择，`gst-libav` 是 FFmpeg/libavcodec bridge。 | GStreamer 官方文档 | 不重写 demux/clock/seek，不直接引入 FFmpeg API。 |
| `E-1705` | 上游事实 | 全局 `ElementFactory::set_rank`/`GST_PLUGIN_FEATURE_RANK` 会影响进程；decodebin autoplug 与 software decoder 选择可逐实例控制。 | GStreamer playback API | `Auto`/`SoftwareOnly` 必须落在当前 pipeline，禁止修改全局 rank。 |
| `E-1706` | 上游事实 | `gpui-video-player` upstream `beb23b09f64d670446099ab902cc048074917993` 已有 GPUI element/GStreamer bridge，但初始化同步等待、错误只日志、Drop join 与 `max-buffers=200` 不符合 owner 契约。 | upstream 源码审计 | 复用分类为 Adapt，建立窄 fork 并先交付生命周期 contract。 |
| `E-1707` | 上游事实 | `hayro 0.7.1` 为 `MIT OR Apache-2.0`、纯 CPU renderer；`RenderCache` 为 `!Send + !Sync`，加密 PDF 不受支持。 | `hayro 0.7.1` 文档 | PDF worker 内创建并持有 parser/cache，不能把 cache 放进 GPUI Entity 或跨线程移动。 |
| `E-1708` | 当前事实 | 当前 CI 是 macOS/Linux/Windows；bootstrap 与 xtask 均没有 GStreamer SDK/runtime/plugin 安装和 staging。 | `.github/workflows/ci.yml`、`script/*`、`crates/xtask/src/bundle/*` | native dependency、CI 与发行打包必须同轮完成。 |
| `E-1709` | 实施证据 | 窄 fork 已固定到 `4f1a6cc49ddab9d0afc73404afc259bba73d6407`；fork 定向 fmt/test/clippy 通过，并与 workspace 解析为同一 GPUI source identity。 | fork commit、fork Cargo 验证、`cargo tree -p http-client -d` | `C-1700` 的 fork source producer 已完成；consumer 可以使用其非阻塞生命周期/视频 element contract。 |
| `E-1710` | 实施证据 | app 已接入 Response asset、Media/PDF runtime、audio/video/PDF viewer 与 i18n；完整测试 154 项，以及媒体 19、PDF 12、i18n 4 项定向证据通过，Clippy/格式/diff-check 通过。 | `app/http-client/src/features/request/response/*`、定向 Cargo 命令 | `WP-1701`–`WP-1706` 已有代码级实施证据，但仍须补齐完整 codec/许可 fixture 与发行验证。 |
| `E-1711` | Release-gated | `runtime-manifest.toml`、`THIRD_PARTY_NOTICES.md`、许可审计 fixture corpus 与三平台安装包 smoke 尚不存在或未验证。 | `app/http-client/build-assets/gstreamer/`、native 打包入口与当前验证记录 | `C-1701`/`WP-1707` 阻塞；只能维持 `In progress`。 |
| `D-1700` | 产品决定 | 音频、视频、PDF 仅预览完整 `Ready` Response；不重新请求、不编辑、不回写。 | 产品草稿 | viewer 是可销毁投影，ResponseData 仍是唯一 authority。 |
| `D-1701` | 产品决定 | Auto 按 Content-Type；用户可手动尝试；音视频默认不自动播放。 | 产品草稿 | 进入 Audio/Video 后停在 Paused，只有明确 Play 才播放。 |
| `D-1702` | 数据决定 | BodyDecoding::Unsupported 时 Audio/Video/Pdf mode 不可用；失败后仍保留 Headers、Hex/Base64 与 Save。 | Response 现有契约 | 不把 content-encoded 原始 bytes 交给 parser/player 猜测。 |
| `D-1703` | 生命周期决定 | 音视频总是经 `ResponseReadLease` 精确复制到 session-owned `TempPath`；即便原 Response 已 spill，也不暴露其路径。 | owner 审计 | 最多增加一份 50 MiB 临时拷贝，换取单一、清晰的文件生命周期。 |
| `D-1704` | 媒体依赖 | 音视频共用 GStreamer；不另引入 Rodio/Symphonia，不直接使用 FFmpeg/libavcodec。 | 用户决定与上游审计 | 只有一个媒体 runtime、插件矩阵与打包链路。 |
| `D-1705` | decoder policy | 首次使用实例级 `Auto`；发生 decoder/pipeline failure 时最多重建一次 `SoftwareOnly`，从最后确认位置 seek 恢复；重建或 seek 失败即 viewer-local Failed。 | 用户决定与推荐方案 | 不热切换、不全局改 rank、不从零静默重播。 |
| `D-1706` | 视频复用 | 维护精确 pin 的 `gpui-video-player` 窄 fork；只修 GPUI revision、异步初始化、typed event/error、abort-on-drop stop、实例策略与有界帧队列。 | 用户决定与复用审计 | fork 之外的字幕、倍速、播放列表等功能一律不接收。 |
| `D-1707` | 格式与预算 | MVP 支持 MP4/H.264/AAC、WebM/VP8/VP9/Opus、MP3、WAV、FLAC、Ogg Vorbis/Opus；视频像素包络不超过 1920×1080（横竖屏均按长边≤1920、短边≤1080）。 | 产品草稿 | 超出上限不解码展示，返回局部预算错误。 |
| `D-1708` | 视频桥接 | 首版保留 appsink→CPU→GPUI 路径；appsink `drop=true,max-buffers=3`，Rust presentation queue 最多 2 帧。 | 推荐方案 | profiling 未通过时停止并修订计划，不静默改为无界或另写 renderer。 |
| `D-1709` | PDF | `hayro 0.7.1` + app-local `PdfPreview`；只显示单页与上一页/下一页，无 zoom；加密 PDF 明确不支持。 | 用户决定 | 不 fork `gpui-pdf`，兼容性失败只触发重新评估 `pdfium-render`。 |
| `D-1710` | PDF 预算 | 完整 PDF 仍受 50 MiB cap；最多 10,000 页；单页长边≤4096、像素 buffer≤64 MiB，同时只持有一张页面图。 | 安全推荐 | checked arithmetic 后才分配/渲染；超限为局部错误。 |
| `D-1711` | 清理顺序 | 新 Send、Clear、mode/Response 切换先安装新 viewer 状态，再停止旧 Task/session；页面 drop 取消 owner Task。 | GPUI/Transition 契约 | 任何 user-defined Drop、pipeline stop 或 TempPath 删除前，owner 都处于合法状态。 |
| `D-1712` | native 分发 | macOS arm64 app 使用官方 Universal GStreamer 1.28.5 runtime，Windows MSVC x86_64 私有携带同版本 runtime/plugin；Linux `.deb` 使用发行版 ≥1.20 系统包并声明依赖。 | 官方安装/部署文档与当前 bundle 结构 | 不承诺 Universal app；不要求最终用户手工安装 macOS/Windows runtime；Linux 以包管理器解决 ABI。 |
| `D-1713` | 许可 | `http-client` 源码许可从 `MIT` 扩为 `MIT OR GPL-3.0-or-later`，不是删除 MIT；每个平台仍按实际 plugin/libav 构建记录并履行许可证。 | 用户确认 | 双许可不能替代 notice、source offer、专利与插件级审计。 |
| `D-1714` | UI | 只用现有 Button、Slider、Progress、Alert、Select、Label 与 GPUI image/surface element；不造通用组件库。 | gpui-component 审计 | 复杂解码归 backend，app 只做 feature-local viewer。 |
| `D-1715` | MIME 手动覆盖 | `application/pdf`、`audio/*`、`video/*` 自动选；未知 MIME 保持 Hex。手动 Audio/Video/Pdf 可让后端 typefind/parse，失败不改 Response。 | Postman 类交互与用户决定 | 不根据 filename 或 body 内容偷偷改变 Auto。 |
| `D-1716` | 完成标准 | Cargo 编译不足以证明交付；三平台 native/plugin/package smoke 与实际播放/翻页是 Done 门禁。 | native dependency 审计 | 无相应证据时只能保持 `In progress`。 |

### 上游权威资料

- [GStreamer 下载与当前 stable runtime](https://gstreamer.freedesktop.org/download/download.html)、
  [macOS 安装](https://gstreamer.freedesktop.org/documentation/installing/on-mac-osx.html)、
  [Windows 安装与部署](https://gstreamer.freedesktop.org/documentation/installing/on-windows.html)、
  [Linux 安装](https://gstreamer.freedesktop.org/documentation/installing/on-linux.html)。
- [GStreamer Rust crate `0.25.2`](https://docs.rs/crate/gstreamer/0.25.2)、
  [`uridecodebin` 逐实例 decoder 策略](https://gstreamer.freedesktop.org/documentation/playback/uridecodebin.html)
  与 [GStreamer licensing FAQ](https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/licensing.html)。
- [`gpui-video-player` fork 基线提交](https://github.com/cijiugechu/gpui-video-player/commit/beb23b09f64d670446099ab902cc048074917993)。
- [`hayro 0.7.1`](https://docs.rs/crate/hayro/0.7.1) 与
  [`hayro` API](https://docs.rs/hayro/0.7.1/hayro/)。

### 上游与仓内复用结论

| 能力 | 分类 | 采用方案 | 明确拒绝 |
| --- | --- | --- | --- |
| Response bytes/临时文件 | Reuse | `Arc<ResponseData>` + `ResponseReadLease`，新增私有完整读取/asset materialize | 第二份业务 Response、裸 `PathBuf`、重新请求 URL |
| 状态转换 | Reuse | `gpui_operation::Transition` trait，媒体/PDF各自 app-private complete enum | `refresh`/`repair` family、phase bool、命令式兼容层 |
| 音视频 pipeline | Reuse | GStreamer playbin/decodebin、registry、clock、seek、platform sink | 自写 demux/clock、直接 FFmpeg API、双音频栈 |
| GPUI 视频 element | Adapt | 窄 fork `gpui-video-player`，保留既有 frame→GPUI bridge | 原样依赖同步阻塞版本、复制成 app 内无来源代码 |
| 播放器 UI | Compose | gpui-component Button/Slider/Progress/Alert/Label | 新建通用播放器组件 crate |
| PDF parser/raster | Reuse | `hayro 0.7.1` | `gpui-pdf` fork、zpdf 完整 reader、WebView/PDFKit |
| PDF UI | Create | app-local 当前页单图 + 三段导航 | zoom/search/continuous reader |
| native runtime | Adapt | 官方 macOS/Windows runtime + Linux packages；xtask 做 app-local staging | 假定用户 PATH、把二进制提交到 Git、只改 CI 不改发行 |

## Producer/consumer 交接契约

| ID | producer 完成条件 | consumer |
| --- | --- | --- |
| `C-1700` | **代码级完成。** fork 从 upstream `beb23b09...` 建立，最终固定为 `4f1a6cc49ddab9d0afc73404afc259bba73d6407`；API、Cargo/GPUI source identity 与生命周期定向测试已固定 | `WP-1702`、`WP-1704` 可消费 fork 依赖 |
| `C-1701` | **Blocked。** 三平台 native runtime manifest、plugin element/license 清单、SDK 安装脚本、bundle staging、notices 与 fixture corpus 均须完成并验证 | 所有 media tests、CI 与发行包 |
| `C-1702` | **代码级实施完成，发行验证待 `C-1701`。** `ResponseAssetLease` 与 `MediaSession` 的 owner、Transition、Task、fallback、stop contract 已接入定向测试 | `WP-1703`、`WP-1704`、`WP-1706` |
| `C-1703` | **代码级实施完成，最终集成未完成。** Audio/Video/Pdf mode、identity/generation、局部错误与 i18n contract 已接入 | `WP-1707` 最终集成、包测试与状态回填 |

consumer 不得在 producer gate 前编写 shim、直接操作 fork 内 pipeline、读取 `StoredBody` 私有字段或绕开
plugin manifest。`C-1700`/`C-1701` 是实施中产生并冻结的工程证据，不是新的产品待确认问题。

`crates/xtask` 在本计划中只是 secondary owner：只增加由 app-local manifest 驱动的可选 native runtime
staging，不能硬编码 HTTP Client product state，也不能改变其他 app 的 bundle。若实施需要把该能力扩成
新的共享 public bundle contract，先停止 `WP-1700` 并为 xtask 建立独立 owner 计划；不得在本文件中扩大。

## 依赖、native runtime 与发行契约

### Rust 依赖

| 依赖 | 精确声明 | 用途与边界 |
| --- | --- | --- |
| `hayro` | `0.7.1`，保留默认 `embed-fonts`、`embed-cmaps` | PDF parse 与单页 CPU raster；不启用 logging 到用户内容 |
| `gstreamer` | `0.25.2`, feature `v1_20` | app-local init、audio pipeline、bus、position/seek 与 plugin inspection |
| `gpui-video-player` fork | git URL 指向 `suxiaoshao/gpui-video-player`，`rev` 为 `C-1700` 产出的唯一提交 | 视频 frame bridge；禁止 branch/tag 浮动依赖 |
| fork 内 GStreamer crates | `gstreamer`、`gstreamer-app`、`gstreamer-base`、`gstreamer-video` 均 `0.25.2`；`glib 0.22.0` | 避免 0.24/0.25 双版本与 ABI 混用 |

继续复用现有 `gpui-operation`、`gpui-tokio`、`async-channel`、`tempfile`、`tokio`、`image`、`url`。
不新增 `ffmpeg-next`、`libav-sys`、`rodio`、`symphonia`、`gpui-pdf`、`zpdf` 或 webview 依赖。
manifest 修改后只由 Cargo 更新 `Cargo.lock`。

### Native runtime manifest

`app/http-client/build-assets/gstreamer/runtime-manifest.toml` 是发行 producer 的唯一清单，至少记录：

```toml
version = "1.28.5"

[[platform]]
target = "aarch64-apple-darwin"
source_url = "..."
sha256 = "..."
deployment = "private-bundle"

[[platform]]
target = "x86_64-pc-windows-msvc"
source_url = "..."
sha256 = "..."
deployment = "private-bundle"

[[platform]]
target = "x86_64-unknown-linux-gnu"
deployment = "system-packages"
minimum_version = "1.20"
```

实施时必须把实际下载 URL、SHA256、文件白名单、transitive native library、plugin element、plugin License
字段和 notice 路径写全；省略号不能进入提交。manifest 只记录来源和文件，runtime 本体不提交 Git。

MVP plugin contract 至少验证：

| 场景 | 必需 element / plugin family |
| --- | --- |
| 公共 pipeline | `playbin`/`uridecodebin`、`appsink`、`audioconvert`、`audioresample`、`videoconvert`、`videoscale`、平台 audio sink |
| MP4 H.264/AAC | `qtdemux`、可用硬件 decoder 或 `avdec_h264`/`avdec_aac` |
| WebM VP8/VP9/Opus | `matroskademux`、`vp8dec`、`vp9dec`、`opusdec` |
| MP3/WAV/FLAC | `avdec_mp3`、`wavparse`、`flacparse`、`flacdec` |
| Ogg Vorbis/Opus | `oggdemux`、`vorbisdec`、`opusdec` |

macOS/Windows bundle 只复制 manifest 白名单并修复 loader/plugin 路径；macOS 在最终 codesign 之前完成
framework/dylib/plugin staging 与 rpath 检查，Windows 保持 `bin` 与 `lib/gstreamer-1.0` 相对结构。Linux
`.deb` 明确依赖 core/base/good/libav 与平台音频 sink 包，并在支持的发行版上运行相同 element smoke。

`THIRD_PARTY_NOTICES.md` 和 machine-readable audit 输出必须与 manifest 一起进入安装包。即使 app 增加
`GPL-3.0-or-later` 许可选项，也不能省略 LGPL/GPL source offer、版权声明、plugin 许可证、libav 构建配置
或适用的 codec 专利评估。

## 文件图与 owner 边界

```text
.
├── Cargo.lock                                                  # F-1701 [修改，Cargo 生成]
├── .github/workflows/ci.yml                                   # F-1702 [修改] 三平台 SDK/plugin contract
├── .github/workflows/gstreamer-windows-audit.yml              # F-1740 [新增] 手动 Windows inventory artifact
├── script/bootstrap                                           # F-1736 [修改] 按平台安装开发 SDK
├── script/install-linux.sh                                    # F-1703 [修改] Linux build/runtime packages
├── script/install-gstreamer-macos.sh                          # F-1704 [新增] 官方 SDK checksum 安装
├── script/install-gstreamer-windows.ps1                       # F-1705 [新增] MSVC SDK checksum 安装
├── script/audit-gstreamer-windows.ps1                         # F-1739 [新增] Windows 文件/plugin/PE 证据采集
├── crates/xtask/src/bundle.rs                                 # F-1706 [修改] http-client runtime staging hook
├── crates/xtask/src/bundle/gstreamer.rs                       # F-1707 [新增] manifest/whitelist/notice/staging
├── crates/xtask/src/cli.rs                                    # F-1737 [修改] SDK/release 校验命令
├── crates/xtask/src/main.rs                                   # F-1738 [修改] SDK/release 校验路由
├── crates/xtask/src/bundle/settings.rs                        # F-1708 [修改] app-local native runtime metadata
├── crates/xtask/src/bundle/macos.rs                           # F-1709 [修改] framework/rpath/sign order
├── crates/xtask/src/bundle/windows.rs                         # F-1710 [修改] DLL/plugin staging
├── docs/dev/issue-199/README.md                               # F-1733 [修改] root 状态索引
└── app/http-client/
    ├── Cargo.toml                                              # F-1700 [修改] Rust deps、双许可、bundle metadata
    ├── build-assets/gstreamer/runtime-manifest.toml            # F-1711 [新增] native source/hash/file/plugin contract
    ├── build-assets/gstreamer/THIRD_PARTY_NOTICES.md            # F-1712 [新增] 随包 notice/source offer
    ├── test-data/response-preview/README.md                     # F-1729 [新增] fixture 来源、许可与生成方式
    ├── test-data/response-preview/*                             # F-1730 [新增] 小型 codec/PDF fixture
    ├── src/features/request.rs                                 # F-1724 [修改] preview start/teardown/completion route
    ├── src/features/request/response.rs                        # F-1713 [修改] mode/session/PDF composition
    ├── src/features/request/response/data.rs                   # F-1714 [修改] read-all 与 asset materialize 边界
    ├── src/features/request/response/decoding.rs               # F-1715 [修改] Audio/Video/Pdf ContentKind
    ├── src/features/request/response/viewer.rs                 # F-1716 [修改] mode/effective-mode/projection contract
    ├── src/features/request/response/media.rs                  # F-1717 [新增] media 入口、UI 与公开给父模块的 façade
    ├── src/features/request/response/media/asset.rs            # F-1718 [新增] session-owned TempPath
    ├── src/features/request/response/media/session.rs          # F-1719 [新增] Transition、driver、fallback、telemetry
    ├── src/features/request/response/media/audio.rs            # F-1720 [新增] audio pipeline adapter
    ├── src/features/request/response/media/video.rs            # F-1721 [新增] fork adapter 与 GPUI video element
    ├── src/features/request/response/pdf.rs                    # F-1722 [新增] PdfPreview state/UI
    ├── src/features/request/response/pdf/worker.rs             # F-1723 [新增] hayro parse/cache/page raster worker
    ├── src/features/request/response/pdf/tests.rs              # F-1741 [新增] PDF budget/lifecycle/race 自动化
    ├── src/features/request/tests.rs                           # F-1725 [修改] page/identity/teardown GPUI tests
    ├── src/foundation/i18n.rs                                  # F-1726 [修改] required-key 与变量 contract
    ├── locales/en-US/main.ftl                                  # F-1727 [修改] 英文 runtime 文案
    ├── locales/zh-CN/main.ftl                                  # F-1728 [修改] 中文 runtime 文案
    ├── docs/dev/issue-199/response-media-and-pdf-preview-plan.md # F-1731 [本文件] 执行与完成证据
    ├── docs/dev/issue-199/README.md                             # F-1732 [修改] owner 子任务索引
    ├── docs/dev/README.md                                      # F-1734 [修改] app 开发索引
    └── docs/dev/issue-199/http-client-product-and-migration-draft.md # F-1735 [修改] 未实施决定与计划入口
```

不新增 `mod.rs`。若实施发现需要新增未列生产文件，必须先在本计划分配新的 `F-17xx`，写明 owner、
producer/consumer 与测试，再修改代码。

### Ownership matrix

| 数据/资源 | 唯一权威 owner | 生命周期与读取者 | 禁止的镜像 |
| --- | --- | --- | --- |
| 完整 HTTP Response | `RequestRuntime::Ready` 的 `Arc<ResponseData>` | Headers、现有 projection、Save、媒体/PDF lease | media 内复制 ResponseData 字段 |
| 媒体派生文件 | `ResponseAssetLease` | `MediaSession`/driver；最后一个 session 清理 `TempPath` | UI `PathBuf`、fork 静态全局路径 |
| GStreamer pipeline/driver Task | `MediaRuntime` active variant | media bus/frame/controls | ResponseData、Store、detached thread owner |
| 媒体 UI telemetry | `MediaRuntime` | Audio/Video controls 只读投影 | 独立 playing/loading bool |
| PDF bytes/cache | PDF worker job | 同一 worker 内 parse/cache/render | GPUI Entity 中的 `RenderCache` |
| PDF 当前页/图片 | `PdfRuntime` | `PdfPreview` render | ResponseData、全局 cache |
| viewer mode | `ResponsePane` | toolbar Select、Auto effective mode | Form/Store |
| Save Task | 现有 `ResponsePane` save controller | 保持 HTTP-199-03 行为 | Media/PDF session |

## 目标类型与 API 契约

### L-1700：viewer mode 与 content kind

```rust,ignore
pub(crate) enum ViewerMode {
    Auto,
    Text,
    Json,
    Xml,
    Hex,
    Base64,
    Image,
    Audio,
    Video,
    Pdf,
}

pub(crate) enum ContentKind {
    Text(SourceLanguage),
    Json,
    Xml,
    Image,
    Audio,
    Video,
    Pdf,
    Bytes,
}
```

Auto 只按已解析 Content-Type 选择；`application/*+json`/`*+xml` 与 SVG 的既有安全规则保持不变。
`application/octet-stream` 仍为 Bytes。显式 mode 可以尝试不匹配 MIME 的完整 bytes，但不能改变 mode 或
Response value。

### L-1701：完整读取与媒体 asset

```rust,ignore
pub(crate) struct ResponseAssetLease {
    path: tempfile::TempPath,
    uri: url::Url,
    len: u64,
}

impl ResponseReadLease {
    pub(crate) async fn read_all_bounded(
        &self,
        limit: u64,
    ) -> Result<bytes::Bytes, ResponseReadProblem>;

    pub(crate) async fn materialize_media_asset(
        &self,
    ) -> Result<ResponseAssetLease, ResponseAssetProblem>;
}

impl ResponseAssetLease {
    pub(crate) fn uri(&self) -> &url::Url;
    pub(crate) fn len(&self) -> u64;
}
```

- `read_all_bounded` 在分配前校验 lease 长度，在 EOF 后校验精确字节数；文件变短/变长均失败。
- `materialize_media_asset` 对 Memory/TempFile 都在 private temp 目录生成独立精确副本，流式 copy、flush、
  close 后才返回；它不借用 collector 的 spill path，也不再持有一份 `Arc<ResponseData>`。
- `ResponseAssetLease` 不实现 `Clone`，不公开 path，不在 `Debug`/Display 泄漏路径。URI 只交给 media adapter。
- 失败或 Task cancel 由 `TempPath` 清理；成功 asset 与 pipeline 同 owner，停止后按 player→asset 顺序 drop。

### L-1702：媒体完整 runtime

```rust,ignore
enum MediaRuntime {
    Idle,
    Preparing(MediaPreparing),
    Paused(MediaActive),
    Playing(MediaActive),
    Ended(MediaActive),
    Failed(MediaFailed),
}

enum MediaMessage {
    Start {
        token: PreviewToken,
        kind: MediaKind,
        decoder_policy: DecoderPolicy,
        resume_position: Duration,
        resume_playing: bool,
        task: Task<()>,
    },
    Prepared {
        token: PreviewToken,
        driver: MediaDriver,
        metadata: MediaMetadata,
        task: Task<()>,
    },
    PrepareFailed { token: PreviewToken, problem: MediaProblem },
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    PollPosition,
    Metadata { token: PreviewToken, metadata: MediaMetadata },
    Position { token: PreviewToken, position: MediaPosition },
    Ended { token: PreviewToken },
    PlaybackFailed {
        token: PreviewToken,
        problem: MediaProblem,
        fallback_task: Option<Task<()>>,
    },
    Stop,
}

impl Transition<MediaMessage> for &mut MediaRuntime {
    type Output = ();
}
```

`MediaRuntime` 是唯一 media Task/asset/driver/telemetry authority。Start 入口先确认当前 viewer 可启动，再构造
lazy owner-bound Task，并在任何 poll 前同步安装 `Preparing`。所有异步 completion/event 先在 ResponsePane
route 校验 token，Transition 再与当前 state token 复核；旧 session 即使进入相同 phase 也不能被接受。
非法消息恢复原状态、脱敏 debug 后丢弃。
Stop/失败/新 Response 先安装最终合法状态，再 drop Task、driver 与 asset；不得用 `mem::take` 暴露临时
Idle，也不得维护平行 `is_playing`/`is_loading`。

### L-1703：driver 与 decoder policy

```rust,ignore
enum MediaKind { Audio, Video }
enum DecoderPolicy { Auto, SoftwareOnly }

enum MediaDriverEvent {
    Metadata(MediaMetadata),
    Position(MediaPosition),
    Ended,
    PlaybackFailed(MediaProblem),
}
```

- fork 的 constructor 只建立 lazy driver，不同步进入 Playing、不在 UI thread 等待 pipeline state。
- pipeline 每实例配置 decoder policy；禁止 `set_rank` 和 `GST_PLUGIN_FEATURE_RANK`。
- Auto failure 且 `fallback_used == false` 时停止原 pipeline、以 `SoftwareOnly` 重建、preroll 后 seek 到最后确认
  position，再恢复原 paused/playing intent；第二次失败直接 Failed。
- `MediaDriver` 是有界 command façade，具体 driver 在自己的 worker 执行 control/query；视频 frame wakeup 在
  adapter 内合并为 position/重绘事件，不作为公开 runtime message。MediaDriver/Task drop 是取消根；fork
  不在 Drop 中同步 join UI thread，也不只把 bus error 写日志。
- 视频 appsink 与 presentation queue 使用 D-1708 的固定上限；每个 session 只有一个 pipeline。

### L-1704：音频与视频控制

`MediaActive` 同时保存 immutable metadata 与当前 `MediaPosition`、volume、muted、play intent；position event
只更新文本/slider，不重建 component Entity。duration 不可用时 seek disabled。Ended 后 Play 发送 seek(0)
再播放；seek 失败进入 Failed。

视频 caps 在分配/展示 frame 前检查 D-1707 像素包络；不符合时停止 pipeline并产生
`ResolutionUnsupported`。视频 element 只消费 fork 提供的最新帧/平台 surface，不复制完整 response bytes。
纯音频 pipeline 使用 fake video sink/无视频分支和平台 auto audio sink，不能创建隐藏 Video element。

### L-1705：PDF runtime 与 worker

```rust,ignore
enum PdfRuntime {
    Idle,
    Reading(PdfReading),
    Loading(PdfLoading),
    Ready(PdfReady),
    Rendering(PdfRendering),
    Failed(PdfFailed),
}

enum PdfMessage {
    BeginRead { token: PreviewToken },
    Load {
        token: PreviewToken,
        page_generation: u64,
        worker: PdfWorkerHandle,
        task: Task<()>,
    },
    Loaded {
        token: PreviewToken,
        page_generation: u64,
        page_count: usize,
        page: usize,
        image: Arc<RenderImage>,
    },
    RenderPage { page: usize, page_generation: u64 },
    Rendered {
        token: PreviewToken,
        page_generation: u64,
        page: usize,
        image: Arc<RenderImage>,
    },
    LoadFailed { token: PreviewToken, problem: PdfProblem },
    Failed { token: PreviewToken, page_generation: u64, problem: PdfProblem },
    WorkerClosed { token: PreviewToken, problem: PdfProblem },
    Stop,
}

impl Transition<PdfMessage> for &mut PdfRuntime {
    type Output = ();
}
```

- `PdfLoading`、`PdfReady` 与 `PdfRendering` 原样移动同一个 `PdfWorkerHandle` 和 event-bridge `Task`；翻页
  不为同一文档另建 worker或 driver Task。
- ResponsePane 先以 `BeginRead` 同步安装 `Reading`，再启动有界 body read；读取成功后建立 mailbox、stop flag
  与 lazy worker/route Task，并以 `Load` 安装 `Loading` 后才允许首次 poll/解析。每个 worker event 同时携带
  PreviewToken 和 page generation。
- `PdfWorker` 在一个 blocking worker 内创建 PDF、`RenderCache` 并复用；`RenderCache` 永不跨 await、线程或
  Entity 边界。`PdfWorkerHandle` 的 Drop 设置 stop flag 并唤醒 mailbox；worker 在当前 raster 完成后退出。
- worker 使用容量为 1 的 latest-only mailbox；发送新翻页请求时原子替换尚未开始的旧请求。已开始的同步
  raster 不保证中断，但完成结果必须同时匹配 `Arc<ResponseData>`、viewer mode 与 generation 才能安装。
- 当前页使用 0-based 内部索引、1-based UI；任何 page 越界在建 Task 前拒绝。上一页/下一页在边界 disabled。
- 按 viewport 与 scale factor 计算 Contain 目标，再 clamp D-1710。使用 checked arithmetic 预留 buffer；
  `hayro` premultiplied RGBA 原地转换为 GPUI 所需 premultiplied BGRA，随后唯一地移入单帧 `RenderImage`。
- worker stop 后不再取新 command；当前 raster 可以完成并销毁，其 event receiver 已断开，不能回写 UI。

### L-1706：ResponsePane 统一 preview owner

```rust,ignore
enum ResponsePreview {
    Projection(ResponseProjection),
    Media(MediaPreview),
    Pdf(PdfPreview),
}

struct PreviewToken {
    response: Arc<ResponseData>,
    requested_mode: ViewerMode,
    effective_mode: ViewerMode,
    generation: u64,
}
```

`ResponsePane::reset_for_send`、Clear、mode change 与新的 Ready 都走同一个 teardown/install 入口：先递增
generation、安装空/新 mode，再给旧 Media/Pdf runtime 发送 Stop 并 drop preview Task。Save controller 保持独立，
新 Send/Clear 不取消已经开始的 Save。普通 Text/Image projection 继续使用现有 2 MiB 安全路径。

`PreviewToken` 只能由 ResponsePane 构造；匹配使用 `Arc::ptr_eq`、requested/effective mode 与 generation。
任何异步 route 必须在调用 Transition 前校验 token，Media/Pdf runtime 也必须保存并二次校验当前 token。

## 状态与时序

### ST-1700：Ready 到 preview

```text
RequestRuntime::Ready(response)
  -> requested ViewerMode
  -> Auto 时只按 Content-Type 求 effective mode
  -> Text/Image: 现有有界 projection
  -> Audio/Video: materialize asset -> MediaRuntime::Preparing
  -> Pdf: PdfRuntime::Reading -> read_all_bounded -> PdfRuntime::Loading
```

非 Ready 不创建 media/PDF session。BodyDecoding::Unsupported 不进入 parser/player。manual mode 的 probe 失败
只更新当前 preview warning。

### ST-1701：媒体生命周期

```text
Idle
  --Start(task)--> Preparing
Preparing
  --Prepared--> Paused
  --PrepareFailed--> Failed
Paused
  --Play--> Playing
Playing
  --Pause--> Paused
  --Ended--> Ended
active
  --PlaybackFailed且未fallback--> Preparing(SoftwareOnly, resume_position)
  --PlaybackFailed且已fallback--> Failed
any
  --Stop/new response/clear/mode switch/drop--> Idle
```

Seek/volume/mute 是 active-state typed messages；Preparing/Failed 中收到时是程序错误并丢弃，不排队。
用户必须先停止当前播放或切换 mode；任意时刻只有一个 media session。

### ST-1702：PDF 生命周期

```text
Idle --Load(page 0)--> Loading --Loaded--> Ready
Ready --RenderPage(n)--> Rendering --Rendered(same generation)--> Ready
Loading/Rendering --Failed(same generation)--> Failed
any --Stop/new response/clear/mode switch/drop--> Idle
```

翻页期间不接受第二个同步 render；UI 更新 desired page 与 generation，worker channel只保留最新目标。
旧 page completion 不回退页码，也不显示为错误。

### ST-1703：清理与析构顺序

1. 确认 user event 对当前 Response/mode 合法。
2. 递增 preview generation并安装新的稳定 UI 状态。
3. 断开旧 completion route/command sender。
4. drop owner-bound Task，让 driver 收取消；pipeline进入 Null。
5. drop player/frame资源，最后 drop `ResponseAssetLease` 并删除 TempPath。
6. `cx.notify()`；任何旧 event 只会失败发送或在 identity/generation gate 被丢弃。

### ST-1704：平台初始化与缺失依赖

GStreamer lazy 初始化只发生在首次 Audio/Video preview 的后台准备阶段。初始化前设置 app-local plugin search
path并验证 C-1701 清单；失败产生 `MediaProblem::RuntimeUnavailable/PluginMissing`，HTTP app 其余功能继续可用。
PDF 与普通 viewer 不依赖 GStreamer，也不因 runtime 缺失被禁用。

## 错误目录与用户反馈

| ID | 稳定 kind | 触发与 UI 行为 |
| --- | --- | --- |
| `ERR-1700` | `AssetRead` | read lease 短读/增长/复制失败；局部 Alert，可切 Hex/Save |
| `ERR-1701` | `TemporaryAsset` | 媒体临时文件 create/write/flush/close 失败；不泄漏路径 |
| `ERR-1702` | `RuntimeUnavailable` | GStreamer init、loader 或 app-local plugin path 不可用 |
| `ERR-1703` | `PluginMissing` | MVP element 缺失；只显示安全 element family 名 |
| `ERR-1704` | `UnsupportedMedia` | container/codec/typefind 不支持 |
| `ERR-1705` | `MediaDecode` | decoder/pipeline/bus error；一次 software fallback 后仍失败 |
| `ERR-1706` | `ResolutionUnsupported` | 视频像素包络超过 D-1707 |
| `ERR-1707` | `MediaControl` | seek/play/audio sink 控制失败 |
| `ERR-1708` | `PdfParse` | 非 PDF、损坏或不受支持结构 |
| `ERR-1709` | `PdfEncrypted` | 加密/密码保护 PDF；不显示密码 UI |
| `ERR-1710` | `PdfBudget` | 页数、尺寸、像素或内存预算超限 |
| `ERR-1711` | `PdfRender` | 当前页光栅化失败 |
| `ERR-1712` | `Internal` | channel/task/状态不变量；脱敏记录 kind/phase |

所有 error type 可以私有保留 source 供 `Error::source`，但手写 Display/Debug、Fluent args 与默认 tracing 禁止
包含 URL、query、header value、body、文件路径、PDF 文本/metadata、媒体 tags、token 或底层完整错误字符串。
codec factory、宽高、position、计数和 plugin family 只有通过白名单字段才能进入诊断。

## UI 与 i18n 契约

- Response Body toolbar 继续使用同一 typed `ViewerMode` Select，增加 Audio、Video、PDF；不可用选项通过
  SelectItem disabled，不靠 renderer 单独挡事件。
- Audio：loading Progress；Paused/Playing 显示 Play/Pause、seek Slider、position/duration、volume Slider、
  Mute；无 duration 时 seek disabled。
- Video：上方 GPUI video element `.flex_1().min_h(0)`、`ObjectFit::Contain` 语义；下方同一控制条。背景不执行
  response HTML，也不加载外部资源。
- PDF：页面图 `.flex_1().min_h(0)` + Contain；底部 Previous、`current / total`、Next。没有 zoom 控件。
- loading/failed 状态仍保留 mode Select、Headers tab、Save/Clear；4xx/5xx Response 不因 viewer 失败改变分类。

两 locale 与 `REQUIRED_REQUEST_KEYS` 至少增加：

| Key | 变量 | 用途 |
| --- | --- | --- |
| `response-view-audio`, `response-view-video`, `response-view-pdf` | 无 | mode Select |
| `response-media-loading`, `response-media-play`, `response-media-pause` | 无 | session loading/transport |
| `response-media-mute`, `response-media-unmute` | 无 | audio control |
| `response-media-position` | `$current`, `$total` | position label |
| `response-media-runtime-unavailable`, `response-media-plugin-missing` | plugin key 只允许 `$plugin` | ERR-1702/1703 |
| `response-media-unsupported`, `response-media-decode-failed`, `response-media-control-failed` | 无 | ERR-1704/1705/1707 |
| `response-media-resolution-unsupported` | `$width`, `$height` | ERR-1706 |
| `response-pdf-loading`, `response-pdf-previous`, `response-pdf-next` | 无 | PDF controls |
| `response-pdf-page` | `$current`, `$total` | 页码 |
| `response-pdf-invalid`, `response-pdf-encrypted`, `response-pdf-too-large`, `response-pdf-render-failed` | 无 | ERR-1708–1711 |

两份 FTL 的 key 与变量集合完全一致；不得把底层 error/path/body 拼接进翻译参数。本文不新增 macOS bundle
可见文案，因此 `InfoPlist.strings` 不变。

## 工作包

### [ ] WP-1700：fork、依赖、native runtime、许可与打包 producer gate

**文件：** F-1700–F-1712、F-1729–F-1730、F-1736–F-1740。
**前置：** D-1703–D-1708、D-1712–D-1713。
**状态：** 部分实施；fork、Rust 依赖、安装/校验脚本和 xtask fail-closed staging 已落地，`C-1701` 的正式
manifest、notices、fixture corpus 与三平台发行证据仍阻塞完成。

1. 从 upstream `beb23b09...` 建立 `suxiaoshao/gpui-video-player` 窄 fork；只实施 C-1700 列出的 API/
   生命周期修正，固定 fork SHA 与所有 GStreamer Rust crate 版本。
2. 增加 Rust dependencies、双许可 metadata；Cargo 生成 lockfile，确认只有一套 GStreamer/glib major/minor。
3. 建立 1.28.5 runtime manifest、下载/checksum/install 脚本、plugin/license/notice 审计；Linux 声明系统依赖。
   Windows 首次清单由手动 discovery workflow 在官方 installer 的隔离安装后生成 artifact；仅在人工审计
   artifact 后，才把 file/plugin/license/source 结论写进唯一 release manifest 与 notices。该采集只记录
   原始技术和许可候选证据，不得把自动 metadata 当作许可证结论。
4. 扩展 xtask，在正确签名/installer 阶段 stage native runtime；增加三平台 manifest/staging unit tests。
5. 引入有明确来源/许可的小型媒体/PDF fixtures，不把 runtime installer、framework 或 DLL 提交 Git。

**完成条件：** C-1700/C-1701 producer-ready；三平台 `cargo check -p http-client` 能找到开发 SDK；bundle
manifest 无占位符，缺文件/校验/license/element 会硬失败，而不是降级成“打包成功”。

### [ ] WP-1701：Response 完整读取、媒体 asset 与 MIME mode

**文件：** F-1713–F-1718、F-1724–F-1725。
**前置：** WP-1700、L-1700–L-1701。
**状态：** 代码已实施；完整 fixture/三平台消费证据随 `C-1701` 收口。

1. 扩展 ContentKind/ViewerMode/Auto matrix，保留既有 text/image/SVG/Unsupported-decoding contract。
2. 实施 `read_all_bounded` 与 session-owned `ResponseAssetLease`；Memory/TempFile 均精确复制到新 TempPath。
3. 建立统一 preview identity/generation 与 teardown，不改 Save Task 行为。
4. 覆盖短读/增长/cancel、asset consumer 存活、stop 后回收、manual mode 与 MIME matrix。

**完成条件：** Response authority 未复制为业务模型，媒体 path 不越过 response media 模块，任何迟到 asset
completion 都不能安装到新 Response/mode。

### [ ] WP-1702：GStreamer MediaSession、Transition 与 decoder fallback

**文件：** F-1717–F-1719、F-1724–F-1725。
**前置：** WP-1700–WP-1701、C-1700–C-1701、L-1702–L-1703。
**状态：** 代码与 fake-driver/WAV 定向自动化已实施；真实 codec matrix 仍消费 `C-1701` fixture corpus。

1. 实施 lazy runtime/plugin init、完整 `MediaRuntime`/`MediaMessage` 合法表、Task owner 与脱敏非法消息。
2. 接通 driver event/command、position/duration/volume/mute、EOS/error 与 final-state-before-drop。
3. 实施 per-instance Auto→SoftwareOnly 最多一次重建与 position 恢复；禁止全局 rank 修改。
4. 用 fake driver 覆盖全状态表、取消、失败、第二次 fallback、stale event、owner drop 与 drop 顺序；用真实
   GStreamer fixture 覆盖 plugin/typefind/decoder path。

**完成条件：** 任意时刻最多一个 pipeline/driver Task；没有 detached worker、parallel booleans、错误日志-only
分支或无限 fallback。

### [ ] WP-1703：纯音频预览

**文件：** F-1717、F-1719–F-1720、F-1724–F-1725。
**前置：** WP-1702、L-1704。
**状态：** 代码与 WAV/fakesink 自动化已实施；MP3/FLAC/Ogg/Opus 分发环境证据尚未完成。

1. 建立 audio-only pipeline adapter和平台 audio sink；不创建视频 element。
2. 组合 Play/Pause、seek、position、volume、mute、loading/error UI；默认 Paused。
3. 用 MP3/WAV/FLAC/Ogg Vorbis/Opus fixture 覆盖准备、播放状态、seek、EOS、stop 与 plugin 缺失。

**完成条件：** 音频只消费 MediaSession authority；控件不因 position tick 重建；切换 mode/Response 后立即停止
声音并最终清理 asset。

### [ ] WP-1704：视频预览与 GPUI frame bridge

**文件：** F-1717、F-1719、F-1721、F-1724–F-1725。
**前置：** WP-1702、C-1700、L-1704、D-1707–D-1708。
**状态：** fork 与 app adapter 已实施并通过定向自动化；codec fixture、性能 profiling 与实机播放未执行。

1. 适配 fork Video element、frame/event API、实例 caps/decoder telemetry 与 owner task。
2. 实施 D-1707 resolution gate、D-1708 有界队列、Contain 布局与 audio/video 同步控制。
3. 覆盖 H.264/AAC、VP8/VP9/Opus、横/竖 1080p、超限分辨率、frame drop、EOS、software fallback 与 stop。
4. 固定样本记录 startup、CPU/RSS、dropped frames、stop latency；若 appsink→CPU→GPUI 未达验收，停止
   WP-1704 并修订本计划，不在当前实现顺手重写 renderer。

**完成条件：** UI thread 不做 pipeline state wait、decode 或 join；缓冲上限可证；错误能进入 viewer-local
problem；不承诺 4K/60。

### [ ] WP-1705：hayro 极简 PDF viewer

**文件：** F-1714–F-1717、F-1722–F-1725、F-1729–F-1730、F-1741。
**前置：** WP-1700–WP-1701、L-1705、D-1709–D-1710。
**状态：** 代码与生成式 PDF 自动化已实施；许可审计 compatibility corpus 与实际翻页未执行。

1. 完整读取 PDF bytes，在 blocking worker 内 parse 并持有 `RenderCache`；不把 `!Send` cache 移出 worker。
2. 实施 PdfRuntime、current/total、Previous/Next、latest-only command、generation 与 Contain raster。
3. 实施 checked page/pixel/buffer budgets与 premultiplied RGBA→BGRA 单副本移交。
4. 覆盖普通/复杂/截断/非 PDF/加密/10,001页或等价 budget fixture/恶意 page size、翻页与所有迟到路径。

**完成条件：** 同时最多一张 page image；翻页不阻塞 UI；加密/损坏/预算错误不改变 Ready Response；若固定
compatibility corpus 未通过则暂停 PDF 工作包并另评估 `pdfium-render`，不 fork `gpui-pdf`。

### [ ] WP-1706：ResponsePane、控件、错误投影与 i18n

**文件：** F-1713、F-1716–F-1728。
**前置：** WP-1703–WP-1705、C-1702、ERR-1700–ERR-1712。
**状态：** 代码、双语文案与媒体/PDF 自动 interaction matrix 已实施；实际 UI 按用户要求未执行。

1. 把 Audio/Video/Pdf 接入 mode Select、Body pane、统一 teardown与 existing Headers/Save/Clear。
2. 组合 gpui-component controls，确保 loading/failed/disabled/edge page 状态与事件入口一致。
3. 增加双语 key、required-key与 error mapping；诊断只用 whitelist field。
4. GPUI tests 覆盖 Auto/manual、默认不播放、controls、mode switch、新 Send/Clear、Headers/Save仍可用。

**完成条件：** 每个可达 viewer 状态都有明确 UI；任何 viewer failure 都不发送 `HttpRunMessage` 或离开
`RequestRuntime::Ready`。

### [ ] WP-1707：集成、三平台包、验证与文档回填

**文件：** F-1700–F-1735。
**前置：** WP-1700–WP-1706、R-1700–R-1714。

1. 执行 app、xtask、fork、locale、residual 与三平台 native/plugin/package gate。
2. 对安装后的 macOS `.app`、Windows MSI 与 Linux `.deb` 运行 runtime path、plugin element、codec fixture
   smoke；记录实际 runtime/plugin/license manifest。
3. 在三平台实际桌面 app 验证一组音频、一组视频与 PDF 翻页；失败不可用 Cargo 测试代替。
4. 仅在全部证据完成后把本计划和索引改为 Done；未执行项明确写“未执行”。产品草稿继续只保留尚未进代码
   的范围，不复制完成实现。

**完成条件：** C-1700–C-1703 consumer-complete；所有 R/T 有证据；无未声明 fork/runtime/plugin/许可偏离。

### 工作包依赖顺序

```text
WP-1700
  -> WP-1701
     -> WP-1702 -> WP-1703
                -> WP-1704
     -> WP-1705
WP-1703 + WP-1704 + WP-1705
  -> WP-1706
     -> WP-1707
```

`WP-1705` 不消费 GStreamer，但要等 `WP-1700` 完成统一 manifest/lockfile/fixture producer，避免 PDF 与媒体
并行改写同一 Cargo/测试资产边界。若 C-1701 阻塞，允许实现与验证 PDF 代码，但不能越过 `WP-1707` 把整个
HTTP-199-04 标成 Done。

## 要求与验证映射

| R-ID | 要求 | T-ID | 自动化/手工证据 |
| --- | --- | --- | --- |
| `R-1700` | 只有完整 Ready Response 启动 preview，Receiving/Unsupported decoding 不启动 | `T-1700` | Runtime/ResponsePane GPUI tests |
| `R-1701` | ResponseAsset 精确复制、无裸路径、consumer期间存活、stop 后回收 | `T-1701` | Memory/TempFile/short-read/cancel lifecycle tests |
| `R-1702` | MediaRuntime 合法表、唯一 Task、final-state-before-drop、非法消息保留原态 | `T-1702` | fake driver Transition/drop-order tests |
| `R-1703` | Auto 失败最多一次 SoftwareOnly，逐实例且恢复位置；第二次失败终止 | `T-1703` | fake + GStreamer decoder-policy fixtures；全局 rank residual |
| `R-1704` | 音频默认 Paused，controls/EOS/stop正确且 mode切换无残留声音 | `T-1704` | 五类音频 fixture + GPUI control tests |
| `R-1705` | 视频≤1080p、队列有界、UI thread不初始化/解码/join | `T-1705` | codec/caps/queue tests + profiling + 实机 playback |
| `R-1706` | PDF只有单页导航，page/pixel/memory预算正确，加密明确不支持 | `T-1706` | PDF corpus与buffer boundary tests |
| `R-1707` | Response/mode/page generation 丢弃所有迟到 media/PDF结果 | `T-1707` | new Send/Clear/switch/drop/page race GPUI tests |
| `R-1708` | Auto只按Content-Type；manual可尝试；失败不改变Response或Save | `T-1708` | MIME/manual mode matrix tests |
| `R-1709` | 所有 viewer problem 脱敏且只更新局部 UI | `T-1709` | Debug/Display/tracing/Fluent mapping assertions |
| `R-1710` | 两 locale key/变量同构，controls与错误全部可本地化 | `T-1710` | `foundation::i18n` parity/required-key tests |
| `R-1711` | fork/runtime/plugin/notice 版本与文件清单完全可复现 | `T-1711` | manifest checksum/whitelist/license/plugin tests |
| `R-1712` | macOS/Windows bundle 自带 runtime，Linux `.deb` 声明并找到系统插件 | `T-1712` | 三平台 bundle inspection与安装后 smoke |
| `R-1713` | MVP codec在实际分发环境可播放，缺plugin时安全失败 | `T-1713` | 三平台 fixture matrix与missing-plugin fixture |
| `R-1714` | 既有 Text/Image/Headers/Save/50MiB/HTTP runtime行为无回归 | `T-1714` | HTTP Client完整 tests、Clippy、现有116项基线 |

## 验证命令与残留扫描

实现阶段至少执行；native 安装脚本所需权限按平台单独申请：

```sh
cargo fmt --all -- --check
cargo check -p http-client --bin http-client --all-features --locked
cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast
cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings
cargo test -p xtask --all-features --locked
cargo clippy -p xtask --all-targets --all-features --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
cargo run -p xtask -- bundle http-client
git diff --check -- app/http-client crates/xtask script .github Cargo.lock docs/dev/issue-199
```

普通源码 CI 只通过 `verify-gstreamer-sdk` 断言 build SDK 的 `pkg-config` 最低版本与 `gst-inspect-1.0` 可用；
发行路径另以 manifest 驱动的 app-local inspector 检查 staged artifact，并断言 C-1701 element/plugin/License。
开发 SDK 的成功不能替代发行清单。codec fixture smoke 必须指向已捕获本地文件，禁止访问外网。

残留扫描：

```sh
! rg -n 'gpui[-_]pdf|zpdf|rodio|symphonia|ffmpeg[-_](next|sys)|TextView::html' \
  app/http-client/src app/http-client/Cargo.toml
! rg -n 'ElementFactory::set_rank|GST_PLUGIN_FEATURE_RANK|max-buffers=200|\.detach\(' \
  app/http-client/src
! rg -n 'pub .*PathBuf|pub .*TempPath|ResponseData.*(Media|Pdf|Player)|autoplay|auto_play' \
  app/http-client/src/features/request/response
```

允许 `gst-libav` 在 native manifest/notice 中出现，但 app Rust 源码不得直接调用 FFmpeg。允许私有
`PathBuf` 作为构造 TempPath 的短暂局部变量；生产 public/crate façade 不得把 Response 临时路径暴露给
UI、Form、Store 或 fork 外部。

## 完成定义与失败/回滚边界

只有 WP-1700–WP-1707、C-1700–C-1703、R-1700–R-1714 与 T-1700–T-1714 全部闭环，
`HTTP-199-04` 才能标记 `Done`：普通 HTTP runtime 不变；完整 Ready Response 是唯一 authority；媒体/PDF
只读、默认不播放、可取消且无迟到回写；PDF 单页预算、媒体 buffer/decoder policy、错误与双语 UI 可证；
三平台开发、CI 和安装包都携带或声明同一 plugin contract与许可清单。

失败时按 owner 回滚，不保留半成品兼容层：

- `C-1700` fork 无法满足非阻塞/错误/stop contract：停止 Audio/Video consumer，不把 upstream 原样接入。
- `C-1701` runtime/plugin/license 无法形成可发行清单：Audio/Video 保持未实现，PDF 可独立继续，但整个
  HTTP-199-04 不标 Done。
- `hayro` 固定 corpus 不达标：删除未交付的 PDF consumer改动，另建 `pdfium-render` 评估，不 fork
  `gpui-pdf`。
- profiling 证明 appsink→CPU→GPUI 不满足 1080p范围：停止 WP-1704，另立 renderer/zero-copy 计划，不在本轮
  引入第二条隐式 pipeline。
- 任一平台只在开发机依赖 PATH/系统安装、bundle manifest/notice/codec smoke 未通过：该平台发行未完成，
  不能用另两个平台或 Cargo tests 代替。

### 实施状态与轮交接

`WP-1700` 的 fork source producer 已完成；`WP-1701`–`WP-1706` 已有 app 代码和上述定向自动化证据，
但 codec/许可 fixture 与发行环境验证仍未完成；实际 UI 按用户要求未执行。`WP-1707` 尚未完成。

后继 #200 继续补齐 `C-1701` 对应的 manifest、notices、fixture 许可证与三平台开发/CI/安装包验证；不得
伪造 runtime 清单，亦不得将本轮的 Cargo 定向结果写成实际 UI、native codec 或发行包 smoke 通过。本文件
不再接收新的实施或完成状态回填。
