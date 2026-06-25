# Issue #159 ai-chat2 Provider 设置专项计划

本文档是 `app/ai-chat2` Provider 设置页的可执行开发文档。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159/README.md`；本文档固定 provider 配置、模型刷新、
模型能力缓存、secret 保存、Rig 对齐、模块结构、GPUI 组件选型和 app-local entity 结构。

最后同步时间：2026-06-03。

当前状态：Provider settings 第一阶段已实现并推送到 `codex/issue-159-ai-chat2-ui`
（`4d4110b feat(ai-chat2): wire provider settings model fetch`）。`ai-chat-db` 已补
provider/model list/update/delete、`provider_models.enabled` 和保留 enabled 的 fetch upsert
合同；`ai-chat2` 已接 Settings Provider 页、provider registry、draft/model/capability 模块、
DB-backed enabled model helper、保存前本地校验、未保存状态标签、GPUI credentials secret
写读、Provider 设置 i18n、未保存 provider 默认 disabled、左右两列独立滚动布局、
`gpui-component::ListState` provider/model 搜索列表、provider/model list panel + row separator 视觉、
模型 enabled toggle，以及真实模型刷新入口。
`crates/gpui-tokio` 已提供 GPUI -> Tokio runtime bridge，`crates/ai-chat-agent` 已提供共享
provider model listing API，并开始为 provider model cache 写入 capability source 和 provider-specific
reasoning control。DB-backed composer model picker 已实现，见
`app/ai-chat/docs/dev/issue-159/composer-model-picker.md`；它只读取 enabled provider/model
cache，不读取 keychain，不启动 agent run。provider brand logo 后续已通过 app-owned
`ProviderVisual` 和 vendored SVG 资产完成。尚未完成 manual model editor、prompt/provider/model
运行时接线、完整 Rig completion client factory 和真实 agent run。

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
  - 保存前执行本地字段/secret 校验；已保存 provider 修改后用 dirty snapshot 显示“未保存”
  - provider 列表和 model 列表使用 `gpui-component::ListState` 内置搜索；业务真相仍在
    `ProviderSettingsPage`
  - provider list 通过 `ListEvent::Select/Confirm` 单向同步到右侧 detail；model enabled switch
    通过 `ListEvent::Confirm` 回传页面统一处理；delegate 不直接更新页面
  - 左侧 provider list 和右侧 detail/model list 使用独立滚动区域；Provider page 禁用外层 body scroll
  - 右侧 detail header 固定，Configuration + Models 共用一个 detail scroll viewport

app/ai-chat2/src/features/settings/provider/catalog.rs
  - Rig-first provider registry
  - provider display metadata、field schema、defaults、endpoint presets、capability strategy
  - provider 品牌名保持原文；description、field label、placeholder、select option 通过 i18n key 渲染
  - provider icon 当前只记录 generic Lucide fallback；brand logo asset 不能放进 `IconName`
  - built-in provider 在左侧列表中始终可见；没有 DB row 时选择它创建 draft，保存后才落库
  - `ModelListingStrategy` 统一驱动 fetch support，避免 catalog 和 fetch support 双份硬编码漂移

app/ai-chat2/src/features/settings/provider/draft.rs
  - `ProviderDraft`、`ProviderModelDraft`、validation result、dirty tracking
  - DB record <-> UI draft <-> `ProviderSettingsPayload` / `ProviderSecretRefs` 转换
  - `ProviderDraftSnapshot` 负责和 DB saved snapshot 比较；secret dirty 只记录 key，不记录 secret 原文

app/ai-chat2/src/features/settings/provider/components.rs
  - app-local `RenderOnce` 组合组件保留为后续抽取位置
  - 当前 provider/model list 已迁入 `list_delegates.rs`；表单和 header 仍在 `provider.rs`

app/ai-chat2/src/features/settings/provider/list_delegates.rs
  - `ProviderListDelegate` / `ProviderModelListDelegate`
  - provider list 和 model list 的 `ListState` 搜索、过滤、虚拟列表 row 渲染
  - provider/model delegate 只持有 rows snapshot，不持有 page entity
  - provider 选择变化通过 `ListEvent` 返回 `ProviderSettingsPage`
  - model row switch 只向自身 `ListState` 发 `ListEvent::Confirm`，由 `ProviderSettingsPage`
    读取当前 filtered row 并执行即时保存
  - provider/model rows 都使用行间 separator；provider row 不再有独立 item border/gap，provider list
    和搜索框一起包在整体 panel 中

app/ai-chat2/src/features/settings/provider/secret_store.rs
  - GPUI keychain wrapper
  - 当前封装 `cx.write_credentials`、`cx.read_credentials`
  - UI 和 DB 永远不接收 secret 原文；`has_provider_secret` 只返回 bool
  - 当前已实现保存 dirty secret 到 GPUI credentials、fetch 前从 credentials 读取 secret values；
    delete/has helper 后续补齐

app/ai-chat2/src/features/settings/provider/model_fetch.rs
  - Settings 侧 fetch support / manual model helper
  - no-listing provider 返回 typed manual-model-required 状态
  - 真实 provider listing API 已下沉到 `crates/ai-chat-agent/src/provider_models.rs`

app/ai-chat2/src/features/settings/provider/capabilities.rs
  - provider-specific capability derivation
  - manual capability override merge for custom provider/manual models
  - 当前使用 `ai_chat_core::conservative_model_capabilities` 和 manual capability draft

app/ai-chat2/src/state/providers.rs
  - DB-backed provider/model query helpers for Settings and later Composer
  - `enabled_provider_models(cx)` 只返回 `provider.enabled && model.enabled` 的模型
  - Composer model picker 已读取该 helper，不直接访问 Settings draft 或 keychain

crates/ai-chat-agent/src/provider_models.rs
  - 共享 provider model listing API，Settings 和后续 Agent runtime 复用
  - `ProviderSecretValues`、`ProviderModelFetchRequest`、`ProviderModelFetchError`
  - OpenAI、Anthropic、Gemini、Ollama、OpenRouter、DeepSeek、Mistral 走 Rig `ModelListingClient`
  - Azure OpenAI、Moonshot/Kimi、Z.AI、xAI、Groq、Perplexity、Together、Custom OpenAI-compatible
    返回 manual-model-required

crates/ai-chat-core/src/capabilities.rs
  - `ModelCapabilitiesSnapshot` 和 conservative capability defaults
  - fetch 返回的 model row 先保存 conservative capability snapshot，后续 manual override 再扩展

crates/ai-chat-db/src/records.rs / repository.rs / tests.rs
  - provider update/list/delete
  - provider model list/delete/bulk upsert/enabled toggle

crates/gpui-tokio
  - repo-local GPUI -> Tokio bridge
  - `gpui_tokio::init(cx)` 注册 2 worker Tokio runtime 为 GPUI global
  - `gpui_tokio::Tokio::spawn(cx, future)` 运行 reqwest/hyper/Tokio I/O future，避免 GPUI async 中
    直接调用 Tokio I/O 出现 `there is no reactor running`
```

