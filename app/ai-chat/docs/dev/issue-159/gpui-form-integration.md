# Issue #159 ai-chat2 gpui-form 接入计划

本文档记录 `app/ai-chat2` Settings 表单接入 `gpui-form` 的 app 侧计划。`gpui-form` crate 自身只保留通用抽象；
Provider Settings、MCP Settings、具体 placeholder key、i18n、icon、config/DB 写回规则都放在本文档。

更新口径：本文保留历史迁移记录；“字段级校验必须进入 `gpui-form` validation/transform pipeline，app 不再手动
回填 field errors”的完整迁移计划见 `gpui-form-full-migration-plan.md`。`gpui-form` crate 侧通用能力计划见
`../../../../../crates/gpui-form/docs/validation-pipeline-strengthening-plan.md`。

最后同步时间：2026-07-03。

当前实现口径：完整迁移已由 `gpui-form-full-migration-plan.md` 接管并落地。Provider、MCP、Prompt 和
Shortcut 的字段级校验/required/normalize 不再由 app submit handler 执行，也不再通过
`SubmitError::Handler(...)` 手动回填 field errors；保存路径只在 `SubmitError::Invalid(FormValidationReport)`
之外处理 DB/config/keychain/runtime 副作用。本文后续较早章节保留历史迁移记录，若出现
`apply_field_error`、`clear_all_errors`、`McpSubmitRowIds` 或 `validate_mcp_submit_output` 方案描述，以
`gpui-form-full-migration-plan.md`、`crates/gpui-form/docs/validation-pipeline-strengthening-plan.md` 和当前代码为准。

历史状态：设计结论已确认；`gpui-form` 已支持字段级 placeholder/mask/required 传入组件 state 创建流程，
`ai-chat2` Provider Settings 已删除 post-creation `configure_inputs`，MCP Settings 已拆成语义化 row
input/store 并删除 `apply_*_placeholders`。MCP row add/remove 已改为 typed GPUI action，删除最后一行后保持空列表；
Provider/MCP 单字段写入已改用 generated setter；Provider、Prompt Edit Dialog 和 Shortcut Edit Dialog 的
保存前字段错误已通过 generated clear/apply error helper 写回 `gpui-form` field。`gpui-form` 已删除旧
`component = "custom"` 接入点，并提供
`#[form(binding = "...")]` / `FormComponentBinding<Value>` 给 app 自定义组件使用；`gpui-form` core 已不再内置
`gpui-component` binding，`ai-chat2` 显式依赖 `gpui-form-gpui-component` adapter crate 或继续提供
app-local binding。Provider/MCP/Prompt/Shortcut
的提交数据源都已改为 generated form submit output；组件 state 只作为 UI 渲染和输入事件来源。
Provider/MCP 的半迁移点已收敛：保存 task/loading 由 form submit runtime 持有，contextual validator、
save request 构造、dirty snapshot 和 secret writes/refs 都从 submit output 或 current output 的同一套
映射生成。MCP Add/Edit dialog
保存前 validator issues 已映射为 generated top-level / row form field errors；dialog 不再持有平行
`validation_errors`，summary 和行内错误都从 form field `visible_errors` 派生。`gpui-form` runtime 已整理为
`core` / `component` / `pipeline` / `view` 分组，宏展开逻辑也已按字段、访问器、数组、validation 和
pipeline 拆分；`gpui-form` 已补齐字段级 required 元数据、generated required helper 和 binding 同步能力。
Prompt Edit Dialog 已迁移到 `PromptEditFormStore`，Shortcut Edit Dialog 已迁移到
`ShortcutEditFormStore`；字段验证梳理范围按本文“全量字段验证审计口径”覆盖所有相关表单面，不限制在
Provider/MCP。`gpui-form` 已收口 meta/submit 状态模型：`FieldMeta` / `FormMeta` 不再保存
`is_valid` 或 `can_submit` 这类合法性/提交能力第二事实源；Settings 保存流程只能依据 field errors、
app validator 结果、`prepare_submit(...)` 结果和 sync/async submit handler outcome 决定是否继续。
`gpui-form` 的目标 submit 模型已调整为：handler 在点击提交时传入，form store 持有 submit runtime/task，
`is_submitting` 从 task 是否存在派生；handler API 使用 `FnOnce(Output, &mut Window, &mut App)` /
`FnOnce(Output, &mut Window, &mut App) -> Result<Task<_>, StartError>`，不新增 submit handler trait。
number 字段的后续通用修复以 `gpui-form` 的
`binding-architecture.md` 为准：number 是 `State -> Draft -> Result<Value, FieldError>`
模型中的 `Draft = String` 场景，dirty/default 以 draft 为基准，不再作为长期特殊 store。
`gpui-form` derive 宏生成边界已收口：组件 subscription 由 binding 自己安装，宏只提供具体字段的 form
event sink；`ai-chat2` 的 app-local binding 已删除 `type Event` / `event_kind(...)`，改到
`install_subscriptions(state, sink, window, cx)` 内订阅自己的组件事件。

## gpui-form binding 架构调整对 ai-chat2 的影响

`gpui-form` core 当前是 UI-library agnostic：

- `gpui-form` 不再默认依赖 `gpui-component`。
- 所有 leaf field 使用 Draft-aware `ComponentFieldStore<Value, Binding>`。
- `Input`、`NumberInput`、`Select`、`Combobox`、`Switch` / `Checkbox` 的 binding 移到
  `crates/gpui-form-gpui-component`。
- `ai-chat2` 的业务组件，例如 `PromptContentInputBinding`、`ShortcutHotkeyBinding`、
  `ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding`，继续放在 app 侧，因为它们依赖 app
  业务 snapshot、hotkey 规则、provider/model choices 或专用 UI。
- subscription 由 binding 拥有：这些 app-local binding 不再依赖宏生成
  `cx.subscribe_in(&state, ...)`，而是在自身 `install_subscriptions(...)` 内把组件事件映射到
  `FormComponentEvent` 并调用 sink。

迁移后字段声明：

```rust
type ProviderNameBinding = gpui_form_gpui_component::TextInputBinding<String>;
type ProviderBaseUrlBinding = gpui_form_gpui_component::TextInputBinding<Option<String>>;
type ProviderEnabledBinding = gpui_form_gpui_component::BoolBinding;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ProviderFormStore)]
struct ProviderFormInput {
    #[form(binding = "ProviderNameBinding", required)]
    name: String,

    #[form(binding = "ProviderBaseUrlBinding")]
    base_url: Option<String>,

    #[form(binding = "ProviderEnabledBinding")]
    enabled: bool,
}
```

这样宏只依赖 `ComponentFieldStore::new(...)` 这个 core contract，不依赖用户或 adapter store 的 inherent
`new`。如果某个字段需要 `gpui-component` 特定 render helper，helper 从 adapter crate 或 app render 层获取，
不由 `gpui-form` core 宏生成。

## 范围

本计划覆盖 `app/ai-chat2` 中已经接入或准备评估接入 `gpui-form` 的表单面：

- Provider Settings：provider 配置、secret 输入、base URL、custom OpenAI-compatible API mode。
- MCP Settings：Add/Edit dialog 的 stdio / streamable HTTP 基础字段、args、env、env vars、headers、
  env-backed headers、OAuth enabled 开关和 bearer token env var。
- Prompt Edit Dialog：prompt name/content。
- Shortcut Edit Dialog：hotkey、prompt/model、input source、enabled。
- General Settings HTTP proxy / temporary hotkey：P2 候选，记录验证边界，不阻塞 P1。
- Appearance Settings：theme mode、theme tiles、custom material theme color picker 的即时配置边界。
- Projects Settings：目录选择、project row、DB project 约束边界。
- Skills Settings：search/filter、refresh、skill content load 的非表单边界。
- NewConversation / project selector：default project config、project existence、ChatForm submit 边界。
- ChatForm generation controls：P3 候选，记录运行态 submit/config 边界，不和 Settings 表单同批迁移。

不在本阶段做：

- 改 `gpui-form` crate 的 app-specific 文档。
- 改 provider / MCP 持久化模型。
- 为表单新增 SQLite migration。
- 将表单状态接入 `gpui-store` 全局数据源。
- async validator。
- project-level MCP definitions、MCP advanced fields、ClientCredentials UI。
- 把搜索/filter、theme grid、language dropdown、即时 action button 强行改成 form store。

## 表单边界和迁移完成度口径

判断某个页面是否已经“由 `gpui-form` 重构”，只看表单层职责，不把业务保存链路算进去。

`gpui-form` 表单层职责：

- 字段 draft value、dirty/touched/revision、required/disabled/readonly 等字段元信息。
- `InputState`、`SelectState`、`HotkeyInput` 等组件 state 的创建、读写和订阅保存。
- 用户输入事件到 typed form event 的转换。
- generated setter/accessor、array `FormItemId` row identity、array add/remove helpers。
- 字段错误的存储、清理、可见性和渲染所需的 `visible_errors` 数据源。
- submit 前 app validator / adapter 产生的字段错误回填。
- submit 点击后把 normalize/trim 后的值写回 form draft 和组件 state。
- `FormMeta` 只提供交互/生命周期 snapshot；`is_pristine()` 是只依赖 meta snapshot 的派生查询。
  `is_submitting()` / `can_attempt_submit()` 是 form store 级别查询，直接组合 submit runtime 与当前 meta，
  不表达数据合法。
- sync/async submit runtime 保存与本表单提交等价的 task；`is_submitting` 从 task 是否存在派生。

不属于 `gpui-form` 表单层职责：

- 决定如何写 SQLite、`config.toml`、GPUI credentials/keychain 或刷新 runtime store；这些业务副作用由
  app 在调用 submit 时传入的 handler 执行。
- 生成 `ProviderDraft`、`McpServerTomlConfig`、`ShortcutDraft`、`ChatFormSubmit` 等业务 payload。
- provider model refresh、MCP connection test、OAuth authorize/sign-out、global hotkey runtime registration。
- DB UNIQUE/FK、TOML schema、credentials deletion、provider/model enabled 检查等最终保护。
- Settings list search/filter、theme tile、language menu、add/delete/test/refresh action。
- 用 `form.meta().is_valid` / `form.meta().can_submit` 判断是否允许保存。保存是否成功必须由 app validator
  结果、`prepare_submit(...)` 结果和 handler outcome 决定。

按这个口径，当前代码的迁移状态是：

| 区域 | 表单层状态 | 仍由 app 持有且合理的内容 | 需要继续收敛的表单层残留 |
| --- | --- | --- | --- |
| Provider Settings | 已完成 typed generated store、required marker、field errors、typed form events、submit runtime、secret changed binding 和 submit output pipeline。 | provider identity、kind、existing secret refs、saved snapshot、DB/keychain 保存、model refresh、enabled model toggle 仍由 app 处理。 | 无；Validate、Save、dirty snapshot、secret writes/refs 都复用 `ProviderSettingsFormOutput`。 |
| MCP Add/Edit Dialog | 已完成字段值、required、array row identity、add/remove、placeholder、field errors、summary、submit runtime 和 output-first validator 迁移。 | `McpServerTomlConfig` merge、OAuth draft promotion、config 写入、runtime status/tools 展示。 | 无；Save 不再 submit 前读取 draft validator，row errors 用 submit-time `FormItemId` 映射。 |
| Prompt Edit Dialog | 已完成表单层迁移：name/content state、required、normalize 写回和字段错误都在 `PromptEditFormStore`。 | prompt duplicate 查询、create/update repository command、通知。 | 无。 |
| Shortcut Edit Dialog | 已完成表单层迁移：hotkey/prompt/model/input_source/enabled state、custom bindings、subscriptions、required 和字段错误都在 `ShortcutEditFormStore`。 | existing shortcut/temporary hotkey validator context、provider/model snapshot、create/update repository command。 | 无。 |
| General Settings | 未迁移，当前是即时 config action / inline hotkey control。 | config 写入、runtime hotkey registration 和 rollback。 | 不列为 P1；如果后续迁移 temporary hotkey，复用 `ShortcutHotkeyBinding`。 |
| ChatForm controls | 未迁移，当前是运行态 submit surface。 | composer、attachments、agent run state、provider/model runtime revalidation、config preference auto-save。 | 不列为 Settings 表单迁移项；若后续拆 controls form，只覆盖 model/reasoning/approval/token budget。 |

## P0 全迁移补齐计划：半迁移变完整迁移

本节只记录 `app/ai-chat2` 的应用侧重构计划；不把 Provider/MCP 的业务字段、i18n、icon、DB/config/keychain
规则写入 `crates/gpui-form/docs/*`。`gpui-form` crate 只保留通用 submit/runtime/binding 能力。

