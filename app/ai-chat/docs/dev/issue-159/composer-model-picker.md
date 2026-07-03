# Issue #159 ai-chat2 Composer 模型选择专项计划

本文档是 `app/ai-chat2` Composer DB-backed provider/model picker 的可执行开发计划。父级 UI
清单仍是 `app/ai-chat/docs/dev/issue-159/README.md`；Provider 配置和模型缓存计划仍是
`app/ai-chat/docs/dev/issue-159/provider-settings.md`。

最后同步时间：2026-06-03。

当前状态：本计划已实现。`ChatForm` 的 preview-only model picker 已切到 fresh DB 中的 enabled
provider/model cache，并会在打开 picker 前刷新；发送 snapshot 已包含 provider/model 和
`ReasoningSelectionSnapshot`。本阶段仍不创建真实 conversation，不启动 `AgentRuntime`，不接
timeline，不扩 `AppSettingsPayload`，不实现 manual model editor。

## 目标边界

- 本阶段只解决 Composer 选择“哪个 provider/model 作为后续 run 输入”的真实数据源。
- Provider settings 继续是 provider 配置、secret refs、model fetch 和 model enabled toggle 的唯一入口。
- Composer picker 只读取 `providers` / `provider_models`，不读取 keychain，不做远端验证。
- 选择结果先保存在 New Conversation 页面内存状态中；后续真正创建 conversation 时再写入已有的
  `conversations.default_provider_id` / `default_model_id` 和 `RunSettingsSnapshot`。
- 发送事件可以携带 provider/model 选择，但本阶段不启动 agent runtime。

## 数据流

### Provider settings 到 Composer

```text
Settings Provider page
  -> insert/update providers
  -> write GPUI credentials refs only
  -> fetch models through ai_chat_agent::fetch_provider_models
  -> replace_fetched_provider_models(provider_id, models)
  -> provider_models.enabled toggle
  -> state::providers::enabled_provider_models(cx)
  -> ChatForm model picker sections
```

读取规则：

- `enabled_provider_models(cx)` 只返回 `provider.enabled && provider_model.enabled`。
- 读取按 repository 当前排序：provider 使用 `list_providers()`，model 使用
  `list_provider_models(provider_id)`。
- `ChatForm` 初始化时读取一次；打开 model picker 前再次读取一次，保证 Settings 刚保存或刷新后无需重启。
- 读取失败时保留 error state，model trigger 显示不可用，send disabled，并在 picker empty state 展示错误文案。
- 无 enabled model 时 selection 为 `None`，send disabled，picker footer 提供打开 Provider Settings 的入口。

### Composer 选择到后续 run

```text
User selects ModelOption
  -> ChatForm.selected_model_key = ProviderModelKey
  -> selected ProviderModelChoice snapshot retained in ChatForm
  -> recompute ReasoningSelectionSnapshot from ModelCapabilitiesSnapshot.reasoning.control
  -> SendRequested(ChatFormSubmit)
  -> later NewConversation creates conversation with default_provider_id/default_model_id
  -> later AgentRunRequest uses provider_id/model_id/reasoning_selection/settings_snapshot
```

本阶段只到 `SendRequested(ChatFormSubmit)` 为止。`NewConversationPage` 可以先接收事件并记录 TODO /
notification；真实 conversation create、timeline item append 和 `AgentRuntime` 接线留给后续专项。

## 数据结构

### `state::providers`

新增稳定选择 key，避免用 preview index 或 DB row id：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderModelKey {
    pub(crate) provider_id: ProviderId,
    pub(crate) model_id: ProviderModelId,
}

impl ProviderModelChoice {
    pub(crate) fn key(&self) -> ProviderModelKey;
    pub(crate) fn display_label(&self) -> String;
}
```

`ProviderModelChoice` 继续作为 UI 和后续 run 的 snapshot：

```rust
pub(crate) struct ProviderModelChoice {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_kind: String,
    pub(crate) provider_display_name: String,
    pub(crate) model_id: String,
    pub(crate) model_display_name: Option<String>,
    pub(crate) capabilities: ModelCapabilitiesSnapshot,
}
```

### `ChatForm`

替换 preview index 状态：

```rust
pub(crate) struct ChatForm {
    composer: Entity<ComposerEditor>,
    model_choices: Result<Vec<ProviderModelChoice>, SharedString>,
    selected_model_key: Option<ProviderModelKey>,
    selected_reasoning_selection: Option<ReasoningSelectionSnapshot>,
    token_budget_input: Entity<InputState>,
    effort_picker_open: bool,
    effort_picker: Entity<ListState<PickerListDelegate<EffortOption>>>,
    model_picker_open: bool,
    model_picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    _subscriptions: Vec<Subscription>,
}
```

发送事件改为携带模型选择：

```rust
#[derive(Clone)]
pub(crate) struct ChatFormSubmit {
    pub(crate) composer: ComposerSnapshot,
    pub(crate) provider_model: ProviderModelChoice,
    pub(crate) reasoning_selection: Option<ReasoningSelectionSnapshot>,
}