Icon 增补只改 `app/ai-chat2/src/foundation/assets.rs` 的 app-local `IconName`。已确认 Lucide
slug 存在，可按需增加：`bot`、`server`、`cloud`、`key-round`、`refresh-ccw`、`eye`、
`eye-off`、`circle-check`、`circle-alert`、`circle-off`、`globe`、`cpu`、`zap`。功能代码不得
散落 raw SVG path 或 `include_bytes!`。

Provider brand logo 不属于 Lucide icon 增补。Lucide v1 已移除品牌图标，并建议改用品牌官方
SVG 或 Simple Icons；因此 OpenAI、Anthropic、Gemini、Ollama、OpenRouter 等 provider logo
后续应作为 `app/ai-chat2` 自有 runtime asset 管理，并为来源、许可或 brand guideline 留下记录。
UI 渲染应使用 provider-logo abstraction：有 brand asset 时渲染品牌 SVG；没有或不允许使用时保留
`IconName::Cloud` / `IconName::Cpu` / `IconName::Server` 等 generic Lucide fallback。不要把品牌
SVG 加入 `define_lucide_icons!`，也不要让 feature code 直接拼 asset path。

## gpui-component 使用清单

优先消费 `gpui-component`，只补 app-local composition，不重写通用控件。

| 组件 | 用途 |
| --- | --- |
| `List` / `ListState` | provider 搜索列表、model 搜索列表、虚拟滚动、选中态和 row rendering |
| `Input` / `InputState` | name/base URL/API version/deployment id 等文本字段；List 内置搜索框由 `ListState::searchable(true)` 提供 |
| `InputState::masked(true)` + `Input::mask_toggle()` | API key、bearer token、Azure token，禁止明文回填已保存 secret |
| `Button` | Save、Validate、Fetch Models、Add Custom Provider、Add Model、Delete；async 时用 loading/spinner |
| `Switch` | provider enabled、model enabled |
| `Select` | API mode、endpoint preset、Anthropic version、endpoint region 等单选字段 |
| `GroupBox` | Configuration、Advanced、Models、Manual Capability 分组 |
| `Tag` | reasoning、tools、vision、structured output、web search、manual、capability source 等能力/状态 badge |
| `Notification` | save/validate/fetch success/error，复用 Settings root notification layer |
| `AlertDialog` / 现有 delete confirm | 删除 custom provider、删除 manual model |
| `ScrollableElement` | provider list、model list、右侧配置详情滚动 |
| `Tooltip` | icon-only action button 说明 |

