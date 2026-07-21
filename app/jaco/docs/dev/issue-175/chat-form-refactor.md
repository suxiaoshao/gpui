# 纯 UI ChatForm 与 ControlSlot 契约

> `ChatForm` 继续是纯 UI。本文件中的旧 form wiring 只记录 issue #175 的实施历史；最终 form API 与迁移步骤统一以
> [Jaco gpui-form 类型化双向绑定迁移](../gpui-form-migration.md) 为准。

本文固定 ChatForm 的纯UI边界、ControlSlot语义、external ChatInput FormStore/controller，以及四个调用方如何
组合相同控件。ChatForm不得import `gpui_form`、state config/repository/provider catalog或domain submit service。
实现已落地：`ChatForm` 保持在 `components/chat_form.rs`，ControlSlot/data contracts 在
`components/chat_form/controls.rs`，Project projection 在 `project_control.rs`；composer、attachment flow 和
option 定义位于 `components/chat_input/`。下文保留契约与调用方约束，独立 `view.rs`/`style.rs` 不再作为必需文件。

## 1. ControlSlot

`components/chat_form/controls.rs`：

```rust
#[derive(Clone)]
pub(crate) enum ControlSlot<T> {
    Hidden,
    Disabled(T),
    Enabled(T),
}

impl<T> ControlSlot<T> {
    pub(crate) fn as_ref(&self) -> ControlSlot<&T>;
    pub(crate) fn value(&self) -> Option<&T>;
    pub(crate) fn is_visible(&self) -> bool;
    pub(crate) fn is_enabled(&self) -> bool;
}
```

Invariants：

- Hidden唯一表示“不渲染”；不得同时保存show bool。
- Disabled必须携带state，才能显示value、placeholder、icon和error presentation。
- Enabled/Disabled使用同一个render函数；只把availability传入控件builder。
- slot组合在ChatForm构造时固定；本issue不支持运行时从Hidden切换为Enabled，control内部value可正常变化。

## 2. ChatFormControls

```rust
#[derive(Clone)]
pub(crate) struct ChatFormControls {
    pub(crate) project: ControlSlot<Entity<ProjectControlState>>,
    pub(crate) composer: ControlSlot<Entity<ComposerEditor>>,
    pub(crate) attachments: ControlSlot<Entity<AttachmentControlState>>,
    pub(crate) add_attachment: ControlSlot<AddAttachmentControl>,
    pub(crate) run_settings: RunSettingsControls,
    pub(crate) primary_action: ControlSlot<Entity<PrimaryActionControlState>>,
}

#[derive(Clone)]
pub(crate) struct RunSettingsControls {
    pub(crate) model: ControlSlot<Entity<ModelControlState>>,
    pub(crate) reasoning: ControlSlot<Entity<ReasoningControlState>>,
    pub(crate) approval: ControlSlot<Entity<ApprovalControlState>>,
}
```

Token Budget是ReasoningControlState的子输入，随reasoning slot可见/可用，不设独立slot。Project bar和
attachment strip只有对应slot visible才进入布局。

## 3. 纯 UI ChatForm

```rust
pub(crate) struct ChatForm {
    controls: ChatFormControls,
    bounds: Option<Bounds<Pixels>>,
    skill_completion_placement: ChatFormSkillCompletionPlacement,
    subscriptions: Vec<Subscription>,
}

#[derive(Clone, Debug)]
pub(crate) enum ChatFormUiEvent {
    AddProjectRequested,
    AddAttachmentFilesRequested,
    AddAttachmentFromClipboardRequested,
    ExternalPathsDropped(Vec<PathBuf>),
    OpenAttachmentRequested(ComposerAttachment),
    RemoveAttachmentRequested(u64),
    PrimaryActionRequested,
}
```

```rust
impl ChatForm {
    pub(crate) fn new(
        controls: ChatFormControls,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self;

    pub(crate) fn focus_composer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool;

    pub(crate) fn set_skill_completion_placement(
        &mut self,
        placement: ChatFormSkillCompletionPlacement,
    );
}
```

ChatForm subscriptions只观察UI state并notify；不把field change转成domain submit。`focus_composer`只在composer
Enabled时调用editor focus，Disabled/Hidden返回false。

## 4. Disabled contract

ChatForm constructor根据固定slot对内部interactive state应用一次availability：

