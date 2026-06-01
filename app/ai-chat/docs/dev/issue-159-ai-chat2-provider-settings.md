# Issue #159 ai-chat2 Provider 设置专项计划

本文档是 `app/ai-chat2` Provider 设置页的可执行开发文档。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档固定 provider 配置、模型刷新、
模型能力缓存、secret 保存、Rig 对齐、模块结构、GPUI 组件选型和 app-local entity 结构。

最后同步时间：2026-06-02。

当前状态：已开始实现。`ai-chat-db` 已补 provider/model list/update/delete、`provider_models.enabled`
和保留 enabled 的 fetch upsert 合同；`ai-chat2` 已接 Settings Provider 页骨架、provider registry、
draft/model/capability 模块、DB-backed enabled model helper、模型 enabled toggle、Provider 设置
i18n、未保存 provider 默认 disabled，以及左右两列独立滚动布局。尚未完成真实 GPUI keychain 写读、
Rig client factory、远端 model fetcher、manual model editor 或 DB-backed composer model picker。

## 目标边界

- Provider 设置页是 agent runtime 的前置能力。真实 `AgentRuntime` 接线之前，必须先能保存
  可被 Rig 构造的 provider 配置、刷新模型列表，并把每个模型的 `ModelCapabilitiesSnapshot`
  写入 fresh DB。
- v1 以 Rig 0.37 内置 provider 为优先范围。Alma 的 Settings/Provider 页面作为布局参考：
  左侧 provider 搜索和列表，右侧选中 provider 的启用状态、配置表单、校验、模型刷新和模型列表。
- Zed 的 provider 设置只作为配置字段和能力推导参考。Zed 的模型列表主要来自 settings 文件或
  provider 内置枚举；`ai-chat2` 不采用这个方向，fresh DB 的 `providers` 和 `provider_models`
  是唯一持久化真相。
- v1 支持每个模型单独启用/禁用。`provider_models` 需要新增 `enabled BOOLEAN NOT NULL DEFAULT 1
  CHECK (enabled IN (0, 1))`；刷新模型时保留已有模型的 enabled 状态，新模型默认 enabled。
- 本阶段不接真实 agent run、timeline、prompt settings、shortcut settings、ACP/CLI provider、
  subscription/OAuth provider 或 GitHub Copilot 类认证。

## 实现模块结构

禁止新增 `mod.rs`。实现时使用现有仓库模式：入口文件与模块同名，子模块放在同名目录下。