v1 不默认使用 `Table` / `DataTable`。Provider list 和 model list 都使用
`gpui-component::ListState`：搜索由 ListState 内置 query input 完成，row snapshot 由页面从
DB/registry 派生后交给 delegate。Provider list 和 model list 均作为带搜索的整体面板渲染：
面板外框负责边界，row 内部不再画卡片 border，行与行之间用 separator 区分。后续确实需要排序、
列管理或大规模表格能力时，再评估 `DataTable`。

## app-local 组件和 Entity 结构

状态集中在 `ProviderSettingsPage`、必要的输入 entity 和 manual editor 中。provider/model list
delegate 只持有过滤后的 row snapshot 和 List 内部 query/selection state；业务真相仍是
`ProviderSettingsPage.providers`、`ProviderSettingsPage.draft` 和 DB。

```rust
pub(super) struct ProviderKindKey(String);

pub(super) struct ProviderSettingsPage {
    provider_list: Entity<ListState<ProviderListDelegate>>,
    model_list: Entity<ListState<ProviderModelListDelegate>>,
    detail_scroll_handle: ScrollHandle,
    selected: ProviderSelection,
    providers: Vec<ProviderListItem>,
    models: Vec<ProviderModelDraft>,
    draft: ProviderDraft,
    saved_snapshot: Option<ProviderDraftSnapshot>,
    text_inputs: BTreeMap<String, Entity<InputState>>,
    secret_inputs: BTreeMap<String, Entity<ProviderSecretInput>>,
    validation: ProviderValidationState,
    save_state: AsyncActionState,
    fetch_state: AsyncActionState,
    manual_model_editor: Option<Entity<ManualModelEditor>>,
    _list_subscriptions: Vec<Subscription>,
    _field_subscriptions: Vec<Subscription>,
    _load_task: Option<Task<()>>,
    _save_task: Option<Task<()>>,
    _fetch_task: Option<Task<()>>,
}

pub(super) struct ProviderListItem {
    spec: ProviderSpec,
    provider: Option<ProviderRecord>,
}

pub(super) enum ProviderSelection {
    Builtin { kind: ProviderKindKey, provider_id: Option<ProviderId> },
    Custom { provider_id: ProviderId },
    NewCustom,
}

pub(super) struct ProviderListRow {
    kind: ProviderKindKey,
    display_name: SharedString,
    icon: IconName,
    enabled: bool,
    search_text: String,
}

pub(super) struct ProviderListDelegate {
    all_rows: Vec<ProviderListRow>,
    rows: Vec<ProviderListRow>,
    last_query: String,
    empty_label: SharedString,
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
}

pub(super) struct ProviderDraftSnapshot {
    provider_id: Option<ProviderId>,
    kind: ProviderKindKey,
    display_name: String,
    enabled: bool,
    fields: BTreeMap<String, ProviderDraftValue>,
    secret_refs: ProviderSecretRefs,
    dirty_secret_keys: BTreeSet<String>,
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
    fetched_at: Option<String>,
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
    Edit,
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

pub(super) enum ProviderValidationState {
    Idle,
    Valid,
    Invalid(SharedString),
}

pub(super) enum AsyncActionState {
    Idle,
    Running,
}
```

