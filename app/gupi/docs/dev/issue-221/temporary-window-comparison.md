# 临时窗口配置与生命周期

当前实现已对齐用户选择的 Jaco 行为；只保留现行窗口契约、状态所有权和验证边界，删除调整前逐项外观对照。

## 现行窗口配置

### 原生窗口与页面

- 使用 `PopUp` 面板配置：960 × 620、固定外窗大小、Opaque 背景、隐藏系统交通灯，不保留主窗口自绘标题栏。创建时 `show: false`、`focus: false`，准备好布局、目标屏幕与焦点后再显现。
- 窗口等级为 `WindowLevel::ModalPanel`，创建时设置，每次重新显示前再次设置；窗口类型统一为 `WindowKind::PopUp`。两者分别配置，不能只改 PopUp 类型后沿用其原生默认层级，也不另选 Normal、Floating 或 PopUpMenu。
- 每次召唤优先在鼠标所在显示器居中，无法识别时回退主屏。调用时序沿用“设置层级 → 定位 → 显示”，同时保留 Gupi 对原生回调重入的防护。Windows 使用与 Jaco 相同的平台接口及配置路径；当前层级接口在非 macOS 上无实际效果，应明确记录，不能声称已经具备相同的跨平台置顶行为。

原生选项：

| 配置 | 对齐值 |
| --- | --- |
| kind / level | PopUp / ModalPanel；创建与重新显示均设置层级 |
| window_bounds | 目标显示器居中，初始 960 × 620 |
| is_resizable / window_min_size | false / None，移除普通窗口的 640 × 420 最小尺寸配置 |
| show / focus | 创建时均为 false，准备完成再显示与聚焦 |
| titlebar | title None、appears_transparent true、traffic_light_position (-100, -100) |
| window_background | Opaque |
| display_id | 本次召唤选定的显示器 |
| app_id | 各自应用标识，Gupi 使用自己的 APP_NAME，不写成 Jaco |
| 其余 WindowOptions | 从 Default 派生，不额外继承主窗口 TitleBar::window_options 的拖动配置 |

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

窗口所有者负责带版本校验的显隐和回收任务；专用 `features/temporary.rs` 负责搜索与双栏，HomeView 临时模式仅输出共享消息和 Composer。后续输入框改造按[接入计划](../issue-243/README.md)等待输入原子标签和 Questionnaire 正式发布；InputGroup 已接入。

## 源码入口与验证边界

- [Gupi 临时窗口所有者](../../../src/app/temporary.rs)、[HomeView 布局](../../../src/features/home.rs)、[标题栏](../../../src/features/home/titlebar.rs)、[临时页面](../../../src/features/temporary.rs)、[分栏](../../../src/features/home/panes.rs)、[输入区](../../../src/features/home/composer.rs)。
- [窗口平台扩展](../../../../../crates/window-ext/src/lib.rs)：set_window_level 的平台差异。
- GPUI Kit 0.6.0 的 TitleBar::window_options、GPUI pre 0.3.3 的 WindowOptions::default 与 macOS window.rs：核对默认选项和 PopUp 对应原生类型，避免把未显式设置等同于未知或关闭。

本次改动通过 `cargo check -p gupi --offline`、临时窗口相关 8 项回归、1 项页面级回归及 `cargo clippy -p gupi --all-targets --offline -- -D warnings`。自动化覆盖隐藏到期、重显取消回收、旧窗口计时器隔离、Global 状态保留、搜索导航和既有双实例运行；页面级测试验证 Tab 双向焦点切换与实际视图重建后的草稿保留。原生显隐与新布局另见 issue-221 README 验证记录；跨屏与 Windows 层级效果尚未实机验收。
