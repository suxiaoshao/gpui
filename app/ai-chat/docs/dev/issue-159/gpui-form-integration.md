# Issue #159 ai-chat2 gpui-form 接入计划

本文档记录 `app/ai-chat2` Settings 表单接入 `gpui-form` 的 app 侧计划。`gpui-form` crate 自身只保留通用抽象；
Provider Settings、MCP Settings、具体 placeholder key、i18n、icon、config/DB 写回规则都放在本文档。

最后同步时间：2026-06-30。

当前状态：设计结论已确认；`gpui-form` 已支持字段级 placeholder/mask 传入 `InputState` 创建流程，
`ai-chat2` Provider Settings 已删除 post-creation `configure_inputs`，MCP Settings 已拆成语义化 row
input/store 并删除 `apply_*_placeholders`。MCP row add/remove 已改为 typed GPUI action，删除最后一行后保持空列表；
provider/MCP 单字段写入和外部错误写入已改用 generated setter / clear/apply error helper。`gpui-form` 已删除旧
`component = "custom"` 接入点，并提供
`#[form(binding = "...")]` / `FormComponentBinding<Value>` 给 app 自定义组件使用；内置 `input`、`number`、
`select`、`combobox` 和 bool 字段都已通过 binding 创建和同步组件 state。Provider/MCP 保存和校验路径已改为读取
generated form `draft()`，组件 state 只作为 UI 渲染和输入事件来源。`gpui-form` runtime 已整理为
`core` / `component` / `pipeline` / `view` 分组，宏展开逻辑也已按字段、访问器、数组、validation 和
pipeline 拆分；Provider/MCP 之外的 ai-chat2 表单候选已评估，下一批优先迁移 Prompt Edit Dialog 和
Shortcut Edit Dialog。

## 范围

本阶段只覆盖 `app/ai-chat2` Settings 内的表单：

- Provider Settings：provider 配置、secret 输入、base URL、custom OpenAI-compatible API mode。
- MCP Settings：Add/Edit dialog 的 stdio / streamable HTTP 基础字段、args、env、env vars、headers、
  env-backed headers、OAuth enabled 开关和 bearer token env var。

不在本阶段做：

- 改 `gpui-form` crate 的 app-specific 文档。
- 改 provider / MCP 持久化模型。
- 为表单新增 SQLite migration。
- 将表单状态接入 `gpui-store` 全局数据源。
- async validator。
- project-level MCP definitions、MCP advanced fields、ClientCredentials UI。

## Provider/MCP 之外的候选评估

本节记录 `app/ai-chat2` 中除 Provider Settings 和 MCP Settings 之外，哪些地方需要或可以继续接入
`gpui-form`。评估原则：

- 需要有明确的 edit draft、校验、提交或保存边界。
- 搜索框、filter 输入和单次菜单选择不默认视为表单。
- 运行态 composer / picker 只有在能明显减少状态同步复杂度时才迁移，避免为了统一而扩大 `gpui-form`
  的职责。

| 优先级 | 区域 | 当前状态 | 结论 |
| --- | --- | --- | --- |
| P1 | Prompt Edit Dialog | 手写 `name_input` / `content_input`、手写必填校验、单个 `validation_error` | 应迁移，收益明确 |
| P1 | Shortcut Edit Dialog | 手写 hotkey、prompt/model select、input source、enabled、单个 `validation_error` | 应迁移，但先补 `HotkeyInput` binding |
| P2 | General Settings HTTP proxy / temporary hotkey | auto-save 输入、inline hotkey 编辑 | 可以迁移，但不阻塞；复用 Shortcut 的 hotkey binding |
| P3 | ChatForm generation controls | composer、attachments、model/reasoning/approval picker、token budget、config auto-save 交织 | 暂缓，属于运行态复杂表单 |
| 不迁移 | search/filter inputs | Settings、Prompts、Shortcuts、Skills、MCP、sidebar/temporary search | 不是提交表单，不应接入 |
| 不迁移 | Appearance theme grid / theme mode | theme tile、dropdown/button 即时写配置；color picker 只是新增 material theme 的临时工具 | 暂不接入 |

### Prompt Edit Dialog

当前代码：

- `app/ai-chat2/src/features/settings/prompts/dialog.rs`
- `PromptEditDialogState` 持有 `name_input: Entity<InputState>`、`content_input: Entity<InputState>` 和
  `validation_error: Option<SharedString>`。
- 保存时读取两个 `InputState`，trim 后手写 `name/content required` 校验，再调用
  `state::prompts::create_prompt` 或 `state::prompts::update_prompt`。

目标文件结构：

