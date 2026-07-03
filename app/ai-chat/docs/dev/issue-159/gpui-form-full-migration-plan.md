# Issue #159 ai-chat2 gpui-form full migration plan

本文档记录 `app/ai-chat2` 在 `gpui-form` 强化后要完成的应用侧迁移。通用 form runtime、macro、
validation/transform 能力只写在 `crates/gpui-form/docs/validation-pipeline-strengthening-plan.md`；
本文只写 `ai-chat2` 的字段、组件、数据源、i18n 和保存链路。

## 当前状态

最后同步时间：2026-07-03。

本计划中的代码迁移已完成。当前 Provider、MCP、Prompt 和 Shortcut 的保存路径已经统一为：

- 字段级校验由 `gpui-form` generated store 调用 app-specific validator，失败返回
  `SubmitError::Invalid(FormValidationReport)`，不进入用户 handler。
- app 不再通过 `SubmitError::Handler(...)` 手动回填 field errors。
- required 空值错误进入 validator / required pipeline；UI marker 只负责展示。
- Prompt trim、Provider trim、Shortcut canonical hotkey、MCP 顶层字段 normalize 进入 `SubmitTransform`。
- MCP array 错误使用 generated field/index path 路由，不再维护 `McpSubmitRowIds` 或 row-specific
  `apply_*_error` helper。

迁移后 app 只负责：

- 提供 form-local validation context。
- 提供 app-specific validator / transform 类型。
- 在 submit output 已经 valid 的前提下执行 DB/config/keychain/runtime 保存。
- 渲染 `gpui-form` 提供的 required/error view state。

## 文件和模块结构

### Provider Settings

```text
app/ai-chat2/src/features/settings/provider/forms.rs
  - ProviderSettingsForm
  - ProviderSettingsFormOutput
  - ProviderValidationContext
  - ApiKeyProviderValidator / OllamaProviderValidator / CustomOpenAiProviderValidator
  - ApiKeyProviderTransform / OllamaProviderTransform / CustomOpenAiProviderTransform
  - common output -> payload/secret helpers

app/ai-chat2/src/features/settings/provider/forms/secret.rs
  - ProviderSecretValue
  - ProviderSecretDraft
  - ProviderSecretInputState
  - ProviderSecretInputBinding
  - impl RequiredValue for ProviderSecretValue

app/ai-chat2/src/features/settings/provider/forms/{api_key,ollama,custom_openai}.rs
  - #[form(validation(adapter = ..., context = ProviderValidationContext), transform(adapter = ...))]
  - keep gpui-form generated field stores as the only field state source

app/ai-chat2/src/features/settings/provider.rs
  - save handler no longer validates fields
  - Validate button calls form.validate(...) and derives status text from first report error
  - dirty snapshot reads normalized current output and secret changed state
```

Removed from normal Provider path:

- `ProviderSettingsFormOutput::validate(...)`
- `ProviderSettingsForm::apply_validation_issue(...)`
- field validation through `SubmitError::Handler(ProviderValidationIssue)`

### MCP Settings

```text
app/ai-chat2/src/features/settings/mcp/form_state.rs
  - McpServerFormInput
  - McpServerFormDraft
  - generated store construction with McpServerValidationContext

app/ai-chat2/src/features/settings/mcp/validation.rs
  - replace McpFormValidationError/McpFormField with gpui_form::ValidationIssue
  - McpServerValidator
  - McpServerValidationContext { original_server_id: Option<String>, existing_server_ids: Vec<String> }
  - McpServerTransform
  - no row-specific apply helper

app/ai-chat2/src/features/settings/mcp/dialog.rs
  - save handler only builds McpServerSaveRequest and starts config/keychain/OAuth task
  - structural changes re-run form validation instead of manually clearing/applying errors
```

Removed from normal MCP path:

- `validate_mcp_submit_output(...) -> Vec<McpFormValidationError>`
- `McpSubmitRowIds`
- `apply_arg_value_error`
- `apply_env_field_error`
- `apply_env_var_value_error`
- `apply_header_field_error`
- `apply_env_header_field_error`

### Prompt Edit

