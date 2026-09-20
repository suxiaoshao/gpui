# Workspace 依赖更新与官方 skill 接入

## 状态与目标

- 状态：Implemented；图标、普通依赖更新、官方 skills 迁移及 InputGroup / on_paste / Markdown stream_fade 已实施，macOS 构建、回归与重点界面检查完成。GPUI 配套版本为 0.6.4 / 0.3.5。验证边界见末尾。
- 版本盘点日期：2026-09-19；changelog 与接入范围复核：2026-09-20。基线：主 Issue 分支合并提交 `d7c16081`，当前工作分支 `codex/222-gupi-queue-interaction`。
- 目标：在队列交互开发前更新项目依赖；覆盖仍维护的应用与共享 crate；Jaco 及专用内容不再维护，不纳入升级和适配。采用最新非撤回正式版本，配套约束明确的依赖按兼容组合更新，不为了版本数字引入第二套跨边界类型或无关架构重写。
- 当前实施范围：按已确认的依赖与组件接入计划执行；不夹带队列产品行为或未发布 Token / Questionnaire。此前 Gupi 能力盘点文档保留。
- 这是新的更新批次，不修改 [#215 旧批次](../issue-215/README.md) 的历史交付。继续复制上游 skill 资料的方式由官方安装替代；其他约束按本次维护范围重新核对。

## Jaco 停止维护与后续删除

用户已确认：Jaco 后续不再维护，等 Gupi 合入后清理。统一范围、连带调整及共享能力保留依据见 [Jaco 退役与关联清理](../jaco-retirement/README.md)，不在本页维护第二份清单。

本次依赖更新仍排除 `app/jaco`、四个 jaco-* crate、旧 app-assets / app-assets-macros 与专用 `tools/mcp-auth-test-server`；共享 crate 按现役消费者保留。不提前删除源码或用户数据。

## 盘点范围与方法

读取根 manifest、24 个 workspace 成员（5 个应用、19 个 crate）及独立 `tools/mcp-auth-test-server`，共 26 个 `Cargo.toml`。包含普通、开发、构建和平台条件依赖，以及 workspace alias 继承，共 **101 个外部直接依赖包名**；逐个查询 Cargo 官方 sparse index 的最高非撤回正式版本。这是完整仓库盘点基线；其中 **20 个包仅被 Jaco 及其专用工具直接引用，现已排除**，其余 81 个继续按实际用途评估。下表保留完整盘点并标记排除项，避免把它们误当成升级待办。

版本区分：manifest 声明是允许范围或精确 pin；锁文件才是当前解析结果。表中主锁/工具锁仅列对应项目直接引用的解析版本，不把同名包的所有传递版本混成直接版本。`gpui-pre-platform` 当前只有 workspace 声明，实际由 kit 间接引入。`toml` 的 `+spec-1.1.0` 是构建元数据，不是预发布标记。

主 `Cargo.lock` 有 1621 条 package 记录，工具锁有 166 条，含本地包、多个版本及传递依赖。仍维护内容的传递依赖也属于本次升级后的依赖图核对范围，但不逐包强行提升到全局最高版本：它们受上游版本范围、features 和平台限制，随 Cargo 解析更新。已生成升级锁图，GPUI 公共边界仍为单一 0.3.5 / 0.6.4 配套组合；具体构建与平台覆盖见实施记录，不宣称所有传递包均升级至全局最高版本。仓库没有独立维护的前端 package.json；Lucide 子模块内部的 JS 依赖由其固定 revision 管理，不单独改写。

## 升级建议与配套约束

### GPUI 全套一起更新

升级前 `gpui-kit`、`gpui-component`、`gpui-kit-assets` 均精确固定 `0.6.0`；`gpui-pre`、`gpui-pre-platform` 声明固定 `0.3.3`。图标改造已将它们配套升级为 **0.6.4 + 0.3.5**。已核对 0.6.4 发布索引中的配套依赖：kit/component 都使用 gpui-pre 0.3.5；必须一起更新，防止共享 crate 与应用出现两份 GPUI 类型。

应用继续经 `gpui_kit` 接入；共享 crate 的 gpui/component alias 与新 gpui-lucide 使用同一配套类型。旧 app-assets 仅供待删除的 Jaco 使用。更新不自动意味着重写本地 gpui-tokio、Form、Store、Operation 或引入新的产品功能。

正式版本与 Gupi 输入组件等待项必须分开：

| 上游能力 | 当前 0.6.4 是否包含 | 本次关系 |
| --- | --- | --- |
| InputGroup（#3042） | 是，提交 `142e4016` 是发布标签祖先 | 本轮采用以替换共用输入外壳；Token/资源交互仍归对应功能计划 |
| 原子内联 Token（#3113） | 否，提交 `7f6d9232` 不在发布标签中 | 继续等包含该能力的正式版本 |
| Questionnaire（#2878） | 否，提交 `f698b4bc` 不在发布标签中 | 继续等包含该能力的正式版本 |

判断来自提交与发布标签的祖先关系，不能仅按合并时间早于发布时间判断。2026-09-19 上游 main 快照为 `f698b4bcac037b8d208b34eca86cc940081c498f`（最新复核见下方 changelog 章节）。沿用用户已确认的等待正式发布策略，不自行改为 Git main。实施当天重新核对发布：若已有包含三项能力的新正式版，更新目标及其配套 GPUI 版本；否则 0.6.4 的依赖升级不代表输入框等待项已解除，见 [统一待处理文档](../issue-217/follow-ups.md)。

### 普通依赖

可按现有 features 尝试更新并完成受影响验证：

- `async-compat` 0.2.5 → 0.2.6；`async-compression` 0.4.44 → 0.4.48。
- `clap` 4.6.6 → 4.6.7；`encoding_rs` 0.8.35 → 0.8.41。
- `reqwest` 0.13.4 → 0.13.5；`sonic-rs` 0.5.8 → 0.5.10。
- `syn` 3.0.5 → 3.0.6；`toml` 1.1.5 → 1.1.6（发布版本带 `+spec-1.1.0`）。
- `trash` 5.2.8 → 5.2.9；`trybuild` 1.0.120 → 1.0.121；`uuid` 1.26.0 → 1.26.1。

跨兼容系列候选为 Gupi 的 `base64` 0.22.1 → 0.23.1（HTTP Client 已用 0.23.1）；检查实际调用后迁移。Jaco agent 的 dirs 不再升级；无需顺带替换仍正常使用的 dirs-next。

`plist` 已锁到 1.10.1、serde 已锁到 1.0.229、which 已锁到 8.0.6：部分 manifest 的低起始版本只需统一声明，不构成新的运行时升级。Jaco 专用的 winresource 不再调整。没有更新的包保留现有 features 和用途。

### 仍需遵守的配套约束

| 组合 | 当前与最新 | 当前证据及处理建议 |
| --- | --- | --- |
| Windows SDK / 生成器 | windows 0.62.2 已最新；core 0.62.2、future 0.3.2、bindgen 0.66.0 的各自最新均为 0.100.0 | windows 0.62.2 的正式依赖仍是 core ^0.62.2、future ^0.3.2；不能只换后两者。现有 platform-ext 还使用 bindgen `--flat` / `--no-allow` 生成 WinMD 绑定。旧批次已发现 0.100.0 生成 API 不配套，见 [platform-ext 证据](../../../crates/platform-ext/docs/dev/issue-215/README.md)。建议保持该组合，待完整配套方案可行再更新；不手改 OUT_DIR 产物 |

Rig/RMCP 和 Diesel/libsqlite3-sys 属于 Jaco 专用范围，本轮排除，不再为其升级或兼容性投入工作。rodio 0.22.2 已最新，保持现有精确 pin 和解码 features。

## Changelog 对照：接入上游能力并删除重复实现

2026-09-20 补充。用户明确要求升级时检查可替换现有自写代码的能力，不能止于改版本号。本轮读了 GPUI Kit 0.6.1、0.6.2、0.6.4 release notes，并下载 **crates.io 的 gpui-component/gpui-base 0.6.4 正式包源码**核对 API；普通候选逐项读取发布包 changelog 或官方 release notes。最新正式版仍为 0.6.4，上游 main 已到 `0e63ea799766c486022a0cecfda6e48c5183a2d7`，下列可接入项不依赖 main。

### 纳入升级后的接入项

| 项目与正式 API | 当前代码/重复工作 | 计划替换与保留边界 | 受影响验证 |
| --- | --- | --- | --- |
| **InputGroup**：`InputGroup::input/addon`、`InputGroupTextarea`、`InputGroupAddonAlignment::BlockEnd`、`InputGroupButton`（0.6.2，#3042） | [共用 Composer](../../../app/gupi/src/features/composer.rs) 自己绘制边框、背景、圆角和底部控件布局；主对话和模板任务编辑器复用它 | 用上游共享输入框架承接 Textarea 和底部 addon，删除重复表面样式和布局。可以保留薄的业务组合函数，但不再维持平行的控件样式系统。草稿 Entity、附件、模型选择、发送/停止行为继续归 Gupi。主窗口、临时窗口和模板编辑器同步接入 | 输入聚焦、只读/禁用、窄窗口布局、模型选择和按钮焦点；模板编辑器禁用的是正文时，不把整个 group 禁用导致模型按钮不可用 |
| **设置复合输入**：`InputGroupInput`、inline addon/button（0.6.2） | [快捷键编辑](../../../app/gupi/src/features/settings/keys.rs) 使用 Input suffix 和外层横排手工组合多按钮；配置路径也是输入与操作组合 | 对有多个附加按钮的复合控件使用 group 统一边框、间距及状态；单一普通 Input 不机械套一层。每个快捷键仍为独立 SettingItem，只读录制，保留清除、取消本次修改、恢复默认各自语义，不恢复手输或两段快捷键 | 点击录制、取消、清除、恢复默认和只读路径复制；动作 tooltip 不重复 |
| **Markdown 流式呈现**：`TextView::stream_fade(true)` 或 `.motion(TextViewMotion)`（0.6.2，#3082） | [Markdown 适配](../../../app/gupi/src/features/home/messages/markdown.rs) 已使用 `TextViewState::push_str/set_text` 增量解析，但没有配置呈现动画 | 在已有受管理 TextView 上接入官方流式渐显，先使用上游默认分块效果；需要调整时使用 `with_stream_fade`、`with_stream_fade_easing`、`with_stream_fade_stagger`，不自建逐字 timer、动画文本副本或消息缓冲。历史/静态预览不重播，遵循系统减少动态效果偏好。无需为此新增设置页 | 快速增量、半截 Markdown/代码围栏、正文替换、结束/中止、历史切换、选择复制和滚动跟随 |
| **粘贴接入**：Input/Textarea/Editor 的 `on_paste`（0.6.2，#3087） | [附件粘贴](../../../app/gupi/src/features/home/attachments.rs) 通过 Composer 外层 `capture_action(Paste)` 读取剪贴板并中止传播 | 改用控件回调接收 ClipboardItem，处理返回 true，普通文本返回 false 交给原输入引擎；删除外层重复的 action 捕获/剪贴板读取接线。继续保留应用的文件优先、图片其次、文本最后规则、附件检查和后台读取；回调不能替代附件业务 | 文件和图片各粘贴一次、普通文字不吞不重复、只读状态、剪贴板多种格式和失败反馈 |

**InputGroup 的外壳迁移与未发布 Token 分开：**用户本轮明确把上游替代纳入升级接入，已发布的 InputGroup、on_paste 和流式呈现可以先做；Skill/模板原子标签、Skill 填入行为及 Questionnaire 仍按输入功能计划等待其正式版本，不通过自建编辑器补齐。

### Markdown 哪些能替换，哪些不能直接删

- 0.6.4 的 `stream_fade(true)` 对新增内容分块渐显（组件默认 350ms），`.motion(...)` 可配置时长、曲线及词级错开；它是呈现策略，不是 Pi 网络流的读取/解析接口，也不是 Shimmer。整段替换立即显示，系统减少动态效果时不渐显。
- 我们已经在 append 情况使用 `push_str`，在前缀变化时使用 `set_text`，不是每次只创建无状态 Markdown。新 API 可以直接叠加在当前状态上，不需要重写接收链路。正式源码的 `set_text` 同样支持对扩展内容渐显，但内部仍提交整段替换解析；不能为了少几行适配代码就声称它等价于增量 append。
- 保留 Pi 全量快照到增量的必要适配、稳定实体身份，以及异步解析完成后通知外层 `MessageScroller` 重新测量的连接。0.6.1 的“相同 block 数量替换后重新测量”修复的是 TextView 内部，不能据此断言外层消息行测量订阅已多余。
- 现有 `snapshots_can_overtake_parsing_without_duplicating_or_losing_text` 回归继续验证不丢字/不重复；增加呈现接入后只补其必要行为验证，不引入与实现逐行对应的测试。

源码依据：[TextView 0.6.4](https://docs.rs/crate/gpui-component/0.6.4/source/src/text/compat.rs)、[TextViewState](https://docs.rs/crate/gpui-base/0.6.4/source/src/text/state.rs)、[TextViewMotion](https://docs.rs/crate/gpui-base/0.6.4/source/src/text/stream_fade.rs)、[InputGroup](https://docs.rs/crate/gpui-component/0.6.4/source/src/input/group.rs)、[on_paste](https://docs.rs/crate/gpui-component/0.6.4/source/src/input/input.rs)。

### 同时检查，但不为了新 API 增加功能

| Changelog 项 | 现有实现对照与结论 |
| --- | --- |
| **Empty** 组合空状态（0.6.2，#3030） | 设置资源列表等有手写空态；带标题、说明和操作的重复空态可用 Empty 家族替代。仅一句“没有结果”或新会话保留给输入框的空白不强制改成大块引导；不据此重做页面 |
| Editor 公开搜索/替换 API（0.6.2，#2533） | HTTP Client 的正文查看已用 EditorState 的 `.searchable(true)`，没有找到需要替换的自写搜索引擎。升级继承搜索修复即可，不因为 API 公开就新增工具栏/搜索状态 |
| FrontmatterPlugin、Markdown inline plugin（0.6.1/0.6.2） | Gupi Skill/模板卡片已有名称与说明，预览特意去掉 frontmatter 避免重复。上游结构化 frontmatter 展示不替代 Pi 资源解析，也不是要求恢复重复展示；本轮不增加自定义 mention/math 交互 |
| SVG bytes 图标、共享 Lucide 名称（0.6.1） | 用户已决定采用本地 gpui-lucide：从官方完整 SVG 目录生成独立字节常量，经 Into<Icon> 使用上游 Icon::data。Gupi/Feiwen 退出旧路径注册与资源宏，不再需要 icon_assets! 选择清单；旧两个 crate 只供待删除的 Jaco 使用 |
| Markdown data URL 图片、行距/断行/缓存优化（0.6.1/0.6.2） | 随升级受益。工具返回的独立图片不是 Markdown 图片，不因此删除 tool_details 的图片显示与 base64 解码；也不引入自定义 Markdown 图片加载器 |
| Setting 搜索、Select 关闭清查询、Dialog 多窗口归属、Menu 首帧快捷键、List 选中描边和测量、Linux popup 定位 | 升级继承修复并复测已有界面；搜索补丁作为历史证据，不再施加到新版本。只删除已经确认重复的 workaround，不把所有焦点/状态处理都当作临时补丁 |
| Dock tiles 移除、移动端、Carousel、Sequence、动态图表 | 已阅读兼容说明；当前仍维护应用未找到 Dock tiles 使用。无对应产品需求的能力不接入，不将 changelog 转成新增功能清单 |

### 普通依赖的功能变化与采用判断

以下范围排除 Jaco；“没有替换项”是对照当前调用点的结论，不等于没检查 changelog。

| 依赖变化 | Changelog 关键内容 | 当前采用判断 |
| --- | --- | --- |
| async-compat 0.2.6 | 新增 fallback runtime 的 `multi-thread` feature、MSRV 1.71 | Feiwen/Novel Download 已使用 Compat。没有证据需要多线程 fallback，也不能用它取代有 GPUI 生命周期归属的本地 gpui-tokio；先保留 feature 选择 |
| async-compression 0.4.45–0.4.48 | zstd 配套升级、等待输入时的进度判断修复、gzip 头验证提前、deflate64 EOF 修复 | HTTP Client 已调用上游 decoder，直接受益；保留编码链、大小限制和进度投影，它们是应用职责，不自写解压算法 |
| base64 0.23.0–0.23.1 | 新增运行时选择 AVX2/NEON 的 `Simd` 引擎，feature 为 `simd-unsafe`；0.23.1 修复非 SIMD 平台测试 | Gupi 附件和工具图片、HTTP Client Base64 预览仍显式用 `general_purpose::STANDARD`。升级不等于切到 SIMD；作为大图片/响应体编码性能候选，先比较实际数据规模与结果一致性再采用官方引擎，不自写 CPU 分派。本轮无需顺带扩大 SIMD 使用 |
| encoding_rs 0.8.36–0.8.41 | 流式双字节边界修复、ASCII 路径重写、部分架构使用 simdutf8、MSRV 1.88、multiversion 配套调整 | 当前 HTTP 文本解码已使用该库，升级继承修复/优化；不把字符集选择、严格错误反馈改成隐式替换字符，不自行新增另一套 SIMD 解码 |
| reqwest 0.13.5 | `Error::is_dns()`、`http1_max_headers`、TlsInfo 的 TLS 版本以及代理/超时修复 | 可用于未来细化诊断，但本轮未找到需要被这些 API 替代的自写 DNS 分类、TLS 解析或头部上限实现。不新增网络设置页；HTTP 传输升级并保留现有产品错误投影 |
| sonic-rs 0.5.9–0.5.10 | SIMD key 查找、数字解析等性能优化，浮点输出修复 | Gupi 已接入按字段解析，直接受益；不因此再次重构会话搜索或全面替换 serde_json，性能变化须测实际 JSONL 才能量化 |
| clap 4.6.7 | derive 的 `#[command(defer = ...)]` 延迟构建子命令 | xtask 命令树很小，暂无值得替换的手写懒初始化；不增加配置 |
| syn 3.0.6 / trybuild 1.0.121 | lifetime 解析修复 / 内部 target 依赖替换 | 宏和编译回归直接受益，不新增宏 API |
| toml 1.1.6 | 减少解析分配 | 原配置读写接口保持，直接继承优化 |
| trash 5.2.9 | Linux 卷回收站创建被拒绝时回退 home trash | 已有会话删除调用直接受益；保留正在使用会话等业务限制，不另写同类跨卷回退 |
| uuid 1.26.1 | v7 计数位置和 Timestamp 转换溢出修复 | Gupi 使用 v4，不为采用新版特性改变会话 ID 策略；Jaco 的 v7 不在维护范围 |
| which 8.0.3–8.0.6 | 修复绝对路径查找、按调用者 CWD 解析相对 PATH；Windows 缺 PATHEXT 时报告非致命错误 | 主锁已为 8.0.6。pi-rpc 已使用 `which_in(executable, paths, cwd)`，直接获得正确的子进程工作目录语义；只统一声明下限，不再复制上游路径修复。xtask 的命令检测继续用 which |
| plist 1.10.1 | 二进制 plist 日期越界不再 panic，更新 base64/quick-xml/indexmap，Rust 2024 / MSRV 1.88 | 主锁已为 1.10.1；xtask 已用 Value 读写 Info.plist，不新增解析器或日期兜底。`plist!` / `plist_dict!` 在原 1.10.0 已有，并非本轮新能力，不为采用宏重写已有字典生成 |
| serde 1.0.229 | derive 内部更新到 syn 3 | 主锁已为 1.0.229。Gupi / pi-rpc 的较低声明可统一；没有新的序列化产品能力或需要替换的手写业务逻辑 |


普通依赖依据：[async-compat changelog](https://docs.rs/crate/async-compat/0.2.6/source/CHANGELOG.md)、[async-compression changelog](https://docs.rs/crate/async-compression/0.4.48/source/CHANGELOG.md)、[base64 release notes](https://docs.rs/crate/base64/0.23.1/source/RELEASE-NOTES.md)、[encoding_rs release notes](https://github.com/hsivonen/encoding_rs#release-notes)、[reqwest 0.13.5](https://github.com/seanmonstar/reqwest/releases/tag/v0.13.5)、[sonic-rs 0.5.9](https://github.com/cloudwego/sonic-rs/releases/tag/v0.5.9)、[0.5.10](https://github.com/cloudwego/sonic-rs/releases/tag/v0.5.10)、[clap 4.6.7](https://github.com/clap-rs/clap/releases/tag/v4.6.7)、[syn 3.0.6](https://github.com/dtolnay/syn/releases/tag/3.0.6)、[trybuild 1.0.121](https://github.com/dtolnay/trybuild/releases/tag/1.0.121)、[toml changelog](https://github.com/toml-rs/toml/blob/main/crates/toml/CHANGELOG.md)、[trash 5.2.9](https://docs.rs/crate/trash/5.2.9/source/CHANGELOG.md)、[uuid 1.26.1](https://github.com/uuid-rs/uuid/releases/tag/v1.26.1)。

### 其他依赖补查与实际适配点

2026-09-20 再次将普通升级项和调用点逐项核对，覆盖上述 12 个普通更新候选（async-compat、async-compression、base64、clap、encoding_rs、reqwest、sonic-rs、syn、toml、trash、trybuild、uuid），以及已锁最新但声明较低的 plist、serde、which。没有版本变化的其余包不虚构“本次升级新增”能力；Jaco 专用库继续排除。

**Lucide：已经确认需要改资源映射。**读取当前 revision `5136572c` 到 1.47.0 的 compare（197 个提交）及 release notes；GitHub compare 的文件清单截断在 300 项，因此不能用它证明图标兼容。另对正式标签的完整 `icons/` 文件树核对仍维护应用 `foundation/assets.rs` 中声明的 69 个唯一 Lucide 名称：唯一缺失的是 Gupi 的 `trash-2.svg`，新标签保留 `trash.svg`。进一步读取新 `trash.svg`，与当前子模块旧 `trash-2.svg` **逐字节相同**。该调查已由新方案取代：不再升级子模块供 Gupi 使用；gpui-lucide 采用正式资源包的 `Trash` 字节常量，Gupi 调用改为 `IconName::Trash`，不新增兼容副本。Feiwen 原本引用 `trash`，路径仍可用；更新后图形变化属于上游资源变更，启动时查看实际效果。这里核对的是声明资源，不宣称已完成编译/打包。

**Base64：性能候选需要显式选择，不只是改版本。**0.23 的 `simd-unsafe` 默认 feature 只是让 SIMD 引擎可用，当前显式 `general_purpose::STANDARD` 仍是标量引擎。HTTP Client 已设置 `default-features = false, features = ["std"]`；即使应用需要 SIMD，也必须同时调整 feature 和调用的 engine。优先考虑大图片附件及 Base64 响应预览，不为了 Basic Auth 的短字符串改所有调用；保留标准 alphabet/padding 及错误行为，测量后才能报告提升。只调用上游运行时分派的 Simd，不自写 AVX2/NEON 检测或用固定平台 engine 假设 CPU 支持。

**HTTP 管线：保留业务限制，采用库内修复。**`async-compression` 修复的是解码内部；`encoding_rs` 改善流式字节边界和 SIMD 实现；我们没有另一套手写 gzip/zstd/字符集解码器可删。HTTP Client 的 Content-Encoding 链、大小限制、进度回调及严格解码反馈仍需保留。reqwest 的 `is_dns` 可以支持新增错误分类，但当前代码只区分请求体读取与传输错误，没有字符串猜 DNS 的实现可以直接替换；本次不以接入新 API 为理由扩展错误分类、TLS 展示或设置页面。

**运行时桥接：async-compat 的 multi-thread 不替代 gpui-tokio。**Feiwen/Novel Download 实际用 `Compat::new` 包装下载/抓取 future；新的 feature 控制没有现成 Tokio runtime 时的 fallback runtime。它并不提供 GPUI Entity/Task 生命周期接线，不能据此删除本地 bridge 或默认新开额外多线程运行时。

**底层 gpui-pre：源码快照没有独立组件式 changelog。**发布工作流从 Zed revision 生成配套 crates，并把来源 revision 记录到发布工作流摘要。0.3.3/0.3.5 发布包没有足以列出完整 API 差异的独立 changelog；目前只能确认与 kit 0.6.4 的版本配套，不能把 Zed 整体 release notes 当作我们已获得的功能。其 API/平台差异仍需按实际构建检查；不借快照升级提前重写 window-ext/platform-ext。

补充依据：[which 8.0.6 changelog](https://docs.rs/crate/which/8.0.6/source/CHANGELOG.md)、[Pi 查找调用](../../../crates/pi-rpc/src/client.rs)、[plist 1.10.1 changelog](https://docs.rs/crate/plist/1.10.1/source/CHANGELOG.md)、[Serde 1.0.229](https://github.com/serde-rs/serde/releases/tag/v1.0.229)、[Lucide 完整更新区间](https://github.com/lucide-icons/lucide/compare/5136572c10214634858fcf5f726b2a9d26683918...1.47.0)、[新 trash.svg](https://github.com/lucide-icons/lucide/blob/1.47.0/icons/trash.svg)、[Gupi 图标声明](../../../app/gupi/src/foundation/assets.rs)、[Base64 SIMD 说明](https://docs.rs/crate/base64/0.23.1/source/README.md)、[GPUI 快照发布流程](https://github.com/longbridge/gpui-kit/blob/0e63ea799766c486022a0cecfda6e48c5183a2d7/.github/workflows/release-gpui.yml)。

### 本轮交付边界

依赖更新实施应包含上面已选的 InputGroup、复合输入、Markdown 渐显和 on_paste 接入及重复代码删除，不能仅修改 Cargo 版本后就算完成。其余候选按实际替代价值处理；未发布 Token/Questionnaire、队列产品行为以及 Jaco 清理不混进这项替代工作。上述已选接入现已完成，结果见实施记录。

## 安装官方 skill，停止维护上游资料副本

官方 README 当前推荐 `npx skills add longbridge/gpui-kit`，上游提供两个 skill：

| skill | 内容 | 本项目处理 |
| --- | --- | --- |
| `gpui-kit` | 组件目录、使用方法、GPUI entity/context/async/focus 等机制及 Coding Guides | 替代本地复制的 GPUI 和组件上游资料 |
| `gpui-kit-design-guides` | 布局、密度、交互状态、弹层和界面文案指南 | 直接安装，不再维护另一份设计指南副本 |

已使用 Codex `skill-installer` 的 `install-skill-from-github.py` 进行项目级安装，固定来源
`longbridge/gpui-kit@0e63ea799766c486022a0cecfda6e48c5183a2d7`，路径为 `skills/gpui-kit` 和 `skills/gpui-kit-design-guides`，目标为本项目 `.agents/skills/`。两个目录均包含 `SKILL.md`、官方 references 和上游 Apache 2.0 LICENSE；未进行全局安装。

该安装器不生成 Vercel `skills` 的锁文件，因此不能声称可用 `skills update` 原地维护；更新时通过相同安装器重新安装经过核对的 revision，并在本文更新来源。Codex 的 skill 列表在后续会话加载，本轮已确认落盘路径和入口，尚未把后续会话发现作为已验证结果。

已删除 `.agents/skills/gpui` 与 `.agents/skills/gpui-component-usage` 的旧 v0.6.0 镜像及专属许可/归属文件。项目 alias/import、共享主题、渐变表面、Form / catalog / 控件状态分工保留在 `gpui-app-development`；图标、国际化、Form / Store / Operation 等专用 skills 保留。AGENTS 和有效入口导航已更新；旧迁移批次文档作为历史记录保留。

官方最新 skill 可能描述尚未发布的组件 API：**实现依据仍是本项目实际依赖版本的源码和示例**。skill 更新不会自动解锁 Input token 等未发布能力。既有 #215 保留两个手工镜像的决定由本轮直接安装官方 skill 的要求取代。

## 其他依赖与仓库工具

| 对象 | 当前状态 | 更新目标或处理 |
| --- | --- | --- |
| Lucide 来源 | 新 gpui-lucide 使用 gpui-kit-assets 0.6.4 随包目录（Lucide 1.43.0 + 保留图标），构建时无网络 | 原 third_party/lucide 不再为 Gupi/Feiwen 提供图标；它与 app-assets/app-assets-macros 暂留给 Jaco，按 Gupi 合入后的清理计划一起删除。无需同步到独立 Lucide 1.47.0 |
| Rust / Cargo | 本机 1.98.1；官方 stable 1.98.1；CI 跟随 stable，无 rust-toolchain 文件 | 当前无需升级；候选依赖的 MSRV 仍须结合新锁图验证，不额外改成 nightly |
| GitHub Actions | checkout v7.0.1、rust-cache v2.9.2 均固定 SHA，均为当前最新 release；rust-toolchain@stable 为滚动引用 | 无版本更新；保留现有策略，不把“滚动引用”误记成落后 |
| Pi 运行时 | 用户安装/配置的可执行文件；当前审计协议 0.85.1，官方最新 v0.85.1 | 非 Cargo 内嵌库，不擅自替用户升级全局 Pi；沿用独立进程启动和 RPC 契约 |
| Linux 系统库 | script/install-linux.sh 使用 runner apt 源，没有固定单独版本 | 随支持平台安装；保留音频、字体、GTK/WebKit、Wayland/X11、Vulkan 等现有依赖，新图确有变化时再调整脚本和 bundle 系统依赖 |
| Windows WinMD | platform-ext/winmd 下 6 个签入文件 | 属于绑定生成输入；当前未从文件本身确定可比较的上游 SDK 包版本，不杜撰“已最新”。本次先保持，与 Windows 配套迁移共同核对来源 |
| 项目内 path crate | 与 workspace 源码一起维护，见下方成员清单 | 没有独立“crates.io 最新版”目标，随直接依赖/API 变化调整；不替换为同名第三方包 |
| 应用内主题、图标和字体文件 | 静态资源，不是独立可解析的包依赖 | 除已明确的 Lucide 来源外，不凭更新依赖之名重绘/替换用户界面资源 |

## 实施顺序与验证

1. 实施当天刷新正式版本和上述配套证据，确定精确 GPUI 组合；先更新 manifest 与根锁（独立 Jaco 工具锁不更新），不混入队列产品行为改动。
2. 完成 GPUI 编译适配、普通依赖更新和 base64 调用适配；实施上方已选上游替代项并删除重复接线/样式；锁图中允许上游正常需要的多版本，重点排查跨公共类型边界的 GPUI/Windows 重复。
3. 完成 gpui-lucide 的字节图标生成、应用迁移与未引用 SVG 剔除验证；安装官方 skill 并清理被替换镜像及导航。
4. 对仍维护的四个应用及共享 crate 执行格式、构建、测试和 Clippy。使用 workspace 命令时，对 build/test/clippy 显式排除 `jaco`、`jaco-agent`、`jaco-core`、`jaco-db`、`jaco-conversation`；不验证专用 MCP 工具。当前 CI 仍构建整个 workspace 是已知配置现状，不能据此把 Jaco 适配重新变成任务；若共享升级受其牵连，明确记录范围影响，后续删除时同步清理 CI/打包入口。
5. 对实际受影响能力复用已有回归：GPUI/Form 宏与控件、Gupi JSONL/附件编码、HTTP 解压和文本解码、路径选择。必要时做各受影响应用的启动检查；三平台构建结果分别记录，本机通过不能代替 Windows/Linux。
6. Gupi 验证设置搜索、模型选择、命令/搜索面板及临时窗口的基础交互。InputGroup、复合输入、流式渐显和粘贴回调按上方检查点验证；若实际依赖版本已包含三项输入组件能力，其他资源交互迁移回到对应输入/问卷计划，不在依赖升级中顺带实现整套队列交互。

## 平台与验证边界

- 本轮受影响测试、Clippy 和原生界面重点复测的实际结果见末尾记录；不等于完整发行验收。
- Windows/Linux 尚未在本机验证；Windows SDK 的配套暂留理由保持不变。
- Jaco 专用内容不维护；未发布输入能力继续等待，不作为本轮未完成的实现工作。

目前没有需要用户立即回答的产品问题；这些是执行时的技术核验，遇到会改变范围或行为的事实再记录讨论。

## 全量直接依赖清单

“主锁/工具锁”是直接使用者的锁定结果，`—` 表示该锁中没有直接使用者；不代表没有同名传递包。使用方为 app/crates/tools 目录名，`workspace` 代表根声明。所有版本来自 2026-09-19 官方 sparse index；“保持”指当前直接解析已是最高正式版，不代表已经完成兼容或安全审计。

| 包 | manifest 声明 | 主锁 / 工具锁 | 最新正式版 | 建议 | 使用方 |
| --- | --- | --- | --- | --- | --- |
| `anyhow` | `1.0.104` | — / 1.0.104 | 1.0.104 | 排除（Jaco） | tools/mcp-auth-test-server |
| `async-channel` | `2.5.0` | 2.5.0 / — | 2.5.0 | 保持 | app/http-client |
| `async-compat` | `0.2.5` | 0.2.5 / — | 0.2.6 | 更新 | app/feiwen, app/novel-download |
| `async-compression` | `0.4.44` | 0.4.44 / — | 0.4.48 | 更新 | app/http-client, crates/http-client-test-server |
| `async-stream` | `0.3.6` | 0.3.6 / — | 0.3.6 | 保持 | app/novel-download, crates/jaco-agent |
| `async-trait` | `0.1.92` | 0.1.92 / — | 0.1.92 | 排除（Jaco） | app/jaco, crates/jaco-agent |
| `axum` | `0.8.9` | — / 0.8.9 | 0.8.9 | 排除（Jaco） | tools/mcp-auth-test-server |
| `base64` | `0.22.1`, `0.23.1` | 0.22.1, 0.23.1 / — | 0.23.1 | 跨系列核验 | app/gupi, app/http-client, crates/http-client-test-server, crates/jaco-agent |
| `block2` | `0.6.2` | 0.6.2 / — | 0.6.2 | 保持 | crates/platform-ext |
| `bytemuck` | `1.25.2` | 1.25.2 / — | 1.25.2 | 保持 | app/http-client |
| `bytes` | `1.12.1` | 1.12.1 / — | 1.12.1 | 保持 | app/http-client, crates/http-client-test-server |
| `clap` | `4.6.6` | 4.6.6 / — | 4.6.7 | 更新 | crates/xtask |
| `diesel` | `2.3.13` | 2.3.13 / — | 2.3.13 | 排除（Jaco） | crates/jaco-db |
| `dirs` | `6.0.0` | 6.0.0 / — | 7.0.0 | 排除（Jaco） | crates/jaco-agent |
| `dirs-next` | `2.0.0` | 2.0.0 / — | 2.0.0 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download |
| `duckdb` | `1.10505.0` | 1.10505.0 / — | 1.10505.0 | 保持 | app/feiwen |
| `encoding_rs` | `0.8.35` | 0.8.35 / — | 0.8.41 | 更新 | app/http-client |
| `fluent-bundle` | `0.16.0` | 0.16.0 / — | 0.16.0 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download |
| `futures` | `0.3.34` | 0.3.34 / — | 0.3.34 | 保持 | app/novel-download, crates/jaco-agent |
| `futures-util` | `0.3.34` | 0.3.34 / — | 0.3.34 | 保持 | app/http-client, crates/http-client-test-server |
| `garde` | `0.23.0` | 0.23.0 / — | 0.23.0 | 保持 | app/jaco, crates/gpui-form, workspace |
| `get-selected-text` | `0.1.6` | 0.1.6 / — | 0.1.6 | 保持 | app/gupi, app/jaco |
| `glob` | `0.3.4` | 0.3.4 / — | 0.3.4 | 保持 | app/gupi |
| `global-hotkey` | `0.8.0` | 0.8.0 / — | 0.8.0 | 保持 | app/gupi, app/jaco |
| `globset` | `0.4.20` | 0.4.20 / — | 0.4.20 | 保持 | app/gupi, crates/jaco-agent |
| `gpui-component` | `=0.6.0` | 0.6.0 / — | 0.6.4 | 配套更新 | crates/app-assets, crates/app-theme, crates/gpui-form, crates/gpui-form-gpui-component, crates/gpui-heatmap, workspace |
| `gpui-kit` | `=0.6.0` | 0.6.0 / — | 0.6.4 | 配套更新 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download, workspace |
| `gpui-kit-assets` | `=0.6.0` | 0.6.0 / — | 0.6.4 | 配套更新 | crates/app-assets, workspace |
| `gpui-pre` | `=0.3.3` | 0.3.3 / — | 0.3.5 | 配套更新 | crates/app-assets, crates/app-theme, crates/gpui-form, crates/gpui-form-gpui-component, crates/gpui-heatmap, crates/gpui-operation, crates/gpui-store, crates/gpui-tokio, crates/window-ext, workspace |
| `gpui-pre-platform` | `=0.3.3` | — / — | 0.3.5 | 配套更新 | workspace |
| `grep-matcher` | `0.1.9` | 0.1.9 / — | 0.1.9 | 排除（Jaco） | crates/jaco-agent |
| `grep-regex` | `0.1.14` | 0.1.14 / — | 0.1.14 | 排除（Jaco） | crates/jaco-agent |
| `grep-searcher` | `0.1.17` | 0.1.17 / — | 0.1.17 | 排除（Jaco） | crates/jaco-agent |
| `hayro` | `0.7.1` | 0.7.1 / — | 0.7.1 | 保持 | app/http-client |
| `hex` | `0.4.3` | 0.4.3 / — | 0.4.3 | 排除（Jaco） | crates/jaco-agent |
| `http` | `1.5.0` | 1.5.0 / — | 1.5.0 | 保持 | app/http-client, app/jaco, crates/jaco-agent |
| `http-body-util` | `0.1.5` | 0.1.5 / — | 0.1.5 | 保持 | crates/http-client-test-server |
| `hyper` | `1.11.1` | 1.11.1 / — | 1.11.1 | 保持 | crates/http-client-test-server |
| `hyper-util` | `0.1.20` | 0.1.20 / — | 0.1.20 | 保持 | crates/http-client-test-server |
| `ignore` | `0.4.33` | 0.4.33 / — | 0.4.33 | 保持 | app/gupi, crates/jaco-agent |
| `image` | `0.25.10` | 0.25.10 / — | 0.25.10 | 保持 | app/gupi, app/http-client, app/jaco, crates/jaco-agent, crates/xtask |
| `libsqlite3-sys` | `0.38.2` | 0.38.2 / — | 0.38.2 | 排除（Jaco） | crates/jaco-db |
| `markdown` | `1.0.0` | 1.0.0 / — | 1.0.0 | 保持 | app/gupi |
| `material-color-utils` | `0.1.3` | 0.1.3 / — | 0.1.3 | 保持 | crates/app-theme |
| `mime` | `0.3.17` | 0.3.17 / — | 0.3.17 | 保持 | app/http-client |
| `mime_guess` | `2.0.5` | 2.0.5 / — | 2.0.5 | 保持 | app/http-client |
| `nom` | `8.0.0` | 8.0.0 / — | 8.0.0 | 保持 | app/feiwen |
| `notify-debouncer-full` | `0.7.0` | 0.7.0 / — | 0.7.0 | 排除（Jaco） | app/jaco |
| `objc2` | `0.6.4` | 0.6.4 / — | 0.6.4 | 保持 | crates/platform-ext, crates/window-ext |
| `objc2-app-kit` | `0.3.2` | 0.3.2 / — | 0.3.2 | 保持 | crates/platform-ext, crates/window-ext |
| `objc2-core-foundation` | `0.3.2` | 0.3.2 / — | 0.3.2 | 保持 | crates/platform-ext |
| `objc2-core-graphics` | `0.3.2` | 0.3.2 / — | 0.3.2 | 保持 | crates/platform-ext |
| `objc2-foundation` | `0.3.2` | 0.3.2 / — | 0.3.2 | 保持 | crates/platform-ext, crates/window-ext |
| `pinyin` | `0.11.0` | 0.11.0 / — | 0.11.0 | 保持 | app/feiwen, app/jaco |
| `plist` | `1.10.0` | 1.10.1 / — | 1.10.1 | 统一声明 | crates/xtask |
| `proc-macro2` | `1.0.107` | 1.0.107 / — | 1.0.107 | 保持 | crates/app-assets-macros, crates/gpui-form-macros |
| `quote` | `1.0.47` | 1.0.47 / — | 1.0.47 | 保持 | crates/app-assets-macros, crates/gpui-form-macros |
| `r2d2` | `0.8.10` | 0.8.10 / — | 0.8.10 | 保持 | app/feiwen |
| `raw-window-handle` | `0.6.2` | 0.6.2 / — | 0.6.2 | 保持 | crates/window-ext |
| `regex` | `1.13.1` | 1.13.1 / — | 1.13.1 | 保持 | app/feiwen |
| `reqwest` | `0.13.4` | 0.13.4 / — | 0.13.5 | 更新 | app/feiwen, app/http-client, app/novel-download, crates/http-client-test-server, crates/jaco-agent |
| `rig` | `0.42.0` | 0.42.0 / — | 0.42.0 | 排除（Jaco） | crates/jaco-agent, workspace |
| `rmcp` | `2.2.0` | 2.2.0 / 2.2.0 | 3.4.0 | 排除（Jaco） | app/jaco, crates/jaco-agent, tools/mcp-auth-test-server, workspace |
| `rodio` | `=0.22.2` | 0.22.2 / — | 0.22.2 | 保持 | app/http-client |
| `rust-embed` | `8.12.0` | 8.12.0 / — | 8.12.0 | 排除（Jaco） | app/jaco |
| `schemars` | `1.2.2` | — / 1.2.2 | 1.2.2 | 排除（Jaco） | tools/mcp-auth-test-server |
| `scraper` | `0.27.0` | 0.27.0 / — | 0.27.0 | 保持 | app/feiwen, app/novel-download |
| `serde` | `1.0.228`, `1.0.229` | 1.0.229 / 1.0.229 | 1.0.229 | 统一声明 | app/gupi, app/jaco, crates/http-client-test-server, crates/jaco-agent, crates/jaco-core, crates/jaco-db, crates/pi-rpc, crates/xtask, tools/mcp-auth-test-server |
| `serde_json` | `1.0.151` | 1.0.151 / 1.0.151 | 1.0.151 | 保持 | app/gupi, app/http-client, app/jaco, crates/app-theme, crates/http-client-test-server, crates/jaco-agent, crates/jaco-core, crates/jaco-db, crates/pi-rpc, tools/mcp-auth-test-server |
| `serde_yaml_ng` | `0.10.0` | 0.10.0 / — | 0.10.0 | 保持 | app/gupi |
| `sha2` | `0.11.0` | 0.11.0 / — | 0.11.0 | 排除（Jaco） | crates/jaco-agent |
| `similar` | `3.2.0` | 3.2.0 / — | 3.2.0 | 排除（Jaco） | crates/jaco-agent |
| `smol` | `2.0.2` | 2.0.2 / — | 2.0.2 | 保持 | app/gupi, app/jaco, app/novel-download, crates/app-theme |
| `sonic-rs` | `0.5.8` | 0.5.8 / — | 0.5.10 | 更新 | app/gupi |
| `syn` | `3.0.5` | 3.0.5 / — | 3.0.6 | 更新 | crates/app-assets-macros, crates/gpui-form-macros |
| `sys-locale` | `0.3.2` | 0.3.2 / — | 0.3.2 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download |
| `tauri-bundler` | `2.9.4` | 2.9.4 / — | 2.9.4 | 保持 | crates/xtask |
| `tauri-utils` | `2.9.3` | 2.9.3 / — | 2.9.3 | 保持 | crates/xtask |
| `tempfile` | `3.27.0` | 3.27.0 / — | 3.27.0 | 保持 | app/gupi, app/http-client, app/jaco, app/novel-download, crates/jaco-agent, crates/jaco-conversation, crates/jaco-db, crates/pi-rpc |
| `thiserror` | `2.0.20` | 2.0.20 / — | 2.0.20 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download, crates/http-client-test-server, crates/jaco-agent, crates/jaco-conversation, crates/jaco-db, crates/pi-rpc, crates/platform-ext, crates/window-ext, crates/xtask |
| `time` | `0.3.55` | 0.3.55 / — | 0.3.55 | 保持 | app/gupi, app/jaco, crates/gpui-heatmap, crates/jaco-agent, crates/jaco-core, crates/jaco-db |
| `tokio` | `1.53.1` | 1.53.1 / 1.53.1 | 1.53.1 | 保持 | app/gupi, app/http-client, app/jaco, crates/gpui-tokio, crates/http-client-test-server, crates/jaco-agent, crates/pi-rpc, tools/mcp-auth-test-server |
| `tokio-util` | `0.7.19` | 0.7.19 / 0.7.19 | 0.7.19 | 保持 | app/http-client, crates/http-client-test-server, crates/jaco-agent, tools/mcp-auth-test-server |
| `toml` | `1.1.5` | 1.1.5+spec-1.1.0 / — | 1.1.6+spec-1.1.0 | 更新 | app/gupi, app/jaco, crates/xtask |
| `tracing` | `0.1.44` | 0.1.44 / — | 0.1.44 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download, crates/gpui-operation, crates/jaco-agent, crates/pi-rpc, crates/platform-ext, crates/xtask |
| `tracing-subscriber` | `0.3.23` | 0.3.23 / — | 0.3.23 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, app/novel-download, crates/xtask |
| `trash` | `5.2.8` | 5.2.8 / — | 5.2.9 | 更新 | app/gupi |
| `tray-icon` | `0.25.1` | 0.25.1 / — | 0.25.1 | 保持 | app/gupi |
| `trybuild` | `1.0.120` | 1.0.120 / — | 1.0.121 | 更新 | crates/gpui-form, crates/gpui-form-macros |
| `unic-langid` | `0.9.6` | 0.9.6 / — | 0.9.6 | 保持 | app/feiwen, app/http-client, app/jaco, app/novel-download |
| `unicode-segmentation` | `1.13.3` | 1.13.3 / — | 1.13.3 | 排除（Jaco） | app/jaco |
| `url` | `2.5.8` | 2.5.8 / 2.5.8 | 2.5.8 | 保持 | app/feiwen, app/gupi, app/http-client, app/jaco, crates/jaco-agent, crates/jaco-db, tools/mcp-auth-test-server |
| `uuid` | `1.26.0` | 1.26.0 / — | 1.26.1 | 更新 | app/gupi, crates/jaco-core |
| `walkdir` | `2.5.0` | 2.5.0 / — | 2.5.0 | 保持 | crates/xtask |
| `which` | `8.0.2`, `8.0.6` | 8.0.6 / — | 8.0.6 | 统一声明 | crates/pi-rpc, crates/xtask |
| `windows` | `0.62.2` | 0.62.2 / — | 0.62.2 | 保持 | app/jaco, crates/platform-ext, crates/window-ext |
| `windows-bindgen` | `0.66.0` | 0.66.0 / — | 0.100.0 | 配套暂留 | crates/platform-ext |
| `windows-core` | `0.62.2` | 0.62.2 / — | 0.100.0 | 配套暂留 | crates/platform-ext |
| `windows-future` | `0.3.2` | 0.3.2 / — | 0.100.0 | 配套暂留 | crates/platform-ext |
| `winresource` | `0.1` | 0.1.31 / — | 0.1.31 | 排除（Jaco） | app/jaco |
| `xcap` | `0.9.8` | 0.9.8 / — | 0.9.8 | 排除（Jaco） | app/jaco |

## 本地成员与独立工具

以下成员均已纳入 manifest 盘点；标记停止维护的成员和工具排除升级与验证。依赖表中的 path 引用不按外部包计数。

- [feiwen](../../../app/feiwen/Cargo.toml)：0.1.0。
- [gupi](../../../app/gupi/Cargo.toml)：0.1.0。
- [http-client](../../../app/http-client/Cargo.toml)：0.1.0。
- [jaco](../../../app/jaco/Cargo.toml)：0.1.0。 **停止维护；本次排除，Gupi 合入后清理。**
- [novel-download](../../../app/novel-download/Cargo.toml)：0.1.0。
- [app-assets](../../../crates/app-assets/Cargo.toml)：0.1.0。
- [app-assets-macros](../../../crates/app-assets-macros/Cargo.toml)：0.1.0。
- [app-theme](../../../crates/app-theme/Cargo.toml)：0.1.0。
- [gpui-form](../../../crates/gpui-form/Cargo.toml)：0.1.0。
- [gpui-form-gpui-component](../../../crates/gpui-form-gpui-component/Cargo.toml)：0.1.0。
- [gpui-form-macros](../../../crates/gpui-form-macros/Cargo.toml)：0.1.0。
- [gpui-heatmap](../../../crates/gpui-heatmap/Cargo.toml)：0.1.0。
- [gpui-operation](../../../crates/gpui-operation/Cargo.toml)：0.1.0。
- [gpui-store](../../../crates/gpui-store/Cargo.toml)：0.1.0。
- [gpui-tokio](../../../crates/gpui-tokio/Cargo.toml)：0.1.0。
- [http-client-test-server](../../../crates/http-client-test-server/Cargo.toml)：0.1.0。
- [jaco-agent](../../../crates/jaco-agent/Cargo.toml)：0.1.0。 **停止维护；本次排除，Gupi 合入后清理。**
- [jaco-conversation](../../../crates/jaco-conversation/Cargo.toml)：0.1.0。 **停止维护；本次排除，Gupi 合入后清理。**
- [jaco-core](../../../crates/jaco-core/Cargo.toml)：0.1.0。 **停止维护；本次排除，Gupi 合入后清理。**
- [jaco-db](../../../crates/jaco-db/Cargo.toml)：0.1.0。 **停止维护；本次排除，Gupi 合入后清理。**
- [pi-rpc](../../../crates/pi-rpc/Cargo.toml)：0.1.0。
- [platform-ext](../../../crates/platform-ext/Cargo.toml)：0.1.0。
- [window-ext](../../../crates/window-ext/Cargo.toml)：0.1.0。
- [xtask](../../../crates/xtask/Cargo.toml)：0.1.0。
- [mcp-auth-test-server](../../../tools/mcp-auth-test-server/Cargo.toml)：0.1.0，独立 workspace / 独立锁文件。 **停止维护；本次排除，Gupi 合入后清理。**

## 来源与调查边界

- [Cargo 官方 sparse index](https://index.crates.io/config.json)：逐包读取版本记录、yanked、rust_version、dependencies；例如 [gpui-kit](https://index.crates.io/gp/ui/gpui-kit)、[Rig](https://index.crates.io/3/r/rig)、[windows](https://index.crates.io/wi/nd/windows)。最高正式版本过滤 prerelease 和 yanked，保留 `+` 构建元数据。
- [GPUI Kit v0.6.4](https://github.com/longbridge/gpui-kit/releases/tag/v0.6.4)、[固定 main README](https://github.com/longbridge/gpui-kit/blob/f698b4bcac037b8d208b34eca86cc940081c498f/README.md)、[官方 skill 目录](https://github.com/longbridge/gpui-kit/tree/f698b4bcac037b8d208b34eca86cc940081c498f/skills)。
- [skills 安装工具说明](https://github.com/vercel-labs/skills#readme)：项目范围、指定 agent/skill、链接与更新方式。
- [Lucide 1.47.0](https://github.com/lucide-icons/lucide/releases/tag/1.47.0)、[Rust 1.98.1](https://github.com/rust-lang/rust/releases/tag/1.98.1)、[Pi 0.85.1](https://github.com/earendil-works/pi/releases/tag/v0.85.1)。
- 本项目：[root manifest](../../../Cargo.toml)、[主锁](../../../Cargo.lock)、[独立工具](../../../tools/mcp-auth-test-server/Cargo.toml)、[CI](../../../.github/workflows/ci.yml)、[系统依赖](../../../script/install-linux.sh)、[图标来源](../../../.gitmodules)。

前期版本与 changelog 调查记录保留作基线；以下记录实际实施与验证。未进行漏洞审计。

## SVG bytes 图标改造（2026-09-20）

- 新库 [gpui-lucide](../../../crates/gpui-lucide/README.md) 由 gpui-kit-assets 正式提供的 Cargo icons-dir 元数据生成完整目录，无私有 API、网络下载或仓库子模块路径依赖。
- `SvgIcon` 是静态 bytes 的可复制值，`IconName` 是其别名，每个图标为独立关联常量；没有引用全目录的 enum match 或 ALL 表。动态代码实际引用的多个图标会同时保留，未引用图标可由链接器剔除。
- 实现 `From<SvgIcon> for gpui_component::Icon` 和 RenderOnce，默认样式、克隆、原生图标转换仍由上游负责；不实现只支持路径的 IconNamed，不另外实现一套渲染器。
- Gupi/Feiwen 改用新库；Gupi Provider SVG 走同一字节转换。默认组件图标和应用黑白 Logo 仍注册原有 AssetSource。
- 因原 0.6.0 无 Icon::data，此步需要同时升级 GPUI Kit 0.6.4 与 GPUI 0.3.5。Jaco 不迁移，旧两个 crate 与子模块留待其删除，避免为停止维护的应用做额外接入。
- 必要兼容适配：删除 app-theme 已失效的 tiles 映射，内置 Aurora 主题将 chart_bullish/chart_bearish 改成上游的 chart.bullish/chart.bearish；按新序列化字段更新主题基线，颜色值未改变。
- 验证通过：四个仍维护应用 `cargo check --locked --offline`；gpui-lucide 转换测试和文档测试；app-theme 全部 23 项测试；gpui-lucide/Gupi/Feiwen/app-theme 的 all-targets Clippy（-D warnings）；cargo fmt 与 diff 空白检查。
- 链接验证：macOS arm64 上编译 one_icon/two_icons 示例（dev 依赖 + 示例 opt-level=3、debuginfo=0），运行成功。以官方包的 1,830 份完整 SVG bytes 检查二进制：one_icon 仅命中 search，two_icons 仅命中 brain/search。该结果证明实际 Into<Icon> 路径没有保留完整目录；它不是整个应用 release 体积或运行时内存基准。
- 没有进行原生 UI 外观复测、Windows/Linux 验证或完整应用打包；该条为图标阶段验证范围；后续依赖与组件实施见下节。

### 普通依赖与组件接入（2026-09-20）

- 已更新上述普通依赖及 manifest 起始版本；base64 0.23.1 的现有 Engine 调用无需代码迁移。sonic-rs 0.5.10 首次构建暴露锁中的 sonic-number 0.1.2 缺少新解析 API，配套更新到 0.1.3 后四个应用及 xtask 的 `cargo check --locked` 通过。
- Composer 由官方 InputGroup + Textarea + BlockEnd addon 组成。保留业务 footer 内容和原草稿 Entity；主/临时对话共用，模板编辑器采用组级 readonly，避免 disabled 内部控件导致整个组及模型按钮禁用。
- 设置的快捷键（应用内与全局）和配置路径使用 InputGroup inline addon。录制、清除、取消、恢复默认及复制路径仍沿用原状态和动作；每个快捷键仍是独立 SettingItem。
- 粘贴通过 Textarea::on_paste 接收 ClipboardItem，路径/图片处理返回 true，普通文本返回 false；删除外层 Paste capture、二次读剪贴板与传播中断。
- assistant 正文与思考使用官方 stream_fade 默认呈现；初始历史不重播。增量快照适配、替换正文及解析完成后的外层滚动测量订阅保留。用户消息、静态资源预览和普通工具输出不额外启用动画。
- 官方 skills 安装和旧资料清理已完成，来源见上方安装章节。

### 最终验证与限制

- 四个维护中的应用（Gupi、Feiwen、HTTP Client、Novel Download）`cargo build --locked --offline` 通过；xtask 编译通过。维护范围的 workspace Clippy（`--all-targets --all-features -D warnings`）通过；最后的 Gupi 调整再次通过对应 Clippy。排除 Jaco、四个 jaco-* crate 及仅为 Jaco 留存的 app-assets / app-assets-macros。
- 按上述排除范围完成 unit / integration / trybuild / doc tests。Gupi **194** 项、HTTP Client **163** 项、Feiwen **93** 项通过；Form 类型契约与适配、Store、Operation、Tokio、JSONL、附件编码、解压与传输等现有覆盖通过。保留既有的 2 项平台集成 ignored 测试，未声称执行。
- 新增 Composer 集成回归：readonly 阻止编辑但模型动作仍可用；disabled 阻止 addon 动作；handled 文件粘贴不再向正文插入路径，普通文本回退只插入一次。现有快捷键原位编辑、只读粘贴拒绝、配置路径同行布局和 Markdown 快照回归通过。
- 新 GPUI 测试调度器发现一条既有启动测试在意外渲染 Home 时启动了真实 smol 文件读取。测试现使用空的窗口 host，保留对 StartupView 页面选择、临时状态及 Pi probe 的原断言，不读取用户历史；生产启动逻辑未修改。
- HTTP 回环服务测试在沙箱内无法 bind，转为宿主权限后全部通过。格式与 diff 检查通过；仍有既有 `block v0.1.6` future-incompatibility 提示。
- 原生验证使用本轮 debug 二进制组成的临时测试 `.app`，独立 config/data/agent/session 目录及 Runtime Gallery faux provider，无真实模型调用。检查了：设置跨页字段搜索、无结果及清空恢复；配置路径与两个 icon 同行；快捷键清除/取消；主输入框和模型入口；普通文本粘贴一次；两轮思考/工具过程与 Markdown 输出；停止后内容保留和输入恢复。运行中的图标可见，无丢失资源。测试窗口已退出。
- 设置原索引错位不再复现；无结果时仍为空白、无说明，记录到[统一待处理文档](../issue-217/follow-ups.md)。图片剪贴板的原生手动回归、动画逐帧时序、完整主题/窗口矩阵、其他三个应用的原生交互及 Windows/Linux 未覆盖。本轮不把截图当成这些验证的替代。
- 临时 debug `.app` 仅供本次验证，不是 xtask release 发行包；未安装到 Applications。官方 skills 的自动发现需在重新加载的会话核实。