```text
app/ai-chat2/src/features/settings/prompts.rs
  - 继续负责列表、搜索、打开 create/edit/preview/delete dialog

app/ai-chat2/src/features/settings/prompts/dialog.rs
  - PromptEditDialogState 持有 Entity<PromptEditFormStore>
  - dialog footer / focus / save flow 保留在 dialog
  - 不再直接持有 name_input / content_input / validation_error

app/ai-chat2/src/features/settings/prompts/form_state.rs
  - PromptEditFormInput
  - PromptEditFormStore
  - PromptContentInputBinding 或 multiline input options
  - PromptEditFormField 到 i18n error 的渲染 helper
```

目标类型：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate, validify::Validify)]
#[garde(allow_unvalidated)]
#[form(
    store = PromptEditFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
pub(super) struct PromptEditFormInput {
    #[form(
        component = "input",
        label = "prompt-field-name",
        placeholder = "prompt-placeholder-name",
        validate(on_change, on_blur, on_submit)
    )]
    #[modify(trim)]
    #[garde(length(min = 1))]
    pub name: String,

    #[form(
        binding = "PromptContentInputBinding",
        label = "prompt-field-content",
        placeholder = "prompt-placeholder-content",
        validate(on_change, on_blur, on_submit)
    )]
    #[modify(trim)]
    #[garde(length(min = 1))]
    pub content: String,
}
```

组件和 binding：

- `name` 使用内置 `TextInputBinding<String>`。
- `content` 当前需要 `InputState::multi_line(true).rows(10)`；现有 `ComponentStateOptions` 没有 multiline/rows。
  第一选择是在 app 侧新增 `PromptContentInputBinding: FormComponentBinding<String>`，不要为了一个字段先扩展
  `gpui-form` 通用 options。若后续多个 app 都需要 multiline，再把 multiline/rows 抽到 `gpui-form`。

数据流：

```text
PromptRecord?
  -> PromptEditFormInput
  -> PromptEditFormStore::from_value(...)
  -> submit runs validify trim + garde required
  -> create_prompt/update_prompt
  -> PromptCatalog refreshes list
```

全局数据和持久化：

- 不新增 `Global`。
- 不新增数据库 migration；仍写入现有 prompts 表。
- 搜索框 `PromptsSettingsPage::search_input` 不接入 `gpui-form`，它只是列表 filter。

icon / i18n / 依赖：

- save 继续用 `IconName::FilePen`，preview/edit/delete 继续用 `Pencil` / `Trash`。
- 复用现有 `prompt-field-*`、`prompt-placeholder-*`、`prompt-validation-*` key；如果改用 `garde` 通用错误，
  需要把 `name/content` 的 required error 映射回现有 prompt validation key，避免显示泛化文案。
- 如果 app form input derive `garde::Validate` / `validify::Validify`，按本文依赖计划补直接依赖。

### Shortcut Edit Dialog

当前代码：

- `app/ai-chat2/src/features/settings/shortcuts/dialog.rs`
- `ShortcutEditDialogState` 持有 `HotkeyInput`、`SelectState<Vec<PromptChoice>>`、
  `SelectState<SearchableVec<SelectGroup<ModelOption>>>`、`input_source`、`enabled` 和
  `validation_error`。
- 保存时手写读取 hotkey/select/switch，调用 `validate_shortcut_hotkey`，再组装 `ShortcutDraft`。

目标文件结构：

```text
app/ai-chat2/src/features/settings/shortcuts/dialog.rs
  - ShortcutEditDialogState 持有 Entity<ShortcutEditFormStore>
  - 继续负责 dialog footer、focus、save 后通知

app/ai-chat2/src/features/settings/shortcuts/form_state.rs
  - ShortcutEditFormInput
  - ShortcutEditFormStore
  - ShortcutHotkeyBinding
  - ShortcutPromptSelectBinding 或内置 SelectBinding 适配
  - ShortcutModelSelectBinding 或内置 SelectBinding 适配
  - ShortcutInputSource value setter helper

app/ai-chat2/src/features/settings/shortcuts/validation.rs
  - 保留 validate_shortcut_hotkey
  - 新增 ShortcutEditValidationIssue -> generated field enum 映射
```

目标类型：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ShortcutEditFormStore)]
pub(super) struct ShortcutEditFormInput {
    #[form(binding = "ShortcutHotkeyBinding", label = "shortcut-field-hotkey", validate(on_submit))]
    pub hotkey: Option<String>,

    #[form(
        component = "select",
        delegate = "Vec<PromptChoice>",
        options = "choices.prompts.clone()",
        label = "shortcut-field-prompt"
    )]
    pub prompt_id: Option<PromptId>,

    #[form(
        binding = "ShortcutModelSelectBinding",
        label = "shortcut-field-model",
        validate(on_submit)
    )]
    pub model: Option<ProviderModelKey>,

    #[form(component = "value")]
    pub input_source: ShortcutInputSource,

    #[form(component = "bool")]
    pub enabled: bool,
}
```

组件和 binding：

- `ShortcutHotkeyBinding` 负责 `Option<String> <-> HotkeyInput`，复用
  `HotkeyInput::current_hotkey_string()` 和 `string_to_keystroke()`。
