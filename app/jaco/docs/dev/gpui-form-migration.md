# Jaco gpui-form 类型化双向表单迁移实施计划

## 1. 状态与范围

- 文档位置：`app/jaco/docs/dev/gpui-form-migration.md`。
- 当前分支：`codex/175-jaco-shortcut-temporary-window`；本计划是跨功能的表单基础设施迁移，不属于 issue #175 的产品需求。
- [当前事实] 目标迁移源码已落地：prompt、provider、MCP、shortcut 与 ChatInput/RunSettings 使用
  generated typed form 和 owning controls；active source 中的旧 submit/source/draft/binding API 与直接
  whole-form read 已清除。
- [执行状态] 2026-07-21 已通过定向测试、workspace 全量测试/build、严格 clippy、dependency tree、
  residual scan 与宏 compile-fail gate，并成功生成本地 `Jaco.app` bundle。隔离数据目录下已完成
  home、provider、shortcut 的定向 Computer Use smoke；临时窗口全局快捷键与有数据列表流程因
  自动化工具不能触发全局快捷键且隔离库无对话数据，仍保留为人工验证缺口。
- [用户决定] 可以做 breaking、大规模重构并删除无用 API、trait 和类型；不保留兼容层。
- [分阶段发布门槛] 四份计划按以下唯一顺序实施；Jaco 迁移不能等待 adapter 全计划结束，否则
  `CONTROL-40` 与 Jaco 调用点迁移会形成循环：

```text
adapter DEP-00
  -> core FORM-10..60
  -> macro MACRO-10..40
  -> adapter CORE-GATE -> CONTROL-10 -> CONTROL-20 + CONTROL-30
  -> JACO-FORM-10..60
  -> adapter CONTROL-40 -> CONTROL-50
  -> JACO-FORM-70
  -> core FORM-70
```

  - core：`crates/gpui-form/dev/typed-form-store.md`
  - macro：`crates/gpui-form-macros/dev/form-store-derive.md`
  - adapter：`crates/gpui-form-gpui-component/dev/typed-bound-controls.md`

### 目标

1. prompt、provider、MCP、shortcut、ChatInput/RunSettings 都以一个 generated form store 作为编辑期业务值唯一来源。
2. 页面只组合 form、bound controls、catalog/context 与 persistence；不保存第二份 leaf draft，也不从控件反向组装提交模型。
3. Jaco 同步业务验证统一使用 Garde 0.23；应用只保留 Garde custom rule、Fluent 国际化和必要的路径映射。
4. persistence 由页面持有任务与 UI 状态；成功结果只用 `FormRevision`/`rebase_if_revision` 合入，过期任务不能覆盖新输入。
5. 模型目录只从 `ProviderCatalogGlobal` 的 `SharedStore<ProviderCatalogSnapshot>` 读取一次快照；无选择或已失效选择都显示错误，不自动选择第一个模型、不回退、不在 form/control 路径读数据库。
6. `ChatForm` 保持纯 UI shell，通过 `ControlSlot` 接收调用者创建的 controls；普通对话、新对话、临时窗口与快捷键编辑器保持一致体验。

### 非目标

- 不修改数据库 schema、Diesel migration、repository query 或 provider secret 存储格式。
- 不在本迁移中收紧 `state::providers` 的所有 repository 访问权限；该工作由已单独创建的 provider catalog/store issue 承担。本计划只保证迁移后的正常 form/control 路径不调用 fallback query。
- 不改变 agent/provider 协议、附件支持策略、快捷键业务语义、MCP OAuth 协议或 ChatForm 视觉布局。
- 不新增通用 Jaco binding 框架；应用直接使用三个 form crate 的公开契约。
- 不修改 icons、runtime assets、bundle assets 或 macOS 本地化资源。

## 2. 证据快照

### 2.1 当前实现

| 区域 | [当前事实] 证据 | 必须消除的边界 |
| --- | --- | --- |
| Prompt | `features/settings/prompts/form_state.rs` 已 derive `FormStore`/Garde；`dialog.rs::save` 在 form 内 `prepare_submit` 后同步保存并无条件 `rebase` | locale context 更新后未显式 Dynamic 重验；成功合入没有 revision 守卫；旧 `SubmitError::Busy` 分支仍存在 |
| Provider | `features/settings/provider.rs` 同时持有 `ProviderDraft`、`ProviderDraftSnapshot`、typed form 和页面 validation；`forms.rs::submit_async_save` 把 handler task 塞回 form submit runtime | leaf 值、dirty、保存状态多源；form 错误地持有 persistence 生命周期；异步成功可能覆盖后续编辑 |
| MCP | `features/settings/mcp/form_state.rs` 为 row 类型生成多个 child stores并由页面重绑 controls；`validation.rs` 已手写 `garde::Validate` 但仍携带 gpui-form trigger/scope 细节 | 动态 row 不是一个 parent store；绑定生命周期分散；Garde 与 form 内部路由耦合 |
| Shortcut | `features/settings/shortcuts/dialog.rs::save` `prepare_submit` 后再次读取 enabled model catalog并调用 `resolve_run_settings` | 提交不是一个 form/catalog 一致快照；旧 `FieldChangeSource` 和自定义 bind helper仍存在 |
| RunSettings | `components/run_settings.rs` 保存 reader/writer closures、三个 picker state、token budget control、source 路由与 subscriptions；`resolve_run_settings` 会为无效 reasoning 计算默认值 | wrapper 过度承担 form 同步；选项变更与业务值混合；提交时可能隐式改变 reasoning 语义 |
| ChatInput | `components/chat_input.rs::can_send` 与 `submit_snapshot` 多次 `form.read(cx).value()` 和 `load_model_choices`；composer snapshot又单独写回 form | 一个发送动作可跨多个 form/catalog快照；调用者直接读 whole-form；附件与模型检查不具原子边界 |
| i18n | `features/settings/form_validation.rs` 已有 `JacoValidationContext`、`JacoGardeI18nProvider` 与 Garde 0.23 的 17 个方法；dialog 使用 `observe_global::<I18n>` | 保留这一共享层，但 locale observer 必须 set context 后显式 Dynamic 重验；form 不持有 global subscription |
| Catalog | `state/providers.rs` 已有 `ProviderCatalogGlobal` 和 `SharedStore<ProviderCatalogSnapshot>`；mutation helper在 DB 成功后 `refresh_snapshot` | form/control 只消费 store snapshot；不调用 `enabled_provider_models` 的无-global DB fallback |

### 2.2 依赖证据