全迁移完成口径：

- Provider/MCP 的保存按钮只能调用一个 form submit 入口；page/dialog 不再在 submit 前手动读取 draft 并跑一套
  平行 validator。
- app-specific validator 必须接收 submit output 或由 submit output 构造的 request candidate；不能再从
  `ProviderDraft`、`McpServerFormDraft` 或组件 state 重新读取字段值。
- 保存 payload、dirty snapshot、secret writes/refs 和 validation 都复用同一套 output -> app request 映射；
  不保留 current-form snapshot 与 submit output 两套字段映射。
- handler error 要映射回 generated form field errors 或 notification；dialog/page 不持有与 form field errors
  平行的错误列表。
- `ProviderDraft` / `McpServerTomlConfig` 只作为 initial context、saved snapshot 或最终持久化结构，不作为编辑期
  字段事实源。

### Provider P0 模块结构

```text
app/ai-chat2/src/features/settings/provider/forms.rs
  - ProviderSettingsForm enum 只负责分发 concrete generated stores
  - 保留 entity_id / enabled / is_submitting / set_enabled / validate_for_save / submit_save 这类表单入口
  - 删除重复的 current draft -> persistent_fields(cx) / secret_fields(cx) 映射

app/ai-chat2/src/features/settings/provider/forms.rs
  - ProviderSettingsFormOutput
  - output -> persistent ProviderSettingsPayload / display_name / enabled / secret writes / secret refs
  - current form -> ProviderSettingsFormOutput 只允许作为 Validate 按钮和 dirty snapshot 辅助，复用 submit output 的同一套方法

app/ai-chat2/src/features/settings/provider/forms/secret.rs
  - ProviderSecretField { ApiKey, BearerToken }
  - ProviderSecretValue { field, value, changed }
  - ProviderSecretDraft { field, value, changed }
  - ProviderSecretInputState { input: Entity<InputState>, field, changed }
  - ProviderSecretInputBinding: FormComponentBinding<ProviderSecretValue>
  - ProviderSecretInputState::input() 供 render 层继续使用 gpui-component Input

app/ai-chat2/src/features/settings/provider/forms/api_key.rs
  - ApiKeyProviderFormInput.api_key 改为 ProviderSecretValue + ProviderSecretInputBinding
  - base_url 继续使用 TextInputBinding<String>

app/ai-chat2/src/features/settings/provider/forms/ollama.rs
  - bearer_token 改为 ProviderSecretValue + ProviderSecretInputBinding，仍 optional

app/ai-chat2/src/features/settings/provider/forms/custom_openai.rs
  - api_key 改为 ProviderSecretValue + ProviderSecretInputBinding
  - ProviderApiModeSelectBinding 保持 app-local binding

app/ai-chat2/src/features/settings/provider.rs
  - ProviderSettingsPage::save 只收集非字段 context：provider id、new id、kind、existing secret refs、saved display fallback
  - 保存入口调用 editor.form.submit_async_save(secret refs, handler, window, cx)
  - 删除 validate_current_draft() 对保存路径的前置依赖；Validate 按钮复用同一 output validator
  - snapshot_for_editor 通过 ProviderSettingsFormOutput 生成 dirty snapshot
```

`ProviderSecretInputBinding` 的职责：

- 所用组件仍是 `gpui_component::input::{InputState, Input}`，render 层继续调用
  `Input::new(&secret_state.input()).mask_toggle()`。
- `new_state(...)` 创建 masked `InputState`，应用 field macro 上的 label/placeholder/mask/required 选项。
- `install_subscriptions(...)` 订阅 inner `InputState` 的 `InputEvent::Change/Focus/Blur`；Change 时把
  `ProviderSecretInputState.changed` 置为 true，再向 `gpui-form` sink 发 `FormComponentEvent::Change(...)`。
- `read_draft(...)` 返回 `ProviderSecretDraft { field, value, changed }`，其中 `changed` 是 sticky flag，
  不是 `value != default`。这样“已保存 secret -> 用户输入临时值 -> 再清空”仍能表达 cleared。
- `parse_draft(...)` 返回 `ProviderSecretValue { field, value, changed }`，submit output 不再读取 field
  revision 判断 dirty。
- required secret 是否满足仍由 app validator 用 `ProviderSecretValue.changed` + existing `ProviderSecretRefs`
  判断：未改动且有 saved ref 视为满足；改动后为空表示清空，required secret 拒绝保存，optional secret 删除 ref。

Provider 数据流目标：

```text
ProviderDraft(saved context + existing secret refs)
  -> concrete Provider*FormInput
  -> generated form store owns fields + ProviderSecretInputBinding state
  -> user input updates form field value and secret changed sticky flag
  -> submit_async_save(secret refs, handler)
  -> gpui-form prepare/normalize returns ProviderSettingsFormOutput
  -> provider app validator validates output + context; invalid returns SubmitError::Handler and writes generated field errors
  -> output + context builds ProviderSaveRequest
  -> async handler writes GPUI credentials/keychain and fresh DB provider row
  -> finish_save refreshes provider list/model list and saved snapshot
```

### MCP P0 模块结构

```text
app/ai-chat2/src/features/settings/mcp/form_state.rs
  - McpServerFormInput remains the submit output field shape
  - add helpers: McpServerFormInput::server_id(original), merge_into_config(original)
  - McpSubmitRowIds captures array FormItemId order for submit-output row error routing
  - no dialog-owned draft reading in save validation

app/ai-chat2/src/features/settings/mcp/validation.rs
  - add validate_mcp_submit_output(output, context) -> Vec<McpFormValidationError>
  - context type: McpSubmitValidationContext { original_server_id, existing_server_ids, row_ids }
  - validator reads output values plus submit-time FormItemId row ids, not McpServerFormDraft values

app/ai-chat2/src/features/settings/mcp/dialog.rs
  - save() only builds non-field context / row ids and calls form.submit_async(handler, window, cx)
  - delete validate_mcp_form(&self.draft, ...) pre-submit path from dialog save
  - finish_save only handles config write result, OAuth runtime promotion, notification and dialog close
```

MCP 数据流目标：

```text
AiChat2Config mcp server snapshot + dialog mode
  -> McpServerFormInput
  -> generated McpServerFormStore owns top-level fields and row stores
  -> form.submit_async(handler)
  -> gpui-form prepare/normalize returns McpServerFormInput output
  -> validate_mcp_submit_output(output, context) maps issues to generated field errors via SubmitError::Handler
  -> output + OAuth context builds McpServerSaveRequest
  -> async handler deletes stale OAuth credentials
  -> finish_save upserts config.toml and promotes/replaces MCP runtime OAuth status
```

### Submit ownership decision

Provider/MCP 保存流程包含必须等待的异步副作用：Provider 写 credentials/keychain，MCP 删除 stale OAuth
credentials。因此它们整体使用 `submit_async(...)`，不是先拆成 `submit_sync(...)` 再由 app state 持有 task。

`gpui-form` 的 async submit handler 是同步 task builder：

```text
form.submit_async(|output, window, cx| {
  validate_output_with_context(&output, context)?;
  Ok(window.spawn(cx, async move |cx| save_request(output, cx).await))
})
```

- prepare/normalize 失败返回 `SubmitError::Invalid`，不创建 task。
- app validator / request construction 失败返回 `SubmitError::Handler`，不创建 task，app 把错误写回 generated
  field errors 或 notification。
- task builder 成功后才把 task 放入 `SubmitRuntime`，`is_submitting()` 只在 task 存在期间为 true。
- 纯同步保存流程继续使用 `submit_sync(...)`；同步提交不提供可观察的 submitting/loading 状态。

## 全量字段验证审计口径

字段验证必须按真实持久化和运行语义决定，而不是只看当前已经迁移到 `gpui-form` 的 Provider/MCP。每个表单面
都需要明确下面事实：

- 这个字段是否由用户直接编辑，还是由 state 层生成。
- 这个字段写 SQLite、`config.toml`、keychain/credentials，还是只存在于运行态 UI。
- DB/config 是否允许 `NULL` / 空字符串 / 默认值，以及这是否等同于 UI optional。
- 非空之外是否有 URL、header、env var、hotkey、enum、FK、duplicate、capability 等业务语义。
- 保存前字段级 validator 要挡在哪里，DB/config/runtime 仍保留哪些最终保护。

总矩阵：

| 区域 | 持久化边界 | 用户可编辑字段 | Required / 验证结论 |
| --- | --- | --- | --- |
| Provider Settings | `providers` 表 + keychain/credentials | `enabled`、`api_key`、`base_url`、`bearer_token`、custom `name`、`api_mode` | `api_key`、Ollama/custom `base_url`、custom `name` required；API-key `base_url` optional 但非空必须 `http/https` URL；secret required 同时看 saved ref 和 dirty input。 |
| MCP Add/Edit | `config.toml [mcp_servers]` | `server_id`、`transport`、stdio command/args/env/env_vars/cwd、HTTP url/bearer/env headers/OAuth | `server_id` required；`command`/`url` 随 transport 条件 required；row 半填才条件 required；URL/header/env/OAuth 冲突与 `state/config/mcp.rs` 对齐。 |
| Prompt Edit | `prompts` 表 | `name`、`content` | 二者 required；`name` trim 后非空且按 DB UNIQUE 语义预检重复；`content` trim 后非空；`enabled/sort_order` 不是用户字段。 |
| Shortcut Edit | `shortcuts` 表 + hotkey runtime + provider/model snapshot | `hotkey`、`prompt_id`、`model`、`input_source`、`enabled` | `hotkey` required 且 canonical/modified key/global conflict 校验；`model` required 且来自 enabled provider/model；`prompt_id` optional；snapshot/FK 由 state 层最终保护。 |
| General Settings | `config.toml app_settings` + global hotkey runtime | `language`、`http_proxy`、`temporary_hotkey` | `language` enum 不迁移；`http_proxy` optional，本阶段不定义 URL scheme 白名单；`temporary_hotkey` optional，非空复用 hotkey 校验和 runtime 注册。 |
| Appearance Settings | `config.toml app_settings.theme` | theme mode、light/dark theme tile、custom material color | 当前都是即时 action/config 写入，不接 `gpui-form`；theme id 来自 registry choices，custom color 由 `ColorPickerState`/hex normalize 控制。 |
| Projects Settings | `projects` 表 | add project directory prompt、project delete/row actions | 目录选择不是 text form；`projects.path` DB `NOT NULL UNIQUE` 由 repository 插入和 `was_existing` 处理；不显示 required marker。 |
| Skills Settings | skill catalog runtime scan/cache | search query、refresh、expand skill content | search/filter/refresh/content load 都不是提交表单；错误走 settings notification/banner，不接 required。 |
| NewConversation project selector | `config.toml app_settings.default_project_id` + `projects` 表 | project picker / no project | optional；选中 project 必须来自当前 project list；stale default project 启动时回退，不是 required。 |
| ChatForm controls | `config.toml chat_form` + runtime submit | composer、attachments、model、reasoning、token budget、approval mode | `model` 仅运行提交 required，config 仍 optional；composer text 或 attachments 至少一个非空；reasoning/token budget 受 model capabilities 约束；approval enum 总有默认值。 |

结论：

- required marker 只用于用户必须填写的业务字段，不跟随 DB `NOT NULL` 机械映射。
- SQLite `NOT NULL` / `UNIQUE`、TOML schema、keychain ref、provider/model enabled、MCP runtime config validation
  都必须映射成字段级 validator 或明确保留为最终保护。
- Provider/MCP 是首批已迁移表单，但 Prompt、Shortcut、General、ChatForm 和非迁移页面也要在计划中给出验证边界。

## Required 字段能力前置计划

本节记录 `ai-chat2` 如何消费 `gpui-form` 的 required 字段能力。`gpui-form` crate 自身的通用 API、
宏和 runtime 计划见 `crates/gpui-form/docs/development-plan.md`。

### 设计结论

- `required` 默认 false。
- app form input 通过 `#[form(required)]` 或 `#[form(required = true/false)]` 声明字段是否必填。
- `required` 第一阶段只驱动 UI marker 和字段语义，不自动生成 validation error。
- 空值、URL、secret refs、MCP config 约束仍由 `garde` 或 app-specific validator 负责。
- Provider/MCP 也必须接入 required marker；Prompt/Shortcut 不是 required 能力的唯一消费方。
- required marker 跟随字段业务语义和当前 validator，而不是跟随 SQLite `NOT NULL`。例如 provider
  `settings_json` 在 DB 中非空，但 `base_url` 是 JSON 内业务字段，需要 app validator 自己判断是否 required
  和 URL 是否有效。