```text
app/ai-chat2/src/features/settings.rs
  - 增加 `mod provider;`
  - `SettingsView` 持有 `provider_settings: Entity<ProviderSettingsPage>`
  - `SettingsPageKey` 增加 `Provider`
  - Provider page title 使用 `settings-page-provider` i18n key
  - `settings_page_specs` 增加 Provider 搜索词：
    provider, providers, model, models, OpenAI, Ollama, Anthropic, Gemini,
    API Key, Base URL, endpoint, keychain, fetch models
  - render match 增加 `SettingsPageKey::Provider => self.provider_settings.clone()`
  - Provider page 使用 `SettingsPageFrame::no_outer_body_scroll()`，避免外层滚动条和列内滚动条叠加

app/ai-chat2/src/features/settings/provider.rs
  - `ProviderSettingsPage` 顶层 entity
  - 页面双栏布局、事件入口、load/save/validate/fetch orchestration
  - 只持有 UI state 和 async task handle，不直接散落 provider-specific 规则
  - 未保存 built-in provider draft 默认 disabled；已保存 provider 使用 DB row 的 enabled 状态
  - 左侧 provider list 和右侧 detail 采用独立 `overflow_y_scrollbar`

app/ai-chat2/src/features/settings/provider/catalog.rs
  - Rig-first provider registry
  - provider display metadata、field schema、defaults、endpoint presets、capability strategy
  - provider 品牌名保持原文；description、field label、placeholder、select option 通过 i18n key 渲染
  - built-in provider 在左侧列表中始终可见；没有 DB row 时选择它创建 draft，保存后才落库

app/ai-chat2/src/features/settings/provider/draft.rs
  - `ProviderDraft`、`ProviderModelDraft`、validation result、dirty tracking
  - DB record <-> UI draft <-> `ProviderSettingsPayload` / `ProviderSecretRefs` 转换

app/ai-chat2/src/features/settings/provider/components.rs
  - app-local `RenderOnce` 组合组件：
    `ProviderListPane`、`ProviderDetailPane`、`ProviderFieldControl`、
    `ProviderModelTable`、`CapabilityTagRow`
  - 这些组件不持有状态，只接收 snapshot props 和 callback

app/ai-chat2/src/features/settings/provider/secret_store.rs
  - GPUI keychain wrapper
  - 封装 `cx.write_credentials`、`cx.read_credentials`、`cx.delete_credentials`
  - UI 和 DB 永远不接收 secret 原文；`has_provider_secret` 只返回 bool

app/ai-chat2/src/features/settings/provider/model_fetch.rs
  - Rig/provider-specific model listing adapter
  - Ollama 使用 tags + show/model metadata 路径
  - no-listing provider 返回 typed manual-model-required 状态

app/ai-chat2/src/features/settings/provider/capabilities.rs
  - provider-specific capability derivation
  - manual capability override merge for custom provider/manual models

app/ai-chat2/src/state/providers.rs
  - DB-backed provider/model query helpers for Settings and later Composer
  - `enabled_provider_models(cx)` 只返回 `provider.enabled && model.enabled` 的模型

crates/ai-chat-db/src/records.rs / repository.rs / tests.rs
  - provider update/list/delete
  - provider model list/delete/bulk upsert/enabled toggle
```

Icon 增补只改 `app/ai-chat2/src/foundation/assets.rs` 的 app-local `IconName`。已确认 Lucide
slug 存在，可按需增加：`bot`、`server`、`cloud`、`key-round`、`refresh-ccw`、`eye`、
`eye-off`、`circle-check`、`circle-alert`、`circle-off`、`globe`、`cpu`、`zap`。功能代码不得
散落 raw SVG path 或 `include_bytes!`。

## gpui-component 使用清单

优先消费 `gpui-component`，只补 app-local composition，不重写通用控件。

| 组件 | 用途 |
| --- | --- |
| `Input` / `InputState` | provider 搜索、model 搜索、name/base URL/API version/deployment id 等文本字段 |
| `InputState::masked(true)` + `Input::mask_toggle()` | API key、bearer token、Azure token，禁止明文回填已保存 secret |
| `Button` | Save、Validate、Fetch Models、Add Custom Provider、Add Model、Delete；async 时用 loading/spinner |
| `Switch` | provider enabled、model enabled |
| `Select` | API mode、endpoint preset、Anthropic version、endpoint region 等单选字段 |
| `GroupBox` | Configuration、Advanced、Models、Manual Capability 分组 |
| `Table` | 模型列表：enabled、model id、display name、capability badges、fetched_at、actions |
| `Tag` | reasoning、tools、vision、structured output、web search、manual 等能力/状态 badge |
| `Notification` | save/validate/fetch success/error，复用 Settings root notification layer |
| `AlertDialog` / 现有 delete confirm | 删除 custom provider、删除 manual model |
| `ScrollableElement` | provider list、模型表、右侧配置详情滚动 |
| `Tooltip` | icon-only action button 说明 |

v1 不默认使用 `DataTable`。模型列表是中小规模静态表，先用 `Table`；后续确实遇到大列表、
排序、列管理或虚拟滚动需求，再切到 `VirtualList` / `DataTable`。

## app-local 组件和 Entity 结构

状态集中在 `ProviderSettingsPage`、必要的输入 entity 和 manual editor 中。
`ProviderListPane`、`ProviderDetailPane`、`ProviderFieldControl`、`ProviderModelTable`、
`CapabilityTagRow` 只作为 `RenderOnce` props 组件，负责布局和回调，不持有业务状态。