Entity 使用规则：

- 所有 entity 通过 `cx.new(...)` 创建；回调里捕获 `WeakEntity`，不要捕获 strong entity 造成 retain cycle。
- `ProviderSettingsPage` render 时只读取自身 snapshot，不在 `RenderOnce` 子组件里读写 parent entity。
- 输入变更通过 subscription 更新 draft 并 `cx.notify()`；不要在 input subscription 里嵌套更新同一个 entity。
- save/fetch 用 `cx.spawn` 前台 task；Rig/reqwest/Tokio I/O 通过 `gpui_tokio::Tokio::spawn`
  派到 Tokio runtime。需要重计算 capability 的纯 CPU 工作可以用 `cx.background_spawn(...).then(cx.spawn(...))`
  回到前台更新 entity。

## 数据和保存规则

- `providers` 保存 provider 实例：`kind`、`display_name`、`enabled`、非 secret
  `ProviderSettingsPayload` 和 `ProviderSecretRefs`。
- `provider_models` 保存设置页刷新或用户手动添加后的模型 cache：`model_id`、display name、
  `enabled`、`ModelCapabilitiesSnapshot`、`ProviderModelMetadata` 和 `fetched_at`。
- API key、bearer token、Azure token 等 secret 不进入 DB，也不进入 run snapshot。DB 只保存
  `ProviderSecretRef { key, storage: "keychain", ref_id }`。
- 当前 `ref_id` 使用 `{provider_id}:{secret_key}`。后续如果需要跨 app/环境命名空间，可迁移为
  `ai-chat2/provider/{provider_id}/{secret_key}`，但迁移必须保留旧 ref 读取兼容。
- 当前保存前强制执行本地校验：必填字段、secret 是否已有 saved ref 或本次输入。校验失败不写
  provider row，也不写 secret。
- 当前 dirty 判断以 `ProviderDraftSnapshot` 对比 DB saved snapshot；已保存 provider 修改字段、enabled
  或 secret input 后显示“未保存”，保存成功后刷新 snapshot。
- 当前保存成功后先写 GPUI credentials，再 insert/update provider row；DB payload 不包含 secret 原文。
- URL 格式校验、Rig completion client 构造校验和低成本远端 verify 尚未接到保存流程；当前远端可用性主要通过
  Fetch Models 或后续首次 run 验证。
- 模型刷新来自设置页。当前只允许已保存且无未保存改动的 provider 执行；fetch 会读取 DB row 和
  GPUI credentials，通过 `gpui_tokio::Tokio::spawn` 运行 `ai_chat_agent::fetch_provider_models`。
  刷新成功后调用 `replace_fetched_provider_models` upsert cache，刷新失败不能删除旧 cache。
- 自定义 OpenAI-compatible provider 必须允许手动模型和手动能力，因为兼容接口通常不会返回完整 capability。

## Provider 配置矩阵