| 依赖 | 当前 source/version | 目标 | [上游事实] 证据 | 本地迁移动作 |
| --- | --- | --- | --- | --- |
| `gpui-component` | Git lock `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` | Git commit `5b45bcb26b9343d91a123a4d5ed8a654360512e5` | PR #2576；`crates/ui/src/combobox.rs` 新增 `ComboboxState::set_selected_values`，使用当前 delegate解析 values、缺失项忽略、静默更新且不发 `ComboboxEvent` | 由 adapter 计划升级；删除本地旧 delegate capture/readback workaround |
| Zed/GPUI | root manifest `rev = 1d217ee39d381ac101b7cf49d3d22451ac1093fe` | manifest 使用与 gpui-component 相同的无 query Git source；`Cargo.lock` 固定 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | gpui-component `5b45bcb...` 的 manifest使用无 query source，其 lock在 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` 验证；Cargo 以完整 source identity 区分 crate | 移除 root Zed `rev` 与旧 `[patch.crates-io]` source 分叉；提交 lock；所有验证使用 `--locked` |
| `garde` | `0.23.0`，features `derive/url/email/pattern` | 不变 | registry source `garde-0.23.0`；`Validate::validate_into` 支持 context 与聚合 report；`garde::i18n::I18n` 签名已由当前 `form_validation.rs` 核对 | Jaco 继续 direct workspace dependency；不新增验证库 |

Zed/GPUI 的完整升级、feature、MSRV、平台与 lockfile验证以 `typed-bound-controls.md` 的依赖工作包为唯一来源；本计划只消费其完成结果，避免两份升级步骤漂移。

### 2.3 明确不变的系统表面

- [决定] 数据库：No change。表、列、索引、外键、query、transaction 与 migration均不变。
- [决定] 数据获取：provider/model 的持久化和刷新仍由 `state::providers` 负责；form 不缓存 DB payload、不定义 TTL、不做网络请求。
- [决定] secrets：No format change。Keychain/secret store 与 SQLite 无跨系统事务；若 secret 写入成功而 DB 保存失败，页面报告失败、不 rebase，后续 retry 以同一 key 覆盖写入。
- [决定] icons/assets：No change。
- [决定] UI布局与可访问文案：No change；仅绑定、验证和保存状态 owner改变。
- [决定] 离线/网络：No change。provider model fetch 与 MCP OAuth 保持现有 timeout、错误和 retry策略。

## 3. 已冻结决策

1. [用户决定] 每个 editor/dialog/controller 只持有一个顶层 form entity；group/array 使用 typed projection，不创建 child form entity。
2. [用户决定] form 只拥有 typed current value、baseline、revision、validation report/generation和已启动的 async validation task；不拥有 FocusHandle、touched/blurred、control state、persistence Task、loading、retry或 submission attempts。
3. [用户决定] bound-control wrapper 只保存 native `Entity<State>` 与 `Vec<Subscription>` 并实现 `Deref`；页面不另存 binding subscriptions。
4. [用户决定] component user event在 emitter update结束后 defer typed form write；任意 form event都让所有 control（包括来源 control）调用 silent setter重新投影，不做 origin echo skip、不返回 authoritative readback。
5. [用户决定] `set_validation_context` 只替换 context并 notify；locale/catalog 等外部状态变化后由页面显式执行 `ValidationTrigger::Dynamic`。
6. [用户决定] on-mount只在初始 value/context安装完成后运行一次；所有已启动 async validation都阻止提交，非阻塞提示归应用自己管理。
7. [用户决定] persistence owner是页面。页面在同一次 form update中取得 `FormRevision` 与 `prepare_submit` output；成功后调用 `rebase_if_revision`。CAS失败无任何 form副作用，保留新编辑和 dirty状态。
8. [用户决定] 用户未选择模型时保持 `None` 并报 required；保存模型被删除/禁用时报 unavailable；任何路径都不自动回退到第一个 enabled model。
9. [用户决定] Jaco 同步验证优先使用 Garde。`#[form(required)]` 只负责通用空值；格式、range、跨字段、catalog 与 stable-row规则由 Garde attributes/custom/manual `Validate::validate_into` 实现，不再构造 gpui-form `ValidationIssue`。
10. [用户决定] FocusHandle只属于具体 native component；同一 field可由多个 control消费。页面根据 `first_error_path()` 和当前可见 control选择 focus目标。
11. [决定] `ChatInputSubmit` 由一次 `prepare_submit` output与一次 provider catalog snapshot构造；其间不 reload catalog、不读数据库、不改变 selected model。
12. [决定] `RunSettingsInput` 的无效 reasoning selection不自动改写。model capability变化后保留 typed值并产生 Dynamic/Submit错误；用户明确选择后才更新。
13. [决定] RunSettings 保留当前分组、可搜索的 `PickerListDelegate`/`ListState` 交互，不强制换成 gpui-component 标准 `SelectState`。三个 picker 各有一个应用内 owning bound-control newtype；ChatForm 只接收其 native state entity，因此布局、键盘与鼠标交互不变。

## 4. 目标架构

### 4.1 文件与职责

```text
app/jaco/src/
├── foundation/i18n.rs                         # 可 clone 的 I18n snapshot；无 form resolver global
├── features/settings.rs                       # 注册 form_validation 模块
├── features/settings/form_validation.rs       # Garde/Fluent 共享 adapter与 message rendering
├── features/settings/prompts/
│   ├── form_state.rs                          # Prompt typed model/context/transform
│   └── dialog.rs                              # controls、locale subscription、同步 persistence/CAS
├── features/settings/provider.rs              # page/editor/save task与 provider orchestration
├── features/settings/provider/editor_state.rs # metadata、catalog/fetch与 manual-model editor state；无 form leaf draft
├── features/settings/provider/forms.rs        # variant form enum/output；无 submit runtime
├── features/settings/provider/forms/
│   ├── api_key.rs
│   ├── ollama.rs
│   ├── custom_openai.rs
│   └── secret.rs                              # owning typed secret control
├── features/settings/mcp/
│   ├── form_state.rs                          # 一个 parent form与 stable-ID rows
│   ├── form_rows.rs                           # row controls；无 child form entity，row store只作 *_in namespace
│   ├── validation.rs                          # manual Garde Validate + stable path helper
│   └── dialog.rs                              # OAuth/save task、locale subscription、CAS
├── features/settings/shortcuts/
│   ├── form_state.rs                          # parent typed model/Garde/transform
│   ├── validation.rs                          # canonical hotkey pure predicates
│   └── dialog.rs                              # controls、catalog snapshot、同步 persistence/CAS
├── components/chat_input.rs                   # parent form owner与单快照 submit composition
├── components/chat_input/form_state.rs        # typed ChatInput model/Garde context/transform
├── components/chat_input/attachment_flow.rs   # typed attachment field commands
├── components/picker.rs                       # picker projection的原地 locale刷新；保留 open/query/selection
├── components/run_settings.rs                 # app-local picker bound controls + thin composition
└── components/run_settings/policy.rs          # pure capability/options/validation helpers
```