- prompt select 如果 `SelectState<Vec<PromptChoice>>` 已满足内置 select binding 约束，就用内置 binding。
- model select 当前是 `SearchableVec<SelectGroup<ModelOption>>`，且有 search placeholder / grouped model options；
  若内置 select binding 无法表达，新增 `ShortcutModelSelectBinding`，不要把 picker 搜索行为塞进宏。
- `input_source` 可以先用 value field + generated `set_input_source_value(...)`，render 中的 `ToggleGroup`
  on_click 调 setter；等多个地方复用 segmented control 后再考虑 binding。
- `enabled` 使用 bool field；render 可以继续使用 `Switch`。

数据流：

```text
ShortcutRecord? + ShortcutDialogChoices + existing ShortcutRecord list + temporary hotkey
  -> ShortcutEditFormInput
  -> ShortcutEditFormStore::from_value(...)
  -> submit reads form draft
  -> validate_shortcut_hotkey + model required app validation
  -> ShortcutDraft
  -> state::shortcuts::create_shortcut/update_shortcut
```

全局数据和持久化：

- 不新增 `Global`。
- 不新增数据库 migration；仍写入 shortcuts 表。
- `existing_shortcuts`、`temporary_hotkey`、prompt/model choices 是 dialog validator context，不进入 form output。
- `ShortcutsSettingsPage::search_input` 继续只是列表 filter，不接入 `gpui-form`。

icon / i18n / 依赖：

- save 继续用 `IconName::Keyboard`，delete 继续用 `Trash`。
- 复用现有 `shortcut-field-*`、`shortcut-validation-*`、`chat-form-model-search-placeholder`。
- `validate_shortcut_hotkey` 依赖 `global_hotkey` canonical check，继续留在 app validator。

### General Settings

当前代码：

- `app/ai-chat2/src/features/settings/general.rs`
- `app_http_proxy_input` 持有 `InputState` 和手写 subscription，change 时直接写 `AiChat2Config`。
- `TemporaryHotkeyControlState` 持有 `HotkeyInput`，save 时更新 runtime hotkey 和 config。
- language dropdown 是菜单即时写配置。

迁移判断：

- HTTP proxy 可以接 `gpui-form`，但收益中等：它是 auto-save single-field form，没有 submit 按钮。若迁移，
  使用 `GeneralHttpProxyFormInput { http_proxy: Option<String> }`，订阅 typed form event 后写 config，并保留
  `last_value` 或用 form meta/revision 防止重复写入。
- temporary hotkey 可以在 `ShortcutHotkeyBinding` 落地后复用同一 binding，目标类型是
  `TemporaryHotkeyFormInput { hotkey: Option<String> }`。保存仍必须先更新 runtime global hotkey，再写 config；
  rollback 逻辑留在 app，不进入 `gpui-form`。
- language dropdown 不迁移：它不是 draft + submit 表单。

### Appearance Settings

当前代码：

- `app/ai-chat2/src/features/settings/appearance.rs`
- `ColorPickerState` 只用于选择新增 material theme 的颜色；theme mode 和 theme tile 都是点击后立即写 config。

迁移判断：

- 暂不接入 `gpui-form`。
- 如果未来出现“编辑 theme form / preview 后 submit”的需求，再新增 `ColorPickerBinding`。
- 当前 theme grid、delete custom material theme、mode button 都是 action-driven 设置，不适合作为 form store。

### ChatForm

当前代码：

- `app/ai-chat2/src/components/chat_form.rs`
- 同时管理 `ComposerEditor`、attachments、model choices、reasoning/approval picker、token budget input、agent
  running 状态和 config auto-save。
- 提交时重新校验 selected model 是否仍存在，并把 composer snapshot + attachments + model + reasoning +
  approval 组装为 `ChatFormSubmit`。

迁移判断：

- 暂缓，不和 Settings 表单同批迁移。
- 可以长期考虑拆出 `ChatFormControlsInput`，只覆盖 model / reasoning / approval / token budget；`ComposerEditor`
  和 attachments 仍留在 `ChatForm`。
- 真要迁移，需要先设计这些 binding：
  - `ComposerEditorBinding` 或明确不纳入 form。
  - `ModelPickerBinding`，支持动态 provider/model choices 和 load error empty state。
  - `ReasoningSelectionBinding`，支持 capability-dependent options 和 token budget bounds。
  - `ApprovalModeBinding`。
  - bounded number/text binding，用于 token budget clamp 和 step。
- 这些 binding 都有强业务语义，不应作为当前 `gpui-form` 通用能力提前实现。

## 已确认设计结论

- domain/input struct 是最终提交结构；form store 是编辑期 draft state。输入时不直接修改 `ProviderDraft`
  或 `McpServerTomlConfig`。
