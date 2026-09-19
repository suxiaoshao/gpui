# 临时窗口：Jaco 对照与对齐方案

2026-09-17，基于当前工作区源码及锁定的 GPUI Kit 0.6.0。已按 Jaco 的临时窗口方案实现。第 1–3 节保留调整前差异用于说明改动，第 4 节是现行行为；验证结果单独记录，不以源码对齐代替平台实机验收。

## 结论

差别较大，主要来自窗口类型和页面结构，而不是配色或几处 padding：

- **Jaco 是召唤式面板**：PopUp、macOS ModalPanel 层级、固定外窗尺寸、每次显示跟随鼠标所在屏幕居中、隐藏系统交通灯，顶部搜索统领左侧列表和右侧内容。
- **Gupi 调整前是普通对话窗口**：Normal、可调整外窗尺寸、普通标题栏，复用 HomeView 的左侧栏、主消息区和可打开的右侧历史面板。临时模式主要改变数据来源和可用操作，没有独立的临时窗口布局。
- 已确认的生命周期边界是：会话数据和运行状态由 Global 持有，不依赖窗口存活；窗口可以销毁、重建而不终止 Pi。目标按 Jaco 采用失焦隐藏、隐藏后 600 秒销毁窗口，重新显示时取消回收。会话不落盘与工作目录手动清理是另一层规则；不能用它们限制窗口回收。

## 1. 原生窗口配置

| 项目 | Jaco | Gupi 调整前 | 影响 |
| --- | --- | --- | --- |
| 实际内容视图 | 专用 `TemporaryWindow` | `HomeView::with_state` | 一个为临时入口设计，一个继承完整会话工作区 |
| WindowKind | 显式 `PopUp` | 沿用默认 `Normal` | macOS 分别创建带 NonactivatingPanel 样式的 Panel 与普通 Window；不是单纯外观名称 |
| 窗口层级 | 创建和显示时设 `ModalPanel` | 未覆盖原生层级 | Jaco 在 macOS 有额外浮层层级；Gupi 没有同类设置 |
| 初始大小 | 960 × 620 | 960 × 640 | 高度只差 20，尺寸本身不是观感差异主因 |
| 外窗缩放 | `is_resizable: false` | 默认 true；最小 640 × 420 | Jaco 面板固定；Gupi 会经历较窄宽度的自适应布局 |
| 首次可见与聚焦 | `show: false`、`focus: false`，准备后统一定位和显示 | 默认 `show: true`、`focus: true` | Jaco 明确拆开创建、定位、显现；Gupi 创建时即展示 |
| 屏幕选择 | 优先鼠标所在显示器，无法识别时退回主屏 | 不设置 display_id，初次 `Bounds::centered(None, …)` | Gupi 没有按鼠标屏幕召唤的逻辑；macOS 底层无 display_id 时使用主屏 |
| 再次显示的位置 | 按当前目标屏幕重新居中；保留能放入屏幕的现有大小，否则回退默认大小 | 仅 native.show，未重新定位 | Gupi 更偏向恢复原窗口位置；Jaco 更偏向当前工作屏幕的召唤器 |
| 标题栏 | title None、透明标题栏、交通灯移到 (-100, -100) | 透明标题栏，保留正常交通灯和应用自绘标题栏 | Jaco 去除可见系统窗口控件；Gupi 保留桌面应用窗口形态 |
| 拖动 | 临时页没有 Gupi 这类显式自绘 TitleBar 拖动区域 | `TitleBar::window_options()` 设置 app_owns_titlebar_drag，TitleBar 处理拖动 | 按 Jaco 使用面板结构，不额外保留主窗口自绘标题栏 |
| 背景 | 显式 Opaque，内容使用 theme background/sidebar | 默认 Opaque，内容使用同类 theme background/sidebar | 没有证据表明差别来自毛玻璃或透明背景 |
| app_id | 显式 APP_NAME | 此窗口未显式设置 | 配置有差异，不能据此断言 Dock/任务栏行为有缺陷 |

**平台限制：**当前仓库 `window-ext::NativeWindowHandle::set_window_level` 只在 macOS 实际设置层级，其他平台是 no-op。不能把 Jaco 调用了 ModalPanel 等同于 Windows 已具备相同置顶行为。PopUp 的实际激活、全屏空间切换等还需平台验证。

## 2. 显隐、焦点与生命周期