不新增 `mod.rs`。`state/providers.rs`、database/repository、assets与 schema不在本计划修改清单中。

### 4.2 共享验证契约

保留并收敛 `features/settings/form_validation.rs`：

```rust,ignore
#[derive(Clone)]
pub(crate) struct JacoValidationContext<D> {
    pub(crate) dependencies: D,
    i18n: I18n,
}

impl<D: Clone> JacoValidationContext<D> {
    pub(crate) fn new(dependencies: D, cx: &App) -> Self;
    pub(crate) fn relocalized(&self, cx: &App) -> Self;
    pub(crate) fn error(
        &self,
        key: &'static str,
        args: &FluentArgs<'_>,
    ) -> garde::Error;
}

pub(crate) struct JacoGardeI18nProvider;
pub(crate) struct JacoGardeI18n { i18n: I18n }
```

- `JacoGardeI18n` 实现 Garde 0.23 全部 17 个 `I18n` 方法；方法签名以 registry source为准，例如 `length_lower_than(&self, min: usize)`、`email_invalid(&self, reason: garde::i18n::InvalidEmail)`。
- Garde 内置/custom错误在验证时使用 context中的 I18n snapshot生成 `ValidationMessage::Localized`；`ValidationMessage::Key` 仍由 render时 Fluent解析。
- 每个 editor持有 `observe_global::<I18n>` subscription。callback capture weak editor owner，并统一通过 `cx.defer` 在当前 global/owner update结束后执行；deferred callback再次 upgrade owner。owner先原地刷新仍挂载的 native control/picker 的本地化 labels、sections、empty/search projection，再对 active form依次 `set_validation_context(relocalized)`、`validate(Dynamic, Form)`；不只捕获 form而留下旧语言的 component projection。
- form与 bound control都不持有 global subscription。

### 4.3 页面保存契约

每个保存入口遵循同一顺序，不新增通用 app wrapper：

```rust,ignore
let prepared = form.update(cx, |form, cx| {
    form.set_validation_context(next_context, cx);
    form.validate(ValidationTrigger::Dynamic, ValidationScope::Form, cx);
    let revision = form.revision();
    let output = form.prepare_submit(cx)?;
    Ok::<_, SubmitError>((revision, output))
})?;

// persistence belongs to the page/controller.
let (revision, output) = prepared;
// ... persist output ...
let rebased = form.update(cx, |form, cx| {
    form.rebase_if_revision(revision, saved_business_value, cx)
});
```

- `prepare_submit` 不返回 busy，也不启动 persistence。
- 页面用 `Option<Task<()>>`/既有 task map表达 one-in-flight；重复点击由页面状态拒绝。
- task drop/页面关闭取消等待或使 weak completion失效；`Drop` 不启动 async。
- persistence失败不 rebase、不改变 revision，保留当前输入并显示既有通知。
- persistence成功但 CAS失败说明用户已继续编辑：外部 store/DB保留成功结果，form不改 current/baseline/report；页面保持 dirty并允许再次保存。
- prompt/shortcut当前为同步 repository调用，也捕获 revision并使用同一 CAS规则；不为同步路径人为创建 Task。

### 4.4 表单和组件组成

#### Prompt

`PromptEditFormInput` 保持 `name: String`、`content: String`，同时 derive `FormStore`/`garde::Validate`。空值只用 `#[form(required)]`；duplicate name为 Garde custom rule，context包含当前 prompt ID与现有名称snapshot。`PromptEditDialogState` 持有 form、两个 `FormInput`和 locale subscription；保存成功把标准化 output作为 saved value做 CAS rebase。

#### Provider

- `ProviderSettingsForm` 保留三种 variant form entity；删除 `is_submitting`、`submit_async_save`、`track_provider_submit`、`current_output` 与依赖 form runtime的 API。
- `ProviderEditorState` 增加/保留页面级 `save_task: Option<Task<()>>`、`ProviderEditorMetadata`、model catalog/fetch状态与 `ManualModelEditor`；这些类型不保存 provider form leaf。
- 删除 `ProviderDraft`、`ProviderDraftSnapshot` 与 `ProviderDraftValue`。替代的 `ProviderEditorMetadata` 字段精确固定为 `provider_id: Option<ProviderId>`、`kind: ProviderKindKey`、`existing_secret_refs: ProviderSecretRefs`；不得包含 `display_name`、`enabled`、`fields`、`dirty`、validation report或 form snapshot。`ProviderSelection`、`ProviderModelDraft` 与 manual-model native controls仍是各自独立的列表/编辑器状态，不并入 metadata。
- provider settings dirty只由 active variant `form.is_dirty()` 派生；secret的 `changed` 是 typed secret field的一部分，已包含在 form dirty中。manual-model editor有自己独立的 model dirty，不与 provider settings form互相镜像。
- `ProviderSettingsFormOutput` 是 transform output；`persistent_fields` 直接返回现有 `ProviderSettingValue`/`ProviderSettingFieldValue`，`settings_payload`、`secret_fields` 保持 pure函数，不再经过 `ProviderDraftValue` 中间枚举。
- secret write成功而 DB失败时不 CAS；retry重用预分配 provider ID与 secret ref owner，不创建重复 provider。

#### MCP

- `McpServerFormInput` 是唯一 parent model。`args/env/env_vars/headers/env_headers` 都使用 `#[form(array(id = "row_id"))]`；row ID immutable、同一数组内唯一且从不复用。
- 五个 row business model继续 derive `FormStore`，并保留 `McpArgRowFormStore`、`McpEnvVarRowFormStore`、`McpEnvRowFormStore`、`McpHeaderRowFormStore`、`McpEnvHeaderRowFormStore` 作为 generated `*_in(parent_field)` namespace；只删除它们的 child `Entity`、runtime与重绑 helper。
- row leaf accessor固定为：
  - `McpArgRowFormStore::value_in(McpServerFormStore::args_item(&form, id))`；
  - `McpEnvRowFormStore::{key,value}_in(McpServerFormStore::env_item(&form, id))`；
  - `McpEnvVarRowFormStore::value_in(McpServerFormStore::env_vars_item(&form, id))`；
  - `McpHeaderRowFormStore::{name,value}_in(McpServerFormStore::headers_item(&form, id))`；
  - `McpEnvHeaderRowFormStore::{name,env_var}_in(McpServerFormStore::env_headers_item(&form, id))`。
  重复调用 accessor只创建廉价 typed handle，绝不创建 row value副本、subscription或 child store entity。