- `gpui-component` 已有 `gpui_component::form::field().required(bool)`，会在 label 后渲染 danger 色 `*`；
  `ai-chat2` 不新增 app-local required marker 组件。
- `/Users/sushao/Documents/code/ui` 的 shadcn/ui 只作为补充参考：shadcn 的 Field 示例在 control 上使用
  HTML `required`，invalid 状态用 `data-invalid` / `aria-invalid`；GPUI 侧不照搬 DOM 属性。

### 文件和模块结构

`gpui-form` 前置实现完成后，`ai-chat2` 侧按下面文件消费：

| 文件 | 计划 |
| --- | --- |
| `app/ai-chat2/src/features/settings/provider/forms/api_key.rs` | `api_key` 声明 `#[form(required)]`；`base_url` 不 required，因为它只是内置 provider 默认 endpoint 的 optional override。 |
| `app/ai-chat2/src/features/settings/provider/forms/ollama.rs` | `base_url` 声明 `#[form(required)]`，默认值仍是 `http://localhost:11434`；`bearer_token` 不 required。 |
| `app/ai-chat2/src/features/settings/provider/forms/custom_openai.rs` | `name`、`api_key`、`base_url` 声明 `#[form(required)]`；`api_mode` 是有默认值的 enum select，不标 required。 |
| `app/ai-chat2/src/features/settings/provider/forms.rs` | validator 从纯 `require_text` 扩展为 required + URL 语义校验；secret field 用 saved `ProviderSecretRefs` 满足 required，但清空 required secret 仍拒绝保存。 |
| `app/ai-chat2/src/features/settings/provider.rs` | `render_text_input_row`、`render_secret_input_row`、`render_select_row` 改为使用 `gpui_component::form::field().label(...).required(required)`，不再手写 label + input column。 |
| `app/ai-chat2/src/features/settings/mcp/form_state.rs` | `server_id` 声明 `#[form(required)]`；`command` / `url` 通过 generated `set_*_required` 随 transport 切换；row 字段不静态 required。 |
| `app/ai-chat2/src/features/settings/mcp/dialog.rs` | MCP 顶层字段渲染改为 `field().required(...)`；transport 切换后同步 `command_required` / `url_required`；`AddMcpRow` / `RemoveMcpRow` 不变。 |
| `app/ai-chat2/src/features/settings/mcp/validation.rs` | validator 对齐 `state/config/mcp.rs` 的 runtime config 约束：server id、stdio command、HTTP URL、env var、headers、OAuth/bearer 冲突。 |
| `app/ai-chat2/src/features/settings/prompts/dialog.rs` | 持有 `Entity<PromptEditFormStore>`；保存前 trim draft 并写回 form，空 name/content 和 duplicate name 通过 generated `apply_field_error` 落到对应字段。 |
| `app/ai-chat2/src/features/settings/prompts/form_state.rs` | 定义 `PromptEditFormInput` / `PromptEditFormStore`；`name` 使用内置 input 且 required，`content` 通过 `PromptContentInputBinding` 保持 multiline/rows 且 required。 |
| `app/ai-chat2/src/features/settings/shortcuts/dialog.rs` | 持有 `Entity<ShortcutEditFormStore>`；保存从 form draft 生成 `ShortcutDraft`，hotkey/model 错误通过 generated `apply_field_error` 落到对应字段。 |
| `app/ai-chat2/src/features/settings/shortcuts/form_state.rs` | 定义 `ShortcutEditFormInput` / `ShortcutEditFormStore`；`hotkey` 和 `model` required，`prompt` optional，`input_source`/`enabled` 不 required；`ShortcutHotkeyBinding`、`ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding` 负责现有组件 state 同步。 |
| `app/ai-chat2/src/features/settings/shortcuts/validation.rs` | Shortcut 继续保留 `validate_shortcut_hotkey` app validator，并把错误回填到 required field；Prompt required/duplicate validator 留在 prompt dialog save flow。 |
| `app/ai-chat2/src/features/settings/general.rs` | P2 候选：HTTP proxy 作为 optional URL-like config field，temporary hotkey 复用 `ShortcutHotkeyBinding`；language dropdown 不迁移。 |
| `app/ai-chat2/src/components/chat_form.rs` | P3 候选：只在文档中固定 submit/config 验证边界；composer、attachments、model/reasoning/approval/token budget 不和 Settings 表单同批迁移。 |
| `app/ai-chat2/src/components/chat_form/{model_select,effort_select,approval_select,thinking_effort}.rs` | 若未来拆 `ChatFormControlsInput`，这些模块提供具体 picker/bounds/enum 规则；当前不改文件。 |
| `app/ai-chat2/locales/{en-US,zh-CN}/main.ftl` | 不为 required marker 新增文案；已补 provider URL error 和 prompt duplicate 文案；MCP bearer/OAuth 管理 Authorization header 的冲突继续复用 `mcp-validation-header-reserved`。 |

### Provider required 和验证矩阵

数据库边界：

- `providers.id` 是 DB primary key；内置 provider 使用 provider kind，自定义 provider 使用生成 id。
- `providers.kind`、`display_name`、`enabled`、`settings_json`、`secret_refs_json` 均为 `NOT NULL`，但
  `settings_json` 里的 `base_url`、`api_mode` 等字段没有 DB 级约束。
- secret 明文不进 DB；`secret_refs_json` 只保存 keychain ref。required secret 是否满足，要同时看
  `ProviderSecretRefs` 和当前 secret input 是否被用户改空。

字段规则：

| Form | 字段 | Required marker | 保存校验 |
| --- | --- | --- | --- |
| API-key provider | `api_key` | required | required secret。已有 saved secret ref 且输入未改动时视为满足；用户清空已保存 secret 后保存必须失败。 |
| API-key provider | `base_url` | 不 required | optional override；空值表示使用 provider 默认 endpoint；非空必须是 `http` 或 `https` URL。 |
| Ollama | `base_url` | required | required text，默认 `http://localhost:11434`；非空必须是 `http` 或 `https` URL。 |
| Ollama | `bearer_token` | 不 required | optional secret；清空后移除 saved secret ref。 |
| Custom OpenAI-compatible | `name` | required | required text；映射到 `providers.display_name`，DB 不要求唯一，所以不做唯一性校验。 |
| Custom OpenAI-compatible | `api_key` | required | required secret；规则同 API-key provider。 |
| Custom OpenAI-compatible | `base_url` | required | required text；必须是 `http` 或 `https` URL。 |
| Custom OpenAI-compatible | `api_mode` | 不 required | enum select 有默认 `responses`，没有空状态；只保存允许的 enum key。 |

Provider validator 目标：

- 用 `url::Url::parse` 校验 provider `base_url`，scheme 只允许 `http` / `https`，和
  `crates/ai-chat-agent/src/provider_models.rs` 的 `validate_base_url` 保持一致。
- 对 API-key provider 的 `base_url` 只在非空时校验；对 Ollama / Custom OpenAI 的 `base_url` 先 required，
  再做 URL 校验。
- required marker 不受 `enabled` 控制。`enabled = false` 只是运行态开关，不表示可以保存一个结构上不完整的
  API-key provider。
- Provider URL 错误已新增稳定 key：`provider-validation-url-invalid` /
  `provider-validation-url-scheme`，并通过 generated `apply_field_error` 落到 `base_url`。

### MCP required 和验证矩阵

配置边界：

- MCP server 不写 SQLite；它保存在 `config.toml` 的 `[mcp_servers.<server_id>]` map 中。
- `server_id` 是 TOML map key，`state/config/mcp.rs::upsert_mcp_server` 负责校验 shape 和 duplicate。
- `McpServerTomlConfig::validate` 是 runtime config 的最终约束；dialog validator 需要在保存前给出同等字段级错误，
  不应把错误延迟到 `upsert_mcp_server` 的全局 notification。

字段规则：

| 字段 | Required marker | 保存校验 |
| --- | --- | --- |
| `server_id` | required | trim 后非空；必须匹配 `^[A-Za-z0-9_-]+$`；新增或 rename 时不能和 existing server id 重复。 |
| `transport` | 不 required | toggle/enum 总有选中值。 |
| `command` | Stdio 时 required | Stdio transport 下 trim 后非空。 |
| `args[]` | 不 required | 空 row 忽略；非空但全空白报 `mcp-validation-arg-empty`。 |
| `env[].key` | 条件式 required | env row 为空时忽略；只填 value 时 key required；key 必须符合 `[A-Za-z_][A-Za-z0-9_]*`；key 去重。 |
| `env[].value` | 不 required | value 允许空字符串，因为 stdio env map 可以显式设置空值。 |
| `env_vars[]` | 不 required | 非空时必须符合 `[A-Za-z_][A-Za-z0-9_]*`；去重。 |
| `cwd` | 不 required | trim 后为空则不保存；不做存在性校验，避免拒绝后续才创建或跨平台路径。 |
| `url` | Streamable HTTP 时 required | trim 后非空；必须是 `http` 或 `https` URL。 |
| `bearer_token_env_var` | 不 required | 非空时必须符合 env var name；OAuth enabled 时必须为空，避免和 OAuth 同时管理 Authorization。 |
| `headers[].name/value` | 条件式 required | row 为空时忽略；只填一边时报 incomplete；name 必须是 `http::HeaderName`，value 必须是 `http::HeaderValue`，name 去重。 |
| `env_headers[].name/env_var` | 条件式 required | row 为空时忽略；只填一边时报 incomplete；name 规则同 header name，env_var 规则同 env var name，name 和 literal headers 共用去重集合。 |
| `oauth_enabled` | 不 required | bool toggle；启用后不能同时设置 bearer token env var，也不能手写 Authorization header。 |

MCP validator 目标：

- Dialog validator 的 URL、env var、reserved header、duplicate header、OAuth conflict 规则和
  `state/config/mcp.rs` 保持一致。
- `Authorization` header 在 `bearer_token_env_var` 或 OAuth enabled 时由 auth 流程管理；literal headers 和
  env headers 都不能再配置 `Authorization`。
- `accept`、`content-type`、`mcp-session-id`、`mcp-protocol-version`、`last-event-id` 继续作为 MCP reserved
  headers 拒绝。
- 对 row 字段不加静态 `#[form(required)]`，因为 UI 默认保留空 row。需要在用户填写同一 row 的另一侧时，通过
  runtime required setter 或 validator error 表达“这一格现在必填”。

### Prompt required 和验证矩阵

数据库边界：

- `prompts.id` 是 DB primary key。
- `prompts.name` 是 `TEXT NOT NULL UNIQUE`；SQLite 默认 binary collation 下大小写敏感，app validator
  应在 trim 后按同样语义做精确重复名检查，DB UNIQUE 仍是最终保护。
- `prompts.content` 是 `TEXT NOT NULL`；DB 不限制空字符串，所以空内容必须由 app validator 拦截。
- `enabled` 和 `sort_order` 是内部状态，不在 edit dialog 中给用户直接输入。

字段规则：

| 字段 | Required marker | 保存校验 |
| --- | --- | --- |
| `name` | required | trim 后非空；create/edit 都要检查同名 prompt，edit 时排除当前 `prompt_id`；错误落到 `name` 字段。 |
| `content` | required | trim 后非空；允许多行；不做最大长度限制。 |
| `enabled` | 不在 dialog 中渲染 | create 默认 `true`；edit 保留当前值。 |
| `sort_order` | 不在 dialog 中渲染 | create 由 `state::prompts::create_prompt` 按现有列表末尾递增；edit 保留当前值。 |

Prompt validator：

- required 和 duplicate name 都由 app-specific validator 处理；duplicate 依赖当前 `PromptCatalogStore` /
  repository snapshot，不能由通用 field adapter 决定。
- duplicate error 已新增稳定 i18n key `prompt-validation-name-duplicate`，保存前按 trim 后 name 预检，不把
  DB UNIQUE error 作为常规用户反馈路径。
- submit 失败时，trim 后的 name/content 仍写回 form draft 和 input state。

### Shortcut required 和验证矩阵

数据库边界：

- `shortcuts.hotkey` 是 `TEXT NOT NULL UNIQUE`；保存前会被 `validate_shortcut_hotkey` canonicalize，DB UNIQUE
  是最终保护。
- `shortcuts.enabled` 是 `BOOLEAN NOT NULL DEFAULT 1`。
- `shortcuts.prompt_id` / `provider_id` 是 nullable FK，`model_id` 也是 nullable。nullable 是为了 prompt/provider
  被删除后保留 shortcut 记录，不代表创建/编辑表单可以不选模型。
- `shortcuts.input_source` 是 `TEXT NOT NULL CHECK (input_source IN ('selection_or_clipboard', 'screenshot'))`。
- `action_json` 和 `settings_snapshot_json` 是 `JSON NOT NULL`，由 state 层根据表单 draft 生成，不是用户输入字段。