| 场景 | Jaco | Gupi 调整前 | 对齐目标或保留边界 |
| --- | --- | --- | --- |
| 再次按召唤快捷键 | 可见则隐藏，不可见则显示 | 同样切换显隐 | 已一致 |
| 显示后的焦点 | 搜索框优先；窗口激活也聚焦搜索 | 再次 show 时聚焦主输入框 | 按 Jaco 默认聚焦搜索，Tab 切换搜索与正文 |
| 失焦 | 可见且失活时请求隐藏 | 当前不因失焦隐藏 | 改为可见窗口失焦时隐藏 |
| 隐藏后的回收 | 启动 600 秒延迟销毁窗口；重新显示取消任务 | 当前无自动销毁计时器；Global 持有会话状态与窗口句柄 | 按 Jaco 隐藏后 600 秒销毁，重显取消回收；只释放窗口视图 |
| 恢复前台应用 | macOS 记录一次，隐藏后恢复并清空记录 | macOS/Windows 有前台目标捕获接口；隐藏时尝试恢复 | 实现不同，不能仅凭接口推断所有系统都恢复成功 |
| 窗口显现调用时机 | 准备后通过 defer 设置层级、位置并 show | native show/hide 放到 foreground executor，避免持有 App 借用时原生回调重入 | 当前 Gupi 已修过借用冲突，不应机械复制 Jaco 的调用时机 |
| Esc | 本临时视图未见对应 Gupi 的统一“忙时停止、闲时隐藏”处理；子控件有各自取消行为 | 无活动对话框时忙碌走 Stop，空闲隐藏；子层先处理取消 | 保留 Gupi 已确认语义；不推断 Jaco 全部 Esc 行为 |
| 多会话 | 独立页面实体缓存，共享应用 runtime；新建与会话详情有独立 route | Global ConversationState 持有多个 session/Pi，HomeView 按选中会话显示 | 两者都不是单任务窗口，可继续复用业务层 |
| 数据来源 | 数据库中 Scratch 项目的会话，可搜索已有会话 | 本次应用持有的临时会话，不进入普通目录、不写 JSONL/草稿 | 数据范围已确认不同，不应为复刻外观引入数据库或恢复旧会话 |
| 清理 | 此窗口生命周期有 UI 销毁；不等同于删除数据库会话 | 当前会话可在空闲时删除并清理目录；设置只清理已释放目录 | Gupi 已有独立产品规则 |

### 窗口与会话所有权核对

`app/temporary.rs::Temporary` 在 Global 中持有 `Entity<ConversationState>`，`ConversationState` 自己订阅 Pi 事件；消费事件和运行状态不由 HomeView 持有。`show` 在旧窗口句柄不可用时会用同一 state 新建 TemporaryView。现实现由专用 TemporaryView 复用同一 state 创建 HomeView，窗口回收只移除 Root。

滚动位置、过程折叠等 SessionView 状态仍属于视图，销毁后不保证原样恢复。不能把“会话数据不依赖窗口”扩大为“所有临时 UI 状态都已经全局保存”。窗口回收不调用会话删除、工作目录清理或应用退出 drain。

## 3. 页面布局与样式

```text
Jaco
┌────────────────── 搜索框 ──────────────────┐
│ 会话列表           │ 新建输入 / 会话详情     │
│                    │                       │
└────────────────────┴───────────────────────┘

Gupi 调整前
┌ 交通灯 / 侧栏开关 ─── 会话标题 / 操作 / 历史 ┐
│ 新建会话           │ 消息区          │历史*│
│ 临时会话列表       │                 │     │
│                    │ 底部输入框       │     │
└────────────────────┴─────────────────┴─────┘
* 历史栏可关闭，窄窗口时以覆盖层展示。
```