- 保存、校验、dirty snapshot 和 config 写回都从 generated form `draft()` 或 `ProviderSettingsForm` /
  `McpServerFormDraft` 暴露的 typed draft API 读取业务数据；app 不再通过 `InputState` / `SelectState`
  反向拼提交结构。
- `#[derive(FormStore)]` 生成 store、字段 store、字段枚举、事件、typed accessors 和 array helpers。
- `#[derive(FormStore)]` 生成的 array helpers 包含 `field_remove_id(row_id, cx)` 和
  `field_values_with_id()`；app 不再先把 row id 转 index，也不再维护自己的 row value DTO。
- `#[derive(FormStore)]` 生成 `set_field_value(...)`、`clear_all_errors(...)`、
  `clear_field_errors(...)` 和 `apply_field_error(...)`；app-specific validator 只负责映射到具体 generated
  field enum。
- app 不再手写类似 `server_id_input()`、`command_input()`、`provider_form_input_state()` 这类重复 getter；
  由宏生成 `field_input_state()`、`field_select_state()`、`field_value()` 等访问器。
- app 不直接订阅每个 `InputState` 再反查字段；`gpui-form` 安装并保存组件订阅，app 只订阅 typed form event
  处理业务副作用。
- 表单 store 需要是 `Entity<GeneratedFormStore>`，因为 GPUI 事件、observe、component subscription
  生命周期需要挂在 entity 上；字段 store 是 form store 的普通字段；`InputState` / `SelectState`
  等组件状态仍是独立 `Entity`。
- 所有 subscriptions 必须保存在 form / field / array item / page 中；不要用 `.detach()` 隐藏生命周期。
- `FormItemId(u64)` 是动态数组 row 的运行时 identity，只在当前表单生命周期内稳定，不写入 DB、
  `config.toml` 或 domain output。
- `FieldPath` 不作为用户侧字段 API。app 侧不要依赖 `FormField::path()`；路径只用于 `gpui-form`
  内部 validation scope 和 report routing。
- 组件接入统一使用 binding 模型。`Input`、`Select`、`Combobox` 和 app 自己的组件都应走同一套
  `FormComponentBinding<Value>` 目标接口；不再保留特殊的 `custom` 分支作为设计目标。
- placeholder、label、description、mask、search placeholder 等 UI 选项应写在字段宏属性上，由 binding
  `new_state` 创建组件 state 时应用。app 不再创建表单后调用 `configure_inputs`、
  `apply_mcp_form_placeholders` 或 `apply_key_value_placeholders` 修正 placeholder。
- validation 交给 `garde`；normalize/sanitize 交给 `validify::Modify`。`gpui-form` 不重复实现专业规则库。
- submit 被点击后，无论后续校验成功还是失败，都先把 normalized value 写回 form draft 和组件 state。

## 目标模块结构

Provider Settings：

```text
app/ai-chat2/src/features/settings/provider.rs
  - ProviderSettingsPage 拥有 ProviderSettingsForm enum
  - 订阅 typed form events，只处理 secret dirty、外部错误清理、保存按钮状态等业务副作用
  - 不再维护动态 field schema 或 input map

app/ai-chat2/src/features/settings/provider/forms.rs
  - ProviderSettingsForm enum
  - ProviderFormField app-level 错误展示枚举
  - ProviderValidationIssue / ProviderSecretFieldValue
  - domain draft <-> typed form input 转换
  - form submit -> ProviderDraftValue / ProviderSecretRefs 转换

app/ai-chat2/src/features/settings/provider/forms/api_key.rs
  - ApiKeyProviderFormInput
  - ApiKeyProviderFormStore

app/ai-chat2/src/features/settings/provider/forms/ollama.rs
  - OllamaProviderFormInput
  - OllamaProviderFormStore

app/ai-chat2/src/features/settings/provider/forms/custom_openai.rs
  - CustomOpenAiProviderFormInput
  - CustomOpenAiProviderFormStore
  - ProviderApiMode
  - ApiModeChoice
```

MCP Settings：

```text
app/ai-chat2/src/features/settings/mcp/form_state.rs
  - McpServerFormInput / McpServerFormStore
  - semantic row input/store types
  - McpServerFormDraft
  - config.toml <-> form input 转换
  - row 删除使用宏生成 `*_remove_id(row_id, cx)`
  - row validator 使用宏生成 `*_values_with_id()`
  - 不再保留 McpStringRowInput / McpKeyValueRowInput 这类跨语义复用 row

app/ai-chat2/src/features/settings/mcp/form_rows.rs
  - `McpRowList`
  - `AddMcpRow` / `RemoveMcpRow`
  - `one_input_rows(...)`
  - `two_input_rows(...)`
  - `validation_error_list(...)`
  - 只保留无状态布局 helper；不保留 `McpRowsView` 或 row handle struct

app/ai-chat2/src/features/settings/mcp/validation.rs
  - MCP app-specific validator
  - 使用 `*_values_with_id()` 读取 row id + row draft snapshot 定位错误
  - 短期可保留 McpFormField 作为 dialog 渲染适配层，但不要让它驱动 form state

app/ai-chat2/src/features/settings/mcp/dialog.rs
  - McpServerDialog 拥有 Entity<McpServerFormStore>
  - 订阅 form event，触发保存按钮状态和错误清理
  - 顶层 `.on_action` 处理 `AddMcpRow` / `RemoveMcpRow`
  - add/remove/reorder 调用宏生成的 array helpers；删除最后一项不自动补空行
```