字段规则：

| 字段 | Required marker | 保存校验 |
| --- | --- | --- |
| `hotkey` | required | 必填；必须能被 `HotkeyInput` / `string_to_keystroke` 解析；必须包含 modifier；通过 `global_hotkey::HotKey` 校验；canonical 后不能和 temporary hotkey 或其它 shortcut 冲突。 |
| `prompt_id` | 不 required | optional；为空表示不带 prompt；非空必须来自当前 prompt choices，state 层读取 prompt content 作为最终 FK/快照保护。 |
| `model` (`provider_id` + `model_id`) | required | 创建/编辑时必选；必须来自当前 enabled provider/model choices；`settings_snapshot_for_draft` 再确认 provider/model 仍存在且 enabled。 |
| `input_source` | 不 required | segmented toggle 总有选中值；只能是 `SelectionOrClipboard` 或 `Screenshot`。 |
| `enabled` | 不 required | bool switch；不影响其它 required 字段是否可保存。 |
| `action_json` | 不在 dialog 中渲染 | 由 state 层固定写 `ShortcutAction::OpenTemporaryConversation`。 |
| `settings_snapshot_json` | 不在 dialog 中渲染 | 由 `settings_snapshot_for_draft` 从 prompt/provider/model/default tool policy 生成。 |

Shortcut validator 目标：

- 继续保留 `app/ai-chat2/src/features/settings/shortcuts/validation.rs::validate_shortcut_hotkey`，不要把
  global hotkey 解析规则塞进 `gpui-form`。
- `model` required error 继续使用 `shortcut-validation-model-required`，并通过 generated `apply_field_error`
  落到 model 字段。
- provider/model disabled 或 missing 是运行快照约束；实现时应尽量在 dialog choices 刷新后避免选到无效模型，
  但 `settings_snapshot_for_draft` 的 DB 检查仍保留为最终保护。
- DB `UNIQUE(hotkey)`、FK 和 JSON NOT NULL 不是 UI marker 来源；UI marker 只表达用户必须填写的 hotkey/model。

### General Settings 验证矩阵

配置边界：

- General Settings 写 `AiChat2Config.app_settings`，不是 SQLite。
- `language`、`theme`、`temporary_hotkey`、`http_proxy`、`default_project_id` 都保存在 `config.toml`。
- 当前代码只保存/读取 `http_proxy`，未发现下游 runtime 使用点；本阶段不在计划中定义 proxy scheme
  白名单，后续迁移时必须以实际 runtime 使用方为准。

字段规则：

| 字段 | Required marker | 保存校验 |
| --- | --- | --- |
| `language` | 不迁移 | dropdown 菜单即时写配置；enum 总有选中值。 |
| `http_proxy` | 不 required | optional；trim 后空值保存为 `None`；非空必须是最终 runtime 支持的 proxy URL。本阶段不定义 `http`/`https`/`socks5` 等 scheme 白名单。 |
| `temporary_hotkey` | 不 required | optional；清空允许；非空复用 `ShortcutHotkeyBinding` 的 canonical/modified key 校验，并以 `GlobalHotkeyState::update_temporary_hotkey` 注册结果作为最终约束。 |
| `default_project_id` | 不迁移 | sidebar/project 选择派生，不在 General form 中编辑。 |

General validator 目标：

- HTTP proxy 迁移为 form 前必须先读取实际 runtime 使用方和 scheme 白名单；如果仍没有使用方，文档和实现都不能擅自引入
  socks/http/https 之外的新语义。
- HTTP proxy 写 config 失败时保留当前 `last_value` 语义：未成功 commit 的输入不能被视为已保存。
- temporary hotkey 保存顺序保持现状：先更新 runtime hotkey，config 写入失败再 rollback runtime。
- temporary hotkey 的错误来自解析/canonical/register 失败；不因为字段 optional 而显示 required marker。

### ChatForm controls 验证矩阵

配置和运行边界：

- ChatForm 持久化 `AiChat2Config.chat_form`，不是 SQLite。
- `chat_form.model` 是 `Option<ChatFormModelConfig>`；配置里允许为空，启动时会回退到第一个可用 model。
- 运行提交 `ChatFormSubmit` 必须有 composer 内容或 attachments、可用 provider/model、有效 reasoning selection、
  approval mode 和 attachment 支持检查通过。

字段规则：

| 字段/状态 | Required marker | 保存/提交校验 |
| --- | --- | --- |
| composer text | 不迁移到 P1/P2 form | submit 时 text 或 attachments 至少一个非空；空 submit 返回 `None`。 |
| attachments | 不迁移到 P1/P2 form | submit 时受 selected model capabilities 限制；unsupported image/file/count 阻止提交。 |
| model | 运行态 required，配置 optional | config 可为空；UI 运行时必须选到仍存在的 provider/model；submit 前调用 `revalidate_selected_model_for_submit` 重新加载确认。 |
| reasoning_selection | 不 required | 只在 selected model 支持 reasoning 时出现；必须匹配当前 model capabilities；无效时回退到 computed default。 |
| token budget | 条件式 optional | 只有当前 reasoning control 支持 token budget 时渲染；输入解析为 `u32` 后按 capability `min/max/default` clamp。 |
| approval_mode | 不 required | enum picker 总有选中值；默认来自 `default_tool_approval_mode()`。 |
| skill tokens | 不迁移到 P1/P2 form | composer 内部解析；无匹配 skill 是 completion 体验问题，不是 required 字段。 |

ChatForm validator 目标：

- P3 如果拆 `ChatFormControlsInput`，只能覆盖 model/reasoning/approval/token budget；`ComposerEditor` 和
  attachments 继续留在 `ChatForm`，避免把运行态编辑器生命周期塞进 Settings form 抽象。
- model 的 required 只在运行提交语义上成立；不能把 config 的 `model: Option<_>` 改成 required，也不新增
  config migration。
- token budget 继续复用 `thinking_effort::token_budget_bounds` 和 clamp 规则，不新增通用 number validator 语义；
  如果后续下沉到 generated form，必须使用 `NumberInput` render helper，并让 raw token budget 文本参与 dirty
  判断。
- approval mode 继续复用 `approval_select::approval_mode_sections`，不新增依赖或 app-local enum duplicate。

### 不迁移输入的验证边界

- Settings / Prompts / Shortcuts / Skills / MCP / sidebar search/filter：只过滤当前列表，不是提交表单；不接
  `gpui-form`，不显示 required。
- Appearance theme mode、theme tile、custom material theme color picker：当前是即时 action/config 写入；没有
  draft + submit 边界，不接 `gpui-form`。
- Provider model list search/fetch/toggle：模型列表属于 provider 管理和 runtime capabilities，不是 provider
  config form 字段；不参与 required marker。
- MCP runtime status、tools list、OAuth 状态展示：只读或 action-driven，不作为 Add/Edit form source of truth。

### 自定义组件和类型结构

Prompt：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = PromptEditFormStore)]
pub(super) struct PromptEditFormInput {
    #[form(
        binding = "StringInputBinding",
        placeholder = "prompt-placeholder-name",
        required
    )]
    pub name: String,

    #[form(
        binding = "PromptContentInputBinding",
        placeholder = "prompt-placeholder-content",
        required
    )]
    pub content: String,
}
```

`PromptContentInputBinding`：

- 实现 `FormComponentBinding<String>`。
- `new_state` 创建 `InputState::new(...).multi_line(true).rows(10)`。
- placeholder 仍通过 `ComponentStateOptions` 传入。
- input state 自身不消费 `required`；required marker 由 render 层的 `field().required(...)` 渲染。

Shortcut：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ShortcutEditFormStore)]
pub(super) struct ShortcutEditFormInput {
    #[form(binding = "ShortcutHotkeyBinding", required)]
    pub hotkey: Option<String>,

    #[form(binding = "ShortcutPromptSelectBinding")]
    pub prompt: ShortcutPromptSelection,

    #[form(binding = "ShortcutModelSelectBinding", required)]
    pub model: ShortcutModelSelection,

    #[form(component = "value")]
    pub input_source: ShortcutInputSource,

    #[form(binding = "BoolInputBinding")]
    pub enabled: bool,
}
```

Provider/MCP：

- Provider/MCP 的 required 和 validator 矩阵以前文“Provider required 和验证矩阵”与“MCP required
  和验证矩阵”为准。
- Provider secret required 是字段语义；saved secret ref 可以满足 required，但不让字段变成 optional。
- MCP `command` / `url` 的 required 状态由 transport 派生，通过 generated setter 或 render-time derived bool
  更新。
- MCP row 字段不静态 required；半填 row 用 validator error 定位，必要时再用 row field generated setter
  表达条件式 required。
- required marker 不写入 DB/config。
- Provider/MCP/Prompt/Shortcut 目前没有 generated number 字段；后续新增 number 字段时，app 只能使用
  adapter `number_input::<N>(&state)`；如果直接使用 state，也必须先取出 `let state = form.field_state();`，
  再渲染 `NumberInput::new(&state)`，不能把 number state 渲染成普通 `Input`。
- `PromptContentInputBinding`、`ShortcutHotkeyBinding`、`ShortcutPromptSelectBinding`、
  `ShortcutModelSelectBinding`、`ProviderApiModeSelectBinding` 已完成 binding trait surface 迁移：事件订阅移动到
  binding 自身，dialog/page 的 DB/config/keychain 保存、validator、i18n 和 icon 都不变。

### 所用组件

- `gpui_component::form::{field, v_form}` 用于 Settings 表单布局和 required marker。
- `Input` / `InputState` 用于 prompt name、prompt content、provider text/secret fields、MCP text fields 和 rows。
- `Select` / `SelectState` 用于 provider API mode，以及 app-local binding 表达的 shortcut prompt/model choices。
- `Switch` / bool field 用于 provider enabled 和 MCP OAuth enabled。
- 现有 `Button`、`Label`、`Tag` 和 notification helpers 继续用于 action/status；required marker 不需要新组件。

`ProviderSettingsPage` 和 `McpServerDialog` 顶层可编辑字段已迁移到 `gpui_component::form::field()` 承载
required marker；动态 row 仍保留现有 row 组件和 validator error，不新增自定义 `RequiredLabel` helper。

### 数据流

Prompt:

```text
PromptRecord?
  -> PromptEditFormInput { required name/content }
  -> PromptEditFormStore::from_value(...)
  -> render field().required(form.name_required()) + Input
  -> save trims name/content and writes normalized draft back to form
  -> app validator checks required name/content and unique name against current prompt list
  -> create_prompt/update_prompt
```

Shortcut:

```text
ShortcutRecord? + ShortcutDialogChoices + validator context
  -> ShortcutEditFormInput { required hotkey/model }
  -> ShortcutEditFormStore::from_value(...)
  -> render field().required(form.hotkey_required()) / field().required(form.model_required())
  -> submit reads form draft
  -> validate_shortcut_hotkey + model required app validation
  -> settings_snapshot_for_draft confirms prompt/provider/model DB state
  -> ShortcutDraft
  -> create_shortcut/update_shortcut
```

Provider/MCP:

```text
existing ProviderSettingsForm / McpServerFormStore
  -> required metadata from static #[form(required)] and transport/row-derived app state
  -> render gpui-component field required marker
  -> provider validator checks required secret + http/https base_url
  -> MCP validator checks server id + transport-specific required + config-compatible URL/header/env/OAuth rules
```

General:

```text
AiChat2Config.app_settings
  -> optional P2 General form input
  -> http_proxy: trim, empty -> None, non-empty -> confirmed proxy URL validator
  -> temporary_hotkey: Option<String> -> ShortcutHotkeyBinding canonical value
  -> runtime hotkey registration succeeds before config write
  -> AiChat2ConfigStore writes config.toml
```

ChatForm:

```text
ProviderCatalog + AiChat2Config.chat_form + composer state + attachments
  -> ChatForm runtime state
  -> config auto-save remains optional for model/reasoning/approval
  -> submit revalidates selected provider/model and attachment support
  -> ChatFormSubmit
```

### 全局数据管理

- 不新增 `Global`。
- 不把 required 状态接入 `gpui-store`。
- Prompt/Shortcut form state 仍由打开的 dialog 持有。
- Provider/MCP required marker 由当前 settings page/dialog 的 form entity 和现有 app state 派生。
- General P2 如果迁移，form state 仍由 `general.rs` 当前 keyed state / row owner 持有；不新增 settings 全局 form。
- ChatForm P3 如果拆 controls form，仍由 `Entity<ChatForm>` 持有；不把 composer 或 attachments 放入全局 form store。