| 项目 | Jaco | Gupi 调整前 |
| --- | --- | --- |
| 顶部内容 | 全窗搜索框，`px_3/py_2`，底部分隔线；Input 无独立边框，带搜索图标和清空 | 46px 自绘 TitleBar；macOS 非全屏为交通灯预留 88px，侧栏开关、文件夹图标、会话标题、更多、历史入口 |
| 临时模式减项 | 专用页面天然不包含主窗口整套标题操作 | 只在 HomeView 中隐藏导出和刷新，其他主窗口结构仍保留 |
| 搜索能力 | 顶部搜索；空查询列出 Scratch 会话，非空查询经服务/数据库获取 | 临时侧栏没有搜索输入；普通快速打开在临时模式禁用 |
| 左栏宽度 | 默认 280，拖动范围 220–420；标准 h_resizable / resizable_panel | 取应用 LayoutState，默认 220，约束 160–480；使用 HomeView 的拖动逻辑 |
| 左栏是否可收起 | 该临时页面未提供收起入口 | 可收起，标题栏按钮仍在 |
| 列表 | List.large + ListDelegate，选中行背景、高亮及空/加载/错误状态；支持搜索框上下键选择 | 手动 v_flex + overflow_y_scroll、导航行和 Activity 标记，新建按钮置于列表上方；没有同一套搜索列表导航 |
| 面板宽度记录 | 临时视图使用组件内部布局，未见写入主窗口布局的路径 | 临时模式 save_layout 直接返回，但 fit 仍读取全局 LayoutState；初始宽度会受主窗口偏好影响 |
| 新建会话页 | 独立 NewConversation route；输入区居中，最大宽 780，外层 px_8/py_12 | 复用 HomeView：消息区占据剩余空间，输入区在底部；最大宽 820，外围 px_5/pt_2/pb_4 |
| 已有会话 | 复用 ConversationDetailPage | 复用 HomeView 的消息与输入区域；Pi 工具与运行状态体系不同，不能用容器差异评判消息细节 |
| 输入控件 | 与 Jaco 主聊天共用 ChatForm | 现在也与 Gupi 主聊天/任务设置共用 Composer 容器，模型、附件和发送仍按场景组合 |
| 右侧面板 | 临时页布局自身只有左右两列 | 可打开第三列历史栏；窗口宽度小于 1100 时历史栏覆盖右侧内容 |
| 键盘导向 | Cmd/Ctrl+F 聚焦搜索，Tab 切搜索/输入，Cmd/Ctrl+N 新建 | 沿用 Gupi 应用动作；没有临时专属“搜索↔输入”的 Tab 循环 |

Gupi 调整前左栏更窄，顶部功能更多，并保留右侧历史入口；Jaco 则把搜索作为最显眼入口。这是即使同样 960px 宽也会明显不同的主要原因。颜色与圆角是否需要改动，应在确定结构后看实际界面，本文不凭源码宣称哪一套视觉已验收。

## 4. 现行对齐方案

### 原生窗口与页面

- 使用 Jaco 的 `PopUp` 面板配置：960 × 620、固定外窗大小、Opaque 背景、隐藏系统交通灯，不保留主窗口自绘标题栏。创建时 `show: false`、`focus: false`，准备好布局、目标屏幕与焦点后再显现。
- 窗口等级明确统一为 Jaco 的 `WindowLevel::ModalPanel`，创建时设置，每次重新显示前再次设置；窗口类型统一为 `WindowKind::PopUp`。两者分别配置，不能只改 PopUp 类型后沿用其原生默认层级，也不另选 Normal、Floating 或 PopUpMenu。
- 每次召唤优先在鼠标所在显示器居中，无法识别时回退主屏。调用时序沿用“设置层级 → 定位 → 显示”，同时保留 Gupi 对原生回调重入的防护。Windows 使用与 Jaco 相同的平台接口及配置路径；当前层级接口在非 macOS 上无实际效果，应明确记录，不能声称已经具备相同的跨平台置顶行为。

原生选项按 Jaco 逐项对齐，而不是只模仿外观：

| 配置 | 对齐值 |
| --- | --- |
| kind / level | PopUp / ModalPanel；创建与重新显示均设置层级 |
| window_bounds | 目标显示器居中，初始 960 × 620 |
| is_resizable / window_min_size | false / 沿用 Jaco 默认 None，移除普通窗口的 640 × 420 最小尺寸配置 |
| show / focus | 创建时均为 false，准备完成再显示与聚焦 |
| titlebar | title None、appears_transparent true、traffic_light_position (-100, -100) |
| window_background | Opaque |
| display_id | 本次召唤选定的显示器 |
| app_id | 各自应用标识，Gupi 使用自己的 APP_NAME，不写成 Jaco |
| 其余 WindowOptions | 同 Jaco 从 Default 派生，不额外继承主窗口 TitleBar::window_options 的拖动配置 |

- 页面按 Jaco 采用顶部搜索、左会话列表、右侧新建/会话内容。左栏默认 280、可拖动范围 220–420；使用组件自带布局和样式，不套用主窗口侧栏宽度偏好。
- 新建页输入区居中；已有会话复用消息展示和输入组件。移除临时页面的主窗口标题栏与第三列历史栏结构，不复制一套消息、附件或模型选择实现。
- 显示时默认聚焦搜索，沿用 Jaco 的搜索框上下键导航、Tab 切搜索/正文、Cmd/Ctrl+F 聚焦搜索、Cmd/Ctrl+N 新建。候选/弹窗仍优先处理自己的按键。
- 搜索入口按 Jaco 展示，但数据只来自 Gupi Global 中的临时会话，不引入 Jaco 的数据库、Scratch 项目或普通会话目录。按标题做不区分大小写的包含匹配，沿用 ConversationState::infos 的 activity 降序、key 稳定排序，不读取文件或新增全文搜索。