```rust
pub(super) type ProviderKindKey = &'static str;

pub(super) struct ProviderSettingsPage {
    provider_search: Entity<InputState>,
    model_search: Entity<InputState>,
    selected: ProviderSelection,
    providers: Vec<ProviderListItem>,
    models: Vec<ProviderModelDraft>,
    draft: ProviderDraft,
    text_inputs: BTreeMap<String, Entity<InputState>>,
    secret_inputs: BTreeMap<String, Entity<ProviderSecretInput>>,
    select_inputs: BTreeMap<String, Entity<SelectState<SearchableVec<ProviderSelectOption>>>>,
    validation: ProviderValidationState,
    save_state: AsyncActionState,
    fetch_state: AsyncActionState,
    manual_model_editor: Option<Entity<ManualModelEditor>>,
    _subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
    _save_task: Option<Task<()>>,
    _fetch_task: Option<Task<()>>,
}

pub(super) enum ProviderSelection {
    Builtin { kind: ProviderKindKey, provider_id: Option<ProviderId> },
    Custom { provider_id: ProviderId },
    NewCustom,
}

pub(super) struct ProviderListItem {
    kind: ProviderKindKey,
    provider_id: Option<ProviderId>,
    display_name: SharedString,
    description: SharedString,
    enabled: bool,
    configured: bool,
    missing_required: bool,
    source: ProviderListItemSource,
}

pub(super) enum ProviderListItemSource {
    Builtin,
    Custom,
}

pub(super) struct ProviderDraft {
    provider_id: Option<ProviderId>,
    kind: ProviderKindKey,
    display_name: String,
    enabled: bool,
    fields: BTreeMap<String, ProviderDraftValue>,
    existing_secret_refs: ProviderSecretRefs,
    dirty: bool,
}

pub(super) enum ProviderDraftValue {
    String(String),
    Bool(bool),
    Number(f64),
    Object(ProviderRawPayload),
}

pub(super) struct ProviderSecretInput {
    key: String,
    input: Entity<InputState>,
    saved_ref_id: Option<String>,
    has_saved_secret: bool,
    dirty: bool,
    validation_error: Option<SharedString>,
    _subscription: Subscription,
}

pub(super) struct ProviderModelDraft {
    row_id: Option<ProviderModelId>,
    provider_id: ProviderId,
    model_id: String,
    display_name: Option<String>,
    enabled: bool,
    capabilities: ModelCapabilitiesSnapshot,
    metadata: ProviderModelMetadata,
    fetched_at: Option<OffsetDateTime>,
    dirty: bool,
}

pub(super) struct ManualModelEditor {
    mode: ManualModelEditorMode,
    model_id_input: Entity<InputState>,
    display_name_input: Entity<InputState>,
    context_window_input: Entity<InputState>,
    capabilities: CapabilityDraft,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

pub(super) enum ManualModelEditorMode {
    Add,
    Edit { model_id: ProviderModelId },
}

pub(super) struct CapabilityDraft {
    text_input: bool,
    text_output: bool,
    streaming: bool,
    image_input: bool,
    image_generation: bool,
    tool_calling: bool,
    hosted_web_search: bool,
    reasoning: bool,
    structured_output: bool,
    context_window_tokens: Option<u32>,
}

pub(super) struct ProviderSelectOption {
    label: SharedString,
    value: String,
    description: Option<SharedString>,
}

pub(super) struct ProviderValidationState {
    status: ProviderValidationStatus,
    field_errors: BTreeMap<String, SharedString>,
    secret_errors: BTreeMap<String, SharedString>,
    message: Option<SharedString>,
}

pub(super) enum ProviderValidationStatus {
    NotValidated,
    Validating,
    Valid,
    Invalid,
}

pub(super) struct AsyncActionState {
    running: bool,
    message: Option<SharedString>,
}
```

Entity 使用规则：