## Provider 表单设计

每个 provider kind 使用确定的表单类型，不使用动态 field schema。

API-key provider：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate, validify::Validify)]
#[form(
    store = ApiKeyProviderFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
pub(super) struct ApiKeyProviderFormInput {
    #[form(component = "bool")]
    pub enabled: bool,

    #[form(
        component = "input",
        label = "provider-field-api-key",
        placeholder = "provider-placeholder-api-key",
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    #[modify(trim)]
    pub api_key: String,

    #[form(
        component = "input",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-base-url-default",
        validate(on_blur, on_submit)
    )]
    #[modify(trim)]
    pub base_url: String,
}
```

Ollama provider：

```rust
pub(super) struct OllamaProviderFormInput {
    #[form(component = "bool")]
    pub enabled: bool,

    #[form(component = "input", label = "provider-field-base-url", placeholder = "provider-placeholder-ollama-base-url")]
    pub base_url: String,

    #[form(component = "input", label = "provider-field-bearer-token", placeholder = "provider-placeholder-bearer-token", mask)]
    pub bearer_token: String,
}
```

Custom OpenAI-compatible provider：

```rust
pub(super) struct CustomOpenAiProviderFormInput {
    #[form(component = "bool")]
    pub enabled: bool,

    #[form(component = "input", label = "provider-field-name", placeholder = "provider-placeholder-provider-name")]
    pub name: String,

    #[form(component = "input", label = "provider-field-api-key", placeholder = "provider-placeholder-api-key", mask)]
    pub api_key: String,

    #[form(component = "input", label = "provider-field-base-url", placeholder = "provider-placeholder-custom-base-url")]
    pub base_url: String,

    #[form(
        component = "select",
        delegate = "Vec<ApiModeChoice>",
        options = "localized_api_mode_choices(cx.global::<crate::foundation::I18n>())",
        label = "provider-field-api-mode",
        placeholder = "provider-placeholder-api-mode",
        validate(on_change, on_submit)
    )]
    pub api_mode: ProviderApiMode,
}
```

Provider app-specific validation：

- `api_key` / `bearer_token` 是否必填需要结合 `enabled`、saved secret ref、dirty/cleared secret 状态判断。
- `base_url` 非空时必须是 URL；custom OpenAI-compatible 的 `base_url` 必填。
- `name` 对 custom provider 必填并 trim。
- `api_mode` 必须是 `responses` 或 `chat_completions`。
- validation issue 必须落到具体字段：`Name`、`ApiKey`、`BaseUrl`、`BearerToken`、`ApiMode`。
- secret 原文只存在于 input state / dirty secret value 中；DB 只保存 `ProviderSecretRefs`，keychain
  只在保存时写入 dirty secret，删除/清空时删除对应 credentials。

Provider 需要删除或迁移的旧痕迹：

- `ProviderFieldSchema`、`ProviderSettingsFormField` 这类动态字段 schema。
- `provider_form_input_state`、`provider_form_input_keys` 这类动态 getter。
- `secret_inputs: BTreeMap<...>`；secret 应是对应 form 字段。
- `ProviderSettingsForm::configure_inputs`；placeholder/mask/select options 由字段宏属性和 binding 处理。
- 使用 `FieldPath` 作为 app 侧错误定位 API。

## MCP 表单设计

MCP 动态行必须按业务语义拆分。不能复用一个 `McpKeyValueRowInput`，再由父字段动态覆盖 placeholder。

目标 row：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpArgRowFormStore)]
pub(super) struct McpArgRowInput {
    #[form(component = "input", placeholder = "mcp-placeholder-arg", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvVarRowFormStore)]
pub(super) struct McpEnvVarRowInput {
    #[form(component = "input", placeholder = "mcp-placeholder-env-var", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvRowFormStore)]
pub(super) struct McpEnvRowInput {
    #[form(component = "input", placeholder = "mcp-placeholder-env-key", validate(on_change, on_blur, on_submit))]
    pub key: String,

    #[form(component = "input", placeholder = "mcp-placeholder-env-value", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpHeaderRowFormStore)]
pub(super) struct McpHeaderRowInput {
    #[form(component = "input", placeholder = "mcp-placeholder-header-name", validate(on_change, on_blur, on_submit))]
    pub name: String,

    #[form(component = "input", placeholder = "mcp-placeholder-header-value", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvHeaderRowFormStore)]
pub(super) struct McpEnvHeaderRowInput {
    #[form(component = "input", placeholder = "mcp-placeholder-header-name", validate(on_change, on_blur, on_submit))]
    pub name: String,

    #[form(component = "input", placeholder = "mcp-placeholder-env-header-var", validate(on_change, on_blur, on_submit))]
    pub env_var: String,
}
```