### 数据库变更

- 不新增 SQLite migration。
- 不修改 `prompts`、`shortcuts`、provider rows、MCP `config.toml` schema 或 credentials。
- Required marker 不持久化；成功 submit 后仍只写现有业务 payload。
- Prompt name duplicate、Shortcut hotkey duplicate 等 DB constraint 只要求 app validator 提前给字段级错误；
  不改变 DB schema。
- General / ChatForm 写 `config.toml`，不写 SQLite。

### 数据获取方式

- 静态 required 来自 form input 的 `#[form(required)]`。
- 条件式 required 从已有 runtime/config/db state 派生：
  - Provider saved secret refs 和 dirty secret value 用于判断 required secret 是否已满足，但不取消字段 required 语义。
  - MCP transport kind 决定 `command` / `url` required。
  - MCP row sibling values 决定 env/header row 的条件式 required。
  - Shortcut dialog choices 和 existing shortcut list 只作为 validator context，不作为 required source of truth。
- Prompt duplicate name validator 从 `PromptCatalogStore` 或 repository 当前 snapshot 读取。
- Shortcut hotkey conflict validator 从 existing shortcuts 和 `GlobalHotkeyState` temporary hotkey diagnostics 读取。
- General HTTP proxy scheme 必须来自实际 runtime 使用方；当前代码没有足够依据在本阶段定义白名单。
- ChatForm model required 来自运行态 `ProviderCatalog`/repository model choices，不来自 config 的 `Option` 类型。
- 不从 validation error 文案反推 required 状态。

### Icon

- 不新增 icon。
- Required marker 使用 gpui-component Field 内建 danger `*`。
- 现有 save/delete/add/test icons 不变。

### i18n

- Required marker 不新增 Fluent key。
- Prompt required errors 继续使用 `prompt-validation-name-required` / `prompt-validation-content-required`，由
  app validator 直接映射到对应字段。
- Prompt duplicate name 已新增 `prompt-validation-name-duplicate`（en-US / zh-CN 同步），避免把 DB UNIQUE
  error 作为常规用户可见 notification。
- Shortcut required errors 继续使用 `shortcut-validation-hotkey-required` / `shortcut-validation-model-required`。
- Provider/MCP required errors 继续使用现有 `provider-validation-required` / `mcp-validation-*-required`。
- Provider URL error 已新增稳定 Fluent key，并在 en-US / zh-CN 同步；MCP OAuth/bearer conflict 继续复用
  `mcp-validation-header-reserved`，不为同一 header 约束新增重复文案。
- General HTTP proxy 如果迁移，需要新增 proxy URL error key；temporary hotkey 继续复用 hotkey/register failure
  notification，不新增 required 文案。
- ChatForm P3 不新增 required marker 文案；继续复用现有 model empty、attachment support、reasoning/approval key。

### 新增依赖

- 不新增依赖。
- Provider URL 校验复用 `app/ai-chat2` 已有 `url = "2.5.8"`。
- MCP header 校验复用 `app/ai-chat2` 已有 `http = "1.4.2"`。
- `app/ai-chat2` 已有 `gpui-form.workspace = true`；Prompt/Shortcut 迁移不新增 `garde` / `validify`
  直接依赖。
- 不引入 `/Users/sushao/Documents/code/ui` 的 shadcn/ui 包、React 包或 DOM helper。

## Provider/MCP 之外的候选评估

本节记录 `app/ai-chat2` 中除 Provider Settings 和 MCP Settings 之外，哪些地方需要或可以继续接入
`gpui-form`。评估原则：

- 需要有明确的 edit draft、校验、提交或保存边界。
- 搜索框、filter 输入和单次菜单选择不默认视为表单。
- 运行态 composer / picker 只有在能明显减少状态同步复杂度时才迁移，避免为了统一而扩大 `gpui-form`
  的职责。

| 优先级 | 区域 | 当前状态 | 结论 |
| --- | --- | --- | --- |
| P1 | Prompt Edit Dialog | 已迁移到 `PromptEditFormStore` | name/content required，trim 后写回 form，duplicate name validator |
| P1 | Shortcut Edit Dialog | 已迁移到 `ShortcutEditFormStore` | hotkey/model required，prompt optional，input_source/enabled 通过 generated setter 写回 |
| P2 | General Settings HTTP proxy / temporary hotkey | auto-save 输入、inline hotkey 编辑 | 可以迁移，但不阻塞；复用 Shortcut 的 hotkey binding |
| P3 | ChatForm generation controls | composer、attachments、model/reasoning/approval picker、token budget、config auto-save 交织 | 暂缓，属于运行态复杂表单 |
| 不迁移 | search/filter inputs | Settings、Prompts、Shortcuts、Skills、MCP、sidebar/temporary search | 不是提交表单，不应接入 |
| 不迁移 | Appearance theme grid / theme mode | theme tile、dropdown/button 即时写配置；color picker 只是新增 material theme 的临时工具 | 暂不接入 |

### Prompt Edit Dialog

当前代码：

- `app/ai-chat2/src/features/settings/prompts/dialog.rs`
- `PromptEditDialogState` 持有 `form: Entity<PromptEditFormStore>`，不再直接持有 `name_input` /
  `content_input` 或单个 `validation_error`。
- 保存时读取 form draft，trim 后通过 generated setter 写回 normalized draft，再执行 app required /
  duplicate-name validator，最后调用 `state::prompts::create_prompt` 或 `state::prompts::update_prompt`。

当前文件结构：

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
  - PromptContentInputBinding
  - field_errors helper
```

当前类型：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = PromptEditFormStore)]
pub(super) struct PromptEditFormInput {
    #[form(
        binding = "StringInputBinding",
        placeholder = "prompt-placeholder-name",
        required
    )]
    pub name: String,

    #[form(
        binding = "PromptContentInputBinding",
        placeholder = "prompt-placeholder-content",
        required
    )]
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
  -> save trims name/content and writes normalized values through generated setters
  -> app validator checks required name/content and duplicate name against current prompts, excluding current prompt id
  -> create_prompt/update_prompt
  -> PromptCatalog refreshes list
```

全局数据和持久化：

- 不新增 `Global`。
- 不新增数据库 migration；仍写入现有 prompts 表。
- 搜索框 `PromptsSettingsPage::search_input` 不接入 `gpui-form`，它只是列表 filter。

icon / i18n / 依赖：

- save 继续用 `IconName::FilePen`，preview/edit/delete 继续用 `Pencil` / `Trash`。
- 复用现有 `prompt-field-*`、`prompt-placeholder-*`、`prompt-validation-*` key；required 和 duplicate
  错误都使用 app validator 映射到现有 prompt validation key。
- 已新增 `prompt-validation-name-duplicate`，用于 DB `UNIQUE(prompts.name)` 的字段级预检。
- 不新增 `garde` / `validify` 直接依赖；当前 Prompt 的 trim、required 和 duplicate 都由 app save flow 处理。

### Shortcut Edit Dialog

当前代码：

- `app/ai-chat2/src/features/settings/shortcuts/dialog.rs`
- `ShortcutEditDialogState` 持有 `form: Entity<ShortcutEditFormStore>`，`existing_shortcuts` 和
  `temporary_hotkey` 作为 validator context 保留在 dialog。
- 保存时读取 form draft，调用 `validate_shortcut_hotkey` 和 model required app validator，再组装
  `ShortcutDraft`。

当前文件结构：

```text
app/ai-chat2/src/features/settings/shortcuts/dialog.rs
  - ShortcutEditDialogState 持有 Entity<ShortcutEditFormStore>
  - 继续负责 dialog footer、focus、save 后通知

app/ai-chat2/src/features/settings/shortcuts/form_state.rs
  - ShortcutEditFormInput
  - ShortcutEditFormStore
  - ShortcutHotkeyBinding
  - ShortcutPromptSelectBinding
  - ShortcutModelSelectBinding
  - ShortcutPromptSelection / ShortcutModelSelection

app/ai-chat2/src/features/settings/shortcuts/validation.rs
  - 保留 validate_shortcut_hotkey
  - hotkey required/invalid/conflict 仍由 app validator 返回 ShortcutValidationError
```

当前类型：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ShortcutEditFormStore)]
pub(super) struct ShortcutEditFormInput {
    #[form(binding = "ShortcutHotkeyBinding", required)]
    pub hotkey: Option<String>,

    #[form(binding = "ShortcutPromptSelectBinding")]
    pub prompt: ShortcutPromptSelection,

    #[form(binding = "ShortcutModelSelectBinding", required)]
    pub model: ShortcutModelSelection,

    #[form(component = "value")]
    pub input_source: ShortcutInputSource,

    #[form(binding = "BoolInputBinding")]
    pub enabled: bool,
}
```

组件和 binding：

- `ShortcutHotkeyBinding` 负责 `Option<String> <-> HotkeyInput`，复用
  `HotkeyInput::current_hotkey_string()` 和 `string_to_keystroke()`。
- `ShortcutPromptSelectBinding` 包装 `SelectState<Vec<PromptChoice>>`；字段值包含当前选择和 prompt choices，
  保存时只使用 `selected`。
- `ShortcutModelSelectBinding` 包装 `SelectState<SearchableVec<SelectGroup<ModelOption>>>`；字段值包含当前选择和
  provider/model choices，以保留 grouped searchable options，保存时只使用 `selected`。
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
  -> settings_snapshot_for_draft validates prompt/provider/model still exist and provider/model enabled
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
  `last_value` 或用 form meta/revision 防止重复写入。空值保存为 `None`；非空值必须按实际 runtime 支持的
  proxy scheme 做 URL 校验，不能在当前文档里擅自决定 socks/http/https 范围。
- temporary hotkey 可以复用已落地的 `ShortcutHotkeyBinding`，目标类型是
  `TemporaryHotkeyFormInput { hotkey: Option<String> }`。保存仍必须先更新 runtime global hotkey，再写 config；
  rollback 逻辑留在 app，不进入 `gpui-form`。该字段 optional，非空时复用 hotkey canonical/modified key 校验，
  runtime registration failure 仍是最终错误来源。
- language dropdown 不迁移：它不是 draft + submit 表单。

### Appearance Settings

当前代码：

- `app/ai-chat2/src/features/settings/appearance.rs`
- `ColorPickerState` 只用于选择新增 material theme 的颜色；theme mode 和 theme tile 都是点击后立即写 config。

迁移判断：

- 暂不接入 `gpui-form`。
- 如果未来出现“编辑 theme form / preview 后 submit”的需求，再新增 `ColorPickerBinding`。
- 当前 theme grid、delete custom material theme、mode button 都是 action-driven 设置，不适合作为 form store。

### Projects Settings

当前代码：

- `app/ai-chat2/src/features/settings/projects.rs`
- `app/ai-chat2/src/state/projects.rs`
- `crates/ai-chat-db/src/migrations.rs`

字段和约束：

| 字段/状态 | 来源 | Required marker | 保存/运行约束 |
| --- | --- | --- | --- |
| add project directory | `cx.prompt_for_paths(PathPromptOptions { directories: true, .. })` | 不显示 | 不是 text form；取消选择不保存；选中值交给 `insert_existing_folder_project`。 |
| `projects.path` | SQLite `projects.path TEXT NOT NULL UNIQUE` | 不显示 | 由系统 path picker 和 repository 插入保护；已存在时返回 `was_existing`，显示 info notification。 |
| `projects.display_name` | `insert_existing_folder_project` 从 path 派生 | 不显示 | `TEXT NOT NULL`，当前 settings 页不可编辑；若未来新增 rename form，再按非空 trim 校验。 |
| `projects.kind` | state 层固定 normal/scratch | 不显示 | DB `CHECK (kind IN ('normal', 'scratch'))`；settings list 只展示 normal projects。 |
| `pinned` / `removed` | project row action/state | 不显示 | DB bool `CHECK`；删除/恢复/置顶是 action，不是 draft 表单。 |
| `metadata_json` | state/repository 生成 | 不显示 | JSON `NOT NULL`，不由 settings form 直接输入。 |

迁移判断：

- 暂不接 `gpui-form`。目录选择、row action 和 duplicate path handling 都不是用户在 text field 中提交的表单。
- 未来如果添加“rename project”对话框，应新增 `ProjectRenameFormInput { display_name: String }`，
  `display_name` 标 required，保存前 trim 非空；`path` 仍不能变成普通文本 required 字段。
- 不新增数据库 migration；当前约束已经能保护 path 唯一性和 kind/bool 枚举范围。

### Skills Settings

当前代码：

- `app/ai-chat2/src/features/settings/skills.rs`
- `app/ai-chat2/src/features/settings/skills/rows.rs`
- `app/ai-chat2/src/state/skills.rs`

字段和约束：

| 字段/状态 | 来源 | Required marker | 保存/运行约束 |
| --- | --- | --- | --- |
| `search_input` | `InputState` | 不显示 | 只过滤当前 `SkillCatalogRow.search_text`；空 query 显示全部，不是保存字段。 |
| refresh action | `state::skills::refresh_global_catalog` | 不显示 | 失败通过 notification；不改变 required/validation 状态。 |
| expanded content | `load_skill_content(row.entry)` | 不显示 | 按 skill file path 加载内容和 sha256；失败显示 content panel error。 |
| content scroll | `ScrollHandle` | 不显示 | 只是 UI state，不写 config/DB。 |

迁移判断：

- 不接 `gpui-form`。Skills Settings 没有 draft + submit，也没有 DB/config 字段需要用户填完整。
- 搜索输入继续作为 filter input；不要因为它是 `InputState` 就机械迁移或显示 required。
- 未来如果新增“编辑 project/user skill metadata”表单，需要先确定 skill manifest schema 和写回位置，再单独设计
  form input、validator、i18n key 和文件写入失败回滚。

### NewConversation / Project Selector

当前代码：

- `app/ai-chat2/src/features/home/new_conversation.rs`
- `app/ai-chat2/src/features/temporary/new_conversation.rs`
- `app/ai-chat2/src/state/config.rs`
- `app/ai-chat2/src/state/projects.rs`

字段和约束：

| 字段/状态 | 来源 | Required marker | 保存/运行约束 |
| --- | --- | --- | --- |
| selected project | current visible normal projects | 不显示 | optional；No Project 合法；选中 project 必须来自当前 loaded list。 |
| `default_project_id` | `AiChat2Config.app_settings` | 不显示 | `Option<ProjectId>`；stale id 通过 `initial_project_id` / `selected_or_initial_project_id` 回退。 |
| add project from picker | OS directory prompt + project catalog | 不显示 | 和 Projects Settings 共用 `insert_existing_folder_project` / `was_existing` 语义。 |
| skill catalog refresh path | selected project path | 不显示 | 只影响 composer skill catalog；project path stale 时重新加载列表并回退。 |

迁移判断：

- 不接 `gpui-form`。project selector 是 picker + config preference，不是 required text form。
- `default_project_id` 在 config 里 optional，不能因为 conversation 必须关联 project record 就把 UI 选择做成 required；
  No Project 会走 scratch project / runtime 创建路径。
- 数据获取继续从 `state::projects::normal_projects(cx)` 和 project catalog subscription 进入页面；不新增全局 form state。

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
- `model` 在 `config.toml` 中是 optional preference，但在提交运行时是 required：submit 前必须重新加载并确认
  selected provider/model 仍存在且可用。
- composer text 与 attachments 是 submit guard，不是 required marker：文本为空但有 attachments 可以发送；
  二者都为空时不提交。
- reasoning selection 和 token budget 由当前 model capabilities 限制；无效 selection 回退到 computed default，
  custom token budget 按 min/max/default clamp。
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
- validate/dirty snapshot 可以读取当前 generated form typed draft；保存 payload 的字段值必须从
  `submit_sync(...)` / `submit_async(...)` 的 submit output 构造。app 不再通过 `InputState` /
  `SelectState` 或 `editor.draft.fields` 反向拼提交结构。
- repository command、config 写回、credentials 写入、runtime refresh 和 dirty snapshot 比对仍是 app 业务层职责，
  不作为 `gpui-form` 迁移完成度判断标准。
- `#[derive(FormStore)]` 生成 store、字段 store、字段枚举、事件、typed accessors 和 array helpers。
- `#[derive(FormStore)]` 生成的 array helpers 包含 `field_remove_id(row_id, cx)` 和
  `field_values_with_id()`；app 不再先把 row id 转 index，也不再维护自己的 row value DTO。