- add/remove/reorder只修改 parent typed vector；删除 row即 drop其 wrapper/subscriptions/control issue。
- `validation.rs` 实现标准 `garde::Validate::validate_into`，只接收 typed input与 `McpServerValidationContext`。Garde index path在进入 form report时由 generated stable-ID path mapper转换；不再传 `ValidationTrigger`/`ValidationScope` 给业务 predicate。
- OAuth登录/退出状态继续由 dialog持有；save request构造和 credential清理顺序不变。保存成功后用实际 persisted config转换出的 form value做 CAS。

#### Shortcut 与 RunSettings

- `ShortcutEditFormInput` 是 parent；`run_settings` 是 group，不创建 child form entity。`RunSettingsInput` 继续以 `#[form(store = RunSettingsFormStore)]` derive；generated `RunSettingsFormStore` 只作为 nested `*_in(parent_field)` accessor namespace。
- hotkey required由 `#[form(required)]`，canonical/冲突由 Garde custom rule；`validation.rs` 仅保留 pure canonical与 predicate helper。
- `components/run_settings.rs` 最终定义 `FormModelPicker`、`FormReasoningPicker`、`FormApprovalPicker` 三个应用内 owning bound control。每个类型字段顺序固定为 `subscriptions: Vec<Subscription>` 在前、对应 native `state: Entity<...>` 在后，并实现 `Deref`；subscriptions因此先于 native state析构。
- `RunSettingsBoundControls::new(parent, ...)` 从同一个 parent group handle精确创建 `RunSettingsFormStore::model_in(parent.clone())`、`RunSettingsFormStore::reasoning_selection_in(parent.clone())`、`RunSettingsFormStore::approval_mode_in(parent.clone())` 三个 typed leaf，再把 leaf传给对应 picker constructor；picker不能绑定或重写整个 `RunSettingsInput` group。
- 三个 native state 继续使用现有 `ListState<PickerListDelegate<...>>`，并保存渲染 picker 所需的 open/query/display projection。这里的 selected projection只是 form typed value的静默 UI投影，不是可独立提交或持久化的第二业务源；任何用户 confirm都只产生 defer后的 typed field intent。
- `RunSettingsBoundControls` 是页面/controller持有的普通组合，拥有三个 picker wrapper与按需挂载的 `Option<FormIntegerInput<u32>>`。`chat_form::controls::RunSettingsControls` 仍是纯视图输入，只包含三个 `ControlSlot<Entity<...>>`；它不拥有 subscriptions、form field、catalog或保存逻辑。
- 标准 gpui-component `FormSelect` 只用于真正采用 `SelectState` 的页面，不用于替换 RunSettings 的分组/搜索 picker。这样迁移不会改变当前 model grouping、搜索、键盘确认、取消或 popup样式。
- locale与 provider catalog subscription都是页面/controller级 orchestration subscription，不是 binding subscription；页面保存它们，deferred callback只 weak-capture页面/controller owner，并调用 `RunSettingsBoundControls` 的原地 refresh/reconcile方法。`RunSettingsBoundControls`/各 picker wrapper不观察 global、store或数据库。
- provider catalog更新取得一次 `ProviderCatalogSnapshot`，由页面/controller调用 native state更新 options/delegate，再以当前 delegate静默重投影；form value不变，随后把同一 snapshot装入 validation context并显式 Dynamic重验。
- model为 `None` 或 key不存在、reasoning selection不受 capability支持、token budget越界都产生明确错误；不计算或写入默认选择。
- 快捷键保存使用同一次 catalog store snapshot验证并解析 `RunSettingsSubmitSnapshot`；不调用 DB fallback。

#### ChatInput

- `ChatInputInput` 继续包含 composer、attachments和 run_settings group；同步 submit验证使用 Garde context中的一次 `ProviderCatalogSnapshot`。
- `can_send` 只读取 generated form的 status/field projection与同一 catalog store snapshot；不直接 `form.read(cx).value()`。
- send入口先把 composer native snapshot通过 typed field写入 form；deferred write完成后在一个 owner update中取得 catalog snapshot与 `prepare_submit` output。
- output只解析一次 selected model，随后用同一个 `ProviderModelChoice.capabilities` 检查 attachments并构造 `ChatInputSubmit`；不存在 fallback或第二次 catalog加载。
- `save_chat_form_config` 由 run-settings field event驱动，将 event/typed projection写入 `gpui-store`；不回读 controls、不把 config反向当成并发业务源。

## 5. 上游复用与删除审计

| 本地实现 | 上游能力 | 语义差异 | 决定 | 删除/修改 | 回归测试 |
| --- | --- | --- | --- | --- | --- |
| Combobox旧 delegate capture与 selected-index投影 | `ComboboxState::set_selected_values` at `5b45bcb...` | 上游按当前 delegate解析、静默、忽略缺失值 | Reuse directly | adapter删除 readback/delegate缓存；Jaco只更新 native delegate/options | catalog reorder/remove后 value投影正确且无 Change echo |
| `RunSettingsReader/Writer` + `FieldChangeSource` | core `FormField<T>` + owning controls | 新契约 form event总是 silent reproject | Delete | `components/run_settings.rs` 删除 source路由closures | model/reasoning/approval双向同步且无 nested update panic |
| Provider `track_provider_submit` | 页面现有 `Task`/weak entity能力 | form不再拥有 persistence | Delete | `provider/forms.rs` 删除 runtime helper | stale save不覆盖新输入 |
| MCP row child form entities | generated identified item projection + row store `*_in` namespace | parent revision/report统一 | Reuse directly | 保留五个 row store namespace；删除 child `Entity`/runtime/重绑helper | reorder后 error跟随 stable ID |
| Jaco手写 `ValidationAdapter`/`ValidationIssue` | Garde 0.23 + macro Garde adapter/path mapper | control issue和真正 async validation仍由 core原生处理 | Adapt | 业务同步规则迁为 Garde；保留 app I18n bridge | 旧 predicate逐项等价、英中消息完整 |
| 页面 `show_error`/form focus状态 | native component FocusHandle + form report | 同一 field可多 control | Delete | 页面按 path选择可见 control | 首错 focus正确，无第二布尔源 |

剩余自定义职责只有：Jaco领域 predicate、Fluent文案、provider/MCP persistence orchestration、RunSettings业务组合和 ChatInput submit composition。

## 6. 工作包

### JACO-FORM-10：依赖与公共 API 接入

**前置**

- adapter `DEP-00`、core `FORM-10..60`、macro `MACRO-10..40`、adapter `CORE-GATE` 与
  `CONTROL-10..30` 已按第 1 节顺序完成；`CONTROL-40/50` 此时明确尚未执行。