pub(crate) enum ChatFormEvent {
    AddRequested,
    SendRequested(ChatFormSubmit),
}
```

发送规则：

- `ComposerSnapshot::is_empty()` 为 true 时不发送。
- `selected_model_key` 找不到对应 choice 时不发送。
- 无可用模型、DB load error、selected model 被禁用或删除时 send button disabled。

### `model_select`

`ModelOption` 不再引用 preview model：

```rust
pub(super) struct ModelOption {
    key: ProviderModelKey,
    provider_display_name: SharedString,
    model_id: SharedString,
    model_display_name: Option<SharedString>,
    capabilities: ModelCapabilitiesSnapshot,
}
```

`SelectItem::Value = ProviderModelKey`。`matches()` 匹配 provider display name、provider kind、model id、
display name 和 capability tokens。

分组规则：

- `model_sections(choices, i18n)` 按 provider 分组。
- section title 使用 `provider_display_name`。
- row 主标题使用 `model_display_name.unwrap_or(model_id)`。
- row 副标题使用 `provider_display_name · model_id`。

### `thinking_effort`

`ReasoningSelectionSnapshot` 从 `ModelCapabilitiesSnapshot.reasoning.control` 派生：

- `Levels` / `AdaptiveLevels` 保留 provider 原始档位值，并兼容 `x_high`、`extra_high` 到 `xhigh`。
- `Boolean` 显示 Enabled / Disabled。
- `TokenBudget` 显示 Off / Dynamic / Custom，并在 picker footer 提供 numeric input。
- `AlwaysOn` 显示 Always On。
- 旧 payload 中只有 `default_effort` / `efforts` 时仍按 legacy level 规则 fallback。
- `reasoning == None` 或没有可识别 control 时，effort picker disabled，label 显示默认“Thinking”文案或不渲染可选项。

## 模块结构

禁止新增 `mod.rs`。本阶段只改现有模块或新增同名普通 `.rs` 文件。

```text
app/ai-chat/docs/dev/issue-159/composer-model-picker.md
  - 本专项计划

app/ai-chat/docs/dev/issue-159/README.md
  - 把 Composer/model select 状态改为“已实现”
  - 指向本文件

app/ai-chat/docs/dev/issue-159/provider-settings.md
  - 把 DB-backed composer model picker 改为“已实现，继续作为后续 run 输入”

app/ai-chat2/src/state/providers.rs
  - ProviderModelKey
  - ProviderModelChoice display/key helpers
  - enabled_provider_models(cx) 保持 DB-backed source of truth

app/ai-chat2/src/features/home/chat_form.rs
  - ChatForm model_choices / selected_model_key state
  - reload_model_choices(window, cx)
  - can_send() checks composer + selected model
  - ChatFormSubmit event

app/ai-chat2/src/features/home/chat_form/model_select.rs
  - ModelOption from ProviderModelChoice
  - model_sections(choices, i18n)
  - row rendering with provider/model/capability tags
  - empty footer action to Provider Settings

app/ai-chat2/src/features/home/chat_form/effort_select.rs
  - effort_sections from ModelCapabilitiesSnapshot
  - disabled/empty effort state

app/ai-chat2/src/features/home/chat_form/thinking_effort.rs
  - mapping from ReasoningCapabilitySnapshot strings

app/ai-chat2/src/features/settings.rs
  - open_settings_window_to_provider(cx)
  - reuse existing settings window, clear search, select SettingsPageKey::Provider

app/ai-chat2/locales/{en-US,zh-CN}/main.ftl
  - model picker empty/error/configure-provider/capability labels
