# GPUI Kit 0.7.0、依赖更新与 Gupi 正文查找

状态：**本轮实现完成；长文定位的无障碍场景与中文混排换行缺陷由后续依赖升级承接**。2026-09-28 完成正式依赖迁移、代码精简与正文查找，本批次基线为 `e41b6397`。本页统筹跨 workspace 的升级，搜索产品契约与状态归属仍以 [#242 开发文档](../../../app/gupi/docs/dev/issue-242/README.md)为准。

## 本轮交付范围

1. GPUI Kit / Component / Assets 从 0.6.4 升到 **0.7.0**，GPUI 快照配套升到 **0.3.7**；完成受影响的 API 迁移。
2. 实现 #242 剩余的当前会话正文查找：原位高亮、匹配计数、上下处导航及滚动定位，主窗口与临时窗口共用。
3. 同步升级下列五个普通依赖，接入能替代现有实现的 Root、Theme、Toolbar 能力；用既有成熟库/标准库收敛任务取消、临时目录和 UTF-8 边界助手，检查主题、图标和已安装官方 skill 的版本配套。

**明确排除：**Token、Skill/模板输入行为、Markdown 资源标签及 Questionnaire 的应用接入归 [#243](../../../app/gupi/docs/dev/issue-243/README.md)，不随本轮控件升级实现。Jaco 专用依赖和功能继续不维护；只做 workspace 共用 GPUI 升级所必需的机械兼容。Jaco/OCR/Quick Look 清理与共享库大重构也不并入。

不改变 Pi 协议、附件发送策略、临时会话生命周期或通知规则；不为了使用新组件重做页面。已有 InputGroup、AttachmentGroup、附件临时文件预览、`on_paste`、Markdown 增量追加与渐显不重复列为新接入。

## 正式版本与盘点依据

- [官方 v0.7.0 release](https://github.com/longbridge/gpui-kit/releases/tag/v0.7.0) 发布于 2026-09-28，tag 为 `0c830f4d257e69fdd17200650533ab4ca9a40cc0`。已核对 crates.io 的 kit/base/component/assets 0.7.0；base/component 发布包的来源提交与 tag 一致。
- 本地参考库 `/Users/sushao/Documents/code/gpui-component` 已更新至 `71bf31ae`；比 tag 多的 Command 行圆角修复不属于本次正式依赖，不能把 main 的效果算作 0.7.0。
- 扫描 25 个 workspace 成员及独立 MCP 测试工具的 normal/dev/build/target 依赖，按实际包名去重，共 **102 个外部直接依赖**；**82 个有仍维护的消费者，20 个仅供 Jaco 及其工具使用**。以 [crates.io 官方稀疏索引](https://index.crates.io/config.json)的非撤回、非预发布版本对照 manifest 和 Cargo.lock；`+spec-1.1.0` 属于版本元数据，不当作预发布。
- 82 个中，9 个已采用本轮新版，3 个 Windows 配套包保留当前版本，70 个在本轮盘点时已锁到最新正式版。根 manifest 的 `gpui_platform` 别名当前未被成员直接继承，仍随 GPUI 家族声明对齐；不计入上述消费者去重数。
- Rust 1.98.1 已满足本次候选的 MSRV。不为本批次另升 Rust、重锁 Nix 输入、更新 Jaco 的 npm/Lucide 子模块或添加新的构建平台；实际构建若发现原生依赖变化，再在现有 flake 中作必要调整。

## 版本目标与 changelog 判断

| 依赖 | 当前 → 目标 | 与现有代码的关系、迁移与采用判断 |
| --- | --- | --- |
| gpui-kit / gpui-component / gpui-kit-assets | 0.6.4 → **0.7.0** | 同批精确锁定，移除已删除的手动浮层 API；使用正式 TextView 高亮/定位接口。其余接入见下节 |
| gpui-pre / gpui-pre-platform | 0.3.5 → **0.3.7** | 与 Kit 的精确 GPUI 快照依赖对齐；由 Cargo 统一配套包，不混用旧 GPUI 类型。底层没有足以替代编译验证的完整独立 changelog |
| encoding_rs | 0.8.41 → **0.8.42** | 新版只在 `std` 且 x86/x86_64 时引入 multiversion，减少其他架构不需要的依赖；HTTP Client 已使用该库，保留严格解码、流式边界及错误行为，不新增解码器 |
| hyper-util | 0.1.20 → **0.1.21** | Rust 2024 / MSRV 1.85；连接池空闲任务、CONNECT 验证及代理修复。直接消费者是 HTTP 测试服务的 TokioIo / GracefulShutdown，不为新增客户端配置改产品。新版 TokioExecutor 不再隐式继承 span；当前测试服务没有依赖这项行为，不开启临时兼容 feature |
| thiserror | 2.0.20 → **2.0.21** | 修复 display 表达式中泛型 unit variant 的解析；仍维护消费者统一声明，错误内容与恢复入口保持原语义。无需新增应用功能 |
| tauri-bundler | 2.9.4 → **2.10.0** | 与 utils 配套升级。MSRV 1.90；WiX 资源目标文件名、资源排序及打包修复直接相关。上游明确提示公共 API 可能随依赖/结构变化破坏兼容，须检查 xtask 的 SettingsBuilder、PlistKind、WixSettings 和打包调用，不能只改锁文件 |
| tauri-utils | 2.9.3 → **2.10.0** | bundler 要求 `~2.10.0`；xtask 直接使用 DeepLinkProtocol。新增 WebView、ACL、便携目录设置不适用于原生 GPUI 应用，不引入 Tauri runtime |

普通依赖依据：[encoding_rs 0.8.42 README](https://docs.rs/crate/encoding_rs/0.8.42/source/README.md)、[hyper-util 0.1.21 changelog](https://docs.rs/crate/hyper-util/0.1.21/source/CHANGELOG.md)、[thiserror 2.0.21](https://github.com/dtolnay/thiserror/releases/tag/2.0.21)、[tauri-bundler 2.10.0](https://github.com/tauri-apps/tauri/releases/tag/tauri-bundler-v2.10.0)、[tauri-utils 2.10.0](https://github.com/tauri-apps/tauri/releases/tag/tauri-utils-v2.10.0)。已对照发布包源码，不以搜索摘要代替 API 核对。

Tauri 的新增 `bundle_vc_runtime`、`binary_patching(false)` 不默认启用：当前没有需要随这次升级改变的运行库/更新器产品契约。其 `.icon` 支持在旧版已经存在，当前实现只选一个 `.icon`；Gupi 打包需要保留多个主题图标及现有资源 staging，不能据此删除多图标编译与最后重签名。Linux AppImage 不再内置 xdg-utils 的变化记为对应平台打包验证点，macOS 成功不代表 Linux/Windows 已验收。

### 保留的 Windows 配套版本

`windows 0.62.2` 仍是该包的最新正式版，依赖 `windows-core ^0.62.2`、`windows-future ^0.3.2`；这两包独立发布的 0.100.0 与它不属同一个兼容家族。`platform-ext` 保留 core 0.62.2 供系统外观使用；直接 future 依赖、windows-bindgen 和 OCR 绑定生成链已随 Jaco 退役删除。依据：[windows 0.62.2 manifest](https://docs.rs/crate/windows/0.62.2/source/Cargo.toml)、[退役范围](../jaco-retirement/README.md)。

## GPUI 迁移与直接接入的优化

| 项目 | 升级前实现 | 本轮处理 |
| --- | --- | --- |
| Root 自动浮层 | Gupi 启动页、临时窗口、设置编辑器和部分测试手调 `render_dialog_layer` / `render_notification_layer`；Jaco 还调 sheet layer | **必须迁移**：删除旧 layer 调用及仅供其使用的包装，由每窗口唯一 Root 托管；WindowExt 操作继续使用。验证浮层只出现一次、Esc/焦点/窗口归属正确 |
| 窗口入口 | `cx.open_window` 后手动 `Root::new` | 仍维护应用普通入口采用 `gpui_kit::open_window`，复用返回的内容 Entity，先调用 init；保留原 WindowOptions、关闭行为和实体所有者。`Root::new` 仍然公开，不声称全部测试宿主必须改写 |
| 主题同步 | app-theme 手动 apply_config + sync_base，调用方另 refresh_windows | 改用 `Theme::update(cx, …)` 统一颜色/token/Base 同步和刷新，收掉相同路径重复刷新；保留主题选择、系统强调色及 Material palette 生成职责 |
| Markdown 查找 | 稳定 TextViewState 增量渲染，尚无正文查找 | 接入 rendered_text / range highlights / reveal；离屏状态和通知区分必须与搜索一起完成，详见 #242 |
| Markdown 流式性能 | 已用 push_str / set_text、stream_fade | 自动继承上游批量解析、块复用、布局缓存和渐显调度优化；保留 RPC 到组件的增量适配，以及真实解析变化对外层消息行的重测。不能把所有订阅都删掉 |
| Message / Attachment 样式 | 已组合 MessageContent 与 AttachmentGroup | 0.7.0 不再由 Message 强制小字号/行高，Attachment 默认形状有变化；检查实际排版和点击预览，维持当前信息、密度和操作语义，不重做附件区 |
| Attachment 删除快捷 API | 当前移除按钮在不可编辑时可见但禁用，另有键盘预览入口 | 新 `on_remove` 没有独立 disabled 参数，不能原样替代。保留这两处必要适配；无上传任务，不接入 progress/retry。track_scroll/edge_fade 暂无需新增 |
| Command / Input / Menu | 已采用上游组件 | 继承未变列表复用、非聚焦光标停闪、输入/IME 修复及菜单订阅释放；只复测现有入口，不重写输入行为 |
| gpui-heatmap / Plot | 自有 ActivityHeatmapPlot 通过 IntoPlot 使用公共接口 | Plot 移到 Base，Component 仍 re-export；核对绘制、tooltip、主题测试，按编译结果做必要兼容，不新增图表能力或单独重构共享库 |

源码：[Root/入口](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/kit/src/lib.rs)、[Theme::update](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/theme/mod.rs)、[TextViewState](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/base/src/text/state.rs)、[Attachment](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/attachment.rs)。

### 资源和官方 skill

- Gupi 内置的 **21 份主题 JSON 与 v0.7.0 的 themes 逐份解析比较一致**，没有待同步的主题文件。主题运行时 token/字体语义仍需随升级验证，不能仅据 JSON 一致判断外观不变。
- gpui-lucide 从 gpui-kit-assets 的公共 `icons-dir` build metadata 生成 SVG 常量，0.7.0 仍保留该契约；升级后重新生成并编译现有图标引用即可。保留链接裁剪方案，不恢复 `icon_assets!` 清单或升级旧 Lucide 子模块。
- 项目安装的 gpui-kit 与设计指南官方 skills 已更新到 **v0.7.0 对应提交**，保留原 LICENSE 和项目自己的其他 skills；没有复制新的私有源码镜像。
- DatePicker、TimeField、新图表及 WebView 等无对应本轮需求的能力仅做兼容性判断，不转成新增页面。Toolbar 已找到现有历史画布操作栏的替代位置，见下节。设置搜索无结果仍渲染空白；0.7.0 未提供空态入口，保留在总待处理文档，本轮正文查找不顺带重做设置搜索。

## 自有组件、函数与共享库替代核对

2026-09-28 补查仍维护应用的实际 UI 组合及共享库公开边界，逐项对照 0.7.0、gpui-pre 0.3.7 和普通库源码。以下区分**本次新增的上游能力**与**项目已具备但尚未利用的能力**；后者可以顺手精简，但不能声称是本次版本升级才支持。这里记录的是源码结论，替换后的行为仍需按对应回归验证。

### 有明确删除目标，可纳入本轮

| 替代项 | 现有代码与可删除部分 | 采用方式与保留边界 | 验证 |
| --- | --- | --- | --- |
| **Root 自动托管与窗口入口（0.7.0）** | Gupi 启动、临时窗口、设置编辑器的手动 layer 渲染，以及普通窗口创建时重复的 Root 包装 | 由 Kit 的窗口 Root 统一处理；保留 WindowOptions、内容 Entity、应用恢复和关闭逻辑。只删除框架已接管的包装，不删除业务窗口管理 | Dialog/通知只显示一次，Esc、焦点、多窗口归属及原入口可用 |
| **Theme::update（0.7.0）** | [app-theme 的 apply_theme_config](../../../crates/app-theme/src/lib.rs) 中显式 sync_base，与 Gupi 等调用点相邻的重复刷新 | 一次上游 update 包含 apply_config、token/Base 同步和刷新；包装函数可保留为共享应用入口，删除其重复实现。系统强调色、预设 ID 和 Material palette 继续归 app-theme | 复用主题投影测试，检查亮暗模式与预设切换 |
| **Toolbar / ToolbarGroup（0.7.0）** | [历史画布操作栏](../../../app/gupi/src/features/home/history/canvas.rs) 的手工 h_flex、重复按钮密度配置及 div 分隔线 | 用 Toolbar + 标准 Separator 组合现有缩放/适配/定位按钮；百分比作为 content，外围圆角/边框仍由原画布容器负责。保留按钮名称、tooltip、disabled 与回调；不将所有 h_flex 或 Composer footer 批量改成 Toolbar，也不改图片预览现有圆形控件 | 缩放与查看全图已原生验证；方向键行为沿用组件，未单独确认原生键盘导航 |
| **tokio-util::task::AbortOnDropHandle（当前已锁 0.7.19）** | [gpui-tokio](../../../crates/gpui-tokio/src/lib.rs) 私有 AbortOnDrop、Option<AbortHandle>、disarm 和自写 Drop，以及等待结果后手动解除守卫的代码 | 用上游 Future 包住 JoinHandle，再交给 GPUI background_spawn；保留 Task<Result<T, JoinError>>、runtime 所有权和外部 handle 接入。向该 crate 增加 `tokio-util = { version = "0.7.19", features = ["rt"] }` 直接声明，未增加 workspace 的新包种类 | 复用现有 panic→JoinError、丢弃 GPUI Task 取消 Tokio future、外部 runtime 存活三项测试；不得改成 detach 后丢弃取消语义 |
| **tempfile::TempDir（当前已锁 3.27.0）** | xtask 的 [图标 staging](../../../crates/xtask/src/bundle/common.rs) 和 [actool 临时目录](../../../crates/xtask/src/bundle/macos.rs) 中 PID/时戳拼名、显式创建及多处 remove_dir_all；BundleIconAssets 的专用清理 Drop | xtask 直接声明 `tempfile = "3.27.0"`，由 TempDir 管理临时目录。BundleIconAssets 持有目录到 bundle 使用完，actool 目录持有到产物复制完；保留图标生成、多主题编译、资源复制和最后签名。受影响测试的同类 TestDir 可一并用 TempDir 替代 | 现有图标/打包配置回归及本机 .app 打包；确认目录不提前释放，错误返回自动清理 |
| **str::floor_char_boundary（Rust 1.91 已稳定）** | [HTTP 预览 viewer.rs](../../../app/http-client/src/features/request/response/viewer.rs) 的同名手写字节回退循环 | 直接调用 `source.floor_char_boundary(limit)` 并删除局部函数；项目 Rust 1.98.1 已满足。不改变 HTTP 预览的字节/行数限制、截断提示或字符集策略 | 复用含多字节字符的 bounded_source 回归；无需新建 Unicode 工具包 |

依据：[Toolbar 0.7.0](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/toolbar.rs)、[AbortOnDropHandle](https://docs.rs/tokio-util/0.7.19/tokio_util/task/struct.AbortOnDropHandle.html)、[TempDir](https://docs.rs/tempfile/3.27.0/tempfile/struct.TempDir.html)、[str::floor_char_boundary](https://doc.rust-lang.org/std/primitive.str.html#method.floor_char_boundary)。不为这些替换新增平行兼容实现；旧私有助手随调用迁移一起删除。

### 看似重叠，但当前不能等价替代

| 现有实现 | 对照结果与保留理由 |
| --- | --- |
| [消息/详情复制按钮](../../../app/gupi/src/features/home/messages/actions.rs)的 CopyAction / CopyState | 上游 Clipboard 已有复制后打勾和两秒恢复，本次对照的 0.6.4/0.7.0 源码一致，属于旧能力。其公开配置只有 value/value_fn、tooltip、size、on_copied，不能控制当前成功态的文案/禁用态、独立无障碍标签或失败时阻止成功反馈。我们还有剪贴板回读失败提示；直接替换会丢行为。暂保留，不仅为少一个 Task 换成另一套外置成功状态 |
| [主窗口 PaneLayout / ResizeEvents](../../../app/gupi/src/features/home/panes.rs) | 0.7.0 resize_handle 增加状态与外观接口，仍只提供拖动起点回调；没有独立尺寸控件的完整 update/end 回调可接管当前全窗鼠标释放处理。ResizablePanelGroup 管相邻面板，不能直接替代当前“两侧宽度互不跟随、历史窄窗覆盖、释放时保存”的策略。保留防抖动所需的布局/拖动归属；不要因为上游改了拖动指示样式就删除它 |
| [图片预览](../../../app/gupi/src/features/home/image_preview.rs)与历史树画布 | 0.7.0 无可直接承接的 ImageViewer 或会话图组件。Carousel/Plot/Tree 不覆盖原图缩放、鼠标锚点、平移及历史分支预览语义。底层继续用现有图像/滚动/绘制组件，局部操作栏可标准化，不能据此删除整个实现 |
| [Composer](../../../app/gupi/src/features/composer.rs)、附件区、模型/思考选择器 | 已复用 InputGroup、AttachmentGroup、Attachment、List、Popover、Slider。保留的封装负责槽位、Pi 状态、默认覆盖/清空及提交语义，不是自造输入组件。Attachment::on_remove 没有独立 disabled 配置，仍不能替代禁用但可见的移除按钮；on_retry/progress 也不对应本地附件业务 |
| [消息 MarkdownState](../../../app/gupi/src/features/home/messages/markdown.rs) | 上游优化内部解析和布局，但外层稳定身份、push_str/set_text 同步、MessageScroller 重测接线仍有实际职责。搜索接入要把纯高亮通知与解析通知分开；只删除重叠路径，不能删除整个状态包装 |
| GroupBox / SettingGroup 的 footer、variant | 0.7.0 可承接卡片外辅助说明和单组样式；当前 Gupi 已用 SettingGroup 的 title/description，整体选用 Normal，没有找到应删除的单组卡片补丁。不能为了采用 footer 改动说明的位置、搜索归属或新增包装 |
| Form::columns / footer、Empty、DescriptionList | 属于可复用的呈现能力，不代替 typed form、动态查询模型或恢复命令。当前错误入口/recovery 是少量标题、说明与业务按钮，换 Empty 不减少实际逻辑；详情列表使用已有控件组合也不意味着每个键值行必须另封装。没有本次升级专用补丁可删 |
| Command / SearchableVec / Tree 的缓存优化 | 升级后组件内部减少重测/复制；应用保留的目录 revision、会话身份、RPC 命令来源和模型能力分组不是组件缓存，不能顺带删。模型选择器的自定义 ListDelegate 还负责 provider 分组、能力标签、选择派发，不等于重复写了通用 List |

范围核对依据：[Clipboard](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/clipboard.rs)、[resize_handle](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/base/src/resizable/resize_handle.rs)、[SettingGroup](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/setting/group.rs)、[Form](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/component/src/form/form.rs)。这些保留项不自动转成新的上游请求或待办。

### 是否能直接删除某个自有 crate

本次核对未发现 0.7.0 能完整等价替代的现役共享 crate。可删的是上表的重复内部实现；原先已确定退役的内容继续按原范围处理。

| Crate | 与上游能力的实际差别 |
| --- | --- |
| gpui-form / gpui-form-macros / gpui-form-gpui-component | Kit Form 是表单布局和控件组合；本地库还有 typed draft/baseline、动态字段身份、校验与提交快照、deferred binding/source suppression。IntegerInput 的呈现已经使用上游 NumberInput，本地部分负责精确整数、未完成输入、溢出与范围错误；上游默认 f64 数值转换不能替代 u64 > 2^53 / i128 / u128 的契约 |
| gpui-store / gpui-operation | Kit Global/Entity、Loading/Notification 不提供本地 selector 发布规则、保留旧数据的刷新状态机或用户指定的修复状态机；不借 UI 升级重写它们。其已有独立重构方向不并入本轮 |
| gpui-tokio | 只替换取消守卫；GPUI Task 与 Tokio runtime 的所有权、JoinError 和外部 runtime 桥接仍需要。async-compat 的 fallback runtime 不是同一个生命周期契约 |
| gpui-lucide | Kit 提供完整 IconName，但路径模式需注册全量 Assets 或选择清单；本地用独立 SVG 字节常量实现无需清单的链接裁剪并实现 Into<Icon>。0.7.0 的 Icon::data 没有让这一生成层失去作用 |
| app-theme | Theme::update 可替换同步步骤，不能替换应用预设选择、系统强调色监听、Material 调色板生成及语法主题选择 |
| gpui-heatmap | 0.7.0 的 Plot 迁层和通用图表没有提供当前连续日历网格、准确 u64 值与本地化标签的 ActivityHeatmap；保持组件职责与现有通用绘制接口 |
| window-ext / platform-ext | gpui-pre 0.3.7 没有完整替代单窗隐藏/无激活显示、原生窗口等级、跨显示器定位及应用图标/角标适配。特别是 GPUI Window::is_visible 表示帧是否展示，遮挡/最小化也会为 false；本地 is_visible 检查原生 shown/hidden，不得仅因同名就替换。系统通知投递和回调已经走 GPUI，不再重复迁移 |
| pi-rpc / http-client-test-server / xtask | 分别承接 Pi 特有协议和子进程生命周期、HTTP 场景测试服务、应用专用资源与打包编排。新版通用传输库/打包器没有接管这些职责；只简化对应 helper 和迁移库 API |
| app-assets / app-assets-macros、Jaco 专用 crates、OCR / Quick Look | 已按 [Jaco 退役清理](../jaco-retirement/README.md)删除；本表保留升级批次的历史边界 |

原生接口依据：[gpui-pre 0.3.7 Window](https://docs.rs/crate/gpui-pre/0.3.7/source/src/window.rs)、[App](https://docs.rs/crate/gpui-pre/0.3.7/source/src/app.rs)；数值/图标依据：[NumberInput](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/base/src/number_input.rs)、[Assets](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/assets/src/native_assets.rs)。以上结论只判断这次依赖能否替代现有职责，不代表现有库无需独立重构。

## 搜索接入与性能约束

产品规则已经确认，完整说明由 [#242](../../../app/gupi/docs/dev/issue-242/README.md#3-查找范围匹配与定位)维护：搜索用户消息文字；助手消息每轮有最终输出只查最终输出，否则查 assistant 文字正文。当前查看分支内、大小写不敏感的字面查找，不包含工具、思考、摘要、插件消息及图片等附件内容。主窗口和临时窗口 Cmd/Ctrl+F 统一正文查找，临时会话过滤继续直接输入和 Tab 切换。

实现包含以下四个接缝：

1. **同一渲染文本。** 匹配基于已提交的 `RenderedText` UTF-8 范围，高亮和计数使用同一快照；不对 Markdown 源码匹配后直接使用其偏移，不通过注入 `<mark>` 改写内容。大小写匹配返回原文范围，具体实现可复用 workspace 已用的 regex 并转义字面量，不另造 Unicode 偏移映射。
2. **离屏可搜索。** 会话视图的弱引用 Registry 按稳定消息/内容单元身份共享 MarkdownState；挂载的正文和打开的查找栏持有 Entity。打开查找时补齐离屏/折叠正文的组件解析状态，关闭后释放查找持有的状态，不挂载整个消息列表、不建第二份持久正文数据。
3. **定位遵循布局。** 先展开必要 assistant 过程区并 scroll_to_item，再在该行参与布局后消费一次 reveal_range；保持当前执行/预览分支。组件只保证起点行可见，请求超时无完成回调，应用不做固定延时轮询或无限重试。
4. **更新局部化。** 比较解析快照后更新对应正文的命中与行测量；高亮通知不触发解析、重搜或重测循环。导航只改变旧/新命中，流式追加只更新受影响正文，不抢焦点或强制滚到底。关闭查找/切换会话取消搜索与待定位目标。

## 实施顺序与必要验证

1. **依赖与资源**：修改上述声明、更新相关 lock 条目及官方 skill；用依赖树确认统一的 GPUI 家族与 Tauri 配套，保留受约束的 Windows 包。不要用无差别全量 cargo update 扩大 Jaco 专用变化。
2. **兼容与简化**：迁移 Root 浮层、窗口入口、Theme 和历史画布 Toolbar；用上表的标准取消守卫、TempDir 和字符串边界 API 删除对应私有助手。按实际编译错误处理其他 API；保留上表不能等价替代的业务和状态边界，修订受影响说明。
3. **正文查找**：按 #242 先接可共享 Markdown 状态与纯匹配/范围选择，再接查找栏、高亮、滚动及快捷键，完成流式和分支切换行为。
4. **交付验证**：在现有 Nix 开发环境执行受影响构建、Gupi/共享组件关键测试和 Clippy；HTTP 解码与测试服务、xtask 配置/资源回归按各自变化运行。依赖集成覆盖 workspace，最终按现有 CI 要求完成 workspace build/test、格式和 Clippy，不因某个应用通过就宣称全部兼容。
5. **本机试用**：运行 macOS Gupi，检查主/临时窗口查找、离屏长段落/代码/表格、Unicode 与跨样式命中、上下处循环、流式最终回答收敛，以及 Dialog/焦点、主题和附件预览。构建一次 Gupi `.app`，确认多主题图标、本地化、启动/Pi 错误恢复入口及打包后签名。Linux/Windows 沿用 CI，未实际验证的平台如实记录，不先写为通过。

测试复用已有覆盖，新增测试集中于新查找的范围选择、原文范围、解析/高亮通知区分与离屏定位；不为纯版本声明或删除包装添加镜像实现的测试。已通过部分不因无关修订反复重跑。

## 待确定项与完成标准

**没有待用户决定的问题。** 用户已决定保留正式依赖，等待下节记录的上游修复。Token / Questionnaire 的交互细节留在 #243，不转成本批次阻塞。

正式依赖迁移、约定的代码精简和正文查找均已接入，workspace 构建、测试、Clippy 和 macOS 打包已通过；原生验收发现的长文定位问题见下节。该边界未解决前，不把整个正文查找标记为完整验收通过。

用户确认两项上游缺陷由后续依赖升级承接，当前应用侧实现可以交付。后续升级到包含修复的正式版后复测，不将已知失败场景改记为通过。

本轮完成条件是：正式依赖迁移可构建、选定简化实际接入、正文查找按 #242 可用、受影响回归与必要 macOS 启动/打包通过，并同步当前文档。已核对版本的完整清单如下。

## 验证结果

2026-09-28，在项目 Nix 环境与 Rust 1.98.1 下执行：

- `cargo build --workspace --locked`、`cargo test --workspace --locked`、`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` 通过。Gupi **273 项**通过，xtask **17 项**通过；既有 Jaco 两项与真实 Pi 集成两项继续忽略，没有计为通过。依赖自身 `block 0.1.6` 仍有 future-incompatibility 提示。
- 新查找回归覆盖最终输出/助手过程的范围投影、Unicode 原文偏移、跨 Markdown 样式/代码/表格匹配、循环导航、流式更新不自动滚动、清理和取消待定位、纯高亮不触发行重测；临时窗口 Cmd/Ctrl+F 不改变会话过滤或草稿，Esc 恢复焦点，原有 Tab 切换有效。
- 调试包原生检查确认计数、前三处导航、跨加粗/代码/表格原位高亮；无最终输出时的两处助手正文命中与思考内容零结果。关闭查找清除高亮；会话信息弹窗只显示一层并可用 Esc 关闭。长回答深处定位的无障碍失败单独记录在下节。
- `cargo run -p xtask --locked -- bundle gupi` 通过，产物为 `target/release/bundle/macos/Gupi.app`，未安装。`codesign --verify --deep --strict` 通过；Assets.car 包含七套图标，Info.plist 和资源目录包含九种本地化。
- 发行包副本在隔离配置/数据/会话目录下启动成功；原生检查了设置进入/返回、浅色到深色的即时切换、深色正文高亮、历史树 Toolbar 的缩放与查看全图。没有调用模型或修改用户配置。临时窗口查找与流式行为由上述自动化回归覆盖，未宣称均完成原生验收。
- Linux/Windows 原生界面及各自安装包未在本机验收，仍需对应平台验证。

用户消息搜索接入后，新增图文混合消息的文字范围、纯图片排除及用户/助手顺序回归，九种语言的查找占位文案同步为会话正文。`cargo test -p gupi --locked -- --test-threads=1` **274 项通过**，`cargo clippy -p gupi --all-targets --locked -- -D warnings` 通过。本次并行测试曾出现既有 `session_info_dialog_from_temporary_actions_copies_updates_and_closes` 的第二次复制断言失败，以及 `unreachable_owner_does_not_allow_another_instance` 的临时文件不存在；前者单独复测通过，两者串行通过。没有修改这两条功能路径，也没有据此认定默认并行测试稳定通过。

加入 Markdown 宽度回归后的最终检查：`cargo test -p gupi --locked` 默认并行执行 **275 项全部通过**；此前两项偶发失败本次未重现。格式、受影响代码 Clippy、调试构建和差异检查通过。workspace 级验证沿用上述升级集成结果，后续改动限定在 Gupi 搜索、宽度约束及说明文档。

`cargo build -p gupi --locked` 通过。最新调试程序在隔离的 24 轮会话中原生验证：初始视口位于末尾，查找跨加粗的 `User**Needle**` 得到全部 **24** 处用户消息命中并跳到第一条；Shift+Enter 循环到第 24 条并高亮，Esc 清除查找与高亮。未调用模型，测试实例已退出。此结果不代表下节的长回答深处无障碍滚动问题已解决。

## 集成中确认的适配

- Material 主题颜色基线的首次 workspace 测试失败来自 0.7.0 新增可选 `chart.grid` 字段（序列化为 null），其他颜色值未改。保留旧颜色基线哈希，单独断言新字段未设置，并从旧字段快照中排除它；网格继续采用组件的边框色回退。

### 消息正文宽度

Gupi 正文与输入框的最大宽度均为 **820 逻辑像素**，随可用空间缩小；此前复现用的 420px 只是测试容器宽度，不是产品设置。保留当前最大宽度。

2026-09-28 对照了以下实际实现：

- 本机 Codex Electron **26.924.22138**，从 `/Applications/ChatGPT.app/Contents/Resources/app.asar` 只读提取前端资源：`app-shared-fa3f1d5d5942.css` 对 Electron body 设置 `--thread-content-max-width:48rem`，默认根字号下约 **768 CSS 像素**；Markdown 宽内容另有 `--markdown-wide-block-max-width:56rem`。具体可见宽度还受可用空间和缩放影响。
- Zed 本地 `main` 已快进到官方 `4f70d91bda7ec5f5600a000a0dc34391d9f1e96f`。`assets/settings/default.json` 的 Agent 默认开启 `limit_content_width`，`max_content_width` 为 **850 逻辑像素**；`thread_view.rs` 将该约束应用到正文和输入区。旁边的 `default_width:640` 是固定停靠面板宽度，不是正文上限。

越界原因在 Gupi 的 Markdown 布局接入：`MessageContent` 将子元素左/右对齐，而普通 `TextView` 根节点默认宽度为 auto，列表内部的百分比宽度缺少明确约束。最小复现中，420px 容器内带行内代码的长列表被测成 3557px；普通列表也可能缩到标记宽度，变成异常高的单字列。

共用 Markdown 包装器给 `TextView` 设置 `w_full().min_w_0()`，让助手正文按所在内容栏测量和换行；不裁切正文、不修改文本，也不改为横向滚动。用户气泡继续由 Bubble 决定内容宽度，短文字仍按内容收缩。该修复不改变下节记录的上游无障碍滚动边界，也不能仅凭这次复现断言问题是 0.7.0 新引入的。

420px / 820px 的组件布局回归已通过，覆盖助手普通段落、普通列表、带行内代码的列表，以及用户短气泡、长段落、代码列表；Clippy 与 Gupi 调试构建通过。该回归使用测试文字系统，只验证容器几何，不能代替 macOS 原生字体检查。

随后使用用户报告问题的实际会话复测：将原记录复制到隔离环境，直接加载截图对应的一轮原消息和工具记录，未改写消息正文、未发送模型请求。整段越出窗口已消除，但 **带行内代码的中文列表仍有行末裁切**，因此不能宣称该场景完整通过。全会话搜索定位还触发了既有的无障碍节点重复断言，后续排版核对直接载入目标轮次。

原生测量将残留问题缩小到正式依赖的文字换行：`gpui-base 0.7.0/src/text/inline_flow.rs::line_ranges` 使用 GPUI `LineWrapper` 的逐字符宽度决定换行，之后 `layout_measured_flow` 按整行字形布局绘制。macOS `.SystemUIFont`、16px 下，以不含用户资料的中文文本复现，给定 **808px** 换行宽度，整行最终宽度分别达到 **825.59px / 841.67px / 841.67px**。普通段落与带行内代码的段落走的排版路径不同，列表内部的裁切又使越界字形不可见。需要上游让换行边界与最终字形宽度一致；Gupi 不以裁切、改写内容或估算预留边距代替修复，继续使用正式依赖。临时诊断代码已移除。

上游对应修复为 [#3293](https://github.com/longbridge/gpui-kit/pull/3293)：根据实际字形宽度校验每行，溢出时收紧换行预算；其中原生测量也确认了全角中文标点在独立字符和行内排版时的宽度差异。已核对代码 diff，当前正式版 v0.7.0 不包含该实现，待包含修复的正式版发布后接入并复测；不重复提交同一缺陷。

最终代码通过格式检查、两项 Markdown 回归、既有图文混合消息/预览回归与调试构建；测试实例退出，本次会话副本已清理。上述通过项不覆盖残留的原生字体缺陷。

## 原生验收发现：长消息定位与无障碍树

2026-09-28 在隔离 Gupi 调试包中稳定复现：36 轮历史会话包含普通段落、代码块、表格和一条 90 段长回答。开启系统无障碍树后查找 `hello`，1–3 处原位高亮、计数和导航正常；第 4 处位于长回答末尾，定位时触发 `Duplicate a11y node id`，进程退出。发行构建跳过断言但会丢弃重复节点，不能作为修复。

回溯指向消息复制按钮的第二次 prepaint。正式 `gpui-pre 0.3.7`（Zed `1a28cff4`）的 `List::prepaint_items` 通过 `Window::transact` 重试自动滚动；事务回退了 hitboxes、tooltip、dispatch tree、元素状态和文字布局，**没有回退这一遍产生的 A11y 节点及父子关系**。TextView 的 `reveal_range` 正式实现使用 `request_autoscroll`，长行定位因此会让同一复制按钮在同帧登记两次；换应用侧 ID 或关闭无障碍会掩盖问题，不采用。

- Gupi 搜索没有注入/改写 Markdown；普通段落、跨加粗、代码和表格的 5 个匹配已经在原生窗口确认。问题出在需二次 prepaint 的深层滚动事务。
- 已查阅相关 [Zed #64218](https://github.com/zed-industries/zed/pull/64218) 与 [#64236](https://github.com/zed-industries/zed/pull/64236)：它们处理缓存视图在滚动重试时的帧记录复用，明确保持 `Window::transact` 不变，不能据此认定无障碍回滚已修复。
- 正确修复位置：GPUI `Window::transact` 的无障碍状态 checkpoint/rollback，涵盖节点集合、父节点 children、焦点及对应映射；不在 Gupi 重写列表和文字测量。
- 上游通用复现与固定版本源码依据见 [gpui-kit #3295](https://github.com/longbridge/gpui-kit/issues/3295)。
- **用户已确认**：保持正式发布依赖，记录并等待上游修复。本轮不引入 Git/path 补丁，不关闭断言或无障碍，不在 Gupi 重写滚动。此项解决前不将长文定位的无障碍场景标记为验收通过。

## 直接依赖盘点

下表保留本轮盘点基线，只列有仍维护消费者的包；“升级前”是对应直接依赖解析到的版本，不把 lock 中其他传递版本当作应用的依赖。未变化包无需虚构新的 changelog 接入工作。

| 包 | 升级前 | 盘点时最新正式版 | 本轮 |
| --- | --- | --- | --- |
| `async-channel` | 2.5.0 | 2.5.0 | 保持 |
| `async-compat` | 0.2.6 | 0.2.6 | 保持 |
| `async-compression` | 0.4.48 | 0.4.48 | 保持 |
| `async-stream` | 0.3.6 | 0.3.6 | 保持 |
| `base64` | 0.23.1 | 0.23.1 | 保持 |
| `block2` | 0.6.2 | 0.6.2 | 保持 |
| `bytemuck` | 1.25.2 | 1.25.2 | 保持 |
| `bytes` | 1.12.1 | 1.12.1 | 保持 |
| `clap` | 4.6.7 | 4.6.7 | 保持 |
| `dirs-next` | 2.0.0 | 2.0.0 | 保持 |
| `duckdb` | 1.10505.0 | 1.10505.0 | 保持 |
| `encoding_rs` | 0.8.41 | 0.8.42 | 升级 |
| `fluent-bundle` | 0.16.0 | 0.16.0 | 保持 |
| `futures` | 0.3.34 | 0.3.34 | 保持 |
| `futures-util` | 0.3.34 | 0.3.34 | 保持 |
| `garde` | 0.23.0 | 0.23.0 | 保持 |
| `get-selected-text` | 0.1.6 | 0.1.6 | 保持 |
| `global-hotkey` | 0.8.0 | 0.8.0 | 保持 |
| `globset` | 0.4.20 | 0.4.20 | 保持 |
| `gpui-component` | 0.6.4 | 0.7.0 | 升级 |
| `gpui-kit` | 0.6.4 | 0.7.0 | 升级 |
| `gpui-kit-assets` | 0.6.4 | 0.7.0 | 升级 |
| `gpui-pre` | 0.3.5 | 0.3.7 | 升级 |
| `hayro` | 0.7.1 | 0.7.1 | 保持 |
| `http` | 1.5.0 | 1.5.0 | 保持 |
| `http-body-util` | 0.1.5 | 0.1.5 | 保持 |
| `hyper` | 1.11.1 | 1.11.1 | 保持 |
| `hyper-util` | 0.1.20 | 0.1.21 | 升级 |
| `ignore` | 0.4.33 | 0.4.33 | 保持 |
| `image` | 0.25.10 | 0.25.10 | 保持 |
| `libc` | 0.2.189 | 0.2.189 | 保持 |
| `markdown` | 1.0.0 | 1.0.0 | 保持 |
| `material-color-utils` | 0.1.3 | 0.1.3 | 保持 |
| `mime` | 0.3.17 | 0.3.17 | 保持 |
| `mime_guess` | 2.0.5 | 2.0.5 | 保持 |
| `nom` | 8.0.0 | 8.0.0 | 保持 |
| `objc2` | 0.6.4 | 0.6.4 | 保持 |
| `objc2-app-kit` | 0.3.2 | 0.3.2 | 保持 |
| `objc2-core-foundation` | 0.3.2 | 0.3.2 | 保持 |
| `objc2-core-graphics` | 0.3.2 | 0.3.2 | 保持 |
| `objc2-foundation` | 0.3.2 | 0.3.2 | 保持 |
| `objc2-user-notifications` | 0.3.2 | 0.3.2 | 保持 |
| `pinyin` | 0.11.0 | 0.11.0 | 保持 |
| `plist` | 1.10.1 | 1.10.1 | 保持 |
| `proc-macro2` | 1.0.107 | 1.0.107 | 保持 |
| `quote` | 1.0.47 | 1.0.47 | 保持 |
| `r2d2` | 0.8.10 | 0.8.10 | 保持 |
| `raw-window-handle` | 0.6.2 | 0.6.2 | 保持 |
| `regex` | 1.13.1 | 1.13.1 | 保持 |
| `reqwest` | 0.13.5 | 0.13.5 | 保持 |
| `rodio` | 0.22.2 | 0.22.2 | 保持 |
| `scraper` | 0.27.0 | 0.27.0 | 保持 |
| `semver` | 1.0.28 | 1.0.28 | 保持 |
| `serde` | 1.0.229 | 1.0.229 | 保持 |
| `serde_json` | 1.0.151 | 1.0.151 | 保持 |
| `serde_yaml_ng` | 0.10.0 | 0.10.0 | 保持 |
| `smol` | 2.0.2 | 2.0.2 | 保持 |
| `sonic-rs` | 0.5.10 | 0.5.10 | 保持 |
| `syn` | 3.0.6 | 3.0.6 | 保持 |
| `sys-locale` | 0.3.2 | 0.3.2 | 保持 |
| `tauri-bundler` | 2.9.4 | 2.10.0 | 升级 |
| `tauri-utils` | 2.9.3 | 2.10.0 | 升级 |
| `tempfile` | 3.27.0 | 3.27.0 | 保持 |
| `thiserror` | 2.0.20 | 2.0.21 | 升级 |
| `time` | 0.3.55 | 0.3.55 | 保持 |
| `tokio` | 1.53.1 | 1.53.1 | 保持 |
| `tokio-util` | 0.7.19 | 0.7.19 | 保持 |
| `toml` | 1.1.6+spec-1.1.0 | 1.1.6+spec-1.1.0 | 保持 |
| `tracing` | 0.1.44 | 0.1.44 | 保持 |
| `tracing-subscriber` | 0.3.23 | 0.3.23 | 保持 |
| `trash` | 5.2.9 | 5.2.9 | 保持 |
| `tray-icon` | 0.25.1 | 0.25.1 | 保持 |
| `trybuild` | 1.0.121 | 1.0.121 | 保持 |
| `unic-langid` | 0.9.6 | 0.9.6 | 保持 |
| `url` | 2.5.8 | 2.5.8 | 保持 |
| `uuid` | 1.26.1 | 1.26.1 | 保持 |
| `walkdir` | 2.5.0 | 2.5.0 | 保持 |
| `which` | 8.0.6 | 8.0.6 | 保持 |
| `windows` | 0.62.2 | 0.62.2 | 保持 |
| `windows-bindgen` | 0.66.0 | 0.100.0 | 保留配套版本 |
| `windows-core` | 0.62.2 | 0.100.0 | 保留配套版本 |
| `windows-future` | 0.3.2 | 0.100.0 | 保留配套版本 |

仅供 Jaco 及其工具使用、排除独立升级的 20 包：`anyhow`, `async-trait`, `axum`, `diesel`, `dirs`, `grep-matcher`, `grep-regex`, `grep-searcher`, `hex`, `libsqlite3-sys`, `notify-debouncer-full`, `rig`, `rmcp`, `rust-embed`, `schemars`, `sha2`, `similar`, `unicode-segmentation`, `winresource`, `xcap`。共享库升级带来的必要传递解析变化不等于继续维护这些功能。