- ComposerEditor新增 `set_disabled/is_disabled/set_placeholder/set_snapshot` facade；Disabled关闭输入、IME、
  paste、外部drop、skill completion、`ComposerEditorEvent::SubmitRequested`和focus。具体在key context/action、
  text/IME mutation、paste/drop handler、completion popup和focus入口统一检查`is_disabled`，但保留shell、
  snapshot读取和placeholder。
- Picker controls使用同一个trigger/popup renderer；Disabled trigger调用现有`Disableable::disabled(true)`且不
  打开popover。
- AttachmentControlState Disabled时strip仅展示已有值但remove/preview mutation禁用；Shortcut传空state。
- AddAttachment和PrimaryAction Disabled时按钮保留相同icon/size/tooltip但不可click/focus。
- Hidden完全跳过child和spacing；Project Hidden时不增加project bar bottom padding。

slots构造后不可变，因此UI state中的disabled是slot availability的初始化cache，无双向同步或invalidating问题。

## 5. View与style

当前实现由 `chat_form.rs` 直接完成以下纯 UI render；样式常量与布局链保持在同一 app-local UI shell 中：

1. project slot visible时渲染project bar背景层；
2. 渲染统一rounded ChatForm shell；
3. composer/attachment strip；
4. footer左侧add/reasoning/approval；右侧model/primary action；
5. composer skill completion overlay。

Project picker自己的option/trigger render在
`project_control.rs`，使用现有FolderOpen/FolderX/FolderPlus和picker_popover。

保留`jaco-chat-form`及现有control debug selectors；新增project slot selector。Shortcut不得复制这些elements。

## 6. Form-agnostic UI states

- `ProjectControlState`：picker ListState、open、labels/options UI cache；不保存default project config或发DB写。
- `AttachmentControlState`：当前attachments render projection；typed attachments draft由external form field拥有。
- `AddAttachmentControl`：无业务callback的菜单presentation；点击转换成ChatFormUiEvent。
- `PrimaryActionControlState`：`Send | Stop`和derived can-activate视觉projection；不持有domain callback或runtime。
- ComposerEditor：现有text/token/focus UI state。
- RunSettings control states见 [run-settings.md](run-settings.md)。

## 7. External ChatInput form

新增 `components/chat_input/form_state.rs`：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ChatInputFormStore)]
pub(crate) struct ChatInputInput {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) attachments: Vec<ComposerAttachment>,
    #[form(group)]
    pub(crate) run_settings: RunSettingsInput,
}
```

`ChatInputFormStore` 持有唯一 typed model；bound controls 写入 typed fields。
submit 通过 `prepare_submit` 获得同一版本的 output，再解析 settings、校验附件并构造 payload。
项目选择仍由 `NewConversationPage` 持有，新增项目只发 `AddProjectRequested`。

## 8. ChatInputController

```rust
pub(crate) struct ChatInputController {
    composer: Entity<ComposerEditor>,
    chat_form: Entity<ChatForm>,
    form: Entity<ChatInputFormStore>,
    run_settings: Entity<RunSettingsController>,
    primary_action_state: Entity<PrimaryActionControlState>,
    chat_form_config: StoreBinding<ChatFormConfig, JacoError>,
    skill_catalog_scope: SkillCatalogScope,
    skill_catalog_task: Task<()>,
    agent_running: bool,
    _subscriptions: Vec<Subscription>,
}