- [发布门槛] `Cargo.lock` 中 gpui-component为 `5b45bcb...`，Zed crates只有一个无 query source identity，并通过 adapter计划的 locked检查。

**证据**

- 本文 2.2；三个库 Guide与实施计划。

**文件**

- `app/jaco/Cargo.toml`：No change；继续使用现有 `garde.workspace = true`，不新增或删除 Jaco direct dependency。
- 修改 `features/settings.rs`、`foundation/i18n.rs`、`features/settings/form_validation.rs`。
- 修改两份 `app/jaco/locales/*/main.ftl`，仅补齐实际被 Garde provider/custom rules引用而缺失的成对 key。

**API contract**

- 使用 4.2 的 `JacoValidationContext`/`JacoGardeI18nProvider`。
- 不新增 Jaco form facade、global resolver或 subscription set。

**实施流程**

1. 切换所有 imports到最终 core/macro/adapter公开路径。
2. 删除 `FormTextResolver`/旧 typed module兼容引用。
3. 对照 Garde 0.23 trait逐个实现并测试 I18n method。
4. 建立 editor locale observer的共同写法，但不抽成持有生命周期的共享类型。

**错误与生命周期**

- 缺失 Fluent key在测试中失败；运行时保持现有 fallback显示 key。
- observer只捕获 weak owner；drop即停止。

**UI/data/database/icons/i18n/dependencies**

- UI：No change。
- DB/data acquisition：No change。
- icons/assets：No change。
- i18n：Garde keys在 en-US/zh-CN成对存在。
- dependencies：只消费已完成的 workspace升级，不再改版本选择。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| Garde i18n签名和 key | `features/settings/form_validation.rs` | `garde_builtin_rule_messages_exist_in_both_locales` | en-US/zh-CN bundle | 17种消息不返回 key；参数存在 |
| locale invalidation | focused GPUI test | `locale_change_revalidates_active_form_once` | invalid form + `set_global(I18n)` | context替换后 Dynamic report变语言；无 reentrancy |

**验证**

`cargo test -p jaco form_validation --locked`

**完成条件**

- Jaco编译只使用最终 form API；无 resolver global；locale切换契约有测试。

### JACO-FORM-20：Prompt 参考迁移

**前置**：JACO-FORM-10。

**证据**：`prompts/form_state.rs`、`prompts/dialog.rs` 当前路径。

**文件**：只修改上述两个文件及缺失的 prompt locale key。

**API contract**

- `PromptEditFormInput`、`PromptEditValidationContext`、`PromptEditTransform`。
- dialog字段：form、两个 owning `FormInput`、`Vec<Subscription>`（仅 locale/owner级订阅）；无 page binding subscription、show-error bool或 submit runtime。

**实施流程**

1. 使用 strict macro grammar重新声明 form。
2. 用 `FormInput::new(field, |window, cx| InputState::new(window, cx) ..., window, cx)` 一次创建+绑定；build closure只接收 `window/cx`，不接收预先存在的 state。
3. save同一次 update取得 revision/output；repository成功后 CAS rebase并关闭/通知。
4. 首错 path为 name/content时 focus对应 native InputState。

**错误与生命周期**

- validation失败不调用 repository；repository失败保留输入；CAS失败不覆盖新输入。

**UI/data/database/icons/i18n/dependencies**

- UI/DB/icons/dependencies：No change。
- i18n：复用现有 required/duplicate/save key。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| typed双向绑定 | `prompts/dialog.rs` tests | `prompt_controls_and_form_reproject_both_directions` | GPUI dialog | user change写 form；replace静默回控件 |
| validation/CAS | 同上 | `prompt_save_validates_and_rebases_matching_revision` | fake prompt store | invalid无写；成功baseline更新；failed CAS无副作用 |

**验证**：`cargo test -p jaco prompt --locked`

**完成条件**：Prompt成为其他页面可照搬的最小 composition；无旧 binding/submit runtime引用。

### JACO-FORM-30：Provider variants与页面持有保存任务

**前置**：JACO-FORM-20、adapter Input/Select完成。

**证据**：`provider.rs`、当前 `provider/draft.rs`、`provider/forms.rs`、四个 `provider/forms/*.rs`。

**文件**

- 修改 `provider.rs`、`provider/forms.rs` 与四个 `provider/forms/*.rs`；把
  `provider/draft.rs` 重命名为 `provider/editor_state.rs` 并收敛为 4.4 固定的 metadata/list/manual-model状态；不修改 provider repository/schema。
- 删除 `provider/forms.rs` 内 `track_provider_submit` 及 form submit runtime相关方法。

**API contract**

- 保留三个 typed input/store与 `ProviderSettingsFormOutput`。
- `ProviderEditorState.save_task: Option<Task<()>>` 是唯一 persistence in-flight owner。
- secret control是 owning typed wrapper，只存 native entity/subscriptions。
- `ProviderEditorMetadata` 只有 `provider_id`、`kind`、`existing_secret_refs` 三个字段；`ProviderDraft`、`ProviderDraftSnapshot`、`ProviderDraftValue` 均不存在。

**实施流程**

1. 删除三个 `ProviderDraft*` 类型；persisted record直接转换为 active variant typed input和
   `ProviderEditorMetadata`，render读取 typed field/status，save读取 output，provider settings dirty只读 `form.is_dirty()`。
2. variants改用 owning controls；catalog/API-mode options由 page更新 native state。
3. start save前在同一 form update capture revision/output。
4. 页面顺序执行 secret writes、DB provider save、catalog store refresh；completion weak-update editor。
5. 成功用 persisted record生成 saved typed value并 CAS；失败保留 form和dirty。

**错误与生命周期**

- one-in-flight由 `save_task.is_some()`；重复点击无副作用。
- page drop取消/失效 task completion。
- secret成功/DB失败显示现有错误且不 rebase；retry安全覆盖相同 secret refs。
- provider kind切换不会让旧 task写入新 editor；completion同时校验 editor key和 revision。

**UI/data/database/icons/i18n/dependencies**

- UI、DB schema、icons、dependencies：No change。
- data：现有 state mutation成功后继续 refresh `ProviderCatalogSnapshot`。
- i18n：复用 provider validation/notification key。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| 无 leaf双源 | `provider.rs` tests | `provider_form_is_only_leaf_value_owner` | variant fixtures | metadata只有三个固定字段；无 `ProviderDraft*`；dirty由 form决定 |
| stale save | 同上 | `provider_save_completion_cannot_overwrite_newer_revision` | controllable Task | DB保存旧输出；form新输入与baseline/report不变 |
| partial failure | 同上 | `provider_db_failure_after_secret_write_keeps_form_dirty` | fake secret/DB | 错误可见、无 rebase、retry复用 ID/ref |
| options refresh | 同上 | `provider_api_mode_refresh_reprojects_with_current_items` | reordered choices | typed value保持；控件显示正确；无 echo |