```text
app/ai-chat2/src/features/settings/prompts/form_state.rs
  - PromptEditFormInput
  - PromptEditValidationContext { prompt_id: Option<PromptId> }
  - PromptEditValidator
  - PromptEditTransform

app/ai-chat2/src/features/settings/prompts/dialog.rs
  - save handler only calls create_prompt/update_prompt and notification
  - no manual required/duplicate field errors
```

Removed from normal Prompt path:

- `PromptSaveError::Field`
- dialog-local `apply_field_error(...)`
- manual `form.clear_all_errors(cx)` before submit

### Shortcut Edit

```text
app/ai-chat2/src/features/settings/shortcuts/form_state.rs
  - ShortcutEditFormInput
  - ShortcutEditValidationContext {
      shortcut_id: Option<ShortcutId>,
      existing_shortcuts: Vec<ShortcutRecord>,
      temporary_hotkey: Option<String>,
    }
  - ShortcutEditValidator
  - ShortcutEditTransform
  - impl RequiredValue for ShortcutModelSelection

app/ai-chat2/src/features/settings/shortcuts/validation.rs
  - keep canonical_hotkey(...) and conflict helpers
  - return ValidationIssue or validator-local issue helper, not dialog field error

app/ai-chat2/src/features/settings/shortcuts/dialog.rs
  - save handler only builds ShortcutDraft from normalized output and writes DB
```

Removed from normal Shortcut path:

- `ShortcutSaveError::Field`
- dialog-local `apply_field_error(...)`
- manual `form.clear_all_errors(cx)` before submit

## 所用组件

No new visual components.

Existing components remain:

- Provider text/secret fields: `gpui_component::input::InputState` rendered through existing provider row helpers.
- Provider API mode: existing app-local `ProviderApiModeSelectBinding` and `gpui_component::select::SelectState`.
- MCP fields: existing text inputs, bool switch, row add/remove buttons, OAuth controls and status rows.
- Prompt content: existing multiline `InputState` via `PromptContentInputBinding`.
- Shortcut hotkey: existing `HotkeyInput`.
- Shortcut prompt/model: existing `gpui_component::select::SelectState`.
- Form rows continue to render with `gpui_component::form::field().required(...)` and visible error text.

`gpui-form` owns required/error view state; `ai-chat2` owns layout and concrete components.

## 自定义类型结构

### Provider

```rust
#[derive(Clone, Debug)]
pub(super) struct ProviderValidationContext {
    pub existing_secret_refs: ProviderSecretRefs,
}

pub(super) struct ApiKeyProviderValidator;
pub(super) struct OllamaProviderValidator;
pub(super) struct CustomOpenAiProviderValidator;

pub(super) struct ApiKeyProviderTransform;
pub(super) struct OllamaProviderTransform;
pub(super) struct CustomOpenAiProviderTransform;
```

Validation rules:

- API-key provider `api_key`: required if no saved ref and current secret is empty.
- API-key provider `base_url`: optional; non-empty must parse as `http` or `https`.
- Ollama `base_url`: required and must be `http` or `https`.
- Ollama `bearer_token`: optional.
- Custom OpenAI-compatible `name`: required.
- Custom OpenAI-compatible `api_key`: required with saved-ref semantics.
- Custom OpenAI-compatible `base_url`: required and must be `http` or `https`.
- `api_mode`: enum always has a selected value.

### MCP

```rust
#[derive(Clone, Debug)]
pub(super) struct McpServerValidationContext {
    pub original_server_id: Option<String>,
    pub existing_server_ids: Vec<String>,
}

pub(super) struct McpServerValidator;
pub(super) struct McpServerTransform;
```

Validator data sources:

- Existing server ids from `AiChat2ConfigStore`.
- Header/env/server id shape helpers from `app/ai-chat2/src/state/config.rs`.
- Current form draft/output and generated array index paths.

Validation rules:

- `server_id`: required, valid id shape, unique on create/rename.
- Stdio `command`: required.
- `args[]`: empty row ignored; whitespace-only non-empty row invalid.
- `env[].key`: required if row value is non-empty; valid env var name; unique.
- `env[].value`: can be empty when key is present.
- `env_vars[]`: non-empty values must be valid env var names and unique.
- `cwd`: optional string, no existence check.
- HTTP `url`: required, `http` or `https`.
- `bearer_token_env_var`: optional valid env var name; invalid if OAuth also manages Authorization.
- `headers[]` and `env_headers[]`: incomplete rows invalid; name valid and not reserved; literal value valid
  `http::HeaderValue`; env value valid env var name; header names unique across both arrays.
- `oauth_enabled`: bool, never required by itself.

### Prompt

```rust
#[derive(Clone, Debug)]
pub(super) struct PromptEditValidationContext {
    pub prompt_id: Option<PromptId>,
}

pub(super) struct PromptEditValidator;
pub(super) struct PromptEditTransform;
```

Validation rules:

- `name`: trim, required, unique except current prompt id.
- `content`: trim, required.

### Shortcut

```rust
#[derive(Clone, Debug)]
pub(super) struct ShortcutEditValidationContext {
    pub shortcut_id: Option<ShortcutId>,
    pub existing_shortcuts: Vec<ShortcutRecord>,
    pub temporary_hotkey: Option<String>,
}

pub(super) struct ShortcutEditValidator;
pub(super) struct ShortcutEditTransform;
```

Validation rules:

- `hotkey`: required, canonicalizable, modified key, valid `HotKey`, no temporary conflict, no other shortcut conflict.
- `model`: required and must reference current enabled provider/model option.
- `prompt`: optional; if selected id is stale, validator returns a field error.
- `input_source`: enum always valid.
- `enabled`: bool always valid.

## 数据流

### Form creation

```text
app state / DB / config snapshot
  -> form input value
  -> validation context
  -> GeneratedFormStore::from_value_with_validation_context(...)
  -> render uses generated *_state(), *_required(), field view state
```

### User input

```text
component event
  -> app-local binding emits FormComponentEvent
  -> gpui-form parses draft
  -> on_change/on_blur validator runs with current validation context
  -> field errors are stored inside generated form store
  -> render reads field view state
```

### Save

```text
Save button
  -> form.submit_sync or form.submit_async
  -> gpui-form parse + transform + required + validator
  -> invalid: returns SubmitError::Invalid and focuses/shows existing field errors
  -> valid output passed to handler
  -> handler writes DB/config/keychain/runtime and pushes notifications
```

### Validation context refresh

Context refresh remains app-owned:

- Provider: refresh context after a provider is saved or existing secret refs change.
- MCP: refresh context when dialog original id changes or config server ids change.
- Prompt: prompt id is fixed per dialog; duplicate query reads current prompt catalog/store during validation.
- Shortcut: refresh context when shortcut list or temporary hotkey changes.

No field values are copied out of `gpui-form` for validation.

## 全局数据管理

No new global store.

Existing global/state sources remain:

- `AiChat2ConfigStore` for `config.toml`, MCP definitions and app settings.
- `PromptCatalogStore` for prompt list and duplicate name checks.
- Existing provider state/repository helpers for provider records and model cache.
- Existing hotkey runtime state for temporary hotkey and registered shortcut diagnostics.

Forms remain page/dialog-local `Entity<...FormStore>` values.

## 数据库变更

No SQLite migration.

Validation changes do not alter:

- `providers`
- `provider_models`
- `prompts`
- `shortcuts`
- `projects`
- MCP config storage in `config.toml`

DB UNIQUE/FK constraints remain final protection, but field-level preflight validation must catch user-facing errors
before submit handler writes.

## 数据获取方式

- Provider validators use existing `ProviderSecretRefs` context and no network.
- MCP validators read `AiChat2ConfigStore` in `&App` for existing ids and use pure config helper functions.
- Prompt validator reads current prompt catalog/store through existing state APIs.
- Shortcut validator uses dialog-provided `existing_shortcuts` snapshot and existing hotkey helper functions.
- Save handlers keep existing DB/config/keychain write paths.

No new remote request is introduced by validation.

## icon

No new icon.

Existing icons remain:

- Provider settings existing provider/logo assets.
- MCP buttons/status existing icons.
- Prompt/shortcut dialog existing actions.
- Required marker remains text/star rendered by `gpui_component::form::field().required(...)`, not Lucide.

## i18n

Add or confirm these locale keys in `app/ai-chat2/locales/{en-US,zh-CN}/main.ftl`:

- `gpui-form-error-required`
- existing provider keys:
  - `provider-validation-required`
  - `provider-validation-url-invalid`
  - `provider-validation-url-scheme`
- existing MCP keys:
  - `mcp-validation-name-required`
  - `mcp-validation-name-invalid`
  - `mcp-validation-name-duplicate`
  - `mcp-validation-command-required`
  - `mcp-validation-url-required`
  - `mcp-validation-url-invalid`
  - `mcp-validation-url-scheme`
  - header/env row validation keys already used today
- existing Prompt keys:
  - `prompt-validation-name-required`
  - `prompt-validation-content-required`
  - `prompt-validation-name-duplicate`
- existing Shortcut keys:
  - `shortcut-validation-hotkey-required`
  - `shortcut-validation-hotkey-invalid`
  - conflict/model required keys already used today

If `gpui-form` emits generic `gpui-form-error-required`, app-specific validators can still emit existing
app-specific keys when a more precise message is needed.

## 新增依赖库

No new dependency.

Continue using current crates:

- `gpui-form`
- `gpui-form-gpui-component`
- `gpui-component`
- `url`
- `http`
- existing hotkey/provider/prompt/MCP helper crates already in `ai-chat2`

## 已完成迁移步骤

1. 已完成：land `gpui-form` strengthening:
   - custom validator context
   - required validation
   - custom transform adapter
   - array path routing
   - `SubmitStart` removal
   - field view state helper

2. 已完成：Provider:
   - add provider validators/transforms
   - move `ProviderSettingsFormOutput::validate` into validators
   - update generated store attrs
   - remove `SubmitError::Handler(ProviderValidationIssue)` field-error path
   - keep save handler focused on secret writes, provider payload and notifications

3. 已完成：MCP:
   - convert `validation.rs` to `ValidationIssue`
   - add `McpServerTransform`
   - remove `McpSubmitRowIds`
   - remove row-specific apply helpers
   - save handler only builds `McpServerSaveRequest`

4. 已完成：Prompt:
   - add `PromptEditValidator` and `PromptEditTransform`
   - remove `PromptSaveError::Field`
   - save handler only writes prompt repository

5. 已完成：Shortcut:
   - add `ShortcutEditValidator` and `ShortcutEditTransform`
   - keep `canonical_hotkey` as pure helper
   - remove `ShortcutSaveError::Field`
   - save handler only writes shortcut repository/runtime update

6. 已完成：Cleanup:
   - search app code for `apply_field_error`, `clear_all_errors`, `SubmitError::Handler(...Field...)`
   - normal business paths must not call generated error helpers
   - tests may keep internal helper use only when testing `gpui-form` itself

## 验收标准

- Provider/MCP/Prompt/Shortcut save handlers do not perform field-level validation.
- Field-level validation failures return `SubmitError::Invalid`, not `SubmitError::Handler`.
- App code no longer calls generated `apply_field_error` / row-specific apply helpers in normal paths.
- Required empty fields show errors without app manually creating required field errors.
- Change/blur validation works for fields configured with `validate(on_change/on_blur)`.
- Submit transform writes normalized values back to component state before persistence.
- MCP array row errors display on the correct row without `McpSubmitRowIds`.
- Dirty checks use normalized form output plus legitimate app-specific facts such as secret changed/ref state.

## 验证命令

- `cargo fmt`
- `cargo test -p gpui-form --features form-pipeline`
- `cargo test -p gpui-form-gpui-component`
- `cargo test -p ai-chat2 provider`
- `cargo test -p ai-chat2 mcp`
- `cargo test -p ai-chat2 prompt`
- `cargo test -p ai-chat2 shortcut`
- `cargo check -p gpui-form -p gpui-form-gpui-component -p ai-chat2`
- `cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p ai-chat2 --all-targets --all-features -- -D warnings`
- `git diff --check`
