# Issue #175 后续：form、component config 与 catalog 所有权重构计划

关联 issue：[suxiaoshao/gpui#175](https://github.com/suxiaoshao/gpui/issues/175)

状态：代码实施完成。纯 draft core、typed field handle、gpui-component adapter、Jaco typed catalogs、attachment
单一 owner 和 submit resolver 已落地；快捷键、提示词、provider、MCP 及动态数组行均已迁移到
caller-owned subscriptions。本文保留设计证据、迁移记录和桌面级 GPUI 验收清单。
本文是删除重复业务 owner、修复 options/form 混合和统一 submit snapshot 的唯一 Jaco 实施入口。

crate 计划：

- [`gpui-form` draft/component 分离](../../../../../crates/gpui-form/docs/external-state-synchronization-plan.md)
- [`gpui-store` typed catalog](../../../../../crates/gpui-store/docs/catalog-snapshot-projection-plan.md)

## 1. 范围与用户决策

目标：form 是唯一用户 draft owner；catalog 是 options/capability owner；component state 只拥有交互状态和
可替换 mirror；submit 使用一次 form + catalog snapshot；删除旧 binding/mirror/sync helpers。

保持：`ChatForm` 纯 UI、`ControlSlot::{Hidden, Disabled, Enabled}`、四个调用场景、快捷键/临时窗口产品语义、
DB schema/repository/provider adapter/Rig/MCP/icons/i18n/platform不变。

允许大规模重构，不为新路径保留旧 API 兼容层；旧 binding surface 已删除，不新增 Cargo 依赖，不修改复制自
上游的 `gpui` skill。

## 2. 当前证据

| 当前事实 | 位置 | 根因 |
| --- | --- | --- |
| attachments 同时在 controller、form、control 和 `ComposerSnapshot` | `components/chat_input.rs`、`form_state.rs`、`composer_editor/snapshot.rs` | 四个可写/可读副本 |
| model/reasoning/approval 同时在 form 和 control state | `components/run_settings.rs`、`chat_form/controls.rs` | component mirror 被当作业务事实 |
| choices/options 与 selected/capability 写入同一 control/picker state | `run_settings.rs`、picker adapters | catalog config 变化被误当成 form 同步 |
| project id、records、label/icon 分散 | `features/home/new_conversation.rs`、`project_control.rs` | presentation cache 需要手工 invalidation |
| attachment support 在 model fallback 之前检查 | `chat_input.rs::submit_snapshot` | submit 读取不同时间和 owner 的值 |
| 历史 `ListState already being updated` / `RefCell already borrowed` | issue #175 runtime logs | observer 同步回写形成 entity update 环 |

依赖固定：`gpui 0.2.2` rev `1d217ee39d381ac101b7cf49d3d22451ac1093fe`、
`gpui-component 0.5.2` commit `c36b0c6ae6d14c33473f6610a27c3abc584afdf9`，无 lockfile 变化。

## 3. 最终所有权

| 数据 | 唯一 owner | 其他位置允许内容 |
| --- | --- | --- |
| composer draft | `ChatInputFormStore` | `ComposerEditor` text mirror + IME/cursor |
| attachments | `ChatInputFormStore.attachments` | preview/task；submit 短生命周期 clone |
| model/reasoning/approval | `RunSettingsFormStore` | picker selected mirror |
| provider models/capabilities | `ProviderCatalogState` SharedStore | component options projection |
| current project id | `NewConversationPage` | 无 selected record/label owner |
| project records | `ProjectCatalogState` SharedStore | presentation/rows projection |
| component config | page/controller | items/capability/disabled |
| component interaction | component entity | open/query/highlight/focus/scroll/task |
| default preferences | `JacoConfigStore` | initial form/session value；不是当前 selection owner |

## 4. 关键设计

### JACO-D-01：ChatForm 继续纯 UI

`ChatForm` 只组合 `ControlSlot` 和 app-created component states。它不依赖 form/store/catalog/repository，
不读取 submit domain。Disabled 只改变交互配置，不改 form value。

### JACO-D-02：组件 state 是 mirror，不是 field owner

Jaco 创建 `InputState`/picker/select/list state，配置 items/options/capability，调用
`gpui-form-gpui-component::bind_*` 连接 form field handle，并把返回的 subscriptions 合并进 page/controller
持有的 `gpui_form::SubscriptionSet`。

```text
user component event -> adapter -> form draft
form setter/normalize -> adapter -> component value mirror
catalog change        -> component options only
```

options update不得触发 form dirty、replace、fallback 或 config persistence。

### JACO-D-03：attachments 只有一个字段

删除 `ChatInputController.attachments`、`AttachmentControlState.attachments` 和
`ComposerSnapshot.attachments`。`ComposerSnapshot::is_empty` 只判断 text/skills；ChatInput 空值判断使用
`composer.is_empty() && draft.attachments.is_empty()`。

### JACO-D-04：typed catalog 使用 MemoryBackend

`state/projects.rs` 和 `state/providers.rs` 定义 `ProjectCatalogState` / `ProviderCatalogState`，安装
`SharedStore<_, MemoryBackend>` global。repository event 触发 app reload task；成功一次替换，失败保留旧 snapshot。
不新增 Jaco backend、load-status store 或 revision field。

### JACO-D-05：project presentation 是纯派生

```rust
pub(crate) fn project_presentation(
    selected_id: Option<ProjectId>,
    catalog: &ProjectCatalogState,
) -> ProjectPresentation;
```

`NewConversationPage` 和项目设置页保存 catalog selection；新对话页保存 selected id，render/explicit projection
command 调用纯函数。`ProjectControlState` 只保存 picker interaction。删除 selected label/icon/records cache 和
`sync_project_presentation`；保留 `sync_project_picker` 作为 options/selected-index 的组件 projection command。

### JACO-D-06：run settings resolver 无副作用

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModelResolutionPolicy {
    FallbackToFirstEnabled,
    RequireSelected,
}

pub(crate) fn resolve_run_settings(
    draft: &RunSettingsInput,
    catalog: &ProviderCatalogState,
    policy: ModelResolutionPolicy,
) -> Result<RunSettingsSubmitSnapshot, RunSettingsSubmitError>;
```

resolver 不接收 `Window`/`Context`，不更新 form/component/config。普通会话用 fallback，Shortcut 用 require。
fallback 结果可直接提交；如果产品要保存为新选择，另发显式 form command。

### JACO-D-07：submit 只读一次事实源

```text
clone ChatInputInput draft
clone ProviderCatalogState snapshot
resolve_run_settings(draft.run_settings, catalog, policy)
validate draft.attachments against resolved model capability
validate composer/agent guards
build ChatInputSubmit from draft + resolved settings
emit request
```

任一错误不清空 draft、不创建 Conversation/AgentRun。禁止读取 control selected/choices/cache。

### JACO-D-08：外部值替换与 options 更新分离

- 打开编辑器时用 domain value 创建 form。
- 明确切换被编辑记录时，app 检查 dirty 后决定是否调用 `replace_from_value`。
- catalog/options 更新只设置 component config 和重新渲染 unavailable 状态。
- live preference persistence 订阅 typed form event；Shortcut 不安装 ChatFormConfig adapter。

## 5. 文件与删除

| 文件 | 目标变更 |
| --- | --- |
| `components/chat_input.rs` | 删除 attachment mirror/sync；单快照 submit |
| `components/chat_input/form_state.rs` | 纯 form draft + field handles |
| `components/chat_input/composer_editor/snapshot.rs` | 删除 attachments |
| `components/run_settings.rs` | app-created controls/config adapters；pure resolver |
| `components/run_settings/policy.rs` | reasoning normalization pure functions |
| `components/chat_form/controls.rs` | 只定义 ControlSlot/render contracts，无业务 selected/options owner |
| `components/chat_form/project_control.rs` | 只保留 interaction/render，presentation 入参化 |
| `features/home/new_conversation.rs` | selected id owner + catalog selection + reload/persistence commands |
| `state/projects.rs` / `state/providers.rs` | typed catalog SharedStore globals |
| settings prompt/shortcut form modules | 已迁移新 field handle + caller-owned `SubscriptionSet` |
| settings provider/MCP form modules | 已迁移到 app-created component state + caller-owned subscriptions |

新路径删除旧 `FormComponentBinding` 调用方、controller/control business mirror、所有逐字段 `sync_*` helper 和
revision-only catalog entity；workspace 不再保留 legacy binding surface。无新 `mod.rs`。

## 6. 错误与生命周期

```rust
pub(crate) enum RunSettingsSubmitError {
    CatalogUnavailable,
    ModelUnavailable(ProviderModelKey),
    InvalidReasoning,
}
```

attachment mismatch 继续使用现有 `ModelAttachmentSupportIssue`。catalog reload failure 由发起 reload 的 app
owner 展示/记录并保留旧 snapshot。reload/attachment/project tasks 由现有 page/controller 保存，drop 取消。

adapter bind function 原子返回 subscriptions，controller/page 按 mount scope 用一个或多个 `SubscriptionSet` 持有；
clear/drop 对应 set 即释放。私有 direction guard 被双向订阅 closure 共同捕获并消除同步回路；不得用 busy flag
掩盖业务重复 owner，也不得无条件 defer。

## 7. 删除优先审计

| 当前实现/旧计划 | 决定 |
| --- | --- |
| `FormComponentBinding`/`ComponentFieldStore` | workspace-wide delete；新路径使用 field handle + caller-owned subscriptions |
| `FormSelection`/draft revision/conflict proposal | Delete |
| catalog -> form rebase | Delete |
| custom catalog backend | Do not add |
| controller/control business values | Delete |
| `ComposerSnapshot.attachments` | Delete |
| pure `ChatForm`/`ControlSlot` | Retain |
| gpui-component controls | Reuse via adapter-returned subscriptions |
| existing repository queries | Reuse |

## 8. 工作包

当前进度：`FORM-10..40`、`WP-80`、`WP-90`、`WP-100` 的代码已实施；`WP-110` 保留为桌面级生命周期验收。

依赖：`FORM-10..40 -> WP-80 -> WP-90 -> WP-100 -> WP-110`。

### WP-80：typed catalogs 与 project

先建立两个 SharedStore global 和 reload commands，再迁移 NewConversation project。测试 project rename/remove、
reload failure、sidebar/picker selection、presentation pure derivation。完成后无 revision-only catalog/cache。

### WP-90：Jaco forms 与 adapters

迁移 Jaco settings 的旧 binding 调用方；prompt/shortcut/provider 的 secret 输入、MCP 的动态字段和数组行
均已迁移。组件 state 由 app 创建，controller/page 保存 core
`SubscriptionSet`，不保存逐字段 binding handle。model items、capability、disabled 更新不写 form。删除
attachment/model/reasoning/approval mirrors。

测试：`options_update_does_not_change_run_settings_draft`、`disabled_update_does_not_persist_config`、
`user_selection_updates_form_once`、`form_replace_updates_component_once`。

### WP-100：pure resolver 与 submit

先测试 resolver policy，再替换 submit path。覆盖 disabled/removed model fallback、Shortcut require-selected、fallback 到
不支持附件模型、reasoning normalize、catalog failure、成功只构造一个 payload。

### WP-110：GPUI 生命周期回归

覆盖 model/reasoning/approval picker、project picker/side bar、temporary conversation list、dirty form during catalog reload。
验收日志不得出现 `already being updated` 或 `RefCell already borrowed`。

## 9. 系统面

| Surface | 决定 |
| --- | --- |
| UI/focus/accessibility | 视觉与键盘语义不变；config 更新不抢 focus |
| Data/persistence | form/catalog/config owner 如上；DB schema/transaction No change |
| i18n/icons/assets | 复用现有 keys/icons；No change |
| Dependencies/MSRV/platform | No change |
| Cancellation/retry | 现有 task ownership；reload 手动重试，无自动 backoff |

## 10. 验证

```bash
cargo fmt --all
cargo test -p gpui-form
cargo test -p gpui-form-macros
cargo test -p gpui-form-gpui-component
cargo test -p gpui-store
cargo check -p jaco
cargo test -p jaco --no-run
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p gpui-store --all-targets --all-features -- -D warnings
cargo clippy -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

本阶段完成条件：新迁移路径不存在重复业务 owner；options 更新不写 form；submit 只使用 form + catalog snapshot；
prompt/shortcut/provider/MCP 与 ChatInput/RunSettings scoped flows 无 nested update/reentrant window error。旧
binding API 已 workspace-wide 删除，不用兼容层扩散到新代码。