目标 server form：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpServerFormStore)]
pub(super) struct McpServerFormInput {
    pub transport: McpTransportKind,

    #[form(component = "input", label = "mcp-field-server-id", placeholder = "mcp-placeholder-server-id")]
    pub server_id: String,

    #[form(component = "input", label = "mcp-field-command", placeholder = "mcp-placeholder-command")]
    pub command: String,

    #[form(component = "input", label = "mcp-field-cwd", placeholder = "mcp-placeholder-cwd")]
    pub cwd: String,

    #[form(component = "array", store = "McpArgRowFormStore")]
    pub args: Vec<McpArgRowInput>,

    #[form(component = "array", store = "McpEnvRowFormStore")]
    pub env: Vec<McpEnvRowInput>,

    #[form(component = "array", store = "McpEnvVarRowFormStore")]
    pub env_vars: Vec<McpEnvVarRowInput>,

    #[form(component = "input", label = "mcp-field-url", placeholder = "mcp-placeholder-url")]
    pub url: String,

    #[form(component = "input", label = "mcp-field-bearer-token-env-var", placeholder = "mcp-placeholder-bearer-token-env-var")]
    pub bearer_token_env_var: String,

    #[form(component = "array", store = "McpHeaderRowFormStore")]
    pub headers: Vec<McpHeaderRowInput>,

    #[form(component = "array", store = "McpEnvHeaderRowFormStore")]
    pub env_headers: Vec<McpEnvHeaderRowInput>,

    #[form(component = "bool")]
    pub oauth_enabled: bool,
}
```

MCP app-specific validation：

- `server_id` 必填、格式为 `[A-Za-z0-9_-]+`、同一 config 中唯一。
- stdio transport：`command` 必填；`args` 不能是纯空白行；`cwd` 如果非空必须有效。
- env key / env var / bearer token env var 必须符合 `[A-Za-z_][A-Za-z0-9_]*`；env key 不重复。
- streamable HTTP：`url` 必填且 scheme 为 `http` / `https`。
- header name/value 使用 `http` crate 校验；reserved headers 不能配置；header name 不重复。
- header row 必须 name/value 同时填写；env-backed header 必须 name/env_var 同时填写。
- 错误定位使用 `FormItemId` + row field，不能依赖当前 index；reorder 后错误仍应落到同一行。

MCP 需要删除或迁移的旧痕迹：

- `McpStringRowInput`、`McpKeyValueRowInput`。
- `StringListField`、`KeyValueField`。
- `StringListDraftRow`、`KeyValueDraftRow` 的泛型 row view 输入。
- `apply_mcp_form_placeholders`、`apply_key_value_placeholders`、`set_input_placeholder`。
- dialog 根据 `McpFormField` 动态匹配泛型 row 的渲染方式；目标改为 typed row field 渲染。

## 数据流

Provider Settings：

```text
DB provider row + model rows + ProviderCatalog + GPUI credentials existence
  -> ProviderDraft
  -> ProviderFormKind selects concrete form input
  -> cx.new(|cx| ConcreteProviderFormStore::from_value(input, window, cx))
  -> gpui-form owns draft value + component state + subscriptions
  -> component event updates field store and emits typed form event
  -> ProviderSettingsPage handles business side effects only
  -> submit runs validify::Modify and writes normalized value back
  -> garde + app-specific validation returns field errors or output
  -> ProviderDraftValue + ProviderSecretFieldValue
  -> repository/config/keychain save
```

MCP Settings：

```text
config.toml [mcp_servers.<id>]
  -> McpServerTomlConfig
  -> McpServerFormInput
  -> cx.new(|cx| McpServerFormStore::from_value(input, window, cx))
  -> array helpers manage FormItemId + row stores + subscriptions
  -> submit normalizes draft and validates
  -> McpServerTomlConfig
  -> AiChat2ConfigStore writes config.toml