- `#[derive(FormStore)]` 生成 `set_field_value(...)`、`clear_all_errors(...)`、
  `clear_field_errors(...)` 和 `apply_field_error(...)`；app-specific validator 只负责映射到具体 generated
  field enum。
- app 不再手写类似 `server_id_input()`、`command_input()`、`provider_form_input_state()` 这类重复 getter；
  由宏生成 `field_state()`、`field_state()`、`field_value()` 等访问器。
- app 不直接订阅每个 `InputState` 再反查字段；`gpui-form` 安装并保存组件订阅，app 只订阅 typed form event
  处理业务副作用。
- app 不用 typed number value 推断 dirty。number field 的 typed draft 只代表最后一次成功 parse 的 domain
  value；raw input 文本才是 dirty/default 的比较基准。
- number binding 不能把所有 `FromStr + ToString` 类型当作同一种输入。`NumberInputBinding<N>::new_state(...)`
  必须根据 `N::input_policy()` 配置 `InputState`：signed integer 允许符号但不允许小数，unsigned integer
  不允许符号和小数，float 允许小数；`i64/u64/isize/usize` 这类大整数的 step 由 binding 用 Rust checked
  arithmetic 处理，不依赖 `gpui-component` 的 `f64` step。
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
- `gpui-form` 不重复实现专业规则库；需要通用规则时走 `garde` adapter，需要 normalize/sanitize 时走
  `validify::Modify` adapter。当前 `ai-chat2` Provider/MCP/Prompt/Shortcut 仍以 app-specific validator 为主，
  validator 输出必须映射回 generated form field errors。
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
  - ProviderValidationIssue / ProviderSettingsFormOutput
  - domain draft <-> typed form input 转换
  - output -> settings payload / display name / secret writes / secret refs 的唯一映射

app/ai-chat2/src/features/settings/provider/forms/secret.rs
  - ProviderSecretValue
  - ProviderSecretDraft
  - ProviderSecretInputState
  - ProviderSecretInputBinding

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
  - 可保留 McpFormField 作为 validator issue 的业务位置类型，但错误应用目标必须映射到 generated form field
  - 不再让 McpFormField + dialog-owned Vec 驱动字段错误渲染

app/ai-chat2/src/features/settings/mcp/dialog.rs
  - McpServerDialog 拥有 Entity<McpServerFormStore>
  - 订阅 form event，触发保存按钮状态和错误清理
  - 顶层 `.on_action` 处理 `AddMcpRow` / `RemoveMcpRow`
  - add/remove/reorder 调用宏生成的 array helpers；删除最后一项不自动补空行
  - 不再持有 `validation_errors: Vec<McpFormValidationError>`；validation summary 和行内错误从 form field
    errors 派生
```

## Provider 表单设计

每个 provider kind 使用确定的表单类型，不使用动态 field schema。

Provider secret 字段使用 app-local binding：

```rust
type SecretInputBinding = ProviderSecretInputBinding;
```

API-key provider：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ApiKeyProviderFormStore)]
pub(super) struct ApiKeyProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub enabled: bool,

    #[form(
        binding = "SecretInputBinding",
        label = "provider-field-api-key",
        placeholder = "provider-placeholder-api-key",
        required,
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub api_key: ProviderSecretValue,

    #[form(
        binding = "StringInputBinding",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-base-url-default",
        validate(on_blur, on_submit)
    )]
    pub base_url: String,
}
```

Ollama provider：

```rust
pub(super) struct OllamaProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub enabled: bool,

    #[form(
        binding = "StringInputBinding",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-ollama-base-url",
        required
    )]
    pub base_url: String,

    #[form(binding = "SecretInputBinding", label = "provider-field-bearer-token", placeholder = "provider-placeholder-bearer-token", mask)]
    pub bearer_token: ProviderSecretValue,
}
```

Custom OpenAI-compatible provider：

```rust
pub(super) struct CustomOpenAiProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub enabled: bool,

    #[form(binding = "StringInputBinding", label = "provider-field-name", placeholder = "provider-placeholder-provider-name", required)]
    pub name: String,

    #[form(binding = "SecretInputBinding", label = "provider-field-api-key", placeholder = "provider-placeholder-api-key", required, mask)]
    pub api_key: ProviderSecretValue,

    #[form(binding = "StringInputBinding", label = "provider-field-base-url", placeholder = "provider-placeholder-custom-base-url", required)]
    pub base_url: String,

    #[form(
        binding = "ProviderApiModeSelectBinding",
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

- API-key provider 和 Custom OpenAI-compatible 的 `api_key` 是 required secret；`ProviderSecretValue.changed`
  为 false 且存在 saved secret ref 时视为满足 required，changed 后空值表示清空，required secret 必须拒绝保存。
- Ollama `bearer_token` 是 optional secret；清空后删除 saved secret ref。
- API-key provider `base_url` 是 optional override，非空时必须是 `http` / `https` URL。
- Ollama 和 Custom OpenAI-compatible 的 `base_url` 是 required text，且必须是 `http` / `https` URL。
- `name` 对 custom provider 必填并 trim；DB 不要求 display name 唯一，因此不做唯一性校验。
- `api_mode` 必须是 `responses` 或 `chat_completions`。
- validation issue 必须落到具体字段：`Name`、`ApiKey`、`BaseUrl`、`BearerToken`、`ApiMode`。
- secret 原文只存在于 `ProviderSecretInputState` / submit output 的 dirty value 中；DB 只保存 `ProviderSecretRefs`，keychain
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
    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-arg", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvVarRowFormStore)]
pub(super) struct McpEnvVarRowInput {
    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-env-var", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvRowFormStore)]
pub(super) struct McpEnvRowInput {
    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-env-key", validate(on_change, on_blur, on_submit))]
    pub key: String,

    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-env-value", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpHeaderRowFormStore)]
pub(super) struct McpHeaderRowInput {
    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-header-name", validate(on_change, on_blur, on_submit))]
    pub name: String,

    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-header-value", validate(on_change, on_blur, on_submit))]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpEnvHeaderRowFormStore)]
pub(super) struct McpEnvHeaderRowInput {
    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-header-name", validate(on_change, on_blur, on_submit))]
    pub name: String,

    #[form(binding = "StringInputBinding", placeholder = "mcp-placeholder-env-header-var", validate(on_change, on_blur, on_submit))]
    pub env_var: String,
}
```

目标 server form：

```rust
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = McpServerFormStore)]
pub(super) struct McpServerFormInput {
    pub transport: McpTransportKind,

    #[form(binding = "StringInputBinding", label = "mcp-field-name", placeholder = "mcp-placeholder-server-id", required)]
    pub server_id: String,

    #[form(binding = "StringInputBinding", label = "mcp-field-command", placeholder = "mcp-placeholder-command")]
    pub command: String,

    #[form(binding = "StringInputBinding", label = "mcp-field-cwd", placeholder = "mcp-placeholder-cwd")]
    pub cwd: String,

    #[form(component = "array", store = "McpArgRowFormStore")]
    pub args: Vec<McpArgRowInput>,

    #[form(component = "array", store = "McpEnvRowFormStore")]
    pub env: Vec<McpEnvRowInput>,

    #[form(component = "array", store = "McpEnvVarRowFormStore")]
    pub env_vars: Vec<McpEnvVarRowInput>,

    #[form(binding = "StringInputBinding", label = "mcp-field-url", placeholder = "mcp-placeholder-url")]
    pub url: String,

    #[form(binding = "StringInputBinding", label = "mcp-field-bearer-token-env-var", placeholder = "mcp-placeholder-bearer-token-env-var")]
    pub bearer_token_env_var: String,

    #[form(component = "array", store = "McpHeaderRowFormStore")]
    pub headers: Vec<McpHeaderRowInput>,

    #[form(component = "array", store = "McpEnvHeaderRowFormStore")]
    pub env_headers: Vec<McpEnvHeaderRowInput>,

    #[form(binding = "BoolInputBinding")]
    pub oauth_enabled: bool,
}
```

MCP app-specific validation：

- `server_id` 必填、格式为 `[A-Za-z0-9_-]+`、同一 config 中唯一。
- stdio transport：`command` 必填；`args` 不能是纯空白行；`cwd` trim 后为空则不保存，不做存在性校验。
- env key / env var / bearer token env var 必须符合 `[A-Za-z_][A-Za-z0-9_]*`；env key 不重复。
- streamable HTTP：`url` 必填且 scheme 为 `http` / `https`。
- header name/value 使用 `http` crate 校验；reserved headers 不能配置；header name 不重复。
- header row 必须 name/value 同时填写；env-backed header 必须 name/env_var 同时填写。
- OAuth enabled 时不能同时设置 `bearer_token_env_var` 或手写 `Authorization` header。
- 错误定位使用 `FormItemId` + row field，不能依赖当前 index；reorder 后错误仍应落到同一行。
- validator 可以继续返回 `McpFormValidationError { field: McpFormField, message_key }` 作为业务 issue；dialog
  保存失败时必须先 `McpServerFormStore::clear_all_errors(...)`，再把每个 issue 映射到 generated top-level
  或 row form field 并调用 `apply_field_error(...)`。