**验证**：`cargo test -p jaco provider --locked`

**完成条件**：form不持有任何 provider persistence task；三个 `ProviderDraft*` 类型均删除，metadata字段精确等于 contract；stale/partial failure均有测试。

### JACO-FORM-40：MCP parent form与 stable-ID rows

**前置**：JACO-FORM-20、macro array projections完成。

**证据**：`mcp/form_state.rs`、`mcp/form_rows.rs`、`mcp/validation.rs`、`mcp/dialog.rs`。

**文件**

- 修改四个文件。
- 保留五个 row generated `FormStore` namespace；删除其 child `Entity`/runtime及所有 child form创建/重绑helper，保留 row business structs。

**API contract**

- 一个 `Entity<McpServerFormStore>`。
- 五个 row `*FormStore` 只能通过 4.4 固定的 `*_in(parent_*_item(...))` accessor产生 root-typed field handle，不能实例化为 `Entity`。
- row identity为 `FormItemId`/业务 `row_id`；immutable、unique、不复用。
- dialog持有 row wrapper集合、OAuth tasks、save task、locale subscription；form不持有这些资源。

**实施流程**

1. strict macro array语法声明 parent vectors。
2. row controls逐项使用 4.4 的精确 identified-item + `*_in` accessor构造。
3. add/remove/reorder直接修改 typed vector并重建受影响 wrapper；未变 row可复用。
4. Garde report path由当前 index映射到 stable ID。
5. save一次 capture revision/output，沿用 `McpServerSaveRequest`和 credential清理顺序；成功CAS。

**错误与生命周期**

- duplicate/missing row ID是 blocking结构错误；不静默重编号。
- removed row drop后其 subscriptions/control issues失效。
- OAuth in-flight继续阻止 save；dialog drop不执行 async Drop工作。
- persistence/CAS规则同4.3。

**UI/data/database/icons/i18n/dependencies**

- UI、DB schema、icons、dependencies：No change。
- i18n：复用并核对所有 `mcp-validation-*` 参数。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| stable path | `mcp/form_state.rs` tests | `mcp_garde_error_path_survives_reorder` | 三个固定 row ID | error随 ID移动，不随 index漂移 |
| lifecycle | 同上 | `removed_mcp_row_drops_control_issue_and_subscriptions` | invalid integer/input row | remove后submit不再被旧 issue阻止 |
| structure | 同上 | `duplicate_mcp_row_id_blocks_submit` | duplicate fixture | 明确 structural issue，无隐式修复 |
| save CAS | `mcp/dialog.rs` tests | `mcp_save_completion_preserves_newer_edits` | controllable request | stale success无 form副作用 |

**验证**：`cargo test -p jaco mcp --locked`

**完成条件**：MCP只有一个 parent store entity；row namespace仍可编译且不存在 child store entity/runtime；row error/lifecycle/save顺序均有测试。

### JACO-FORM-50：RunSettings与Shortcut

**前置**：JACO-FORM-10、core `FormControl`/deferred attachment契约、adapter `CONTROL-10` 与
`CONTROL-30` 完成。RunSettings不依赖 adapter `FormSelect`/`FormCombobox`；provider等实际使用标准组件的页面可独立消费 `CONTROL-20`。

**证据**：`components/picker.rs`、`components/run_settings.rs`、`components/run_settings/policy.rs`、`shortcuts/form_state.rs`、`shortcuts/validation.rs`、`shortcuts/dialog.rs`、`state/providers.rs` store接口。

**文件**：修改前述 Picker/RunSettings/Shortcut文件；不修改 `state/providers.rs` fallback ownership。

**API contract**

- `RunSettingsInput` 保持 typed model；generated `RunSettingsFormStore` 只作 namespace。三个 controls分别绑定 `model_in`、`reasoning_selection_in`、`approval_mode_in` leaf。
- `FormModelPicker`、`FormReasoningPicker`、`FormApprovalPicker` 的结构固定为 `subscriptions: Vec<Subscription>` 后跟 `state: Entity<...>`；通过 `Deref` 暴露现有 native state API，不保存 reader/writer closure、`FieldChangeSource`、field handle或 form snapshot。
- `RunSettingsBoundControls` 持有上述三个 wrapper和 `token_budget: Option<FormIntegerInput<u32>>`，并通过 `view_states()` 生成给纯 UI `RunSettingsControls` 使用的 entity clones；clone entity不会复制 subscription owner，页面必须保留 `RunSettingsBoundControls` 生命周期。
- page/controller持有单独的 `orchestration_subscriptions: Vec<Subscription>`，且该字段声明在 `RunSettingsBoundControls` 之前以先 drop；它只保存 locale/catalog/model/reasoning依赖编排，不保存单控件 binding subscription。
- `PickerListDelegate::replace_projection(sections, empty_label, selected_value)` 是应用内静默 setter：保留 open与 `last_query`，用新 sections重新执行 query，按 selected value重算 highlight，并 notify；不重建 `ListState`、不发 confirm/cancel或 form event。
- `RunSettingsSubmitSnapshot`/`RunSettingsSubmitError` pure resolver只接收 `&RunSettingsInput`和 `&ProviderCatalogSnapshot`。
- resolver不默认选择、不修正 form值；错误包含 catalog unavailable/model required/model unavailable/reasoning unsupported/token budget invalid。

token budget不是 `RunSettingsInput` 的真实 leaf，唯一 accessor固定为 parent group projection：

```rust,ignore
let token_budget = parent.project_value(
    "token_budget",
    |settings| custom_token_budget_value(settings.reasoning_selection.as_ref()),
    |settings, value| {
        set_existing_custom_token_budget(&mut settings.reasoning_selection, value)
    },
);
```

`set_existing_custom_token_budget` 是 `policy.rs` 的 pure recursive helper，只更新已经存在的
`TokenBudgetSelectionMode::Custom` leaf并返回 `true`；找不到 custom leaf返回 `false`，绝不把其他 reasoning mode改成 custom。用户在 picker明确 confirm Custom时，写入该 option随当前 capability提供的初始 typed budget，之后 projection才可挂载；catalog refresh不重新计算、clamp或覆盖这个值。

**实施流程**