```

## 数据监听和生命周期

- `ProviderSettingsPage` 持有当前 `ProviderSettingsForm` 和对应 form subscription。
- 切换 provider 时 drop 旧 form entity 和 subscription，创建新 typed form。
- `McpServerDialog` / `McpServerFormDraft` 持有 `Entity<McpServerFormStore>`。
- add/remove/reorder dynamic row 只调用宏生成 helpers；remove 时对应 row store 和 subscriptions 被 drop；
  reorder 不重建仍存在的 row state。删除最后一项后数组可以为空，由 add action 再创建新行。
- app 可以订阅 form event 做这些业务副作用：
  - 清除保存/测试产生的外部错误。
  - 更新 dirty badge / Save button enabled。
  - 标记 secret field dirty 或 cleared。
- app 不应订阅 `InputState` 来同步 draft value；这是 `gpui-form` 的职责。
- 不在 render 中创建 subscription。
- 不在 form event 里嵌套 update 同一个 entity。

## 全局数据管理和持久化

- 表单状态只属于打开它的 settings page/dialog。
- 不使用 `Global` 存所有活跃表单。
- 不把表单 draft 写入 `gpui-store`。
- Provider 成功 submit 后才写 fresh DB provider row 和 GPUI credentials；失败时只更新表单错误。
- MCP 成功 submit 后才写 `config.toml`；失败时只更新表单错误。
- `FormItemId` 不写入 DB / TOML / keychain。
- 本计划不新增数据库 migration。

## 数据获取方式

Provider：

- provider list 从 `ProviderCatalog` + fresh DB provider rows 派生。
- model list / refresh 不属于表单状态；仍由 provider settings 现有 model fetch flow 处理。
- secret 原文只在保存/fetch 需要时从 GPUI credentials 读取，不进入 form initial value；form initial
  value 只表达当前 input 中用户可编辑的 dirty secret 文本。

MCP：

- initial value 从 `AiChat2ConfigStore` 当前 config snapshot 构造。
- OAuth credentials 从 GPUI credentials 读取，不进入 form value。
- `state::mcp` runtime status / tools list 只用于 detail/status 展示，不作为 Add/Edit form source of truth。

## 组件、icon 和 i18n

gpui-component：

- `Input` / `InputState`：provider text/secret/base URL，MCP text fields 和 row fields。
- `Select` / `SelectState<Vec<ApiModeChoice>>`：custom OpenAI-compatible API mode。
- `Switch` 或 `Checkbox`：enabled / OAuth enabled。
- `Button`：save、add row、remove row、refresh/test。
- `Tag` / `Label`：validation/status summary。
- `ScrollableElement`：MCP dialog body 和 settings detail body。

Icon：

- 不为表单新增 icon asset。
- 复用 app-local `IconName`：
  - `Plus`：add row / add server。
  - `Trash`：remove row / delete。
  - `RefreshCcw`：refresh/test/retry。
  - `CircleAlert`：error state。
  - `CircleCheck` / `Check`：success/saved state。
- 如果后续做 reorder handle，再补 `GripVertical`；没有 UI 需求前不新增。

i18n：

- Provider 已有 keys：`provider-field-*`、`provider-placeholder-*`、`provider-api-mode-*`。
- MCP 已有 row-level keys：`mcp-placeholder-arg`、`mcp-placeholder-env-*`、
  `mcp-placeholder-header-*`；顶层字段也应使用独立 placeholder key，例如
  `mcp-placeholder-server-id`、`mcp-placeholder-command`、`mcp-placeholder-cwd`、
  `mcp-placeholder-url`、`mcp-placeholder-bearer-token-env-var`，不要继续把
  `mcp-field-*` label key 当 placeholder 使用。
- MCP 已有 label/action/validation key family：`mcp-field-*`、`mcp-validation-*`、
  `mcp-action-add-*`。
- 字段 label/placeholder/search placeholder 写在宏属性中，binding 通过 app 的 `I18n` resolver 写入组件 state。
- 新增字段或 row 类型时，同步补齐 `app/ai-chat2/locales/en-US/main.ftl` 和
  `app/ai-chat2/locales/zh-CN/main.ftl`。
- `gpui-form` error code 到 Fluent key 的通用映射如果不足，app 侧只补 key/params mapping，不在 UI
  里拼接最终文案。

## 依赖计划

当前 `app/ai-chat2` 已依赖：

```toml
gpui-form.workspace = true
```

目标如果 app form input 直接 derive `garde::Validate` / `validify::Validify`，需要在 `app/ai-chat2/Cargo.toml`
新增直接依赖，版本必须完整：

```toml
garde = { version = "0.23.0", default-features = false, features = ["derive", "url", "email", "pattern"] }
validify = "2.0.0"
```

如果只通过 `gpui-form` adapter 执行但 app 不直接使用 derive，则不新增 app 直接依赖。实现前按实际宏展开
要求确认一次，避免无用依赖。

`gpui-form` 需要启用 pipeline 时使用 workspace/path dependency feature：

```toml
gpui-form = { workspace = true, features = ["form-pipeline"] }
```

MCP header validation 当前需要 `http` crate；如果 validation 模块直接解析 `HeaderName` / `HeaderValue`，
继续使用现有 app dependency，不为 form integration 新增其它解析库。

## 迁移步骤

1. 已完成：完成 `gpui-form` binding 目标 API：`FormComponentBinding`、field UI options、placeholder resolver、
   移除特殊 `custom` 设计入口。当前已完成 placeholder resolver、input placeholder/mask 初始化和
   app 自定义组件的 `binding` 接入；内置 `input`、`number`、`select`、`combobox` 和 bool 字段已通过
   binding 创建和同步组件 state。
2. 已完成：Provider forms 迁移字段宏属性：把 `configure_inputs` 中的 placeholder/mask/select options
   移到字段定义。
3. 已完成：Provider page 删除动态 input lookup 和 `ProviderFieldSchema` 旧痕迹，只保留 typed form enum。
4. 已完成：MCP row 拆分为 `McpArgRowInput`、`McpEnvVarRowInput`、`McpEnvRowInput`、`McpHeaderRowInput`、
   `McpEnvHeaderRowInput`。
5. 已完成：MCP row rendering 改为无状态 helper + typed row action，删除 `McpRowsView`、row handle struct
   和 `apply_*_placeholders`。
6. 已完成：MCP dialog add/remove 全部走宏生成 array helpers；remove 使用 `*_remove_id`，删除最后一行后不自动补行。
7. 已完成：provider/MCP 单字段写入使用 generated `set_field_value`，不再读整份 draft 再 patch。
8. 已完成：provider app-specific validator 的错误映射到具体 generated field enum，并通过 generated
   `clear_all_errors` / `apply_field_error` 写回。
9. 已完成：删除 app 侧 `InputState` 订阅同步 draft 的逻辑，只保留 form event 业务订阅。
10. 已完成：Provider/MCP 保存、校验和测试 helper 改为以 generated form `draft()` 为业务数据源，组件 state
   不再作为提交结构的读取来源。
11. 已完成：补齐当前实现所需 i18n key 使用、test 和 focused checks。
12. 下一批 P1：迁移 Prompt Edit Dialog。
    - 新增 `app/ai-chat2/src/features/settings/prompts/form_state.rs`。
    - 新增 `PromptEditFormInput` / `PromptEditFormStore`。
    - 新增 `PromptContentInputBinding`，只解决 prompt content 的 multiline/rows，不扩展通用宏参数。
    - 保存路径改为 `form.submit(...) -> PromptEditFormInput -> create_prompt/update_prompt`。
    - 删除 dialog 里的 `name_input`、`content_input` 和单个 `validation_error`。
13. 下一批 P1：迁移 Shortcut Edit Dialog。
    - 新增 `app/ai-chat2/src/features/settings/shortcuts/form_state.rs`。
    - 新增 `ShortcutEditFormInput` / `ShortcutEditFormStore`。
    - 新增 `ShortcutHotkeyBinding`；确认 prompt/model select 是否能复用内置 select binding，不能则新增 app-local
      `ShortcutModelSelectBinding`。
    - `input_source` 先作为 value field，用 generated setter 接 `ToggleGroup`。
    - `validate_shortcut_hotkey` 保留在 app validator，错误通过 generated `apply_field_error` 回填字段。
14. 可选 P2：在 Shortcut 的 hotkey binding 稳定后，再评估 General Settings 的 temporary hotkey inline editor。
15. 可选 P2：HTTP proxy 输入可以迁移为 single-field auto-save form，但必须保留 config 写入失败时不覆盖
    `last_value` 的语义。
16. 暂缓 P3：ChatForm controls 需要单独设计，不和 Settings 表单迁移混做。

## 验证计划

文档/格式：

- `git diff --check`
- `cargo fmt`

crate：

- `cargo test -p gpui-form --features form-pipeline`
- `cargo clippy -p gpui-form-macros -p gpui-form --features form-pipeline --all-targets -- -D warnings`

app：

- `cargo check -p ai-chat2`
- `cargo test -p ai-chat2 provider`
- `cargo test -p ai-chat2 mcp`
- 迁移 Prompt Edit Dialog 后：`cargo test -p ai-chat2 prompt`
- 迁移 Shortcut Edit Dialog 后：`cargo test -p ai-chat2 shortcut`
- `cargo clippy -p ai-chat2 --all-targets -- -D warnings`

行为测试重点：

- Provider secret dirty / cleared / saved secret unchanged。
- Provider base URL trim 后无论 submit 成功失败都写回 input。
- Custom OpenAI API mode 通过 `SelectState` 正常读写。
- MCP add/remove/reorder 后 row input state 不错位。
- MCP env/header duplicate error 在 reorder 后仍落到同一 `FormItemId`。
- 删除 row 后对应 subscription 不再触发。
- 所有 placeholder 来自字段宏属性，不需要 post-creation patch helper。
- Prompt edit：name/content trim 后写回 form；空 name/content 错误落到具体字段；create/edit 仍写入 prompts 表。
- Shortcut edit：hotkey required/invalid/conflict 错误落到 hotkey 字段；model required 错误落到 model 字段；
  prompt optional；input source 和 enabled 能正确写入 `ShortcutDraft`。