- row 错误映射使用 `FormItemId` 查找 generated array item store；找不到 row id 时忽略该 stale issue，并由下一次
  validate 重新计算。

MCP 已删除或迁移的旧痕迹：

- `McpStringRowInput`、`McpKeyValueRowInput`。
- `StringListField`、`KeyValueField`。
- `StringListDraftRow`、`KeyValueDraftRow` 的泛型 row view 输入。
- `apply_mcp_form_placeholders`、`apply_key_value_placeholders`、`set_input_placeholder`。
- dialog 根据 `McpFormField` 动态匹配泛型 row 的渲染方式；目标改为 typed row field 渲染。
- `McpServerEditDialogState::validation_errors` 和基于 dialog-owned Vec 的 `render_validation_summary(...)`。
  当前保留的 `field_error_messages(...)` 只是从 form field `visible_errors` 读取渲染消息的 helper。

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
  -> submit produces ProviderSettingsFormOutput
  -> app-specific validation consumes output + non-field context and maps issues to generated form field errors
  -> ProviderSettingsSaveCandidate { settings payload, secret writes/refs, display name, enabled }
  -> app repository/keychain save
```

MCP Settings：

```text
config.toml [mcp_servers.<id>]
  -> McpServerTomlConfig
  -> McpServerFormInput
  -> cx.new(|cx| McpServerFormStore::from_value(input, window, cx))
  -> array helpers manage FormItemId + row stores + subscriptions
  -> submit produces McpServerFormInput output
  -> MCP validator consumes output + dialog context and maps issues to generated top-level/row field errors
  -> output + OAuth context builds McpServerSaveRequest { server_id, McpServerTomlConfig, credential cleanup }
  -> app AiChat2ConfigStore writes config.toml
```

Prompt Edit Dialog：

```text
prompts table row?
  -> PromptEditFormInput { name, content }
  -> submit handler trims name/content and writes normalized draft back to form
  -> app required + duplicate-name validator against current prompt snapshot
  -> state::prompts::create_prompt/update_prompt
```

Shortcut Edit Dialog：

```text
shortcuts table row? + prompt/model choices + existing shortcuts + temporary hotkey
  -> ShortcutEditFormInput { hotkey, prompt, model, input_source, enabled }
  -> validate_shortcut_hotkey
  -> model required app validation + provider/model snapshot check in state layer
  -> state::shortcuts::create_shortcut/update_shortcut
```

General Settings P2：

```text
AiChat2Config.app_settings
  -> GeneralHttpProxyFormInput / TemporaryHotkeyFormInput
  -> optional value validators
  -> runtime hotkey registration or config commit
  -> AiChat2ConfigStore writes config.toml
```

ChatForm P3：

```text
AiChat2Config.chat_form + ProviderCatalog + ComposerEditor + attachments
  -> ChatForm runtime controls
  -> config auto-save for model/reasoning/approval preferences
  -> submit guard confirms non-empty content, attachment support, and valid model
  -> ChatFormSubmit
```

## 数据监听和生命周期

- `ProviderSettingsPage` 持有当前 `ProviderSettingsForm` 和对应 form subscription。
- 切换 provider 时 drop 旧 form entity 和 subscription，创建新 typed form。
- `McpServerDialog` / `McpServerFormDraft` 持有 `Entity<McpServerFormStore>`。
- add/remove/reorder dynamic row 只调用宏生成 helpers；remove 时对应 row store 和 subscriptions 被 drop；
  reorder 不重建仍存在的 row state。删除最后一项后数组可以为空，由 add action 再创建新行。
- `PromptEditDialogState` 持有 `Entity<PromptEditFormStore>`；duplicate validator 读取当前 prompt snapshot，
  不把 prompt list 存入 form output。
- `ShortcutEditDialogState` 持有 `Entity<ShortcutEditFormStore>`；existing shortcuts、temporary hotkey 和 choices
  是 validator context，随 dialog 生命周期持有。
- General P2 single-field auto-save form 必须保留 config commit subscription；不能在 render 中即时创建新
  subscription。
- ChatForm P3 如果拆 controls form，仍挂在 `Entity<ChatForm>`，不能重建 composer/attachment state。
- app 可以订阅 form event 做这些业务副作用：
  - 清除保存/测试产生的外部错误或对应 form field errors。
  - 更新 dirty badge / Save button enabled。
  - 标记 secret field dirty 或 cleared。
- app 不应订阅 `InputState` 来同步 draft value；这是 `gpui-form` 的职责。
- app 不应在 dialog/page 中维护与 form field errors 平行的字段错误列表；MCP 已删除旧的 dialog-owned
  `validation_errors`，保存失败后只写回 generated form field errors。
- 不在 render 中创建 subscription。
- 不在 form event 里嵌套 update 同一个 entity。

## 全局数据管理和持久化

- 表单状态只属于打开它的 settings page/dialog。
- 不使用 `Global` 存所有活跃表单。
- 不把表单 draft 写入 `gpui-store`。
- `gpui-form` store 持有与当前表单提交等价的 submit task；`FnOnce` handler 在 app 调用
  `submit_sync(...)` 或 `submit_async(...)` 时传入，不长期存进 store。
- DB/config/keychain/runtime 更新的业务逻辑仍由 app handler 调用现有 state/repository/config helper；
  form core 不直接访问这些全局资源。
- Provider/MCP/Prompt/Shortcut 保存 payload 的字段值必须来自 submit output；app context 只提供
  provider id、existing secret refs、original config、OAuth draft keys、duplicate-check snapshots 等非字段上下文。
- Provider handler 成功后才写 fresh DB provider row 和 GPUI credentials；失败时只更新表单错误或通知。
- MCP handler 成功后才写 `config.toml`；失败时只更新表单错误或通知。
- Prompt handler 成功后才写 prompts 表；duplicate name 在 app validator 前置，DB UNIQUE 继续兜底。
- Shortcut handler 成功后才写 shortcuts 表；provider/model snapshot 仍由 state 层生成，DB nullable FK 不改变
  UI required 语义。
- General P2 成功 auto-save 后才更新 saved value；temporary hotkey 保持 runtime-first/config-second/rollback
  顺序。
- ChatForm P3 config auto-save 只保存 preferences；运行提交不写 config。
- `FormItemId` 不写入 DB / TOML / keychain。
- 本计划不新增数据库 migration。

表单提交任务迁移边界：

| 当前任务 | 目标 owner | 原因 |
| --- | --- | --- |
| Provider save | generated provider form store 的 submit runtime | 与 Save 按钮一一对应，loading/disabled/attempt/outcome 应由表单提供。 |
| MCP Add/Edit save | `McpServerFormStore` submit runtime | create/edit validation、OAuth draft promotion 和 config upsert 都属于同一次表单提交。 |
| Prompt save | `PromptEditFormStore` submit runtime | duplicate 校验和 repository create/update 是表单提交结果。 |
| Shortcut save | `ShortcutEditFormStore` submit runtime | hotkey/model validator 与 shortcuts DB write 是表单提交结果。 |
| MCP test / refresh / OAuth sign-out | app state | 不是表单保存；可能在未提交表单时独立运行。 |
| Provider model fetch / refresh | app state | 依赖 provider/runtime 状态，不等同于保存当前表单。 |
| General immediate auto-save | app state，P2 再评估是否包成 form submit | 当前是即时 action 和 rollback 流程，不阻塞 Settings P1。 |

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

Prompt：

- initial value 从 `PromptRecord` 构造；create 模式使用空 name/content。
- duplicate validator 从 `PromptCatalogStore` 或 fresh repository snapshot 读取当前 prompts。

Shortcut：

- initial value 从 `ShortcutRecord`、`ShortcutDialogChoices` 和 temporary hotkey diagnostic 构造。
- hotkey duplicate/conflict validator 使用 existing shortcuts + temporary hotkey；model validator 使用当前 enabled
  provider/model choices，state 层再次从 DB 确认。

General：

- initial value 从 `AiChat2Config.app_settings` 构造。
- HTTP proxy 的 scheme 白名单必须来自最终 runtime 使用方；当前代码只提供存储，不足以确定 scheme。
- temporary hotkey current value 从 config 读取，最终可注册性由 `GlobalHotkeyState` 判断。

ChatForm：

- initial controls 从 `AiChat2Config.chat_form` + provider model choices 构造；无效或缺失 model 回退到第一个可用
  choice。
- reasoning/token budget options 从 selected model capabilities 和 `thinking_effort` helper 计算。

## 组件、icon 和 i18n

gpui-component：

- `Input` / `InputState`：provider 普通 text/base URL、MCP text fields 和 row fields；后续通过
  `gpui-form-gpui-component::TextInputBinding<T>` 接入 generated form。
- `Input` / `InputState` + `mask_toggle()`：provider api key / bearer token；通过 app-local
  `ProviderSecretInputBinding` 接入 generated form，binding 保存 sticky `changed` 语义，render 层仍使用
  `Input::new(&state.input()).mask_toggle()`。
- `Select` / `SelectState<Vec<ApiModeChoice>>`：custom OpenAI-compatible API mode；后续通过 adapter
  `SelectBinding<T, D>` 或 app-local wrapper binding 接入。
- `Input` / `InputState::multi_line(true)`：Prompt content binding；继续使用 app-local
  `PromptContentInputBinding`，因为 multiline rows 是 Prompt 业务 UI 选择。
- `HotkeyInput`：Shortcut hotkey 和 General temporary hotkey binding。
- `Select` / `SelectState`：Shortcut prompt/model selection；model select 需要保留 grouped searchable options。
- `NumberInput`：ChatForm token budget 继续由 ChatForm 自己使用，P3 前不下沉到 Settings form；若后续进入
  generated form，使用 adapter `NumberInputBinding<N>`，其 `Draft = String`，并通过 `N::input_policy()` 在
  `new_state(...)` 阶段应用类型化输入策略。
- `Switch` 或 `Checkbox`：enabled / OAuth enabled；后续通过 adapter `BoolBinding` 或 app-local binding 接入。
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
  - `FilePen`：Prompt save。
  - `Keyboard`：Shortcut save。
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
- Prompt 继续使用 `prompt-field-*`、`prompt-placeholder-*`、`prompt-validation-name-required`、
  `prompt-validation-content-required`，新增 `prompt-validation-name-duplicate`。
- Shortcut 继续使用 `shortcut-field-*`、`shortcut-validation-*`、`chat-form-model-search-placeholder`。
- General P2 如迁移 HTTP proxy，新增 proxy URL error key；temporary hotkey 复用现有 hotkey/register
  notification。
- ChatForm P3 不新增 required 文案；继续使用现有 model empty、attachment support、reasoning/approval/token
  budget keys。
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

执行 `gpui-form` binding 架构拆分后，`app/ai-chat2` 已新增 workspace 依赖：

```toml
gpui-form-gpui-component.workspace = true
```

Provider、MCP、Prompt 和 Shortcut 的 required / URL / duplicate / hotkey 校验都复用现有 app validator
与已有依赖；`gpui-form` 继续作为 workspace 依赖提供 form store、Draft-aware binding contract 和 error helpers，
`gpui-form-gpui-component` 只提供 `gpui-component` state adapter。

如果未来某个 ai-chat2 表单需要直接 derive `garde::Validate` / `validify::Validify`，再按实际宏展开和字段规则
新增直接依赖，版本必须完整；不要为当前 Prompt/Shortcut 迁移预留未使用依赖。

`gpui-form` 需要启用 pipeline 时使用 workspace/path dependency feature：

```toml
gpui-form = { workspace = true, features = ["form-pipeline"] }
```

MCP header validation 当前需要 `http` crate；如果 validation 模块直接解析 `HeaderName` / `HeaderValue`，
继续使用现有 app dependency，不为 form integration 新增其它解析库。

Prompt duplicate name、Shortcut hotkey conflict、General temporary hotkey 和 ChatForm reasoning/token budget 都复用现有
app/state/helper 代码，不新增依赖。General HTTP proxy URL 校验在读取实际 runtime 支持的 scheme 后优先复用现有
`url = "2.5.8"`；如果未来 runtime 明确支持非 URL 形态的 proxy 配置，再单独更新计划，不在本阶段预加依赖。

`gpui-form-gpui-component` 是 workspace 内新增 crate，不引入新的第三方库；它把既有 `gpui-component`
依赖从 `gpui-form` core 边界移到 adapter 边界。

## 迁移步骤

1. 新增 P0：跟进 `gpui-form` binding 架构拆分。
   - `ai-chat2` 增加 `gpui-form-gpui-component` workspace dependency。
   - Provider/MCP 当前使用内置 `binding = "StringInputBinding"` / `"select"` / `"bool"` 的字段，改成显式
     `binding = "..."`，binding 类型来自 adapter crate 或 app-local type alias。
   - Prompt/Shortcut 的 app-local binding 实现新的 Draft-aware `FormComponentBinding`。
   - app render 层继续直接使用 `gpui-component` 控件；core 宏只暴露 `<field>_state()` 这类通用 state accessor。
   - 保存、validator、DB/config/keychain 写回和 i18n key 不变。
1. 已完成：完成 `gpui-form` binding 目标 API：`FormComponentBinding`、field UI options、placeholder resolver、
   移除特殊 `custom` 设计入口。当前已完成 placeholder resolver、input placeholder/mask 初始化和
   app 自定义组件的 `binding` 接入；内置 `input`、`number`、`select`、`combobox` 和 bool 字段已通过
   binding 创建和同步组件 state。
1. 已完成：Provider forms 迁移字段宏属性：把 `configure_inputs` 中的 placeholder/mask/select options
   移到字段定义。
1. 已完成：Provider page 删除动态 input lookup 和 `ProviderFieldSchema` 旧痕迹，只保留 typed form enum。
1. 已完成：MCP row 拆分为 `McpArgRowInput`、`McpEnvVarRowInput`、`McpEnvRowInput`、`McpHeaderRowInput`、
   `McpEnvHeaderRowInput`。
1. 已完成：MCP row rendering 改为无状态 helper + typed row action，删除 `McpRowsView`、row handle struct
   和 `apply_*_placeholders`。
1. 已完成：MCP dialog add/remove 全部走宏生成 array helpers；remove 使用 `*_remove_id`，删除最后一行后不自动补行。
1. 已完成：provider/MCP 单字段写入使用 generated `set_field_value`，不再读整份 draft 再 patch。
1. 已完成：provider app-specific validator 的错误映射到具体 generated field enum，并通过 generated
   `clear_all_errors` / `apply_field_error` 写回。
1. 已完成：删除 app 侧 `InputState` 订阅同步 draft 的逻辑，只保留 form event 业务订阅。
1. 阶段性已完成：Provider/MCP 保存 payload 的字段值已改为来自 generated form submit output；不再从组件
   state 或 `editor.draft.fields` 同步提交结构。仍需 P0 收敛 validator/request/snapshot 重复路径。
1. 已完成：补齐当前实现所需 i18n key 使用、test 和 focused checks。
1. 已完成：补齐 `gpui-form` required 字段支持。
    - `gpui-form` 新增 `#[form(required)]` / `#[form(required = bool)]`，默认 false。
    - generated form store 暴露 `<field>_required()` 和 `set_<field>_required(required, window, cx)`。
    - `ComponentStateOptions` 传递 required 给 binding；adapter/app-local binding 不渲染 marker。
    - `ai-chat2` render 层统一使用 `gpui_component::form::field().required(...)`。
    - Provider/MCP 同步接入 required marker，不能只服务 Prompt/Shortcut。
    - required 不自动生成 validation error；Prompt/Shortcut/Provider/MCP 仍由 `garde` 或 app validator 写错误。
    - Provider validator 补 URL 语义校验；MCP dialog validator 对齐 `McpServerTomlConfig::validate`。
