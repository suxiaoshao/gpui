# Jaco：显式 form owner API 迁移

## 状态与范围

- 状态：`Implemented`（自动化通过；定向 UI smoke 已执行，MCP 删除交互由自动化覆盖）
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 计划 ID：`issue-199`
- Jaco 子任务跟踪：[Issue #199 子任务](README.md)
- 根计划：[工作区 Issue #199 计划](../../../../../docs/dev/issue-199/README.md)
- Jaco 开发文档索引：[开发计划](../README.md)
- 负责范围：`app/jaco`
- 引用的根计划 ID：`S-01`–`S-12`、`C-01`–`C-04`、`ERR-01`–`ERR-04`；
  本计划直接消费 `C-03`、`C-04`，消费方工作等待 `C-02`/`C-04` `producer-ready`
- 本地 ID 范围：`E/D/F/L/ST/R/T-400..499`、`WP-400..499`
- 已分配工作包：`WP-400`–`WP-406`
- 实施引用：当前分支 `codex/199-adopt-gpui-store-form-operation` 工作树（待提交/PR）

本文是 Issue #199 中 Jaco form 迁移的专题开发文档；目录 `README.md` 只跟踪多轮子任务与
文档入口，不承载本计划细节。本文只规划 Jaco 对 breaking `gpui-form` v2 契约的迁移。
Jaco 已经是三个 form crate 的主要
消费方，本轮保留现有 typed draft、验证、控件、保存与 UI 行为，只把隐式 field-owned form
改为静态 descriptor + 显式 strong form。Issue #199 中其他 app 的 store/form/operation 接入和
Jaco 复杂逻辑的 `Transition` 重构不属于本文。

## 实施结果（2026-08-02）

- `WP-400`–`WP-405` 已完成：所有 Jaco form derive、static/partial descriptor、owning control、
  validation 和 prepared-submit 调用面已迁移；Provider 不再镜像 validation report。
- `cargo test -p jaco --locked` 355/355 通过；Provider 修改后的 30 个定向测试通过；与三个 form
  crate 合并执行的 all-target/all-feature clippy 在 `-D warnings` 下通过。
- active Jaco form consumer 中旧 derive/accessor/adapter/submit API 零命中；现有其他
  `gpui-operation` 状态机属于 Issue #199 后续范围，未在本轮改动。
- 本地 bundle 以隔离配置/数据运行：Provider 校验失败与成功、Prompt 必填、Shortcut 嵌套
  RunSettings 必填、MCP 动态参数新增均通过。Computer Use 未能触发 MCP 垃圾桶按钮，删除语义由
  `remove_last_array_row_leaves_empty_list` 自动化测试覆盖；未保存测试数据或 secret。

### 高影响变更摘要

Jaco 当前由 `#[derive(gpui_form::FormStore)]` 生成 `*FormStore`、field enum 和
`State::field_field(&form)` accessor；descriptor 隐式保存 weak form。RunSettings controller、
Provider secret control、MCP dynamic rows 与多个 subscription 因而 clone 或长期保存 descriptor，
并在确定持有 strong entity 的路径上处理 `Result`。

目标迁移后，每个 model derive `FormModel` 并生成 `*Form` state；静态字段以 associated const
使用。页面、controller 和 application container 显式持有 `Entity<*Form>`，同步 helper 每次传
`&form`。partial 仅用于 MCP identified row、RunSettings token-budget projection、Shortcut prompt
selection projection 与 Provider API mode projection；弱 form 只存在于 adapter `ControlBinding`、
deferred closure、subscription 或 async completion。

### 目标

1. 把 Jaco 所有 form model 原子迁移到 `FormModel` / `FormState` 与 associated const API。
2. 删除所有 `*_field(&form)`、`*_in(...)`、`identified_item(..., id_fn)`、`set_user_value`、
   `FormEvent<Form::Field>` 与 descriptor lifetime `expect`。
3. Prompt、Provider、Shortcut、ChatInput 的静态字段和 total group child 使用 total API；MCP row
   与 computed projection 使用 partial `try_*`，不把真实 availability 错误扩散到 total path。
4. bound control 构造显式接收 form。Jaco 自定义 Provider secret control 遵守最小 wrapper layout；
   RunSettings orchestration 可以持有 strong form owner，但不缓存字段 descriptor。
5. 所有 submit 直接消费 `PreparedSubmit { revision, output }`，不在两次 form read 之间拼装 revision
   与 model；transform 改成 static、pure、infallible。
6. 保持当前 Jaco 业务规则、catalog/resource gate、持久化 operation、布局、本地化和数据格式不变。
7. Jaco 只调用 `gpui-form` 的领域 API 和 adapter binding；core 内部用于 mutation/validation 的
   message、effect 与 `gpui_operation::Transition` 实现不成为 Jaco 的集成协议。

### 非目标

- 不迁移 Jaco 的 Config/Database/catalog/conversation operation 或 store 架构。
- 不改变 Prompt、Provider、MCP、Shortcut 或 ChatInput 的业务字段和持久化 payload。
- 不重做 RunSettings picker UI、catalog refresh、read-only gate 或 reasoning policy。
- 不引入 Jaco-local form facade、field registry、binding framework 或 compatibility extension trait。
- 不处理其他 app；不修改三个 form crate 的实现步骤，分别由其 owner plan 负责。
- 不把 Jaco 其他复杂逻辑迁移到 `Transition`；那是 Issue #199 的后续独立子任务，范围确认后另建
  专题文档。
- 不因 core-private `Transition` 改写 `gpui-form` README/guide 等公开文档；Jaco 也不公开内部
  mutation/validation 消息。
- 不修改 locale key、assets、icons、database migration 或 `Cargo.lock`。

### 兼容策略

- Jaco 与三个 form crate 在同一 breaking changeset 中迁移，不保留旧 generated name 的 type alias。
- generated state 统一去掉 `Store` 后缀，例如 `PromptEditFormStore` -> `PromptEditForm`；model 的
  `*Input` 名称保持不变。
- tests 与 test helper 同时迁移，不允许生产代码走 v2、test fixture 继续依赖 v1。
- 历史 API 只允许留在 [Issue #175 归档](../issue-175/gpui-form-migration.md)。

## 适用范围

| 表面 | 适用性 | 决定 |
| --- | --- | --- |
| Prompt 设置表单 | 改造 | total 文本 descriptor、静态 transform、prepared submit |
| Provider 设置表单 | 改造 | 三个 generated state、secret control、total 文本和 partial API mode binding、单一 report source |
| MCP server 表单 | 改造 | total root field、stable-ID partial row、component rebuild |
| Shortcut 表单 | 改造 | hotkey binding、prompt projection、嵌套 RunSettings |
| ChatInput 表单 | 改造 | composer/attachments/RunSettings 静态 descriptor 与发送快照 |
| RunSettings controller | 改造 | strong form owner + descriptor constant，不缓存 field handle |
| Jaco validation/i18n | 仅改变机制 | 保留 Garde message/context/可见规则；stable path 与 bucket 交给 core/macro |
| persistence/store/operation | 无变化 | 继续由现有 page/resource owner 持有 |
| database/config/secret 格式 | 无变化 | payload 与事务语义不变 |
| UI layout/assets/i18n key | 无变化 | 仅迁移 ownership/API |

## 证据

### 当前流程

```text
页面创建 Entity<GeneratedFormStore>
  -> GeneratedFormStore::name_field(&form)
  -> FormField 捕获 WeakEntity<GeneratedFormStore>
  -> 控件/controller 保存或克隆 field
  -> 即使页面持有 form，value/set/subscribe 仍返回 Result

提交
  -> 读取 revision
  -> 单独调用 prepare_submit
  -> 在应用代码中拼装二元组
```

### 目标流程

```text
页面创建 Entity<GeneratedForm>
  -> GeneratedForm::NAME 是可复用 descriptor
  -> value/set/控件构造器显式接收 &form
  -> controller 保存 Entity<Form>，不保存 FormField handle

提交
  -> form.update(... prepare_submit ...)
  -> 解构 PreparedSubmit { revision, output }
  -> application operation 持久化 output
  -> 成功后调用 rebase_if_revision
```

### 证据登记

| ID | 证据位置 | 当前事实 | 迁移影响 |
| --- | --- | --- | --- |
| E-400 | `components/chat/input/form_state.rs`、`components/chat/run_settings.rs` | `ChatInputInput` 与 `RunSettingsInput` 分别 derive `FormStore`；前者把后者声明为 group | 两个 model 分别改 derive/state 名，再以静态 total group descriptor 组合 |
| E-401 | `components/chat/{input.rs,input/attachment_flow.rs,form.rs,form/controls.rs}` | controller 已持有 form，但 composer/attachments 重复构造 field；`AttachmentControlState` 还保存可选 form，视觉层会吞掉读取错误 | controller 显式读写；视觉层只接收 attachments 投影；composer 使用 owning binding |
| E-402 | `components/chat/run_settings.rs::RunSettingsController` | production 保存三个 field 与 `PhantomData`，test 另存 root field；closures 还 clone descriptor，并使用 `*_in`/`project_value` | 保存 strong form/native controls；root descriptor 由调用点传入并即时组合；computed path 显式 partial |
| E-403 | `features/settings/prompts/{form_state,dialog}.rs` | 两个 total text field；infallible normalize transform 仍返回 `Result`；submit 先读 revision 再 prepare | 最小端到端 total、static transform 与 prepared snapshot fixture |
| E-404 | `features/settings/{provider.rs,provider/draft.rs,provider/forms.rs,provider/forms/{api_key,ollama,custom_openai,secret}.rs}` | 三个 generated store；secret wrapper 保存 generic marker；页面镜像 validation report；variant submit 再单独读 revision | 三组静态 descriptor、非泛型 wrapper、单一 form report 与 variant prepared snapshot |
| E-405 | `features/settings/mcp/form_state.rs` | parent + 五类 row derive；row field 从 parent `identified_item(..., id_fn).project(...)` 定位；统一 binder 会默认化访问错误 | 消费 generated ID metadata；拆分 total root binding 与 partial row `item/within/try_new` |
| E-406 | `features/settings/mcp/{dialog,form_rows,validation}.rs` | root query、add/remove/rebind 与手写 trigger/scope、Garde index -> stable-ID 映射分散 | component 原子 `try_bind`；core/macro负责路径映射与 bucket filtering；UI/业务规则不变 |
| E-407 | `features/settings/shortcuts/{form_state,dialog}.rs` | hotkey 手写 defer、prompt computed projection、nested RunSettings 与 revision/prepare 双读 | hotkey owning binding；prompt partial；RunSettings child total；统一 prepared snapshot |
| E-408 | `features/settings/form_validation.rs` 与六个 `SubmitTransform` impl | typed context/message 已存在；所有 transform 返回 `Result<_, TransformReport>` | 保留可见 validation policy，transform 改 static/pure/infallible |
| E-409 | F-400..F-407 各文件中的 inline `#[cfg(test)]` | 覆盖 submit、MCP stable rows、RunSettings、binding lifecycle；没有独立 `app/jaco/tests` | production 与 inline fixture 一起迁移为 v2 app-level regression suite |
| E-410 | `app/jaco/Cargo.toml` 与各 form owner 的 save/resource task | Jaco 已因资源与保存流程依赖 `gpui-operation`；form 调用仍经 `gpui-form` 领域 API | 保留现有 app operation；不让 Jaco 导入或驱动 core-private form transition protocol |

## 架构决定

### D-400：生成 state 的命名一次性切换

| Model | 当前生成的 state | 目标生成的 state |
| --- | --- | --- |
| `ChatInputInput` | `ChatInputFormStore` | `ChatInputForm` |
| `RunSettingsInput` | `RunSettingsFormStore` | `RunSettingsForm` |
| `PromptEditFormInput` | `PromptEditFormStore` | `PromptEditForm` |
| `ShortcutEditFormInput` | `ShortcutEditFormStore` | `ShortcutEditForm` |
| `McpServerFormInput` | `McpServerFormStore` | `McpServerForm` |
| `McpArgRowInput` | `McpArgRowFormStore` | `McpArgRowForm` |
| `McpEnvVarRowInput` | `McpEnvVarRowFormStore` | `McpEnvVarRowForm` |
| `McpEnvRowInput` | `McpEnvRowFormStore` | `McpEnvRowForm` |
| `McpHeaderRowInput` | `McpHeaderRowFormStore` | `McpHeaderRowForm` |
| `McpEnvHeaderRowInput` | `McpEnvHeaderRowFormStore` | `McpEnvHeaderRowForm` |
| `ApiKeyProviderFormInput` | `ApiKeyProviderFormStore` | `ApiKeyProviderForm` |
| `OllamaProviderFormInput` | `OllamaProviderFormStore` | `OllamaProviderForm` |
| `CustomOpenAiProviderFormInput` | `CustomOpenAiProviderFormStore` | `CustomOpenAiProviderForm` |

全部改用 `#[derive(gpui_form::FormModel)]`、`#[form(state = ...)]`。不保留旧 alias；Jaco 类型名
本身也作为 residual gate。

### D-401：页面和 controller 明确持有 strong form

已有 dialog/controller 的 `form: Entity<...>` 字段继续作为编辑会话 owner。同步 helper 接收
`&Entity<Form>`；不传裸 `&Form`，不把 entity 塞回 descriptor。需要跨 native event 延迟写入时，
通过 adapter `ControlBinding` 的 `defer_*` intent；binding 内部可以 weak-own form，但 Jaco 的
field descriptor 和 native control state 不保存 entity。`AttachmentControlState::form` 删除，
`ChatForm` 只接收 controller 在 render turn 从 root form 读出的 attachments 投影。composer 与
Shortcut hotkey 在构造时绑定；普通 controller 方法已经处于安全 update 边界时才显式
`FIELD.set(&form, ...)`。

### D-402：不缓存静态 descriptor

调用点直接使用 `PromptEditForm::NAME`、`ChatInputForm::ATTACHMENTS` 等 associated const。
`RunSettingsController<Form>` 不再保存 production 的三个 child field、test-only root field、
`PhantomData` 或 closure 外的 descriptor。它持久保存 `Entity<Form>`、orchestration subscriptions
和 native control handles；构造/订阅/rebuild 调用点显式传 total root descriptor，并即时组合
`RunSettingsForm::*`。不新增 Jaco-local `RunSettingsRootForm` trait、field registry 或 facade。
需要 descriptor 的 callback只捕获该次组合所需的 root/located value；`sync_token_budget_control`
等方法显式接收 form/root，不把 descriptor重新升级为 controller state。

### D-403：total 与 partial 在应用边界保持可见

- declared root field 与 total group child（包括
  `RunSettingsForm::MODEL.within(ShortcutEditForm::RUN_SETTINGS)`）直接调用
  `value/set/errors/validate/subscribe_in/bind_control`；partial subscription 使用
  `try_subscribe_in`。
- `McpServerForm::HEADERS.item(id)` 及 child `within` 是 partial，使用 `try_value/try_set/try_new`。
- `project_value`（Shortcut prompt selection、RunSettings token budget、Provider API mode control）
  是 partial，即使 projection 逻辑对当前值通常有定义，也不伪装成 total。对应 adapter 使用
  `try_new`/`try_bind_control`。
- Jaco 不给 partial 写 `.unwrap_or_default()` 来吞掉 missing/duplicate；同一 GPUI turn 根据已验证
  snapshot 建立 row control，或在刚创建 root form 后挂载 setter 恒可达的静态 projection 时，
  才可用说明 invariant 的 `expect`；render 和其他 ordinary runtime 路径必须传播或显式处理
  `ERR-01`/`ERR-02`。

### D-404：MCP 稳定行只由 macro metadata 定位

删除应用传入的 `identified_item(row.row_id, |row| &row.row_id)`。目标形式为：

```rust,ignore
let row = McpServerForm::HEADERS.item(row_id);
let name = McpHeaderRowForm::NAME.within(row);
let input = FormInput::try_new(&form, name, build, window, cx)?;
```

新增/删除/重排继续先读取 whole-array value、构造新 `Vec<Row>`，再用 total array `set`。应用生成的
`FormItemId` 唯一性保持现状；identified-item 写不得改变 `row_id`。`McpServerFormComponents::bind`
改成原子 `try_bind`：先从 total array snapshot 完整建立所有 partial controls，全部成功后才替换
container；失败保留旧 container 并把 `ERR-01` 交回 dialog，不能半替换或默认成空列表。
`McpServerFormDraft::input` 改为一次 root model snapshot；transport/OAuth 与数组增删仍通过 total
descriptor 写入。

### D-405：Jaco 自定义控件遵守 adapter 布局

`ProviderSecretInputState<Form>` 改为非泛型 `ProviderSecretInput`，其持久字段严格为：

```rust,ignore
struct ProviderSecretInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}
```

`new<Form>(form: &Entity<Form>, field: FormField<Form, ProviderSecretValue>, ...)` 在 mount 时创建
`ControlBinding`；closures 捕获 binding/descriptor，wrapper 不保存 form、field、binding 或 marker。
删除 generic `FormEvent` bound、`PhantomData` 与手写 `Drop`；依靠字段声明顺序先释放 subscriptions。
`FormModelPicker`、`FormReasoningPicker`、`FormApprovalPicker` 与 adapter owning handles继续保持
subscriptions-first/native-entity-second。

### D-406：验证策略不因 API 迁移变化

Jaco `JacoValidationContext`、`JacoGardeMessageProvider`、Fluent key/params、scope 与 trigger 的可见
业务语义保持不变。derive 指向同一 static adapter policy；non-generic `FormEvent` 只改变
subscription bound 和 path filtering。MCP 的 Garde adapter只产出标准 display/index path；
macro/core 根据 array metadata映射 stable ID，并负责 trigger/scope bucket filtering。删除 Jaco
validation module 中的手写 trigger/scope 过滤、`garde_path` 与 stable-ID -> index 反查；重排或删除
row 后，sibling/item issue仍必须归属正确 stable path。

### D-407：提交只消费一个 prepared snapshot

Prompt、MCP、Shortcut 与 generated Provider variant 统一为：

```rust,ignore
let PreparedSubmit { revision, output } = form.update(cx, |form, cx| {
    form.prepare_submit(cx)
})?;
```

所有 `SubmitTransform` 改为 associated static `fn transform(&Model) -> Output`；当前 normalize/secret
转换本来不可失败，因此删除 `TransformReport` 和 `Ok(...)`。catalog/provider/database 等失败继续由
Jaco submit/persistence operation 返回，不移入 form transform。保存成功仍以 captured revision 调
`rebase_if_revision`。`ProviderSettingsForm` 的 enum boundary 返回私有
`ProviderPreparedSubmit { revision, output }`，删除独立 `revision()` helper。ChatInput 不持久化
rebase，但 composer snapshot 写入与 `prepare_submit` 必须位于同一次 root `form.update`；`can_send`
也只读取一次 root model，再与 catalog gate 做纯判断，不能从 component 与多个 field 拼第二份模型。

### D-408：form `Transition` 是 core-private 实现细节

`gpui-form` core 可以在一次 `Entity<Form>::update` 内用私有 message/effect 和
`gpui_operation::Transition` 组织 mutation、validation scheduling、report replacement、revision、
event 与 notify 顺序；Jaco 只消费 `set`、`validate`、`prepare_submit`、`rebase_if_revision`、
`ControlBinding::defer_*` 等领域入口。Jaco 不导入、实现或构造 form 内部 message/effect，不把
validation 消息转发给 app operation，也不为此新增 reducer/phase enum。现有 `gpui-operation`
继续服务 catalog、save/resource 等 app-owned runtime，两者不能因共用 `Transition` trait 而合并。

### D-409：form report 是 validation 的唯一事实源

Prompt、Provider、MCP、Shortcut 的字段错误、summary 与 focus path 都从当前 form report派生。
Provider 删除 `ProviderValidationState` 对 Valid/Invalid report 的镜像；仅 provider 未注册、请求失败
等 form 外资源问题保留为 page-owned state。form event只触发投影更新，不复制 report。

## 文件与所有权

| F-ID | 文件/产物 | 计划动作 | 所有权边界 |
| --- | --- | --- | --- |
| F-400 | `components/chat/input/form_state.rs` | `ChatInputInput` derive/state rename，static root/group descriptors | ChatInput model declaration only |
| F-401 | `components/chat/{input.rs,input/attachment_flow.rs,form.rs,form/controls.rs}` | explicit reads/writes、composer binding、attachments投影、submit/readiness snapshot | `ChatInputController` owns strong form and page state；visual shell不持有form |
| F-402 | `components/chat/run_settings.rs` | `RunSettingsInput` derive/state rename；strong form orchestration、nested constants、partial token projection | catalog/native picker remains controller-owned |
| F-403 | `features/settings/prompts/{form_state,dialog}.rs` | total fields、static transform、prepared submit | Prompt dialog owns save task |
| F-404 | `features/settings/{provider.rs,provider/draft.rs,provider/forms.rs,provider/forms/{api_key,ollama,custom_openai,secret}.rs}` | three states、text/select/secret controls、删除report镜像、variant prepared submit | Provider editor owns variant, catalog, external validation, secret/save policy |
| F-405 | `features/settings/mcp/{form_state,dialog,form_rows,validation}.rs` | stable-ID arrays、atomic partial row controls、root snapshot、validation path/report queries | MCP dialog owns component container and persistence |
| F-406 | `features/settings/shortcuts/{form_state,dialog}.rs` | hotkey binding、prompt projection、nested RunSettings、prepared submit | Shortcut dialog owns choices/save task |
| F-407 | `features/settings/form_validation.rs` | adapt type-level Garde provider contract without changing visible messages | Jaco owns Fluent rendering and validation dependencies |
| F-408 | F-400..F-407 内的 inline `#[cfg(test)]` 模块 | 将相邻生产代码的 fixture/assertion 迁移到 v2 API | 不新增 compatibility test harness 或 `app/jaco/tests` 目录 |

直接依赖 form v1 契约的测试入口至少包括 `components/chat/{input,run_settings}.rs`、
`features/settings/prompts/dialog.rs`、`features/settings/{provider,provider/forms}.rs`、
`features/settings/mcp/{form_state,dialog,validation}.rs` 与
`features/settings/shortcuts/dialog.rs`；`features/settings/form_validation.rs` 继续承担 Jaco
验证文案策略测试。实施时以 WP-400 的实时搜索为准，不能把此清单当作最终零残留证明。

本计划不新增生产模块、清单依赖、本地化文件、资源、数据库迁移或生成产物。

## 数据流

### L-403：total 字段与原生控件

1. Dialog 创建 `Entity<PromptEditForm>`。
2. `FormInput::new(&form, PromptEditForm::NAME, build, window, cx)` 同步读取初值并挂载。
3. native change 只向 owning binding 提交 `defer_*` intent；同步 controller helper 才显式
   `NAME.set(&form, value, cx)`。
4. form `ValueChanged` 触发 silent projection；dialog 对 form 的单一 observe 负责 rerender error/button。
5. dialog drop 先 drop subscriptions/controls，再 drop form；queued weak work静默取消。

### L-404：MCP 稳定 ID 行

1. 从 `McpServerForm::HEADERS.value(&form, cx)` 取得当前 row snapshot。
2. 对每个 stable ID 建立 `HEADERS.item(id)` 和 child `within` partial descriptor。
3. `try_bind` 在临时 container 内用 `try_new` 验证每条 path；全部成功后一次替换旧 container。
4. bind失败保留旧 container并返回 `ERR-01`；不得保留半套新control或用空数组继续。
5. 删除 row使用 whole-array total `set`；旧 binding后续 deferred intent发现 path missing并静默失效。
6. Garde index path由 core/macro映射成 stable path；重排后其他 row 的 validation bucket 与 control
   保持不变，container/ancestor summary按 scope重算。

### L-405：RunSettings 编排

1. ChatInput 或 Shortcut owner把 strong root form和 total nested RunSettings descriptor交给 controller。
2. controller 组合 `RunSettingsForm::{MODEL,REASONING_SELECTION,APPROVAL_MODE}.within(root)`，不保存
   组合结果。
3. 每次构造、订阅和 control rebuild由调用点显式传入 form/root；controller只保存 native handles与
   subscriptions。
4. catalog update只更新 native picker/capability；不 rebase、不自动选择 fallback，用户确认通过
   binding/explicit form写 typed value。
5. token budget projection为 partial；非法 editor text只产生 control issue，不写 typed form。
6. submit从同一个 root prepared output解析 catalog snapshot，保持现有 readiness gate。

### L-406：保存与异步结果

1. 页面在一次 `form.update` 中更新 submit validation context并调用 `prepare_submit`，得到同一
   snapshot 的 revision/output；Provider enum boundary在该 update 后只做 variant mapping。
2. ChatInput在一次 root update内写 composer snapshot并立即 prepare；readiness只读一次 root model。
3. 页面启动现有 save task/operation；form 不保存 busy/problem/retry，也不接收 app operation消息。
4. 失败只更新页面 operation/UI；typed draft和baseline不变。
5. 成功以 captured revision调用 `rebase_if_revision`；若用户已继续编辑，失败 CAS无副作用。

## 生命周期

### L-400：对话框编辑会话

创建顺序固定为 form -> native controls/bindings -> page subscriptions。drop 时 owning wrapper 的
subscription 字段先释放，再释放 native entity，最后由 page释放 form。任何 callback不得依赖 Rust
字段 drop 顺序之外的隐藏 descriptor ownership。

### L-401：动态组件重建

MCP transport/row变化仍可重建 `McpServerFormComponents`。重建先构造新 container并验证 partial
paths，再替换旧 container；旧 subscriptions drop后失效。不能在失败时保留一半新、一半旧 control。

### L-402：资源与选项刷新

Provider model/prompt choices refresh不 rebase form、不自动选择第一项。native picker使用现有 delegate
更新 API；selected typed value继续由 form拥有。不可用选择在 validation/submit policy 中报告。

## 状态与错误契约

### ST-400：不新增应用状态机

本迁移不新增 Jaco operation、reducer、phase enum或 form message adapter。core-private transition
只能在 form entity的一次 update内运行；Jaco不得导入 message/effect、实现 form transition，或把
validation message送进 application operation。`save_task`、resource Operation、ChatInput 的
skill catalog/submission task、Provider fetch/save task、MCP OAuth/sign-out/save task以及 read-only
capability gate保持当前 owner与状态转换；form只暴露领域 API、model/revision/validation lifecycle。

### L-407：partial binding 错误处理

- `ERR-01` 在 ordinary runtime 路径必须返回到 component rebuild/dialog逻辑，不能变为空值。
- 同一 turn 从已验证、唯一 ID snapshot建立 control可使用带具体 invariant 文案的 `expect`；对应 test
  必须构造 missing/duplicate path证明 core error，而不是让 Jaco panic路径承担验证。
- `ERR-02` 拒绝稳定 ID 变化后，Jaco不得重试为 whole-array write；只有显式 row replace helper可选择
  whole-array operation。

### L-408：控件与提交错误处理

- adapter integer policy错误沿 `ERR-03` 返回；Jaco的静态合法 policy可在构造处 `expect`，并由 unit
  test固定 bounds。
- `ERR-04` 继续驱动现有 inline error/summary/focus逻辑；不转成 persistence error。
- form drop不再是同步错误；deferred drop静默取消，不显示“form released”。

## 风险

| ID | 风险 | 防护 |
| --- | --- | --- |
| R-400 | 大量机械 rename掩盖 total/partial错误分流 | 按功能纵切迁移，并加 residual + compile contract |
| R-401 | RunSettings改为显式 owner后形成 entity cycle | controller只 strong-own根 form；根页面持有 controller时核对方向，callback捕获 weak controller，drop test证明可释放 |
| R-402 | MCP row在 snapshot后被删除 | 同一 GPUI turn mount；后续 intent由 partial binding重新解析并取消 |
| R-403 | `ValidationChanged`导致不必要 value projection或反之 | non-generic event path tests + Jaco component projection count |
| R-404 | submit签名迁移时重新引入 revision/model双读 | 全量查找 `revision()` + `prepare_submit`，只允许 `PreparedSubmit` destructure |
| R-405 | custom secret control继续用 generic marker隐藏旧设计 | layout assertion/review + residual `PhantomData<Form>` |
| R-406 | rename遗漏只在平台/feature test编译 | `cargo test -p jaco --locked` + all-target/all-feature clippy |
| R-407 | 因 Jaco 已依赖 `gpui-operation` 而把 form 内部消息暴露到 app | form consumer定向 import/API audit；只允许领域入口与binding |
| R-408 | 删除Jaco validation映射时改变 stable row归属或保留report镜像 | 重排/删除path tests + Provider单一report source测试 |

## 测试契约

| ID | 层级 | 场景 | 验收 |
| --- | --- | --- | --- |
| T-400 | Prompt unit/GPUI | total input change/blur、prepared submit、save后CAS rebase | 无 access Result；revision/output同一snapshot；错误语义不变 |
| T-401 | Provider GPUI | secret输入/drop、URL/name、api mode partial、variant submit/rebuild | wrapper非泛型且释放安全；无report镜像；variant snapshot原子 |
| T-402 | MCP unit | add/remove/reorder、missing/duplicate ID、item leaf write、Garde index mapping | partial错误精确；stable ID不可变；重排后issue仍归正确row |
| T-403 | MCP GPUI | transport切换、`try_bind`失败与components原子重绑 | 失败保留旧container；无半挂载control、无re-entry panic |
| T-404 | Shortcut GPUI | hotkey queued/drop、prompt computed projection、nested settings | queued work静默取消；prompt partial与RunSettings total child分流正确 |
| T-405 | RunSettings unit/GPUI | model refresh/unavailable selection、reasoning、token budget control issue | production/tests均不缓存descriptor/marker；invalid text阻断submit |
| T-406 | ChatInput unit | composer/attachments、`can_send`、send prepared snapshot | composer write/prepare同一revision；readiness只读root model；catalog gate不变 |
| T-407 | lifecycle | page/form/control drop后执行queued callback | weak boundary静默取消，无cycle/notify |
| T-408 | residual | production/tests/generated API names与form integration imports | active Jaco form源码无v1 API/`*FormStore`，不消费core-private transition协议 |

## 工作包

### WP-400：冻结 inventory 与 rename matrix

- 刷新 `rg` inventory，覆盖本文 E-400..E-410 的 production/test调用面与全部13个 derive model。
- 固化 D-400 名称，检查与现有 enum/module冲突；确认 total/partial、owner、submit、validation 与
  core-private transition边界均已分类。
- 确认 C-02/C-04 已达到 `producer-ready`，C-03 generated signatures可直接消费。
- 验收：没有未归类 form consumer；本工作包不修改源码。

### WP-401：迁移 model derive、validation 与 submit glue

- 修改所有 form model derive/helper attribute和imports。
- 迁移 Jaco `SubmitTransform` 到 static/infallible；删除 `TransformReport`。
- 用 `PreparedSubmit` 替换 revision/output双读；Provider enum boundary返回单一 variant snapshot。
- 让 macro/core承担 MCP Garde path -> stable path 与 trigger/scope bucket处理，删除Jaco重复机制。
- 验收：generated states全部使用目标名；T-400基础fixture可编译。
- 依赖：WP-400、C-01、S-08、S-09。

### WP-402：迁移 total settings forms

- Prompt与Provider root字段改 associated const + explicit entity。
- `ProviderSecretInput` 按D-405重写；Provider text使用total `new`，API mode projection使用
  partial `try_new`。
- validation render只查询explicit form，删除 `ProviderValidationState` 的Valid/Invalid report镜像；
  form外资源错误仍由页面持有。
- 验收：T-400、T-401；无 `FormReleased` handling。
- 依赖：WP-401、C-02、C-04。

### WP-403：迁移 MCP identified arrays

- root与row derives、array ID metadata、`item/within`、add/remove/set、validation query全量迁移。
- `McpServerFormComponents::try_bind` 以新container原子替换；失败保留旧container，partial path
  error遵守L-407。
- draft/submit读取一次root model；删除Jaco手写stable-ID/index与trigger/scope转换。
- 更新form_state/dialog/form_rows inline tests。
- 验收：T-402、T-403；不再传row ID closure。
- 依赖：WP-401、S-03、S-04、ERR-01、ERR-02。

### WP-404：迁移 RunSettings、Shortcut 与 ChatInput

- RunSettings controller strong-own root form，调用点传root descriptor；删除production/test descriptor
  fields与marker，使用static nested total composition。
- token budget、prompt selection改partial；Shortcut model/reasoning/approval保持total；hotkey、composer
  等native callback使用owning binding。
- ChatInput删除visual attachment state中的form；attachments/render显式投影，composer write/submit在
  同一update，`can_send`读取单一root model。
- 验收：T-404..T-406；不改变catalog/resource gate。
- 依赖：WP-401、WP-402、C-03、C-04。

### WP-405：事件、生命周期与残留清理

- 所有generic `FormEvent` bound改非泛型，检查observe/subscribe的value/validation分流。
- 删除Jaco-local旧helper、descriptor cache、`PhantomData<Form>`与无意义Result handling。
- 定向确认 form consumer不导入/构造core-private message/effect，不为form实现
  `gpui_operation::Transition`；Jaco其他operation transition不属于残留。
- 执行T-407、T-408 residual gate；历史归档目录排除。
- 依赖：WP-402..WP-404、S-05、S-07、S-10、S-11。

### WP-406：定向验证与UI验收

- 执行下方最小充分Cargo命令一次。
- 运行Prompt、Provider、MCP、Shortcut/ChatInput场景；记录自动化与人工边界。
- 将真实结果回写root Completion Evidence；未执行UI不能写“验证完成”。
- 依赖：WP-405，以及 residual-certification wave 的 core `WP-104`、macro `WP-204`、adapter
  `WP-303`；只针对最终无 v1 surface 的 API 运行一次。

## 验证

实现完成后的Jaco owner门禁：

```text
cargo fmt --all
cargo test -p jaco --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
git diff --check
```

如果一个失败来自三个 form crate 的 contract，回到对应 owner work package修复，不在Jaco新增shim。
UI 验收按 root plan 的四类场景记录；本开发文档编写阶段不运行Cargo或UI验证。

Residual scan 至少覆盖：

```text
derive(gpui_form::FormStore)
form(store
FormStore
_field(
_in(
identified_item
set_user_value
FormEvent<
TransformReport
PhantomData<Form>
ProviderValidationState
garde_path
```

命中必须逐项分类；普通GPUI `subscribe_in`/`update_in` 不属于 generated `*_in` API，不能用过宽
替换误删。另做结构审阅：`AttachmentControlState` 不得保存 form；RunSettings production/tests不得
保存 descriptor/marker；form consumer不得引用 core-private message/effect。Jaco 其他资源模块合法的
`gpui_operation::Transition` 不能用全 app 零命中规则误判。

## 完成证据

完成时登记：

- 实施commit/PR和D-400最终rename matrix；
- WP-401..406状态；
- T-400..408实际结果；
- Cargo命令、residual query与命中分类；
- form consumer未接入core-private transition协议、validation report无页面镜像的审阅结果；
- UI场景、截图/日志或明确未执行原因；
- DB/config/secret/i18n/assets No change核对；
- 未解决问题及其独立issue，而不是Jaco-local workaround。

在C-01..C-04均满足且active Jaco源码无v1残留前，本文不得标记`Done`。

## 实施交接检查

- [x] core、macro、adapter owner plans的目标签名与当前实现分支一致。
- [x] live inventory覆盖所有Jaco form production/test模块，不复用Issue #175旧清单代替搜索。
- [x] D-400 generated names没有和现有application type冲突。
- [x] total/partial调用已逐项分类，未用`unwrap_or_default`隐藏partial错误。
- [x] RunSettings strong ownership没有形成entity cycle，queued callback使用正确weak边界。
- [x] MCP stable path在重排/删除后仍正确，components rebind保持原子。
- [x] Provider无validation report镜像，secret wrapper和Chat attachment shell不保存隐式owner。
- [x] submit只解构`PreparedSubmit`或Provider variant wrapper，transform无业务/IO失败。
- [x] Jaco form consumer只调用领域API/binding，不导入或驱动core-private transition消息。
- [x] persistence operation、resource gate、数据库和UI范围保持No change。
- [x] 完成状态区分代码、自动化验证、实际 UI smoke 与 Computer Use 未覆盖交互。