- 所有 entity 通过 `cx.new(...)` 创建；回调里捕获 `WeakEntity`，不要捕获 strong entity 造成 retain cycle。
- `ProviderSettingsPage` render 时只读取自身 snapshot，不在 `RenderOnce` 子组件里读写 parent entity。
- 输入变更通过 subscription 更新 draft 并 `cx.notify()`；不要在 input subscription 里嵌套更新同一个 entity。
- save/fetch/remote verify 用 `cx.spawn` 前台 task。需要重计算 capability 的纯 CPU 工作可以用
  `cx.background_spawn(...).then(cx.spawn(...))` 回到前台更新 entity。

## 数据和保存规则

- `providers` 保存 provider 实例：`kind`、`display_name`、`enabled`、非 secret
  `ProviderSettingsPayload` 和 `ProviderSecretRefs`。
- `provider_models` 保存设置页刷新或用户手动添加后的模型 cache：`model_id`、display name、
  `enabled`、`ModelCapabilitiesSnapshot`、`ProviderModelMetadata` 和 `fetched_at`。
- API key、bearer token、Azure token 等 secret 不进入 DB，也不进入 run snapshot。DB 只保存
  `ProviderSecretRef { key, storage: "keychain", ref_id }`。
- `ref_id` 使用稳定 provider secret scope：`ai-chat2/provider/{provider_id}/{secret_key}`。
  GPUI credential `url` 使用 `ref_id`，`username` 使用 secret key，password bytes 使用 secret value。
- `has_provider_secret` 可以读取 credential 并立即丢弃 password，但不能把 secret 明文交给 UI state、
  notification、日志、DB payload 或 run snapshot。
- 保存前必须先校验配置：必填字段、URL 格式、同 provider 下模型 id 唯一性、secret 是否可读或本次已输入、
  以及该配置能否构造对应 Rig client。
- 校验通过后再写 secret、upsert provider。可联网 provider 如果支持低成本 verify 或 model listing，
  保存流程应执行验证；不支持无成本验证的 provider 要清楚标记为“已做本地 Rig 构造校验，远端验证需模型刷新或首次运行”。
- 模型刷新来自设置页。刷新成功后把所有返回模型 upsert 到 `provider_models`，同步保存能力 snapshot；
  刷新失败不能删除旧 cache。
- 自定义 OpenAI-compatible provider 必须允许手动模型和手动能力，因为兼容接口通常不会返回完整 capability。

## Provider 配置矩阵

| Provider | v1 状态 | 必填配置 | 可选配置 | 模型获取 | 能力保存策略 |
| --- | --- | --- | --- | --- | --- |
| OpenAI | first-class | API key | Base URL，API mode 默认 Responses | Rig `/models` | OpenAI 规则推导 reasoning、tools、image、structured output、continuation |
| Anthropic | first-class | API key | Base URL，Anthropic version，betas | Rig `/v1/models` | Anthropic 规则推导 tools、image、reasoning/cache 相关能力 |
| Google Gemini | first-class | API key | Base URL，API mode 默认 GenerateContent | Rig model listing | Gemini metadata 加规则推导 |
| Ollama | first-class | Base URL 默认 `http://localhost:11434` | Bearer token，auto discover | tags + show/model metadata | show metadata 推导 tools、vision、thinking、context |
| OpenRouter | first-class | API key | Base URL | Rig model listing | provider metadata 加 conservative fallback |
| DeepSeek | first-class | API key | Base URL | Rig model listing when available | known DeepSeek model rules |
| Moonshot/Kimi | first-class | API key | Global/China/Anthropic-compatible endpoint | Rig/static-compatible fallback | known model rules，无法 listing 时允许手动 override |
| Z.AI | first-class | API key | General/Coding/Anthropic-compatible endpoint | no listing fallback | known model rules + manual model add |
| Azure OpenAI | first-class | API key/token，Azure endpoint，deployment/model id | API version | no generic listing | deployment rows 是用户管理模型 |
| Mistral | first-class | API key | Base URL | Rig model listing | metadata + known model rules |
| xAI | first-class | API key | Base URL | no listing fallback | manual/known model rules |
| Groq | first-class | API key | Base URL | no listing fallback | known model rules |
| Perplexity | first-class | API key | Base URL | no listing fallback | known model rules；Sonar 类模型标记 hosted web search |
| Together | first-class | API key | Base URL | no listing fallback | manual/known model rules |
| Custom OpenAI-compatible | first-class custom | Name，API key，Base URL，至少一个模型 | 自定义 headers 后置 | 用户添加 / 可选 `/models` probe | 手动 capability toggles |
| ACP/CLI/subscription providers | deferred | TBD | TBD | outside Rig path | 不进入 v1，后续单独设计认证和 runtime |