1. 删除 `RunSettingsReader`、`RunSettingsWriter`、`SelectionOrigin`、source路由和 controller内的业务值缓存；保留 picker渲染需要的 native projection。
2. 通过 `RunSettingsFormStore::model_in(parent.clone())`、`RunSettingsFormStore::reasoning_selection_in(parent.clone())`、`RunSettingsFormStore::approval_mode_in(parent.clone())` 把现有三个 `ListState<PickerListDelegate<...>>` 分别包进 owning controls，再由 `RunSettingsBoundControls` 组合。
3. picker confirm只调用对应 leaf attachment的 deferred typed intent；每个 wrapper的 form subscription只从自己的 leaf重新读取值并静默重投影自己的 native state，不读取或更新 peer、capability、token结构、catalog或 validation context。
4. page/controller另持有唯一的 model与reasoning leaf orchestration subscriptions，所有 peer/capability/token跨字段编排只在这里发生。model event defer到 owner后只读一次当前 catalog snapshot，按 selected model推导 capability并替换 reasoning sections，静默投影原 reasoning typed值，然后 reconcile token结构并用同一 snapshot更新 validation context、Dynamic重验；不写 reasoning默认值。reasoning event defer到 owner后 reconcile token结构；各 leaf自身 native projection仍只由第3步对应 wrapper负责，approval没有跨字段编排。
5. token reconcile仅在 `project_value` 当前可读且当前 capability仍提供 token-budget control时挂载 `FormIntegerInput<u32>`；非 custom、custom value缺失、capability不再提供 token budget或 path unavailable时立即 drop wrapper。capability的 min/max/step变化时，用当前 typed value和新 `IntegerInputPolicy<u32>` 原地更新 native policy；若 adapter不支持 policy setter则 drop/rebuild wrapper。两条路径都不修改、clamp或 fallback form值，越界只由 Dynamic/Submit report显示，incomplete raw text只由 control issue持有。
6. catalog subscription由 page/controller持有，事件 defer到 weak owner并调用与 model event相同的 `reconcile(snapshot)`；一次 callback只取得一个 snapshot，更新 sections/capability/token policy后重投影当前 typed leaves，再安装同一 snapshot context并 Dynamic重验。
7. locale subscription由 page/controller持有，事件 defer到 weak owner；owner先用 `replace_projection` 重建 reasoning/approval localized sections与 model/reasoning empty labels，同时保留 query/open/selected value，再 relocalize validation context并 Dynamic重验一次。trigger/search/footer等 render-time文案随同一次 owner notify刷新。
8. Shortcut save在同一 update capture revision/output；使用同一 catalog snapshot解析；同步 persistence成功后以 captured revision调用 `rebase_if_revision`，CAS false不关闭 editor、不改 current/baseline/report/revision/control projection。
9. ChatForm通过 `view_states()` 返回的 `ControlSlot` 复用同一 native states，无 shortcut特制样式副本。

**错误与生命周期**

- invalid integer editor text由 control issue阻止 submit但不写 form。
- model失效保留 key并显示错误；catalog恢复后 Dynamic report清除。
- picker回调只 emit intent，form写入 defer，避免 ListState nested update。
- orchestration callback只 weak-capture page/controller owner；owner、wrapper或 projected path已释放时排队工作为 no-op，不能恢复旧 picker或 control issue。

**UI/data/database/icons/i18n/dependencies**

- UI、DB、icons、dependencies：No change。
- data：只读 catalog store snapshot；无 repository query。
- i18n：复用 shortcut/run-settings keys；静态存入 picker delegate的 section/empty labels必须在 locale callback中刷新，不能只依赖 render-time global读取。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| 无默认模型 | `run_settings.rs` tests | `missing_or_removed_model_never_falls_back` | empty/changed catalog | value保持 None/old key；明确错误 |
| capability变化 | 同上 | `reasoning_value_is_not_rewritten_by_catalog_refresh` | capability snapshots | form不变；Dynamic error出现/清除 |
| 依赖编排 | 同上 | `model_change_reconciles_reasoning_and_token_structure_without_rewriting_values` | model fields + two capabilities | reasoning原值保持；sections更新；budget按 projection mount/drop；一次 snapshot |
| exact integer | 同上 | `token_budget_incomplete_text_blocks_submit_without_changing_value` | u32 control | native text保留；form保持旧 u32；control issue active |
| shortcut一致快照 | `shortcuts/dialog.rs` tests | `shortcut_save_uses_one_form_and_catalog_snapshot` | catalog mutation harness | output不混用两版 catalog；无 DB fallback |
| shortcut stale CAS | 同上 | `shortcut_save_rebase_is_revision_guarded` | capture后修改 form的 fake repository | persistence成功但 CAS false；current/baseline/report/revision/control projection均保持新编辑 |
| reentrancy | focused GPUI test | `run_settings_picker_confirm_defers_form_write` | ListState harness | 无 nested update panic；最终值正确 |
| locale顺序 | focused GPUI test | `run_settings_locale_change_refreshes_picker_projection_before_validation` | open picker + query + invalid form + zh-CN切换 | query/open/selected保持；sections/empty与 report同为新语言；Dynamic一次；无 reentrancy |
| 纯 UI ownership | `chat_form/controls.rs` tests | `run_settings_view_states_do_not_own_bindings` | drop `RunSettingsBoundControls` 后保留 entity clone | subscriptions随 wrapper释放；ChatForm只渲染 native state |

**验证**：`cargo test -p jaco run_settings --locked && cargo test -p jaco shortcut --locked`

**完成条件**：RunSettings无旧 source/reader/writer；普通/temporary/shortcut surface共享 composition；无 fallback。

### JACO-FORM-60：ChatInput单快照提交

**前置**：JACO-FORM-50。

**证据**：`components/chat_input.rs`、`components/chat_input/form_state.rs`、`components/chat_input/attachment_flow.rs`。

**文件**：修改上述三个文件；ChatForm纯 UI文件只在编译适配需要时修改调用签名，不改变布局/style。

**API contract**

- `ChatInputTransform::Output = ChatInputInput`；`ChatInputFormStore::prepare_submit` 返回由同一 form snapshot克隆出的 `ChatInputInput`，不在 transform中读取 catalog或控件。
- `build_chat_input_submit(prepared, catalog) -> Result<ChatInputSubmit, ChatInputSubmitError>` 是 pure函数；一次解析模型并复用 capabilities检查 attachments。
- `ChatInputSubmitError`区分 empty、agent running（owner gate）、catalog unavailable、model required/unavailable、reasoning invalid、attachment capability issue。

**实施流程**

1. composer/attachment mutations统一走 typed fields。
2. 删除 whole-form direct read和多次 `load_model_choices`。
3. send入口取得一次 form output与一次 catalog snapshot，调用 pure builder。
4. can-send projection复用同一 pure policy或 form status，不维护第二 business bool。
5. run-settings变化事件同步 `ChatFormConfig` store；外部 config加载使用显式 `replace/rebase`，不 observer双向争抢。

**错误与生命周期**