pub(crate) enum ChatInputEvent {
    SendRequested(Box<ChatInputSubmit>),
    StopRequested,
}
```

constructors：

```rust
pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self;
pub(crate) fn new_without_focus(window: &mut Window, cx: &mut Context<Self>) -> Self;
pub(crate) fn new_with_project(
    project: Entity<ProjectControlState>,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Self;

pub(crate) fn clear_after_submit(&mut self, window: &mut Window, cx: &mut Context<Self>);
pub(crate) fn set_agent_running(&mut self, running: bool, cx: &mut Context<Self>);
pub(crate) fn refresh_skill_catalog(&mut self, project_root: Option<&Path>, cx: &mut Context<Self>);
```

`ChatInputController`直接订阅`ComposerEditorEvent`：`Changed`提交显式 form command，
`PasteAttachmentRequested`进入 attachment command，`SubmitRequested`调用 primary action/submit流程。
`ChatForm`只渲染editor，不代理这些业务事件。

`ChatInputController` 从 `ChatInputFormStore::run_settings_field(&form)` 取得 parent group field，并用该 field
创建唯一 `RunSettingsController`。Token Budget 这类子控件通过 `FormField::project_value`
读写同一个 parent group，不创建 child form store或平行 draft。Controller 直接用同一组
`RunSettingsControlStates` 构造 `ChatFormControls`，项目场景只替换 project `ControlSlot`。
controller处理attachment/drop/primary-action等ChatFormUiEvent、attachment tasks、skill catalog、submit snapshot
resolution 和 ChatFormConfig persistence。`AddProjectRequested`由NewConversationPage路由到project logic。纯ChatForm不
订阅controller domain event，也不保存Rc domain callback。

## 9. Project external form

当前实现没有新增 `NewConversationFormStore`：`NewConversationPage` 继续拥有 canonical
`selected_project_id`、catalog owner/subscription 和 folder prompt/add 逻辑；`ProjectControlState` 只保存
open/query/highlight 等 interaction，selection 与 catalog 通过纯函数派生 presentation 并作为 render input，通过
`ChatInputController::new_with_project(project_state, ...)` 将 project slot 注入 ChatForm。项目选择仍更新默认配置和
skill scope；不得为 project page 另建平行 ChatInput 或 RunSettings draft。

## 10. 四个调用方

- ConversationDetail：创建standalone ChatInputFormStore/Controller；project Hidden；其他slots Enabled。
- NewConversationPage：controller 使用 `new_with_project(project_state, ...)`；project 使用
  `Enabled(project_state)`；其他 slots Enabled。
- TemporaryNewConversationPane：standalone ChatInput；project Hidden；其他slots Enabled；不persist project。
- ShortcutEditDialog：Shortcut form只含RunSettings child；保留hotkey/prompt/input source/enabled四个Shortcut
  专属字段，删除现有独立model field和对应顶层validator/render；创建空presentation-only composer/attachments/
  primary action，包装Disabled，嵌入同一个ChatForm shell；project Hidden，RunSettings controls Enabled。model、
  reasoning、approval只由ChatForm渲染一次，Shortcut dialog不得再自行创建Select或popover。

调用方负责把ChatFormUiEvent路由到对应controller。ChatInputController继续发保持现有payload的
`ChatInputEvent::SendRequested/StopRequested`，页面的Conversation创建/Run启动逻辑不改。

## 11. 测试

- `control_slot_reports_visibility_and_enabled_state`
- `hidden_slot_does_not_render_or_take_focus`
- `disabled_slot_renders_value_but_rejects_interaction`
- `disabled_composer_rejects_key_ime_paste_drop_skill_completion_and_submit`
- `project_slot_changes_chat_form_stack_padding`
- `chat_form_does_not_import_or_own_form_store`（residual/module boundary assertion）
- `chat_input_form_tracks_composer_attachments_and_run_settings`
- `chat_input_submit_merges_attachments_once`
- `chat_input_controller_persists_defaults_only_when_configured`
- `conversation_new_temporary_and_shortcut_build_expected_slots`
- existing composer/attachment/skill/config/agent-running tests迁移后保持断言。

测试归属固定为：ControlSlot和ChatForm view测试留在`components/chat_form/{controls,chat_form}.rs`，controller
测试留在`components/chat_input/{form_state,chat_input}.rs`，Shortcut child validation/render测试留在
`features/settings/shortcuts/{form_state,dialog}.rs`，不使用无法定位实现位置的纯source扫描替代行为测试。

## 12. 当前所有权边界

本文件描述 issue #175 已落地的 UI/form 边界，并遵循
[Jaco gpui-form 类型化双向绑定迁移](../gpui-form-migration.md)：generated form store 持有 typed current value，
owning bound control 持有 focus/subscriptions，catalog/options 只更新 control config。当前实现不使用
child form store、平行 string draft 或 `SubscriptionSet`。

## 13. 明确不做

- 不创建ChatFormMode或show/disabled bool集合。
- 不把Shortcut包装成dummy ChatInputFormStore。
- 不让ChatForm调用gpui-form、repository、config或runtime。
- 不将ControlSlot提升到gpui-component；它是Jaco ChatForm组合语义。
