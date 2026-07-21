# 外置共享 RunSettings 契约

> 当前实现已经采用最终所有权：generated parent form store 持有 typed current values，owning bound controls
> 持有 component interaction，catalog 持有 options/capability；完整迁移 gate 见
> [Jaco gpui-form 类型化双向绑定迁移](../gpui-form-migration.md)。

RunSettings 负责 model、reasoning、Token Budget 和 Tool Access。`RunSettingsInput` 作为父 form 中的
typed group；form-agnostic UI control states 和 controller 只投影该 group。ChatForm 只接收
`RunSettingsControls`，不依赖任何 form store。

## 1. Form data

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = RunSettingsFormStore)]
pub(crate) struct RunSettingsInput {
    #[form(required)]
    pub(crate) model: Option<ProviderModelKey>,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
    pub(crate) approval_mode: ToolApprovalMode,
}
```

choices 和 capability 不进入 typed form model，但其事实源应是 app-owned typed catalog snapshot；UI control state 只缓存
用于 render/picker 的 projection。提交和快捷键保存/触发路径必须读取同一份 fresh catalog/capability snapshot，校验
enabled provider/model 和 reasoning compatibility。

## 2. UI control states

`run_settings.rs`定义 form-agnostic control state：

```rust
pub(crate) struct ModelControlState {
    choices: Result<Vec<ProviderModelChoice>, SharedString>,
    picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    open: bool,
}

pub(crate) struct ReasoningControlState {
    capability: Option<ModelCapabilitiesSnapshot>,
    picker: Entity<ListState<PickerListDelegate<ReasoningOption>>>,
    token_budget_input: Entity<InputState>,
    open: bool,
}

pub(crate) struct ApprovalControlState {
    picker: Entity<ListState<PickerListDelegate<ApprovalModeOption>>>,
    open: bool,
}

#[derive(Clone)]
pub(crate) struct RunSettingsControlStates {
    pub(crate) model: Entity<ModelControlState>,
    pub(crate) reasoning: Entity<ReasoningControlState>,
    pub(crate) approval: Entity<ApprovalControlState>,
}
```

这些 state 是 picker/token input 的 UI projection。`token_budget_input` 是 owning
`FormIntegerInputState<Form, u32>` 的 presentation handle；controller 以 type-erased entity 保留 control
生命周期，应用代码不解析其文本。control 通过 parent `RunSettingsInput` 的 typed value projection 读写 custom
budget，临时不完整、overflow 与 range error 作为 lifecycle-scoped control issue 阻止 parent form submit。
Controller 将其他用户事件显式写入 parent form 的 value/group fields，不能把 control cache 当成
submit/validation 事实源。
ChatForm view根据外层ControlSlot availability传disabled给trigger；ReasoningControl内部另从capability派生“没有
合法选项”的disabled，Token Budget随reasoning state。

## 3. Policy

移动`chat_form/thinking_effort.rs`为`run_settings/policy.rs`，保留Boolean、Levels、AdaptiveLevels、TokenBudget、
AlwaysOn、Composite和legacy全部语义/测试：

```rust
pub(crate) struct TokenBudgetBounds { min, max, default_value }
pub(crate) fn reasoning_selections(...);
pub(crate) fn computed_default_reasoning_selection(...);
pub(crate) fn reasoning_selection_is_valid(...);
pub(crate) fn token_budget_bounds(...);
pub(crate) fn custom_token_budget_value(...);
pub(crate) fn reasoning_selection_label(...);
```

reasoning option 依 capability 生成；Token Budget 的 computed default 必须位于 bounds 内，small 范围
step 为 1，否则为 1024。已有 custom budget 不做静默 clamp，越界时由 typed control issue 显式报错。

## 4. RunSettingsController

```rust
pub(crate) struct RunSettingsController {
    read_value: RunSettingsReader,
    write_value: RunSettingsWriter,
    token_budget_default: Rc<Cell<u32>>,
    token_budget_sync: TokenBudgetSync,
    _token_budget_control: AnyEntity,
    states: RunSettingsControlStates,
    _subscriptions: Vec<Subscription>,
}

impl RunSettingsController {
    pub(crate) fn new<Form>(
        field: FormField<Form, RunSettingsInput>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self
    where
        Form: FormStore + EventEmitter<FormEvent<Form::Field>>;

    pub(crate) fn control_states(&self) -> RunSettingsControlStates;
    pub(crate) fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>);
}