- builder失败不发送、不清空 composer/attachments。
- 发送成功后的 form清理由 owner在既有 agent-run启动边界执行；失败/取消行为保持现状。
- catalog在 snapshot后变化只影响下一次发送，不修改本次 prepared output。

**UI/data/database/icons/i18n/dependencies**

- UI/DB/icons/dependencies/i18n：No change。
- data：只读一次 provider catalog store snapshot。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| one snapshot | `components/chat_input.rs` tests | `chat_submit_uses_one_form_and_catalog_snapshot` | catalog mutation counter | catalog只读一次；字段来自同一 output |
| attachment/model | 同上 | `chat_submit_checks_attachments_against_prepared_model` | two models with different capabilities | 无 fallback；错误/成功对应 selected key |
| no selection | 同上 | `chat_submit_requires_explicit_model_selection` | enabled catalog + model None | 不选第一项；不发送 |
| config sync | 同上 | `run_settings_event_persists_config_without_control_readback` | gpui-store fixture | store收到 form typed值；无 direct control read |

**验证**：`cargo test -p jaco chat_input --locked`

**完成条件**：send/can-send/config sync均无 whole-form direct read、多 catalog加载或 fallback；评论中的附件/模型一致性风险由结构消除。

### JACO-FORM-70：Legacy删除与全应用收口

**前置**：JACO-FORM-20至60完成，然后 adapter `CONTROL-40` 与 `CONTROL-50` 完成；本包是最后一个 Jaco residual gate，完成后交接给 core `FORM-70`，不是整体 release/document-status gate。

**证据**：三个库的 legacy deletion gate和本计划证据表。

**文件**

- 删除只服务旧 Jaco form API的 helpers/types/imports。
- 同步 `app/jaco/docs/dev/README.md` 的实现事实与 FORM-70 交接状态；最终“已实现/已发布”状态只由 core `FORM-70` 在 workspace gate通过后统一切换。不修改 issue #175 的已冻结产品设计，除非链接失效。

**API contract**：不新增兼容 API。

**实施流程**

1. residual scan。
2. 删除 dead helpers、旧 tests和无引用 locale key；仅删除能证明不再使用的内容。
3. 跑 Jaco/adapter 定向 locked验证，保存 residual结果并交接 core `FORM-70`；全 workspace、平台CI与最终文档状态由 FORM-70执行。

**错误与生命周期**

- 所有长期 Task有明确 owner；所有 completion使用 weak entity；Drop无 async。

**UI/data/database/icons/i18n/dependencies**

- 除已列修改外全部 No change。

**测试**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| residual gate | command evidence | `rg` gates | active source | 下列旧符号为零 |
| shared UI | existing/focused GPUI tests | `chat_form_surfaces_share_controls_without_layout_change` | home/temporary/shortcut | controls一致；无 reentrancy/focus回归 |

**验证**

```bash
rg -n 'SubmitRuntime|submit_runtime|SubmitError::Busy|FieldChangeSource|set_with_source|FormFieldHandle|FormDraftEvent|FieldCodec|FormTextResolver|WeakControlAttachment|SelectionOrigin|RunSettingsReader|RunSettingsWriter|source_id|authoritative[_-]?(readback|value)|bind_(input|number|select|combobox|bool)|form\.read\(cx\)\.value\(\)' app/jaco/src
rg -n 'gpui_form(::typed)?::ControlId|Entity<Mcp(Arg|EnvVar|Env|Header|EnvHeader)RowFormStore>|Mcp(Arg|EnvVar|Env|Header|EnvHeader)RowFormStore::(new|from_value|from_value_with_validation_context)' app/jaco/src
rg -n '\bProviderDraft(Snapshot|Value)?\b' app/jaco/src
```

五个 MCP row store名称仍应在 `*_in` namespace调用中出现，不能对类型名做零命中 gate。正常的
`WeakEntity`/`Entity::downgrade()` 是 locale、catalog与 persistence completion的 owner生命周期机制，也不能加入零命中 gate；这里只禁止应用看到 core-private weak attachment、control/source identity或 authoritative readback。

**完成条件状态**：Jaco active-source residual 为零、定向与 workspace 自动化验证通过、无兼容层；
定向 Computer Use smoke 已完成，临时窗口全局快捷键与有数据列表键盘流程仍是人工验证缺口，
不能标记为完整跨平台发布证据。

## 7. 跨包验证

JACO-FORM-70 完成定向验证与 Computer Use smoke 后，把证据交给 core `FORM-70`。以下全 workspace
命令由 `FORM-70` 按顺序执行并保留输出，不是 MACRO 或 JACO-FORM-10..70 的反向前置：

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p jaco --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo tree -d --locked
git diff --check
```

平台门槛与 `.github/workflows/ci.yml` 一致：macOS、Linux、Windows全部通过。Linux依赖只允许通过 `script/bootstrap`/`script/install-linux.sh`维护；本计划预期无需改动。

实施完成后执行一次有明确脚本的 Computer Use smoke（运行需用户授权）：

1. Prompt：无效/重复/保存/语言切换。
2. Provider：三个 variant、保存中继续编辑、model刷新、secret失败。
3. MCP：row增删重排、HTTP/stdio切换、OAuth、保存中编辑。
4. Shortcut：model/reasoning/approval/token budget、picker键盘与鼠标。
5. ChatInput：home/new/temporary/conversation四个 surface，无模型、失效模型、图片/文件附件。

发布证据必须同时包含自动化结果与 smoke结果；未执行的项不能写成“完成”。

## 8. 执行交接审计

- 所有 public/architecture选择已经由用户确认；实施者无需选择 source policy、submit owner、validation owner、binding echo语义、required语义、model fallback或 wrapper形态。
- 三个库计划是 API与依赖唯一来源；Jaco计划只描述应用组合，不复制库内部实现。
- 每个 mutable resource都有单一 owner：form business/validation、control native state/subscriptions、page persistence/locale、gpui-store catalog、service/DB。
- 每个异步路径都规定 task retention、weak completion、stale revision、partial persistence与 retry边界。
- database/data acquisition/icons/assets/i18n/dependencies/UI等系统表面均已明确修改或 No change。
- 每个 requirement映射到具体 test、fixture、assertion、validation和 done condition。
- 跨计划发布顺序唯一固定为 `DEP-00 -> FORM-10..60 -> MACRO-10..40 -> CORE-GATE/CONTROL-10..30 -> JACO-FORM-10..60 -> CONTROL-40/50 -> JACO-FORM-70 -> FORM-70`；macro不得等待最终FORM-70，JACO-FORM-70完成后必须明确交接给core FORM-70执行最终workspace/release/document-status gate。Exact commit/source/locked验证已写入对应计划，不需要实施者重新做广泛研究。