## Provider registry 计划

实现时在 `catalog.rs` 建立 provider registry，而不是把字段散落在 UI 组件里。每个 registry entry
至少定义：

- `provider_kind`、display name、description、icon、默认 endpoint、是否支持 Rig verify、是否支持 model listing。
- config field schema：非 secret fields、secret refs、默认值、必填/可选、输入类型和验证规则。
- endpoint presets：例如 Moonshot/Kimi Global/China/Anthropic-compatible、Z.AI General/Coding/Anthropic-compatible。
- Rig client factory：从 `ProviderRecord` + keychain secret 构造 Rig provider client 和 completion model。
- model fetcher：调用 Rig `ModelListingClient`、provider-specific fetch，或返回“需要手动模型”的 typed 状态。
- capability mapper：从 provider kind、model id、Rig model metadata、provider raw metadata 和用户手动 override
  生成 `ModelCapabilitiesSnapshot`。
- settings snapshot builder：为 `RunSettingsSnapshot` 提供去 secret 的 provider settings 和 model capabilities。

## Repository 和 secret API 计划

`ai-chat-db` 现有 `insert_provider`、`get_provider`、`upsert_provider_model`、`get_provider_model`
不足以支撑设置页。实现时需要补充并修改：

- `provider_models` schema 增加 `enabled BOOLEAN NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))`。
- `ProviderModelRecord` / `NewProviderModel` 增加 `enabled: bool`。
- `upsert_provider_model` 插入新模型时使用 `NewProviderModel.enabled`；发生 conflict 时不得覆盖已有
  enabled，除非调用显式 toggle API。
- `list_providers`，按 display name / kind 稳定排序，并保留 custom provider。
- `update_provider`，支持 display name、enabled、settings、secret refs 更新。
- `delete_provider`，删除 provider 时级联删除 provider models，但不得删除 keychain secret；secret 清理走显式动作。
- `list_provider_models(provider_id)`，供设置页和 composer model picker 使用。
- `set_provider_model_enabled(provider_id, model_id, enabled)`，只改 enabled 和 `updated_at`。
- `delete_provider_model(provider_id, model_id)`，供手动模型管理使用。
- `replace_fetched_provider_models(provider_id, fetched)`，事务型批量 upsert 本次 fetch 返回的模型；不删除旧模型。
- `replace_fetched_provider_models` 必须保留已有模型的 enabled 和手动 capability override；新模型默认 enabled。

Secret wrapper 需要封装 GPUI credentials API：

- `write_provider_secret(provider_id, key, value)`。
- `read_provider_secret(provider_ref)`。
- `delete_provider_secret(provider_ref)`。
- `has_provider_secret(provider_ref)`，用于显示“已保存 secret”而不把明文交给 UI。

`state::providers` 需要提供：

- `list_provider_settings(cx)`，给 Settings Provider page 加载 providers 和 models。
- `enabled_provider_models(cx)`，只返回 `provider.enabled && model.enabled` 的模型，供后续 composer picker。
- `save_provider_draft(cx, draft, secret_refs)`，内部只调用 repository，不接触 secret 原文。

## UI 状态流

加载页面：