pub(crate) fn resolve_run_settings(
    draft: &RunSettingsInput,
    choices: &Result<Vec<ProviderModelChoice>, SharedString>,
) -> Result<RunSettingsSubmitSnapshot, RunSettingsSubmitError>;
```

Controller是唯一联动owner。它只持有`RunSettingsControlStates`，不得引用ChatForm侧的`RunSettingsControls`；
调用方从`control_states()`取得同一组Entity后，再按自己的`ControlSlot` availability包装。

- model user confirm -> 只写 model field；reasoning/approval保持并按新 capability 动态验证；
- catalog snapshot -> 只更新 model items/capability/disabled/unavailable presentation，不修改 form draft、不选
  fallback；
- reasoning confirm/token input -> 写入同一 reasoning field；out-of-range 由 typed control issue 报告，不静默 clamp；
- approval confirm -> approval field；
- opening picker -> 关闭另外两个；
- `resolve_run_settings` 是无副作用纯函数：接收一次 form value + catalog snapshot，验证显式选择与 capability，
  并返回后续 attachment 校验和 `ChatInputSubmit` 唯一使用的
  `RunSettingsSubmitSnapshot`；它不更新 form/UI/config，禁止 submit 后再从 control cache 读取
  selected/capability。

## 5. Rendering/style

`run_settings.rs`提供 ChatForm 调用的三个 render 函数，输入 state、`enabled: bool` 和 controller event sink；
Enabled/Disabled 使用同一 picker trigger/popover，Hidden 由 ChatForm 在调用前跳过。trigger size、popover
width/max-height、token footer 和 provider icon 与 render 函数共置，复用现有 Lightbulb、Shield、provider visuals
及 Fluent keys。

RunSettings模块不引用ControlSlot或ChatFormControls。调用方从`control_states()`取得 states，再构造 ChatForm
侧的 RunSettingsControls；state 内的 open-change handler 只作为 picker UI event sink，form、catalog 和联动逻辑仍
只由 RunSettingsController 持有，ChatForm 不再持有 controller entity。

model choices的唯一来源是 provider catalog typed snapshot，controller订阅 snapshot 变化并调用 projection
刷新；加载错误保留旧 catalog snapshot 和 form selected draft，只更新 error/unavailable presentation；空列表保持
model required，不自动选择不存在的值。model不可用时Shortcut状态优先显示`ModelUnavailable`；model可用但已保存
reasoning snapshot不被当前capability接受时显示`CapabilityMismatch`。

`shortcut_status`的判断顺序固定为：invalid hotkey -> hotkey conflict -> prompt unavailable -> model unavailable
-> capability mismatch -> registration failure -> enabled/disabled；只有provider/model都可用时才比较snapshot的
reasoning capability。

Shortcut和Conversation不得各自重新构造Select或popover。

## 6. Initial values与persistence adapters

- ChatInput conversation：只读取ChatFormConfig中显式保存且仍可解析的model/reasoning/approval；缺失或失效时保持
  `None`/原值并显示验证错误，不选择首个 enabled model 或默认权限。
- Create Shortcut：复制同一config中显式存在的偏好作为RunSettingsInput初值；没有偏好时保持未选择，之后只更新Shortcut form。
- Edit Shortcut：model来自Shortcut列，reasoning/approval来自settings snapshot；不可用model不自动替换为首个可用
  model，而是保留required状态等待用户选择；reasoning不兼容时保留原值并显示CapabilityMismatch，直到用户修改并成功保存。

ChatInputController可选择安装config persistence subscription；Shortcut controller绝不安装。共享controller本身不
引用ChatFormConfig。

## 7. Shortcut form/persistence

`ShortcutEditFormInput`删除独立model字段并增加：

```rust
#[form(group)]
pub(super) run_settings: RunSettingsInput,
```

`ShortcutDraft`增加reasoning_selection和approval_mode。保存从 parent form 当前 typed value 构造 draft，
`settings_snapshot_for_draft` fresh验证provider/model、capability和enabled状态后写
reasoning，并在default_tool_policy clone上只覆盖approval。父form的validation scope包含`run_settings` group，
child validator负责model/reasoning/approval，不能再读取已删除的顶层model field。DB schema、record和transaction
No change。

## 8. Capability mismatch与trigger

`ShortcutStatus::CapabilityMismatch`复用现有key。触发前以当前ProviderModelChoice校验snapshot reasoning；失败时不
创建Conversation/窗口。创建request使用snapshot reasoning和tool_policy.approval_mode，不再hard-code Ask。

## 9. 测试

- policy 覆盖全部 reasoning variant、computed default、bounds、step 和越界判定。
- component-owned value event、picker互斥、control state event、reasoning derived disabled。
- model confirm/reset reasoning/keep approval；catalog reload 不修改 selected value。
- 普通 ChatInput 与 Shortcut 都把 unavailable selected model 作为明确错误，不回退首个可用 model。
- ChatInput和Shortcut从同一config得到相同initial RunSettingsInput。
- Shortcut编辑不写ChatFormConfig。
- create/update/reopen覆盖reasoning variants与三种approval。
- invalid reasoning保存/触发均失败且DB/conversation不变。
- Conversation和Shortcut传给ChatForm的是同一组RunSettingsControlStates包装出的同类RunSettingsControls和相同debug
  selectors。

## 10. 后续所有权修正

本文件已经同步最终 RunSettings 所有权。Generated parent form store 持有 selected business values，owning
bound controls 持有交互状态和订阅，catalog snapshot 只提供 options/capability，`resolve_run_settings` 不回退
unavailable model。Token budget 使用 exact typed integer control；应用层没有字符串解析或独立 numeric draft。

## 11. 明确不做

- 不让ChatForm持有RunSettingsFormStore或controller。
- 不复制controller到调用方；调用方只决定slot availability和persistence adapter。
- 不持久化choices/capability/UI open state。
- 不增加 form-local selection/rebase/conflict API。Typed field handle 和 app-owned catalog projection已按 crate
  计划接入；不新增 form↔store 隐式依赖，不改 DB schema、icon 或 asset。