### 窗口生命周期

```text
召唤 → 取消旧回收任务 → 复用或重建视图 → 定位目标屏幕 → 显示并聚焦搜索
可见窗口失焦 / 再按召唤键 / 关闭入口 → 隐藏 → 启动 600 秒窗口回收
隐藏期间再次召唤 → 取消回收 → 显示
持续隐藏满 600 秒 → 仅销毁窗口和视图 → Global 会话与 Pi 保留
之后再次召唤 → 用保留的会话状态重建视图
```

- 显式隐藏时按 Jaco 恢复召唤前的前台目标并释放该次焦点记录；处理失焦回调时避免重复隐藏或反复激活旧目标。原生操作继续避免在持有 App 可变借用时触发同步回调。
- 运行中的会话不阻止窗口隐藏或回收；Pi 事件由应用状态层持续消费。回收窗口不调用 abort、会话删除、目录清理或应用退出 drain。
- 回收任务由应用级所有者持有；重新显示取消任务，并校验任务对应的窗口，防止旧任务销毁刚重建的窗口。
- 重建至少恢复会话集合、当前会话、消息、正文草稿、附件、模板关联、运行状态与待处理扩展请求。已核对扩展输入 Change 事件写回 Global 中的 pending_ui.text，重建视图从该字段恢复。滚动、折叠、搜索焦点等纯 UI 状态不等同于会话数据，不据此保活窗口。
- 保留 Gupi 已确认的 Esc 分层语义：弹窗/候选先取消，生成中停止，空闲时隐藏；不从 Jaco 的局部源码推导另一套 Esc 规则。

### 与会话清理的边界

多会话独立运行、默认不保存、手动清理工作目录、普通文件路径引用和图片 RPC 发送保持不变。隐藏满 600 秒只销毁 UI；即使窗口已销毁，Global 仍持有的会话目录也不是“已释放目录”，设置批量清理不能删除它们。只有用户明确删除会话或退出应用时，才走相应的 Pi 关闭和数据释放流程。

### 实现分工与必要验证

1. 按配置表调整窗口类型与等级、跨屏定位和应用级显隐/回收任务；核对首次显示、再次显示、销毁后重建均采用同一 ModalPanel 等级，且无窗口时 Pi 继续消费事件。
2. 以专用临时页面复用现有业务层、消息和 Composer，接入 Jaco 式双栏与搜索导航。
3. 针对运行中失焦、隐藏超时、超时前重显、回收后重建验证：无重复实例、无丢失草稿/附件、旧计时任务不误删新窗口；再检查实际布局与焦点。

窗口所有者负责带版本校验的显隐和回收任务；专用 `features/temporary.rs` 负责搜索与双栏，HomeView 临时模式仅输出共享消息和 Composer。后续输入框改造按[接入计划](composer-resources.md)等待 InputGroup 与内联标签均进入正式版本。

## 5. 源码入口与验证边界

- [Jaco 原生窗口与显隐](../../../../jaco/src/app/temporary_window.rs)：窗口选项、ModalPanel、目标显示器、600 秒计时器。
- [Jaco 临时页面](../../../../jaco/src/features/temporary.rs)、[列表](../../../../jaco/src/features/temporary/list.rs)、[新建页面](../../../../jaco/src/features/temporary/new_conversation.rs)、[搜索](../../../../jaco/src/features/temporary/search.rs)。
- [Gupi 临时窗口所有者](../../../src/app/temporary.rs)、[HomeView 布局](../../../src/features/home.rs)、[标题栏](../../../src/features/home/titlebar.rs)、[临时页面](../../../src/features/temporary.rs)、[分栏](../../../src/features/home/panes.rs)、[输入区](../../../src/features/home/composer.rs)。
- [窗口平台扩展](../../../../../crates/window-ext/src/lib.rs)：set_window_level 的平台差异。
- GPUI Kit 0.6.0 的 TitleBar::window_options、GPUI pre 0.3.3 的 WindowOptions::default 与 macOS window.rs：核对默认选项和 PopUp 对应原生类型，避免把未显式设置等同于未知或关闭。

本次改动通过 `cargo check -p gupi --offline`、临时窗口相关 8 项回归、1 项页面级回归及 `cargo clippy -p gupi --all-targets --offline -- -D warnings`。自动化覆盖隐藏到期、重显取消回收、旧窗口计时器隔离、Global 状态保留、搜索导航和既有双实例运行；页面级测试验证 Tab 双向焦点切换与实际视图重建后的草稿保留。原生显隐与新布局另见 issue-221 README 验证记录；跨屏与 Windows 层级效果尚未实机验收。