1. 已完成：迁移 Prompt Edit Dialog。
    - 新增 `app/ai-chat2/src/features/settings/prompts/form_state.rs`。
    - 新增 `PromptEditFormInput` / `PromptEditFormStore`。
    - 新增 `PromptContentInputBinding`，只解决 prompt content 的 multiline/rows，不扩展通用宏参数。
    - `name` 和 `content` 都声明 `#[form(required)]`，render 使用 `field().required(...)`。
    - 保存路径改为 `form.draft() -> trim/write normalized draft -> required/duplicate-name validator -> create_prompt/update_prompt`。
    - duplicate-name validator 使用当前 prompt snapshot，edit 时排除当前 prompt id，错误落到 `name` 字段。
    - 删除 dialog 里的 `name_input`、`content_input` 和单个 `validation_error`。
1. 已完成：迁移 Shortcut Edit Dialog。
    - 新增 `app/ai-chat2/src/features/settings/shortcuts/form_state.rs`。
    - 新增 `ShortcutEditFormInput` / `ShortcutEditFormStore`。
    - 新增 `ShortcutHotkeyBinding`、`ShortcutPromptSelectBinding`、`ShortcutModelSelectBinding`。
    - `hotkey` 和 `model` 声明 `#[form(required)]`；`prompt_id`、`input_source`、`enabled` 不 required。
    - `input_source` 先作为 value field，用 generated setter 接 `ToggleGroup`。
    - `validate_shortcut_hotkey` 保留在 app validator，错误通过 generated `apply_field_error` 回填字段。
    - `model` 必选，但 provider/model 的存在和 enabled 状态仍由 `settings_snapshot_for_draft` 做最终 DB 检查。
1. 已完成：收敛 MCP Add/Edit dialog 的错误状态。
    - 删除 `McpServerEditDialogState::validation_errors`。
    - `validate_mcp_form` 继续返回 `McpFormValidationError`，save 失败时映射到 generated
      `McpServerFormField` 或对应 row generated field，再调用 `apply_field_error(...)`。
    - `field_error_messages(...)` 读取 form / row field `visible_errors`；summary 从当前 form errors 收集，
      不再读取 dialog-owned Vec。
    - transport / add row / remove row / OAuth toggle 时通过 `clear_all_errors(...)` 清理 form field errors。
1. 已完成：Provider 从半迁移收敛到完整 submit output pipeline。
    - 新增 `provider/forms/secret.rs`，把 api key / bearer token 从普通 `TextInputBinding<String>` 改为
      `ProviderSecretInputBinding`。
    - `ProviderSettingsFormOutput` 成为 settings payload、display name、secret writes/refs、dirty snapshot
      的唯一字段映射源。
    - 删除 `ProviderSettingsForm::{persistent_fields(cx), secret_fields(cx)}` 与 output 同构的重复逻辑；
      current snapshot 通过 current form output 复用同一套 output 方法。
    - `ProviderSettingsPage::validate_current_output` 使用 current output；Save 路径不再 submit 前
      单独读 form draft。
    - Validate 按钮和 Save 按钮复用 `ProviderSettingsFormOutput::validate(...)`。
    - secret dirty/cleared 不再通过 `form.<field>.core().revision() > 0` 推断，改由
      `ProviderSecretValue.changed` 表达。
1. 已完成：MCP 从半迁移收敛到完整 submit output pipeline。
    - `validate_mcp_submit_output` 接收 `McpServerFormInput` submit output 和
      `McpSubmitValidationContext`。
    - `McpServerEditDialogState::save` 不再 submit 前调用 `validate_mcp_form(&self.draft, ...)`。
    - `McpServerSaveRequest` 构造集中到 output + OAuth context 的单一函数，dialog finish flow 只处理结果。
    - row errors 继续通过 `FormItemId` 定位 generated row field；不回退到 dialog-owned validation vec。
1. 已完成：Provider/MCP 保存 task owner 决策。
    - 包含 credentials/keychain 或 OAuth cleanup 的保存流程整体使用 `submit_async(...)`。
    - async handler 是同步 task builder；output/context validation 或 request construction 失败时返回
      `SubmitError::Handler`，不创建 task。
    - task builder 成功后由 form store 持有 task，`is_submitting()` 从 task 是否存在派生。
1. 可选 P2：在 Shortcut 的 hotkey binding 稳定后，再评估 General Settings 的 temporary hotkey inline editor。
1. 可选 P2：HTTP proxy 输入可以迁移为 single-field auto-save form，但必须保留 config 写入失败时不覆盖
    `last_value` 的语义；迁移前必须读取实际 runtime 使用方和 URL scheme 白名单。
1. 暂缓 P3：ChatForm controls 需要单独设计，不和 Settings 表单迁移混做。
    - `model` 是运行态 required、config optional。
    - composer text/attachments 是 submit guard，不是 required marker。
    - reasoning/token budget 使用 capability-derived options 和 clamp 规则。

## 验证计划

文档/格式：

- `git diff --check`
- `cargo fmt`

crate：

- `cargo test -p gpui-form --features form-pipeline`
- required 前置完成后：`cargo test -p gpui-form --features form-pipeline required`
- `cargo clippy -p gpui-form-macros -p gpui-form --features form-pipeline --all-targets -- -D warnings`

app：

- `cargo check -p ai-chat2`
- `cargo test -p ai-chat2 provider`
- `cargo test -p ai-chat2 mcp`
- required 前置完成后：`cargo test -p ai-chat2 provider`
- required 前置完成后：`cargo test -p ai-chat2 mcp`
- 迁移 Prompt Edit Dialog 后：`cargo test -p ai-chat2 prompt`
- 迁移 Shortcut Edit Dialog 后：`cargo test -p ai-chat2 shortcut`
- `cargo clippy -p ai-chat2 --all-targets -- -D warnings`

行为测试重点：

- Provider secret dirty / cleared / saved secret unchanged。
- Provider `ProviderSecretInputBinding`：已保存 secret 未改动时 `changed = false`；输入后再清空仍保持
  `changed = true`，required secret 拒绝保存，optional secret 删除 saved ref。
- Provider output mapping 单一来源：Save、Validate、dirty snapshot、secret writes/refs 都从
  `ProviderSettingsFormOutput` 生成，不再走 `persistent_fields(cx)` / `secret_fields(cx)` 的第二套映射。
- Provider required marker：API-key provider `api_key`，Ollama `base_url`，Custom OpenAI `name/api_key/base_url`
  显示 required；API-key provider `base_url`、Ollama `bearer_token`、Custom `api_mode` 不显示 required。
- Provider base URL：API-key provider 空值允许、非空必须是 http/https；Ollama/Custom 空值拒绝、非空必须是
  http/https；错误落到 `base_url`。
- Provider base URL trim 后无论 submit 成功失败都写回 input。
- Custom OpenAI API mode 通过 `SelectState` 正常读写。
- Provider Validate 按钮和 Save 路径对同一 output/context 返回同一字段错误；Save 不在 submit 前单独读取
  draft 再跑 validator。
- MCP required marker：`server_id` 始终 required，stdio `command` required，streamable HTTP `url` required；
  transport 切换后 marker 同步更新。
- MCP output-first validator：`validate_mcp_submit_output(output, context)` 与保存 request 使用同一个 submit
  output；Save 不在 submit 前调用 `validate_mcp_form(&self.draft, ...)`。
- MCP validator：server id shape/duplicate、stdio command、HTTP URL scheme、env var shape、header name/value、
  reserved header、duplicate header、OAuth 与 bearer token env var/Authorization header 冲突都落到对应字段。
- MCP dialog 不再持有 `validation_errors`；保存失败后的行内错误和 summary 都从 generated form field
  errors 派生。
- MCP add/remove/reorder 后 row input state 不错位。
- MCP env/header duplicate error 在 reorder 后仍落到同一 `FormItemId`。
- 删除 row 后对应 subscription 不再触发，旧 row 的 field errors 不再出现在 summary。
- 所有 placeholder 来自字段宏属性，不需要 post-creation patch helper。
- Required marker 在 UI 中通过 gpui-component `field().required(...)` 展示；空值和格式错误仍由 validator 拦截。
- Prompt edit：name/content trim 后写回 form；空 name/content 错误落到具体字段；create/edit 仍写入 prompts 表。
- Prompt edit：重复 name 在 create/edit 中落到 `name` 字段；edit 当前 prompt 保持原 name 不报重复；DB UNIQUE
  error 不作为常规用户路径。
- Shortcut edit：hotkey required/invalid/conflict 错误落到 hotkey 字段；model required 错误落到 model 字段；
  prompt optional；input source 和 enabled 能正确写入 `ShortcutDraft`。
- Shortcut edit：hotkey canonical 后和 temporary hotkey/其它 shortcut 冲突会阻止保存；provider/model 被删除或 disabled
  后，保存失败路径仍不会写入坏的 `settings_snapshot_json`。
- General P2：HTTP proxy 空值写 `None`，非空值按实际 runtime scheme 校验，config 写入失败不更新 saved/last value。
- General P2：temporary hotkey 可清空；非法 hotkey 不改变 config/runtime；config 写失败时 runtime rollback。
- ChatForm P3：config 中无 model 时可启动并回退到可用 model；submit 前 selected model 被删除/disabled 时不提交；
  token budget 输入按 capability bounds clamp；空 composer 且无 attachments 不提交。
