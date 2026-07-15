# Issue #175：纯 UI ChatForm、快捷键运行设置与临时窗口

关联 issue：[suxiaoshao/gpui#175](https://github.com/suxiaoshao/gpui/issues/175)

## 1. 状态与范围

本文档集先作为 issue #175 的实施规格，现同步记录已落地的实现。工作分支为
`codex/175-jaco-shortcut-temporary-window`，基线为 `origin/main@3cb5687`。代码已完成重构与行为调整；代码级
验证已通过，快捷键/临时窗口的实际桌面 UI 验收按用户要求跳过。

### 用户决定

- 自定义快捷键保持现状：取得 selection、clipboard 或 screenshot 后自动创建 Conversation 并立即启动
  AgentRun。
- 自动创建的 Conversation 必须由与普通临时会话相同的 popup `TemporaryWindow` 生命周期承载。
- `ChatForm` 重构为纯 UI 组件；不拥有 gpui-form、业务 draft、验证、ChatFormConfig、provider catalog或DB。
- gpui-form 外置到调用方。普通会话、新对话、临时新对话和快捷键编辑分别只声明自己需要的数据。
- ChatForm 使用 `ControlSlot::{Hidden, Disabled, Enabled}` 统一每个控件的布局、disabled、focus和交互。
- 新对话的 Project 控件为 Enabled；普通会话、临时新对话和快捷键编辑为 Hidden。
- 快捷键编辑的 composer、attachment/add和 primary action为 Disabled；model、reasoning、Token Budget、
  Tool Access为 Enabled。
- model/reasoning/budget联动仍由一个共享 RunSettings form/controller实现，不能因为表单外置而复制逻辑。
- 不扩展 Skills、MCP逐项权限、prompt系统、provider adapter或Conversation runtime设计。

### 已落地的关键行为

- 快捷键创建 Conversation 后只通过 `temporary_window::show_created_conversation` 路由到 `WindowKind::PopUp`；
  不再由 hotkey 层直接 downcast `TemporaryWindow`，也没有普通窗口 fallback。
- Shortcut 编辑器复用同一个 ChatForm shell：composer、attachment/add、primary action 为 Disabled，model、
  reasoning/Token Budget、approval 为 Enabled。
- `RunSettingsFormStore` 是 model/reasoning/approval 的 typed source of truth；ChatInput 的 config persistence
  从 nested form draft 读取，不再维护平行 selected-settings 字段；ChatForm 只接收 state 和 UI event sink。
- Disabled Composer 的删除、换行、撤销、重做、粘贴、IME 和提交入口均拒绝修改；已有值仍可作为 presentation 显示。

### 目标

1. 将 ChatForm 限定为统一 UI shell与control slots，使同一个组件覆盖四个调用场景。
2. 用类型表达 Hidden/Disabled/Enabled，删除调用方散落的 show/disable/mode判断。
3. 把可提交数据和交互逻辑外移到 `ChatInputController`、`RunSettingsController`及调用方FormStore，保持一个
   业务source of truth。
4. 把当前 NewConversationPage 外置的 Project bar迁入 ChatForm project slot，但保留项目数据/持久化owner。
5. 快捷键完整保存并执行 model/reasoning/approval，继续复用现有 `settings_snapshot_json`。
6. 将 shortcut created Conversation的route + reveal收口到TemporaryWindow lifecycle owner。

### 非目标

- 不把 ChatForm 变成通用跨app组件；它仍是 Jaco app-local UI composition。
- 不把 Project、attachment、run settings或Conversation业务逻辑放回 ChatForm。
- 不迁移到 `gpui-store`，不修改 `gpui-form` crates。
- 不新增数据库migration、依赖、icon、runtime asset或普通窗口fallback。
- 不改变临时窗口尺寸、级别、屏幕定位、失焦隐藏、延迟销毁和前台app恢复。

## 2. 文档地图

- [chat-form-refactor.md](chat-form-refactor.md)：纯 UI ChatForm、ControlSlot、外置ChatInput FormStore、Project
  slot和四个调用方契约。
- [run-settings.md](run-settings.md)：外置共享 RunSettings FormStore/controller、picker states、持久化与
  capability mismatch。
- [temporary-window-runtime.md](temporary-window-runtime.md)：快捷键输入路径、临时窗口生命周期、错误与测试。

## 3. 实施前证据快照

| 分类 | 当前事实 | 证据 | 设计后果 |
| --- | --- | --- | --- |
| Current fact | ChatForm Entity同时拥有business draft、pickers、config、attachments、skills、runtime guards和Render | `components/chat_form.rs::ChatForm` | 拆成纯 UI ChatForm + 外部ChatInputController/FormStore |
| Current fact | NewConversationPage在ChatForm外单独维护和渲染Project bar | `features/home/new_conversation.rs::{projects,render_project_bar}` | Project UI迁为slot；project data/logic仍归NewConversationPage |
| Current fact | ConversationDetail和TemporaryNewConversation只渲染ChatForm，没有Project selector | 对应constructors/render | 两者project slot为Hidden |
| Current fact | ShortcutEdit已使用gpui-form，只有独立model field | `features/settings/shortcuts/form_state.rs` | 只嵌套RunSettings group，不嵌套无关composer/attachments |
| Current fact | ComposerEditor和picker都是context-managed Entity | `chat_form/composer_editor.rs`、picker modules | ControlSlot携带现有UI state，Disabled仍能显示value/placeholder |
| Current fact | ComposerSnapshot `Clone + PartialEq` | `composer_editor/snapshot.rs` | 可继续由external custom binding读取，无需重写editor |
| Upstream fact | gpui-form支持custom binding和nested group，父store跟踪child draft/meta | `crates/gpui-form/README.md`、`tests/derive.rs` | ChatInput/RunSettings可复用group，但ChatForm本身不依赖FormStore |
| Upstream fact | gpui-component controls支持disabled；picker为现有app-local composition | repo component refs和当前代码 | ChatForm view统一应用slot availability，不建第二套控件 |
| Current fact | Shortcut JSON已保存完整RunSettingsSnapshot | jaco-db records/repository、`state/shortcuts.rs` | DB/schema/repository No change |
| Current fact | hotkey层先open temporary window再自行downcast view | `state/hotkey.rs::open_created_shortcut_conversation` | route + reveal归temporary lifecycle owner |

## 4. 最终架构决定

### D-01：ControlSlot 是唯一availability模型

```rust
pub(crate) enum ControlSlot<T> {
    Hidden,
    Disabled(T),
    Enabled(T),
}
```

Hidden不渲染、不占布局、不进入focus；Disabled保留state/value并使用相同控件渲染，但禁止输入、picker、drop、
action和focus；Enabled正常交互。不再保存 `show_*`、`*_disabled` 或 `ChatFormMode` 镜像字段。

### D-02：ChatForm 是纯UI Entity

ChatForm只持有 `ChatFormControls` 和bounds/skill-popup-placement等瞬时视觉状态。它不知道gpui-form、field path、
validation、provider catalog、ChatFormConfig、repository或submit domain。它渲染统一shell并把UI action作为
`ChatFormUiEvent`发给调用方。

### D-03：外置表单分两层复用

- `ChatInputFormStore`：composer、attachments、RunSettings group，供普通会话、新对话和临时新对话复用。
- `RunSettingsFormStore`：model、reasoning、approval，供ChatInput和ShortcutEdit复用。
- `NewConversationPage`：拥有 project state、project persistence 与 ChatInputController；不新增平行
  `NewConversationFormStore`。
- `ShortcutEditFormStore`：hotkey、prompt、RunSettings group、input source、enabled；没有空composer或attachments。

### D-04：UI control state 与 business draft 分离

ControlSlot携带的是form-agnostic UI state Entity。ChatInput/RunSettings 的 gpui-form value/group fields 负责 typed
draft；controller 在 UI state 与 form draft 之间做显式同步。Shortcut的 Disabled composer/attachments/action 使用
presentation-only state，不进入 `ShortcutEditFormInput`。UI projection 不是第二份 business owner。

### D-05：RunSettings controller唯一拥有联动

model confirm、catalog reload、reasoning default/validity、Token Budget bounds/clamp、picker互斥和approval选择
全部由RunSettingsController实现。Conversation调用方安装ChatFormConfig persistence adapter；Shortcut不安装。

### D-06：Project slot统一布局，owner不变

Project picker的trigger/bar/footer视觉移动到ChatForm project control/render。projects列表、selected project、default
project config、add-folder task、catalog reload和skill catalog refresh仍由NewConversationPage及其 state/controller
持有。Hidden project slot使普通会话/临时/Shortcut不产生bar或bottom padding。

### D-07：四个调用场景固定组合

| Slot | ConversationDetail | NewConversationPage | TemporaryNewConversation | ShortcutEditor |
| --- | --- | --- | --- | --- |
| project | Hidden | Enabled | Hidden | Hidden |
| composer | Enabled | Enabled | Enabled | Disabled |
| attachments/add | Enabled | Enabled | Enabled | Disabled |
| model | Enabled | Enabled | Enabled | Enabled |
| reasoning/budget | Enabled* | Enabled* | Enabled* | Enabled* |
| approval | Enabled | Enabled | Enabled | Enabled |
| primary action | Enabled | Enabled | Enabled | Disabled |

`*` 表示slot为Enabled，但control仍可因selected model没有reasoning capability而呈现derived disabled；这不是
第二个ControlSlot状态，而是control内部从capability派生的合法性。

### D-08：Shortcut快照是运行来源

保存与触发都校验当前model capability；snapshot写reasoning和approval。触发不再hard-code Ask，不兼容时
CapabilityMismatch并终止，不静默降级。

### D-09：TemporaryWindow负责route + reveal

新增 `show_created_conversation` 消费CreatedConversation，在lifecycle owner内create/find popup、route、start
run一次、prepare和schedule reveal。hotkey的快捷键路由不再知道Root/TemporaryWindow view结构；通用 notification
helper保留自己的Root查找不属于本issue的窗口路由。

## 5. 实际文件结构

实现没有按计划再拆出独立的 `view.rs`、`style.rs`、`logic.rs` 文件：纯 UI render 保持在 `chat_form.rs`，共享
run-settings render 保持在 `run_settings.rs`，数据与逻辑分别由以下模块承载。这样保留了当前 app-local 组件的
边界，同时避免为薄封装增加模块跳转。

### 新增

- `app/jaco/src/components/chat_form/controls.rs`：ControlSlot、ChatFormControls、UI control contracts。
- `app/jaco/src/components/chat_form/project_control.rs`：form-agnostic ProjectPickerControlState/render。
- `app/jaco/src/components/chat_input.rs`：ChatInputController、事件和提交/附件/技能逻辑。
- `app/jaco/src/components/chat_input/form_state.rs`：ChatInputInput/Store 和 nested RunSettings group。
- `app/jaco/src/components/run_settings.rs`：RunSettingsInput、controller、control state 和共享 render。
- `app/jaco/src/components/run_settings/policy.rs`：reasoning/token budget policy 与测试。
- `app/jaco/docs/dev/issue-175/{README,chat-form-refactor,run-settings,temporary-window-runtime}.md`

### 删除/移动

- 原 `components/chat_form.rs` 业务实现移至 `components/chat_input.rs`；原 `components/chat_form/` 下的 composer、
  attachment flow 和 run-settings option 模块移至 `components/chat_input/`。
- `model_select.rs`、`attachment_views.rs` 的渲染职责移入纯 UI `ChatForm`；`effort_select.rs` 和
  `approval_select.rs` 仅保留共享 option/section 定义。
- reasoning policy 从旧 ChatForm 模块移至 `components/run_settings/policy.rs`。

### 修改

- `app/jaco/src/components.rs`
- `app/jaco/src/components/chat_form.rs`：缩为纯UI facade/Entity，不含业务draft。
- `app/jaco/src/components/chat_input/{attachment_flow,attachments,composer_editor}.rs`
- `app/jaco/src/components/conversation_detail.rs`
- `app/jaco/src/features/home/new_conversation.rs`
- `app/jaco/src/features/temporary/new_conversation.rs`
- `app/jaco/src/features/settings/shortcuts/{form_state,dialog,rows}.rs`
- `app/jaco/src/state/{shortcuts,hotkey}.rs`
- `app/jaco/src/app/temporary_window.rs`
- `app/jaco/docs/dev/README.md`

### 明确不修改

- `crates/gpui-form*`、`crates/jaco-core/*`、`crates/jaco-db/*`。
- `Cargo.toml`、`Cargo.lock`、bootstrap、submodule、icons/assets。
- Conversation runtime/provider/Rig/MCP/skills domain实现。

## 6. 复用与删除审计

| 当前实现 | 决定 | 结果 |
| --- | --- | --- |
| ChatForm业务状态 + Render混合Entity | Split | 纯ChatForm UI + external ChatInputController/FormStore |
| NewConversation独立Project bar renderer | Adapt | UI/render迁入project slot；business owner保留 |
| Shortcut独立ModelSelectBinding | Delete | Shortcut嵌套共享RunSettingsStore/control state |
| ChatForm三个私有run-setting renderer | Delete/Move | RunSettings共享controls/view/style |
| reasoning policy | Retain/Move | 保留全部variant与测试 |
| ComposerEditor | Retain | 保留snapshot，并增加 disabled facade |
| gpui-form group/value fields | Reuse directly | 不修改form crates |
| DB JSON snapshot | Reuse directly | 不迁移schema |
| hotkey view downcast | Delete | temporary lifecycle入口替代 |

无依赖变化，version/features/MSRV/native/transitive/lockfile全部No change。

## 7. 工作包

依赖：`WP-10 -> WP-20 -> WP-30 -> WP-40 -> WP-50 -> WP-60`。WP-10 至 WP-50 已实施，WP-60 已完成代码级回归；
实际桌面 bundle/Computer Use 验收按用户要求不执行。

### WP-10：ControlSlot与纯UI ChatForm（已完成）

建立controls/project UI state，ChatForm只消费slots和发UI events。按
[chat-form-refactor.md](chat-form-refactor.md)的API、layout、focus和测试实现。

### WP-20：外置ChatInput与RunSettings forms/controllers（已完成）

建立ChatInputFormStore、RunSettingsFormStore和controllers，将当前ChatForm业务draft、catalog、config、
attachment、skills和submit逻辑移出UI。API与测试见两个子文档。

### WP-30：迁移Conversation/New/Temporary调用方（已完成）

三个调用方复用 ChatInputController，构造固定 slot 组合。NewConversationPage 保留 project state/persistence 并
注入 project slot；其他两个传 Hidden project。保持现有 submit/runtime 行为。

### WP-40：Shortcut使用纯ChatForm + RunSettings group（已完成）

ShortcutEditFormStore只嵌套RunSettings。dialog创建Disabled presentation controls和Enabled run settings，渲染
同一个ChatForm；保存读取RunSettings child draft，不产生dummy chat data。

### WP-50：Shortcut持久化/触发与TemporaryWindow收口（已完成）

扩展ShortcutDraft/snapshot/trigger settings，增加CapabilityMismatch；按
[temporary-window-runtime.md](temporary-window-runtime.md)实现popup route/reveal入口。

### WP-60：回归、bundle和隔离桌面验收（代码级完成）

已完成格式化、编译、clippy、测试目标编译及 focused policy/shortcut/hotkey 测试。实际 bundle、Computer Use、
快捷键 popup 路由和窗口交互验收明确跳过，后续可单独作为桌面验收任务执行。

## 8. 系统面决策

| 系统面 | 决定 |
| --- | --- |
| UI | ChatForm纯UI；ControlSlot统一Hidden/Disabled/Enabled；gpui-component/picker复用 |
| 数据 | ChatInput/RunSettings/Shortcut各自FormStore；NewConversationPage保留project state；无ChatForm business draft |
| 逻辑 | ChatInputController、RunSettingsController、NewConversation project logic、Temporary lifecycle分属owner |
| 状态 | UI state Entity可作为binding adapter；form field仍是唯一typed draft |
| Focus | Hidden跳过；Disabled不可focus；Enabled保持现有keyboard behavior |
| 并发 | attachment/project path/OCR/selection tasks保持owner和foreground回调；无新锁/channel |
| DB | schema/repository/transaction No change；只改变Shortcut JSON值 |
| i18n | 复用现有`chat-form-placeholder`；现有project/run-settings/mismatch keys复用；No new key |
| icon/assets | 继续FolderOpen/FolderX/FolderPlus/Lightbulb/Shield/provider/Plus/Send；No change |
| dependencies/platform | No change；三平台CI和macOS popup behavior保持 |

## 9. 验证

```bash
cargo fmt --all
cargo check -p jaco
cargo clippy -p jaco --all-targets --all-features -- -D warnings
cargo build -p jaco
cargo test -p jaco --no-run
cargo test -p jaco components::chat_form::controls::tests
cargo test -p jaco components::run_settings::tests
cargo test -p jaco components::run_settings::policy::tests
cargo test -p jaco features::settings::shortcuts::rows::tests
cargo test -p jaco features::settings::shortcuts::validation::tests
cargo test -p jaco state::hotkey::tests::temporary_hotkey
cargo test -p jaco components::chat_input::tests::selected_model_choice
git diff --check
```

上述命令均已通过。按用户要求未运行实际桌面 UI、bundle 或 Computer Use 验收；`cargo test --no-run` 只用于编译
测试目标，不会启动 GPUI 窗口。

Residual scans：

```bash
rg -n "ChatFormStore|ChatFormInput|ChatFormMode|selected_model_key|selected_reasoning_selection" \
  app/jaco/src/components/chat_form.rs app/jaco/src/components/chat_form
rg -n "ShortcutModelSelectBinding|render_project_bar|render_model_selector|render_effort_selector" \
  app/jaco/src/features app/jaco/src/components/chat_form.rs
rg -n "TemporaryWindow|open_temporary_window|open_created_shortcut_conversation|downcast::<TemporaryWindow>" \
  app/jaco/src/state/hotkey.rs
git diff -- Cargo.toml Cargo.lock crates/gpui-form crates/jaco-db
```

隔离bundle验收：

1. `JACO_CONFIG_DIR=/tmp/jaco-issue-175-qa` 且storage data dir指向临时目录。
2. ConversationDetail：project无布局，其他controls enabled。
3. NewConversation：project bar位于同一ChatForm shell，选择/新增项目和skill scope保持。
4. TemporaryNewConversation：project hidden，其余enabled。
5. ShortcutEditor：project hidden；composer/attachment/send同样式disabled；run settings enabled。
6. 四个surface的model/reasoning/approval trigger、popover、spacing和focus一致。
7. selection/clipboard/direct-image/OCR Shortcut进入同一popup并立即运行。
8. 失焦后复用popup、重新定位、路由新Conversation且不重复run。

## 10. 交接审计

- 已撤销“Shortcut嵌套完整ChatFormStore”和“ChatFormMode”设计。
- ControlSlot三态、四场景组合、外置FormStore、Project owner、RunSettings owner和Temporary owner均已固定。
- Project当前实现位置、gpui-form group/binding API及disabled边界均由当前源码验证。
- hotkey中允许保留通用notification helper对`Root`的downcast；快捷键路径只保留
  `temporary_window::show_created_conversation`调用，不再直接操作TemporaryWindow。
- 没有实现时再决定的架构项、框架改造或依赖release gate。