| Provider | v1 状态 | 必填配置 | 可选配置 | 模型获取 | 能力保存策略 |
| --- | --- | --- | --- | --- | --- |
| OpenAI | first-class | API key | Base URL，API mode 默认 Responses | Rig `/models` | OpenAI 规则推导 reasoning、tools、image、structured output、continuation |
| Anthropic | first-class | API key | Base URL，Anthropic version，betas | Rig `/v1/models` | Anthropic 规则推导 tools、image、reasoning/cache 相关能力 |
| Google Gemini | first-class | API key | Base URL，API mode 默认 GenerateContent | Rig model listing | Gemini metadata 加规则推导 |
| Ollama | first-class | Base URL 默认 `http://localhost:11434` | Bearer token，auto discover | Rig/Ollama listing；show metadata 后续补强 | conservative fallback，后续用 show metadata 推导 tools、vision、thinking、context |
| OpenRouter | first-class | API key | Base URL | Rig model listing | provider metadata 加 conservative fallback |
| DeepSeek | first-class | API key | Base URL | Rig model listing when available | known DeepSeek model rules |
| Moonshot/Kimi | first-class | API key | Global/China/Anthropic-compatible endpoint | manual/no-listing | known model rules + manual model add |
| Z.AI | first-class | API key | General/Coding/Anthropic-compatible endpoint | manual/no-listing | known model rules + manual model add |
| Azure OpenAI | first-class | API key/token，Azure endpoint，deployment/model id | API version | no generic listing | deployment rows 是用户管理模型 |
| Mistral | first-class | API key | Base URL | Rig model listing | metadata + known model rules |
| xAI | first-class | API key | Base URL | manual/no-listing | manual/known model rules |
| Groq | first-class | API key | Base URL | manual/no-listing | known model rules |
| Perplexity | first-class | API key | Base URL | manual/no-listing | known model rules；Sonar 类模型标记 hosted web search |
| Together | first-class | API key | Base URL | manual/no-listing | manual/known model rules |
| Custom OpenAI-compatible | first-class custom | Name，API key，Base URL，至少一个模型 | 自定义 headers 后置 | manual；可选 `/models` probe 后续 | 手动 capability toggles |
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

`ai-chat-db` provider settings 基础 API 已落地：

- `provider_models` schema 已增加 `enabled BOOLEAN NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1))`。
- `ProviderModelRecord` / `NewProviderModel` 已增加 `enabled: bool`。
- `upsert_provider_model` 插入新模型时使用 `NewProviderModel.enabled`；发生 conflict 时不覆盖已有
  enabled，除非调用显式 toggle API。
- `list_providers`，按 display name / kind 稳定排序，并保留 custom provider。
- `update_provider`，支持 display name、enabled、settings、secret refs 更新。
- `delete_provider`，删除 provider 时级联删除 provider models，但不得删除 keychain secret；secret 清理走显式动作。
- `list_provider_models(provider_id)`，供设置页和 composer model picker 使用。
- `set_provider_model_enabled(provider_id, model_id, enabled)`，只改 enabled 和 `updated_at`。
- `delete_provider_model(provider_id, model_id)`，供手动模型管理使用。
- `replace_fetched_provider_models(provider_id, fetched)`，事务型批量 upsert 本次 fetch 返回的模型；不删除旧模型。
- `replace_fetched_provider_models` 保留已有模型的 enabled；新模型默认 enabled。手动 capability override
  的 schema/API 仍待 manual model editor 设计。

Secret wrapper 当前已封装：

- `ProviderSecretStore::refs_for(provider_id, writes)`，生成 `ProviderSecretRefs`。
- `ProviderSecretStore::write_values(cx, refs, writes)`，写入 GPUI credentials。
- `ProviderSecretStore::read_values(cx, refs)`，fetch 前读取 credentials 并返回
  `ai_chat_agent::ProviderSecretValues`。
- `delete_provider_secret` / `has_provider_secret` 尚未实现；当前 UI 通过 DB secret refs 判断是否已有
  saved secret。

`state::providers` 当前已提供：

- `providers_with_models(cx)`，返回 provider row 和当前 model rows。
- `enabled_provider_models(cx)`，只返回 `provider.enabled && model.enabled` 的模型，供后续 composer picker。
- `save_provider_draft(cx, draft, secret_refs)` 尚未抽出；当前保存逻辑仍在 Provider Settings page 内编排。

## UI 状态流

加载页面：

1. 读取 registry built-in providers。
2. 从 DB 读取 `providers` 和 `provider_models`。
3. 合并成左侧列表：built-in 永远显示；custom rows 追加显示。
4. 当前默认选中 registry 第一项 OpenAI；后续可改为优先选中已启用且配置完整的第一个 provider。

选择 provider：

1. `ListState` 负责 provider 搜索、虚拟滚动和 selected index。
2. `ProviderListDelegate` 从当前过滤 rows 把 `IndexPath` 映射为 `ProviderKindKey`。
3. `ProviderSettingsPage` 订阅 `ListEvent::Select/Confirm`，把 kind 写入页面 canonical selection。
4. 如果 DB row 存在，用 row + registry schema 构造 `ProviderDraft`。
5. 如果是 built-in 且没有 row，用 registry defaults 构造 unsaved draft。
6. 重建 text/secret inputs；secret input 只显示 empty/saved/dirty 状态。
7. 加载该 provider 的 model drafts；model search 由 model `ListState` 过滤当前 provider models。