```

`preview_models.rs` 已在实现中删除，`ChatForm` 不再依赖 preview model 数据源。

## GPUI 组件和 UI 细节

复用现有 `chat_form/picker.rs`：

- `Popover`：保持 `Anchor::BottomLeft`、`appearance(false)`、`occlude()`。
- `List` / `ListState`：继续用 `PickerListDelegate<ModelOption>`，开启 `.searchable(true)`。
- `Button`：model trigger 继续用现有 pill button。
- `Label`：row 标题、副标题、empty state。
- `Tag`：能力标签。
- `Icon`：只使用 app-local `IconName`。

Icon 约定：

| UI | Icon |
| --- | --- |
| model trigger | `IconName::Sparkles` |
| effort trigger | `IconName::Lightbulb` |
| model row | `IconName::Cpu` |
| no model / configure provider footer | `IconName::Settings` |
| capability tool/search/vision 暂不新增专用 icon | `Tag` 文案表达 |

当前 `IconName::Sparkles`、`Lightbulb`、`Cpu`、`Settings` 已存在。当前实现仍用 generic Lucide
icon，不新增 Lucide 资源。provider brand logo 是独立后续工作：Lucide v1 已移除品牌图标，后续
应使用品牌官方 SVG 或 Simple Icons 作为 app-owned runtime asset，并保留 generic fallback。

Row 布局：

- 高度保持 36-44px，避免 picker 打开后跳动。
- 左侧 `Cpu` icon，右侧两行文字。
- 第一行 model display name medium/truncate。
- 第二行 provider name + raw model id muted/truncate。
- 能力 tags 只显示最多 3 个：`reasoning`、`tools`、`vision`、`structured` 优先级按此顺序。

Empty state：

- DB load error：显示 `chat-form-model-load-failed` 和 error message。
- 无 enabled model：显示 `chat-form-model-none-configured`。
- footer button 使用 `chat-form-configure-providers`，点击打开 Provider Settings。

## 数据获取和失败模式

- `ChatForm::new` 同步读取 DB。当前 provider/model cache 来自本地 SQLite，不需要 async task。
- `set_model_picker_open(true, ...)` 先调用 `reload_model_choices`，再 focus list。
- `reload_model_choices` 成功后：
  - 保留当前 selected key，如果它仍存在。
  - 否则选择第一个 enabled model。
  - 同步 model picker sections 和 effort picker。
- `reload_model_choices` 失败后：
  - 保存错误 message。
  - 清空 selection。
  - sync picker 到 empty/error state。
  - send disabled。

本阶段不读取 keychain的原因：

- Provider settings 保存/刷新模型时已经验证 secret 是否存在。
- Composer 选择模型只需要 DB cache 和 capability snapshot。
- 真正 agent run 前再读取 credentials，避免 picker 控件承担 provider runtime 校验。

## 后续接线预留

后续 conversation create / agent run skeleton 可以直接消费：

- `ChatFormSubmit.composer.content_parts`
- `ChatFormSubmit.composer.skill_requests`
- `ChatFormSubmit.provider_model.provider_id`
- `ChatFormSubmit.provider_model.model_id`
- `ChatFormSubmit.provider_model.capabilities`
- `ChatFormSubmit.thinking_effort`

创建 conversation 时写：

- `NewConversation.default_provider_id = Some(provider_id)`
- `NewConversation.default_model_id = Some(model_id)`
- `ConversationSettingsSnapshot.provider_id/model_id/model_capabilities`
- `RunSettingsSnapshot.provider_id/model_id/model_capabilities/provider_settings`

## 验收和验证

本次实现已完成：

- `ProviderModelKey` 和 `ProviderModelChoice` key/display helper。
- `ChatForm` 读取 `state::providers::enabled_provider_models(cx)`，维护 `model_choices` /
  `selected_model_key`，打开 picker 前刷新，send disabled 依赖 composer + selected model。
- `ChatFormEvent::SendRequested` 携带 `ChatFormSubmit`，包含 composer snapshot、provider/model snapshot
  和 reasoning selection。
- `model_select` 从 `ProviderModelChoice` 生成分组、搜索、能力标签和 Provider Settings footer。
- `thinking_effort` 从 `ModelCapabilitiesSnapshot.reasoning.control` 派生可选项、默认值和 token budget input。
- Settings 增加 `open_settings_window_to_provider(cx)`，复用已有 Settings window、清空搜索并切到
  Provider 页。
- `preview_models.rs` 已删除，新增 en-US / zh-CN model picker 文案。

代码级测试：

- `state::providers::enabled_provider_models` 过滤 disabled provider 和 disabled model。
- `model_sections` 按 provider 分组，row value 使用 provider/model composite key。
- model search 匹配 provider display name、provider kind、model id、display name 和 capability tokens。
- selected model 被禁用或删除后 fallback 到第一个可用模型。
- 无可用模型时 `can_send == false`。
- reasoning capability 到 `ReasoningSelectionSnapshot` 的映射覆盖 levels、boolean、token budget、
  always-on 和 legacy effort payload。
- 新增 i18n key 在 `en-US` 和 `zh-CN` 都存在。

验证命令：

- `cargo fmt`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 provider`
- `cargo test -p ai-chat-agent provider_models`
- `cargo test -p ai-chat-agent model_capabilities`
- `cargo test -p ai-chat-agent reasoning_params`
- `cargo test -p ai-chat-core reasoning`
- `cargo test -p ai-chat2 settings`
- `cargo check -p ai-chat2`
- `git diff --check`

文档-only 更新只需运行 `git diff --check`。

本次验证记录：

- `cargo fmt`
- `cargo test -p ai-chat-core reasoning`
- `cargo test -p ai-chat-agent provider_models`
- `cargo test -p ai-chat-agent model_capabilities`
- `cargo test -p ai-chat-agent reasoning_params`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 provider`
- `cargo test -p ai-chat2 settings`
- `cargo test -p ai-chat-db`
- `cargo check -p ai-chat2`
- `git diff --check`