1. 读取 registry built-in providers。
2. 从 DB 读取 `providers` 和 `provider_models`。
3. 合并成左侧列表：built-in 永远显示；custom rows 追加显示。
4. 默认选中已启用且配置完整的第一个 provider；否则选中 OpenAI draft。

选择 provider：

1. 如果 DB row 存在，用 row + registry schema 构造 `ProviderDraft`。
2. 如果是 built-in 且没有 row，用 registry defaults 构造 unsaved draft。
3. 重建 text/select/secret inputs；secret input 只显示 empty/saved/dirty 状态。
4. 加载该 provider 的 model drafts；model search 只过滤当前 provider models。

保存 provider：

1. 从 UI draft 收集非 secret fields、secret draft 和 enabled 状态。
2. 校验 field schema、URL、模型 id、manual capability 和必填 secret。
3. 用 draft 配置和 secret 构造 Rig client；构造失败则阻止保存。
4. 如果 provider 支持无成本 verify，则调用 verify；如果只支持 model listing，则提示用户使用 Fetch Models 验证。
5. 写入/更新 keychain secret，并生成 `ProviderSecretRefs`。
6. insert/update provider row。DB payload 不包含 secret 原文。
7. reload provider list 和 draft；成功后 notification。

刷新模型：

1. 只允许对已保存 provider 执行；unsaved draft 先保存。
2. 读取 provider row 和 secret refs。
3. 构造 Rig client 或 provider-specific client。
4. 获取模型列表；对 Ollama 额外调用 show/model metadata。
5. 对每个模型生成 `ProviderModelMetadata` 和 `ModelCapabilitiesSnapshot`。
6. `replace_fetched_provider_models` upsert cache；保留已有 enabled 和 manual override。
7. 刷新失败只显示 error notification，不清空旧 cache。

手动模型：

1. Custom OpenAI-compatible、Azure OpenAI、no-listing providers 支持 Add Model。
2. `ManualModelEditor` 校验 model id 非空、同 provider 唯一、context window 为正整数或空。
3. 保存 manual model 时默认 enabled，能力来自 `CapabilityDraft`。
4. 删除 manual model 使用 `AlertDialog` / 现有 delete confirm。

## 验证计划

- 文档阶段：`git diff --check`；新增未跟踪文档用 `git diff --check --no-index -- /dev/null <file>` 检查 whitespace。
- DB API：覆盖 list/update/delete provider、enable/disable、secret ref roundtrip 不含 secret 原文、模型 list/delete、model enabled toggle、批量 upsert 保留 enabled、capability persistence。
- Rig 对齐：每个 first-class provider 的最小有效配置能构造对应 Rig client；缺字段、坏 URL、缺 secret 返回 typed validation error。
- 模型刷新：mock 成功、认证失败、解析失败、空列表、重复 model id、保留旧 cache、保留 disabled/manual models。
- UI state：`cargo test -p ai-chat2 provider` 覆盖 draft validation、field schema mapping、capability derivation、manual model editor。
- Provider i18n / layout baseline：覆盖 Provider title i18n、Provider 搜索词、Provider 文案 key、未保存 provider 默认 disabled、已保存 provider 保留 DB enabled。
- UI compile：`cargo fmt`、`cargo check -p ai-chat2`。
- 手动 Settings smoke：OpenAI 保存/验证/fetch、Ollama localhost fetch、custom OpenAI-compatible manual model + capability toggle。

## 后续顺序

1. 已补 `ai-chat-db` provider/model repository API 和 `provider_models.enabled`。
2. 已增加 `ai-chat2` provider registry、draft model、secret refs wrapper stub 和 Settings Provider 页面骨架。
3. 已补 Provider i18n、未保存 provider 默认 disabled、左右列独立滚动布局和对应测试。
4. 下一步补 GPUI keychain secret store、Rig client factory、真实 validation/fetch、manual model editor。
5. 再把 ChatForm model picker 从 preview data 切到 DB-backed enabled provider models。
6. 再接 prompt selector、conversation create/timeline 和真实 `AgentRuntime`。