保存 provider：

1. 从 UI draft 收集非 secret fields、secret draft 和 enabled 状态。
2. 执行当前本地校验：必填字段、必填 secret 是否已有 saved ref 或本次输入。
3. 校验失败显示 inline error + error notification，不写 secret、不写 DB。
4. 校验通过后写入/更新 GPUI credentials，并生成 `ProviderSecretRefs`。
5. insert/update provider row。DB payload 不包含 secret 原文。
6. reload provider list 和 draft；刷新 `ProviderDraftSnapshot`，成功后 notification。
7. URL 格式校验、Rig completion client 构造和远端 verify 后续补齐。

刷新模型：

1. 只允许对已保存 provider 执行；unsaved draft 先保存。
2. 如果当前 form dirty，提示先保存；fetch 只使用已保存 DB row 和 keychain secret。
3. 读取 provider row 和 secret refs，再通过 `ProviderSecretStore::read_values` 取 secret values。
4. 使用 `gpui_tokio::Tokio::spawn` 调用 `ai_chat_agent::fetch_provider_models`，保证 Rig/reqwest
   Tokio I/O 在 Tokio runtime 中执行。
5. 支持 listing 的 provider 当前为 OpenAI、Anthropic、Gemini、Ollama、OpenRouter、DeepSeek、
   Mistral；manual/no-listing provider 返回 manual-model-required notification。
6. 对每个模型生成 `ProviderModelMetadata` 和 conservative `ModelCapabilitiesSnapshot`。
7. `replace_fetched_provider_models` upsert cache；保留已有 enabled。
8. 刷新失败只显示 error notification，不清空旧 cache。

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
- UI state：`cargo test -p ai-chat2 provider` 当前覆盖 draft validation、field schema mapping、capability
  derivation、dirty snapshot、provider/model ListState delegate 搜索、provider/model row separator、filtered
  model row toggle target；manual model editor 后续补测试。
- Provider i18n / layout baseline：覆盖 Provider title i18n、Provider 搜索词、Provider 文案 key、未保存 provider 默认 disabled、已保存 provider 保留 DB enabled。
- Tokio bridge：`cargo test -p gpui-tokio` 覆盖 initialized runtime、external handle 和 dropped task abort。
- Agent model listing：`cargo test -p ai-chat-agent provider` 覆盖 missing secret、bad base URL、manual/no-listing
  provider 和 Rig model -> `NewProviderModel` mapping。
- UI compile：`cargo fmt`、`cargo check -p ai-chat2`。
- 手动 Settings smoke：OpenAI 保存/验证/fetch、Ollama localhost fetch、custom OpenAI-compatible manual model + capability toggle。

## 后续顺序

1. 已补 `ai-chat-db` provider/model repository API 和 `provider_models.enabled`。
2. 已增加 `ai-chat2` provider registry、draft model、GPUI credentials secret write/read 和 Settings Provider
   页面骨架。
3. 已补 Provider i18n、未保存 provider 默认 disabled、左右列独立滚动布局和对应测试。
4. 已补保存前校验、未保存状态标签、GPUI credentials secret write/read、真实 fetch 链路、
   `ai-chat-agent` provider model listing API 和 `gpui-tokio` runtime bridge。
5. 已把 provider/model list 迁移到 `gpui-component::ListState`，并把 provider 选择收敛为
   ListEvent -> `ProviderSettingsPage` 的单向业务状态流。
6. 已修复右侧 detail 整体滚动、model enabled switch 事件链路，并把 provider list 调整为带搜索的
   整体 panel + row separator 视觉。
7. 下一步补 manual model editor、manual capability override persistence、delete/has secret helper、
   URL/Rig completion client validation。
8. 已把 ChatForm model picker 从 preview data 切到 DB-backed enabled provider models。
9. 再接 prompt selector、conversation create/timeline 和真实 `AgentRuntime`。
