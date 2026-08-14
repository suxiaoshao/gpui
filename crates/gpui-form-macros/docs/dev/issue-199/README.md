# Issue #199：gpui-form-macros 子任务跟踪

## 根计划与所有权

- 状态：历史 `WP-200`–`WP-205` 保持 `Done`；本轮 macro 更新计划为 `Done`
- 跟踪 Issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 计划 ID：`issue-199`
- 根计划：[../../../../../docs/dev/issue-199/README.md](../../../../../docs/dev/issue-199/README.md)
- 所有者目录：`crates/gpui-form-macros`
- 所有者计划：`crates/gpui-form-macros/docs/dev/issue-199/README.md`
- 所有者索引：[../README.md](../README.md)
- 所引用的根所有者 ID：`S-01`–`S-12`、`C-01`–`C-04`、`ERR-01`–`ERR-04`，以及根计划的跨所有者排期/汇总验证 ID。该所有者只直接实现 `C-01`；`C-02`–`C-04` 是下游发布门槛。
- 所有者编写的本地 ID/范围：`E-200`–`E-206`、`D-200`–`D-206`、`F-200`–`F-214`、`L-200`–`L-208`、`ST-200`–`ST-202`、`R-200`–`R-217`、`T-200`–`T-212`、`WP-200`–`WP-205`。
- 分配的 WP：`WP-200`–`WP-205`。
- 负责：proc-macro 语法、一份规范化 derive 模型、生成的状态/类型/描述符/schema 代码、宏诊断与 trybuild fixture，以及宏文档同步。
- 不负责：`gpui-form` 的描述符/运行时实现和错误语义（`C-01`、`ERR-01`、`ERR-02`、`ERR-04`）、`ControlBinding`/组件 adapter（`C-02`、`ERR-03`）、Jaco 迁移（`C-03`、`C-04`）或根计划/状态/索引。

该计划集中的所有者索引有意由根计划负责。本文件在规划期间不得更新它、根计划或历史性的 `docs/dev/issue-175/README.md`。

## 本轮新计划

- [FormSchema 生成更新实施计划](form-schema-generation-update-plan.md)：消费 core `C-900`–`C-904`，
  使用 `E/D/F/L/ST/ERR/R/T-1000..1099` 与 `WP-1000..1009`；状态 `Done`。
- 本轮允许 breaking 且不保留兼容层；只规划宏单元测试、trybuild 与 Cargo 门禁，不安排实际 UI 操作测试。
- 下文 `WP-200`–`WP-205` 及其证据是上一轮原始实施记录，不作为本轮执行计划。

## 本轮实施结果（2026-08-09）

- `WP-1000`–`WP-1003` 已完成；最终 grammar、descriptor/traversal、External metadata 与稳定诊断已落地。
- macro trybuild、core typed compile fixture、Clippy、workspace check 与残留扫描通过；实际 UI 操作测试不适用。
- 完整证据见[本轮实施计划](form-schema-generation-update-plan.md#实施证据)。

## 实施结果（2026-08-02）

- 唯一 derive 已切换为 `FormModel`，`state =` 为唯一状态命名配置；生成 state、静态关联常量、
  stable-ID/schema/validation/submit glue，未保留 `FormStore`、field enum 或 entity accessor alias。
- macro manifest/source 不依赖 `gpui-operation`，生成代码不暴露或实现 core-private transition 协议。
- `cargo test -p gpui-form-macros --locked` 的 8 个单元测试与 18 个 trybuild fixture 全部通过。

## 所有者本地证据

| E-ID | 分类 | 主张 | 证据 | 对计划的影响 |
| --- | --- | --- | --- | --- |
| `E-200` | 当前事实 | 唯一导出的 derive 是 `FormStore`；它调用 `derive_form_store`。 | `src/lib.rs` | `WP-200` 替换公开 derive 入口；不保留别名。 |
| `E-201` | 当前事实 | `attributes.rs` 解析 `store`，而 `expand.rs` 生成 `ModelFormStore`、`ModelField`、捕获 entity 的 accessor 和 `EventEmitter<FormEvent<ModelField>>`。 | `src/derive/attributes.rs`、`src/derive/expand.rs` | `WP-201`–`WP-203` 移除这三类过时的生成接口。 |
| `E-202` | 当前事实 | 当前 `FormField::new` 接收 `form.downgrade()`，且字段调用返回 `Result<_, FormFieldError>`，因为描述符保留了 form 存活性。 | `src/derive/expand.rs:field_accessor`、`crates/gpui-form/src/field.rs` | 宏必须消费 `C-01` 中新的显式 form 静态描述符生产者；不得生成 weak handle。 |
| `E-203` | 当前事实 | 现有 parser 单元测试和 `tests/ui.rs` 仅覆盖 v1 语法；所有 compile-fail fixture 都导入 `FormStore`。 | `src/derive/attributes.rs`、`tests/ui.rs`、`tests/ui/fail/*.rs` | 替换而非扩展 fixture，使诊断覆盖 `FormModel` 和 v2 语法。 |
| `E-204` | 当前事实 | `README.md` 和 `docs/guide.md` 已描述 Issue #199 预览：`FormModel`、`state =`、关联常量、完全/部分描述符以及非泛型事件。 | `README.md`、`docs/guide.md` | 实现后保留 API 示例，但仅移除预览/当前 v1 状态措辞，并保持中文镜像语义一致。 |
| `E-205` | 用户决策 | 所需的公开 API 是破坏性变更：带 `#[form(state = ProviderForm)]` 的 `#[derive(FormModel)]`、静态 `ProviderForm::NAME`、显式 `&Entity<Form>`、完全/部分可用性、稳定 ID 数组，且没有兼容 shim。 | Issue #199 任务说明；根 `S-01`–`S-12`、`C-01`、`ERR-01`–`ERR-04` | 此所有者不创建 `FormStore` 别名、旧字段 enum、旧 accessor 别名或 weak-entity fallback。 |
| `E-206` | 用户决策 | `gpui-form` 在 core 内部使用 `gpui_operation::Transition` 和私有 message/effect，但公开/generated API 继续使用领域方法；宏调用方不发送消息，也不需要导入 `Transition`。 | Core `E-110`/`D-109`；Issue #199 后续设计讨论 | 本 owner 只补隐藏委托和 generated-surface 边界；不修改 Issue、公开 README/guide、根计划或 Jaco 文档。 |

## 所有者本地决策

| D-ID | 决策 | 证据 | 实质性否决的替代方案 | 影响/所有者 |
| --- | --- | --- | --- | --- |
| `D-200` | 将 proc macro 重命名为 `FormModel`；`state = StateIdent` 是唯一的生成状态命名选项，默认将 `Model` 命名为 `ModelForm`。 | `E-200`、`E-205`、根 `S-09`、`S-10` | 保留 `FormStore` 或接受 `store =` 作为别名。 | `WP-200` 中的宏入口/parser/fixture；破坏性迁移由根负责。 |
| `D-201` | 生成一个实现根契约 `C-01::FormState` 的公开状态类型，并为每个静态字段生成一个关联 `const` 描述符；不生成公开的 `ModelField` enum 或 `FormFieldId` API。 | `E-201`、`E-204`、根 `S-01`、`S-02`、`S-04` | 基于 enum 的 schema 查询或 form 绑定 accessor 函数。 | `WP-201` 发出 `ProviderForm::NAME` 风格常量和静态 schema/path/lens 元数据。 |
| `D-202` | 每个生成的描述符都是静态 schema/access lens。读、写、验证、错误和绑定从调用者接收 `&Entity<Form>`；生成的描述符存储不含 `Entity`、`WeakEntity`、值、subscription、control binding 或 runtime。 | `E-202`、根 `S-01`–`S-03`、`S-07`、`C-01`、`C-02` | 捕获/降级 form 以保留旧 `FormReleased` 行为。 | `WP-201` 消费 `C-01` 定义的 core constructor；运行时生命周期错误只留在 adapter 的延迟边界。 |
| `D-203` | 在生成的组合中保持可用性：静态根和完全父级 `within` 是完全的；`item(id)` 和 `project_value` 是部分的；部分父级的后代仍为部分。 | `E-204`、根 `S-03`、`S-04`、`ERR-01`、`ERR-02` | 使所有描述符都可失败，或让 item/projection access 静默选择/fallback。 | `WP-202` 选择完全或部分 core 组合入口，不定义 core 错误语义。 |
| `D-204` | 保持一个根状态和一个 core runtime。derive 仅提供模型 lens、schema 遍历、稳定 ID 元数据、验证/transform 关联策略、非泛型事件实现及 `C-01` 所需的隐藏领域 bridge；core 可在 runtime 内部以 `Transition<PrivateMessage>` 组织状态变化。derive 绝不创建子 entity、生命周期状态、持久化、task、focus 或 control。 | `E-201`、`E-206`、根 `S-05`–`S-08`、`S-11`、`S-12`、`C-01` | 在生成代码中重建字段/写入生命周期或 control 所有权；让 derive 生成消息 enum 或 `Transition` impl。 | `WP-201`–`WP-203` 仅委托给 C-01 并保持跨所有者边界。 |
| `D-205` | 宏承诺静态关联常量构造零分配，且无每 form 字段分配。它**不**承诺所有定位/组合描述符值都实现 `Copy`。 | `E-204`、根 `S-02`–`S-04` | 添加会约束 core 描述符内部实现的全局 `Copy` 主张/derive。 | API 文档和测试断言分配/存活性语义，而非不受支持的通用 trait bound。 |
| `D-206` | Macro manifest、parser semantic model、展开 token 与 public/generated signature 均不依赖/命名 `gpui-operation`、`Transition` 或 core-private message/effect。生成的 `FormState` 只调用 `C-01` 的领域方法和必要的 doc-hidden bridge，由 core 在 bridge 内构造消息。 | `E-206`、core `D-109`、`C-01` | 给 generated state 添加 message associated type/dispatch；re-export core message；让 macro 直接依赖 `gpui-operation`；把私有消息名固化进 token snapshot。 | `L-202`、`L-204`、`ST-201`、`WP-201`、`WP-203`、`WP-204`。 |

## 所有者本地目标设计

### 文件与所有权树

```text
crates/gpui-form-macros/
├── src/lib.rs                                      # F-200 [修改，手写] 仅导出 `FormModel` derive
├── src/derive.rs                                   # F-201 [修改，手写] 路由 parser/model/expansion 模块
├── src/derive/attributes.rs                        # F-202 [修改，手写] 严格的 v2 helper 语法和诊断
├── src/derive/model.rs                             # F-203 [新增，手写] 由所有 expansion 共享的规范化 named-struct 语义模型
├── src/derive/expand.rs                            # F-204 [修改，手写] 仅作编排；删除 v1 单体输出
├── src/derive/expand/state.rs                      # F-205 [新增，手写] `FormState` 声明、关联策略、领域 bridge、constructor、事件实现
├── src/derive/expand/descriptor.rs                 # F-206 [新增，手写] 静态关联常量和完全/部分组合元数据
├── src/derive/expand/schema.rs                     # F-207 [新增，手写] 递归 path/schema/稳定 ID 生成
├── src/derive/expand/validation.rs                 # F-208 [新增，手写] 验证策略、结构遍历、Garde stable-path 映射
├── src/derive/expand/transform.rs                  # F-209 [新增，手写] 静态 submit-transform 关联策略粘合层
├── tests/ui.rs                                     # F-210 [修改，手写] 按 grammar/removal 类分组的 v2 trybuild harness
├── tests/ui/fail/*.rs + *.stderr                   # F-211 [替换/新增/删除，手写 fixture] 精确的 v2 诊断契约
├── tests/expand.rs                                 # F-212 [新增，手写] 不需要 core dev dependency 的单元级 generated-token 断言
├── README.md + README.zh-CN.md                     # F-213 [修改，手写文档] 实现后移除 preview/v1 措辞；保持对应一致
└── docs/guide.md + docs/guide.zh-CN.md             # F-214 [修改，手写文档] 最终版 grammar/descriptor 指南和对应一致
```

`F-203` 防止 parser 和 expansion 独立决定命名、字段、泛型、group/array bounds 或 grammar 有效性。`F-205`–`F-209` 是现有 `derive/` 模块下的同级文件；从 `src/derive/expand.rs` 添加其模块声明，且不得引入 `mod.rs`。

本 crate 的 `Cargo.toml` 不增加 `gpui-operation` 依赖。`Transition` 是 `gpui-form` core 的实现细节；proc-macro
expansion 只引用 `gpui-form` 的最终 `C-01` 路径，不能让 consumer crate 因 derive 而直接解析该内部协议。

`F-213`–`F-214` 是运行时契约完成后的文档编辑，而不是第二个事实来源。不得编辑 `docs/dev/issue-175/README.md`：它是历史性的 v1 计划，必须作为保留记录。

### 所有者本地契约

#### L-200：proc-macro 入口和严格的类型 helper

`F-200` 仅导出如下新 derive：

```rust
#[proc_macro_derive(FormModel, attributes(form))]
pub fn derive_form_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream;
```

- 它解析一个 `syn::DeriveInput`，调用 `DeriveModel::parse`，然后执行 expansion；所有 `syn::Error` 均成为 compile error。
- 它既不导出 `FormStore`，也不导出弃用别名。因此 `#[derive(FormStore)]` 调用会得到 Rust 的 missing-derive 诊断；不会被静默重写。
- 调用者必须是有命名字段的模型。enum、union、unit/tuple struct、第二个 `#[form]` helper 或无效 helper grammar 都在 expansion 前失败。

#### L-201：规范化模型和 grammar

`F-202` 与 `F-203` 定义并消费同一个语义模型；其名称可以保持私有，但字段与含义固定如下：

```rust
struct DeriveModel<'a> {
    input: &'a syn::DeriveInput,
    state_ident: syn::Ident,
    form: FormAttributes,
    fields: Vec<DeriveField<'a>>,
}

struct FormAttributes {
    state: Option<syn::Ident>,
    validation: Option<ValidationSpec>,
    transform: Option<TransformSpec>,
}

enum FieldShape {
    Leaf,
    Group,
    IdentifiedArray { item: syn::Type, id_field: syn::Ident },
}
```

接受的 grammar 严格如下：

```text
state = StateIdent
validation(adapter = "garde"[, messages = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)

required
validate(on_mount, on_change, on_blur, on_dynamic, on_submit)
group
array(id = "stable_id_field")
```

- 一个模型及其每个字段至多允许一个 `#[form(...)]`；每个 option 和 trigger 至多出现一次，必须使用逗号分隔，空 clause 失败。
- `state` 是不带引号的 identifier；adapter/context/message/transform 类型是不带引号的 `TypePath`。只有 `"garde"` 和 `"validify"` 是带引号的内置项。
- `group` 和 `array` 互斥。`array` 仅接受 `Vec<NominalItem>`；在 item-type span 拒绝裸模型类型参数和限定/关联 item 类型。生成的 `item.stable_id_field` access 留给 Rust 在 ID field span 进行类型检查。
- `required` 独立于可选的前期 validation trigger，表示 submit-required。
- 已移除的 type option `store` 必须诊断为 `` `store` was removed; use `state = StateIdent` ``。已移除的 derive-era field option（`component`、`binding`、`codec`、`focus`、`touched`、`blurred`、`show_error`、`group(store = ...)`、`array(... store = ...)`）诊断其实际的 adapter/page 所有者，且不提供别名。
- Garde 拒绝 `context`；custom/no adapter 拒绝 `messages`；已移除的 `i18n` 诊断为 `messages = ProviderType`。parser 绝不采用 last-write-wins 行为。

#### L-202：生成的命名状态和静态描述符

对于带 `#[form(state = ProviderForm)]` 的命名字段模型 `ProviderInput`，`F-205` 和 `F-206` 生成以下公开接口；其参数化遵循模型的可见性、lifetime、type/const 泛型、声明上的合法默认值以及 `where` clause；生成的 impl 在 Rust 要求处省略类型默认值：

```rust
pub struct ProviderForm {
    runtime: gpui_form::__private::FormRuntime<ProviderInput, ValidationContext>,
}

impl gpui_form::FormState for ProviderForm {
    type Model = ProviderInput;
    type ValidationContext = ValidationContext;
    type ValidationAdapter = ValidationAdapter;
    type SubmitTransform = SubmitTransform;
    // C-01 的领域方法通过 doc-hidden runtime bridge 委托；
    // generated surface 不出现 Transition 或内部消息。
}

impl ProviderForm {
    pub const NAME: gpui_form::FormField<Self, String> = /* static schema/path/lens */;
    pub const RETRY_LIMIT: gpui_form::FormField<Self, u32> = /* static schema/path/lens */;
}

impl gpui::EventEmitter<gpui_form::FormEvent> for ProviderForm {}
```

- 该结构体恰有一个运行时字段。验证适配器和转换器是关联类型，而非存储的实例；验证上下文按 `C-01` 位于该运行时内。
- 每个直接模型字段恰获得一个 `SCREAMING_SNAKE_CASE` 的 `pub const`。该常量仅包含 `C-01` 提供的静态 schema/path/read/write 函数元数据；对其求值或访问不会分配内存、注册订阅，也不包含 entity/value/control 状态。
- 生成的状态通过 `C-01` 提供的精确 `FormState` 构造函数创建一个编辑会话 entity；不会为 group 或 array field 生成子状态/entity。
- `FormRuntime` 的 message/effect 与 `Transition` 实现完全由 `gpui-form` core 私有拥有。生成的 `FormState`
  只暴露并委托 `C-01` 领域方法及必要的 doc-hidden runtime accessor/bridge；展开后的公开类型、trait impl、
  associated item 和调用示例均不得出现 `gpui_operation`、`Transition`、message/effect 类型或 dispatch 方法。
- 此宏不得生成 `ProviderInputField`、`FormFieldId`、`ALL`、`*_field`、`*_in`、`*_item` 或 `*_item_in` API。静态 schema 从 descriptor（`ProviderForm::NAME.schema()`）读取，而非 enum lookup。
- 生成的状态 emit/使用非泛型 `FormEvent`；字段身份通过根 `C-01` 的 path/revision 事件数据传递给使用方，而非通过生成的 enum type parameter。

#### L-203：描述符组合和稳定身份

`F-206` 使用 `C-01` 提供的精确 core constructor/combinator；不得重新实现数据 transaction 或错误类型。

```rust
let name: FormField<ProviderForm, String> = ProviderForm::NAME;
let username = AuthForm::USERNAME.within(ServerForm::AUTH);
let item: PartialFormField<ServerForm, HeaderRowInput> =
    ServerForm::HEADERS.item(FormItemId::new(row_id));
let header_name: PartialFormField<ServerForm, String> = HeaderRowForm::NAME.within(item);
let computed: PartialFormField<ServerForm, u64> =
    ServerForm::SETTINGS.project_value(/* static projection metadata */);
```

- 根 const 以及完全父级上的 `within` 产生完全 `FormField`；其 read/write/validation/error/bind 调用使用不会失败的 C-01 完全 API，并显式接收 `&Entity<Form>`。
- `item(id)` 和 `project_value` 产生 `PartialFormField`；其 `try_value`、`try_set`、`try_errors`、`try_validate` 和 `try_bind_control` 使用由 root/core 定义的 `ERR-01`/`ERR-02`。`within(partial)` 仍为部分。
- 生成的 lens 只接收 `&Model` 和 `&mut Model` 候选值。它们既看不到生成的 state/runtime/context，也不会 emit、notify、revise、validate 或 persist。
- 对于 `Vec<Item>`，将声明的 ID getter 提供给 C-01 的 item metadata/locator。ID 不能通过 identified descriptor 修改；缺失/重复 item 和尝试修改 ID 保留 C-01 的 typed error，且没有 macro 生成的 fallback、首项选择、repair 或 retired-ID history。
- 即使来自完全父级，`project_value` 始终是部分的。其定位 key 是静态 projection name；validation 仍在 C-01 下附着于最近的真实模型 path。
- 不得为 `within`、`item` 或 `project_value` 声明或测试通用 `Copy` 实现；调用者按其实际 core type move/borrow descriptor。

#### L-204：schema、结构遍历、验证和 submit 委托

`F-207`–`F-209` 仅生成 `C-01` 所需的模型专用实现：

- `FormModelSchema` 递归解析精确 path：direct leaf、group root/descendant、array container、array item root、item descendant 及嵌套 group/array 组合。它在下行前验证 current-array ID 唯一，并返回 core 结构化 path failure，而非使用 prefix/ancestor fallback。
- Garde mapper 将公开 display path 转换为当前 stable-ID path。它在每一层映射 array container（`rows`）、item root（`rows[index]`）及 item descendant（`rows[index].field`）；绝不按 trigger/schema 过滤。无效 index、out-of-bounds、无效/重复 ID、格式错误 suffix 或未知 field 返回供 `C-01` 规范化的 typed core mapping failure。
- 结构性 required/array-ID 遍历遵循精确 scope/path schema。macro 不提供 runtime bucket mutation：C-01 拥有 candidate commit、单次 revision increment、bucket invalidation、async task cancellation/attempt、event 和 notify（`S-05`、`S-06`）。macro 只把静态 policy output 交给隐藏 bridge；它不构造 core-private message，也不选择 transition。
- constructor、context replacement、`replace`、`reset`、`rebase`、CAS rebase、validation execution 和 `prepare_submit` 都是精确的 C-01 `FormState` 领域委托。生成实现仅选择 no-op/custom/Garde validation 和 identity/custom/Validify transform associated type；core 可在领域方法/bridge 内把 intent 转成私有 message 并调用 `Transition`，但 expansion 不引用 `gpui_operation`、不实现状态转换。除非 C-01 最终 static-policy contract 要求，否则不要求 adapter/transform `Default`。
- `prepare_submit` 从一个已验证 snapshot 返回 C-01 的 `PreparedSubmit<Output>`/`ERR-04` 行为。macro 不添加 preview/context transform、I/O、busy/retry state、persistence callback 或第二次 model read。

### 状态与数据流

##### ST-200：derive 时语义模型

- **权威来源：**proc-macro invocation 中的 `DeriveModel`；没有运行时持久化或 GPUI 所有者。
- **初始化和生命周期：**`FormModel` 从一个 `syn::DeriveInput` 解析一次；它被所有 expansion module 消费，并随 compiler invocation 一同丢弃。
- **读取者：**`L-202`–`L-204` expansion module 以及 parser unit/trybuild test。
- **变更：**仅由 parser 构建；任何 expansion module 均不得重新解释 raw attribute。
- **发布和投影：**生成的 token 是唯一输出；diagnostic 保留违规 attribute/field 的 span。
- **重置和取消：**不适用；proc macro execution 没有保留的 task/resource。

##### ST-201：生成的根 form 状态

- **权威来源：**generated `State` 的单个 C-01 `FormRuntime<Model, Context>` field。
- **初始化和生命周期：**每个 edit session 在一个 `Entity<State>` 中由 C-01 `FormState` constructor 创建；随该 entity 一同丢弃。
- **读取者：**显式的 `&Entity<State>` descriptor call、经由 `C-02` 的 adapter binding，以及经由 `C-03` 的 application code。
- **变更：**仅通过 C-01 领域方法；core 在内部以私有 message 执行 `Transition`。macro lens 只修改 transaction candidate，不能触碰 runtime field、投递消息或选择 transition。
- **发布和投影：**C-01 在成功的内部 transition 完成并应用 effect 后 emit 非泛型 `FormEvent`，且每次成功 transaction 仅 notify 一次；静态 descriptor 不缓存 projection，macro 不拥有发布顺序。
- **持久化边界：**无；submit snapshot 经由 `C-03` 转交给 app ownership。
- **重置和取消：**C-01 拥有 lifecycle reset/rebase 和 validation-task cancellation；这些是 core-private transition 语义，不进入 generated/public message surface。此 crate 没有 async resource。

##### ST-202：静态/定位描述符元数据

- **权威来源：**由 `within`、`item` 和 `project_value` 产生的 immutable associated constant 和 value；core 在 `C-01` 下拥有其 concrete representation。
- **初始化和生命周期：**静态 descriptor 在 const-evaluated 时不会分配内存；定位 descriptor 存活于 caller-owned 的普通 Rust value 中。
- **读取者/变更：**caller 对其进行组合并传递；不存储 mutable model、entity、weak entity、subscription、control 或 ID history。
- **发布和投影：**descriptor 仅针对调用处显式 form 进行 resolve；缺失 partial path 返回 root-defined `ERR-01`/`ERR-02`。
- **重置和取消：**重新排序/替换可能使 partial item 不可用；不会发生 descriptor mutation/fallback。

### 边界实现

| 消费的契约 | 此 crate 的精确实现 | 使用方/rollout 检查 |
| --- | --- | --- |
| `C-01` core <-> macro | 针对最终的 doc-hidden macro-support API emit `FormState`、静态 descriptor metadata/lens、schema、结构遍历、Garde mapping、policy associated type、领域 bridge delegation 和 `EventEmitter<FormEvent>`；不 emit `Transition` impl、message/effect/dispatch token。 | 在 adapter/Jaco consumer migration 前，Core owner 提供 runtime integration test，并证明内部协议不进入 generated/public surface。 |
| `ERR-01` | 仅生成 partial descriptor composition；不得捕获、stringify、remap，或将不可用的 projection/item 变成 empty/default value。 | Core test 断言 `try_*` path 报告 canonical error。 |
| `ERR-02` | 将声明的 stable-ID metadata 传递给 C-01 的 identified item lens；不生成第二次 mutation check 或 fallback。 | Core test 覆盖 ID-leaf/item replacement 的 complete no-op semantics。 |
| `ERR-04` | 仅生成静态 transform policy selection 和直接的 `FormState::prepare_submit` delegation。 | Core submit test 断言一个 snapshot 以及 failure 时不变的 state。 |

这里刻意不实现 `C-02`/`ERR-03`：macro output 为 adapter binding 提供 total/partial descriptor，而 `ControlBinding` 拥有其 weak/liveness boundary。`C-03`/`C-04` 是 Jaco/app migration，必须仅在 root sequencing gate 之后消费此输出。

## 所有者本地工作包

### WP-200：替换公开派生宏语法

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- Root `S-09`, `S-10`; `D-200`; `L-200`, `L-201`.
- 无需 C-01 runtime implementation 即可完成仅 parser 的 diagnostic，但在其最终 macro-support boundary 可用前，不开始 generated-code migration。

**文件 ID**

- `F-200`–`F-203`, `F-210`–`F-212`.

**实现顺序**

1. 将导出的 derive 替换为 `FormModel`；移除 `FormStore` proc-macro declaration，而非为其设置 alias。
2. 用 `state` 替换 `store` parsing，将每个 model/field helper 解析为 `DeriveModel`，并为 duplicate、unknown、removed option、malformed array 和 invalid generic array item type 保留 span。
3. 替换所有 v1 fixture import/name，并在接入 C-01 expansion 前建立 v2 compile-fail fixture，确保 grammar failure 永不依赖 unresolved core symbol。

**失败和生命周期行为**

解析失败时，在第二个或无效 token 处发出单个或聚合的 `syn::Error`。不存在 proc-macro fallback、
旧语法兼容或运行时资源。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-200` | `T-200` `attributes.rs` | 规范的 `state` 及内建/自定义策略归一化 | `parse_quote!` 属性 | 一个 `DeriveModel`；类型为 `Ident`/`TypePath`/`LitStr`，而非字符串 |
| `R-201` | `T-201` `tests/ui/fail/removed_form_store.rs` | 已移除的 derive | 带有 `#[derive(FormStore)]` 的最小 crate | 不导出 alias |
| `R-202` | `T-202` `tests/ui/fail/{duplicate,invalid}_*.rs` | 重复、空值、带引号、未知项与互斥语法 | 每个问题 span 一个夹具 | 已检查的 stderr 指明 token 和迁移动作 |
| `R-203` | `T-203` `tests/ui/fail/{non_vec,generic,associated}_array_item.rs` | 数组语法与具名条目边界 | 模型/类型夹具 | 精确的字段/条目 span；没有依赖展开的错误 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form-macros --locked` | parser 单元测试和 trybuild 诊断 | 已锁定的 workspace 工具链 | 所有 v2 `.stderr` 夹具均被接受 |
| `git diff --check` | 文档/源码空白检查 | 本地工作树 | 没有空白错误 |

**完成条件**

仅 `FormModel` 和所述 grammar 仍被接受。每种旧 grammar 都是稳定 diagnostic 或缺失的 derive；不会为仅 parser 的 fixture 调用 generated code。

### WP-201：生成一个状态及其静态完全描述符

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- `WP-200`, root `S-01`, `S-02`, `S-05`, `S-09`, `C-01`; `D-201`, `D-202`, `D-204`–`D-206`; `L-202`, `L-204`; `ST-201`, `ST-202`.

**文件 ID**

- `F-204`–`F-206`, `F-212`.

**实现顺序**

1. 拆分单体 `expand.rs`，使 `state.rs` emit 一个 state、C-01 领域方法所需的 doc-hidden runtime bridge delegation 与静态 policy 委托，`descriptor.rs` 则从共享 semantic model emit 所有 direct-field associated constant。任何 expansion module 都不得生成 `Transition` impl 或 message/effect 类型。
2. 保留 model visibility/generic/where clause，emit 非泛型 `FormEvent`，并将 C-01 的静态 descriptor constructor 仅绑定到静态 schema/path/model lens metadata。
3. 删除 `ModelField`、`FormFieldId`、动态 `*_field` accessor、`form.downgrade()`、field-held entity data 和任何 v1 event generic。

**失败和生命周期行为**

生成的 model lens 只生成候选值。C-01 负责错误、修订、验证、事件、通知和 form 生命周期；macro
不得引入 weak-entity 失败路径。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-204` | `T-204` `tests/expand.rs` | custom state/default state、visibility 和 generic token output | 已解析的 generic model | 恰有一个 state name；没有 `Field` enum/accessor output |
| `R-205` | `T-205` `tests/expand.rs` | direct static field constant 与 generated boundary | `name`、`retry_limit` model | `NAME`/`RETRY_LIMIT` 使用静态 schema/lens output；没有 entity/weak/subscription、`gpui_operation`、`Transition` 或内部 message/effect 声明 token |
| `R-206` | 分配给 `C-01` 的 Root-core `T` | generated state runtime behavior | core integration model | 一个 runtime、显式 entity 领域 API、非泛型 event、一次 transaction；consumer fixture 不导入/构造内部消息，macro owner 不编辑 core test file |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form-macros --locked` | 本地展开/trybuild 回归 | 已锁定的 workspace | token 测试和 UI 测试通过 |
| `cargo test -p gpui-form --test derive --all-features --locked` | C-01 生成状态的运行时契约 | core 所有者的夹具/测试目标 | 显式 entity/const 行为通过 |

**完成条件**

`ProviderForm::NAME` 是不分配内存且没有 entity handle 的静态 descriptor，生成的 state 恰有一个 runtime field，
只委托领域 API，且不再保留 v1 enum/accessor/event 或 core-private transition surface。

### WP-202：生成精确的组合、schema 和稳定 ID 元数据

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- `WP-201`, root `S-03`, `S-04`, `S-05`, `C-01`, `ERR-01`, `ERR-02`; `D-203`; `L-203`, `L-204`; `ST-202`.

**文件 ID**

- `F-206`–`F-208`, `F-212`.

**实现顺序**

1. 为 group constant emit total `within` composition，并为 `item(id)`/`project_value` emit C-01 partial composition；不得添加 compatibility function 或 manually-created path。
2. 为 direct/group/array-container/item-root/item-leaf path emit recursive schema ownership，包括 nested group 和 array，并在 declaring field span 处使用 model-specific bound。
3. 为每个 nesting level 的 container/item/item-leaf emit stable-ID getter metadata 和 Garde display-path mapping；删除 prefix/ancestor schema fallback 和任何 first-match selection。

**失败和生命周期行为**

部分组合精确传播；未解析的 path/item 和 immutable-ID mutation 使用来自 C-01 的 `ERR-01`/`ERR-02`。生成的代码不吞掉 mapping/path error、emit event 或 repair ID。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-207` | `T-206` `tests/expand.rs` | 生成的完全与部分组合调用 | group + array model | `within(total)` 调用完全 API；item/projection 调用部分 API；不对 `Copy` 做断言 |
| `R-208` | `T-207` `tests/ui/fail/array_item_*.rs` | generic/associated array item diagnostic | generic model | 不生成不受支持的 identity abstraction |
| `R-209` | 分配给 `C-01` 的 Root-core `T` | 重排序、缺失/重复 ID、ID mutation 和 nested resolver matrix | core runtime fixture | canonical `ERR-01`/`ERR-02`，失败时无 mutation/event |
| `R-210` | 分配给 `C-01` 的 Root-core `T` | nested reorder 后的 Garde container/item/item-leaf path | Garde derived model | stable path 和精确 schema owner，而非 prefix fallback |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form-macros --locked` | 本地语法/token 回归 | 已锁定的 workspace | macro 测试通过 |
| `cargo test -p gpui-form --test derive --all-features --locked` | 生成的组合 | core 所有者目标 | 运行时 total/partial 测试通过 |
| `cargo test -p gpui-form --test validation --all-features --locked` | schema/Garde 映射 | 启用 Garde feature 的 core 所有者目标 | 精确路径/触发器测试通过 |

**完成条件**

所有生成的组合都保留可用性；array identity 具有 caller-owned 的稳定 metadata；精确的递归 schema/mapping 处理每种声明的 path form，且不会 fallback 到 root/ancestor schema。

### WP-203：将验证、submit 和事件生命周期委托给 core

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- `WP-201`, `WP-202`, root `S-05`–`S-08`, `C-01`, `ERR-04`; `D-204`, `D-206`; `L-204`; `ST-201`.

**文件 ID**

- `F-205`, `F-208`, `F-209`, `F-212`.

**实现顺序**

1. 从 `DeriveModel` 选择 validation/transform associated type，并 emit C-01 的精确 static-policy delegation；保留来自 `L-201` 的 Garde context/message 和 custom context grammar restriction。
2. 仅生成 structural validation/schema/Garde mapper metadata。移除生成的 write transaction、origin/source branching、adapter/transform stored value、form-local async task state、submit runtime、preview/context transform 和 typed `FormEvent<Field>` output。
3. 确保所有 constructor/lifecycle/submit surface 都只委托 C-01 的领域方法及必要的隐藏 runtime accessor。私有消息定义、合法性检查和 `Transition` 调用全部留在 core；macro 不新增 `gpui-operation` 依赖，也不把 message/effect token 展开到 consumer crate。最后对旧 generated name、weak form capture 和 transition 越界做 residual scan。

**失败和生命周期行为**

没有 macro code 转换或显示 error。C-01 控制 bucket replacement、async attempt/cancellation、内部消息合法性、
`ERR-04`、effect/event/notify ordering 和 one-snapshot submit；这些内部拒绝/无变化结果不成为 macro/public error。
macro 不产生 I/O/task/retry/shutdown behavior。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-211` | `T-208` `tests/expand.rs` | no-op/Garde/custom 与 identity/Validify/custom transform 的 policy output | syntax model | 仅 associated type/static policy glue；没有 runtime policy field/default construction、`Transition` impl、message enum 或 `gpui_operation` path |
| `R-212` | 分配给 `C-01` 的 Root-core `T` | mount/context/lifecycle/write bucket/effect/event | recording adapter 和 generated model | 仅通过公开领域方法观察一个 runtime transaction、非泛型 event/单次 notify 与取消语义；fixture 不依赖内部消息名称 |
| `R-213` | 分配给 `C-01` 的 Root-core `T` | submit snapshot 和 validation/pending rejection | counting transform 加 invalid/pending form state | valid submit 恰调用一次 transform；`ERR-04` validation/pending path 调用零次，并保持 model/revision 不变 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form-macros --locked` | 本地展开/诊断覆盖 | 已锁定的 workspace | 通过 |
| `cargo test -p gpui-form --test validation --all-features --locked` | C-01 生命周期/验证证据 | core 所有者目标 | 通过 |
| `cargo test -p gpui-form --test submit --all-features --locked` | C-01 提交证据 | core 所有者目标 | 通过 |
| `rg -n 'FormStore|FormFieldId|FormEvent<|WeakEntity|downgrade\(|SubmitRuntime|set_user_value|preview|transform_context|gpui_operation|Transition<' crates/gpui-form-macros/src crates/gpui-form-macros/tests` | 残留审计 | 本地工作树 | 生效源码/positive fixture 零命中；只解释有意保留的负向夹具文本 |

**完成条件**

macro 不提供 model-specific static metadata/领域 bridge 之外的 lifecycle ownership。Generated form 仅使用非泛型
event 和 C-01 的 prepared-submit path，且不知道 core-private transition protocol。

### WP-204：冻结诊断和生成运行时验收边界

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- `WP-200`–`WP-203`；core/adapter producer gate；Jaco `WP-400..405` 和 root C-03/C-04
  consumer-complete 证据；root `C-01`、`C-02`、`ERR-01`–`ERR-04`；`D-206`；`R-200`–`R-213`。

**文件 ID**

- `F-210`–`F-212`.

**实现顺序**

1. 审计 `WP-200`/`WP-201` 已完成的 v1 derive/grammar removal，只移除 residual legacy fixture/helper，然后运行/更新 macro-only parser、expansion-token 和 trybuild fixture；仅在审阅每个 span/message 后接受 `.stderr`。同时认证 manifest/active source 没有 `gpui-operation`，generated token 没有 `Transition`/message/effect。
2. 将 generated-model runtime matrix 作为 C-01 acceptance 交给 core owner，不添加 macro -> core dev dependency cycle，也不复制 core fixture。
3. 将 adapter `C-02` 和 Jaco `C-03`/`C-04` test 视为 downstream rollout gate，而非恢复 legacy macro name 的理由。

**失败和生命周期行为**

编译失败保持为确定性的源码诊断。运行时错误和存活行为在其所属的 core/adapter 边界中测试，绝不由
token snapshot 伪造。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-214` | `T-209` `tests/ui.rs` | 所有 grammar/removal compile failure | 已检查的 `.rs`/`.stderr` 集合 | diagnostic 在 `FormModel` 下运行；旧 symbol 没有 positive fixture |
| `R-215` | `T-210` core `tests/{derive,validation,submit}.rs` | 真实 generated model lifecycle/partial/submit behavior | C-01 test fixture | generated consumer 只编译/调用领域 API；core 独立覆盖私有 transition，macro snapshot 不固化消息名称 |
| `R-216` | `T-211` adapter/core integration target | descriptor binding 的完全和部分 boundary | C-02 fixture | ControlBinding weak boundary 归 adapter 所有；descriptor 中没有 entity |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo fmt --all --check` | 格式化 | workspace toolchain | 无改动 |
| `cargo test -p gpui-form-macros --locked` | macro 测试 | 已锁定的 workspace | 通过 |
| `cargo check -p gpui-form-macros --all-targets --locked` | macro 编译 | 已锁定的 workspace | 通过 |
| `rg -n 'gpui[_-]operation|use .*Transition|impl .*Transition' crates/gpui-form-macros/Cargo.toml crates/gpui-form-macros/src` | Manifest/source 边界扫描 | 本地工作树 | 零匹配；macro 不拥有 transition dependency 或实现。 |
| `cargo tree -p gpui-form-macros --edges normal --depth 1 --locked` | Macro 依赖边界 | 已锁定的 workspace | 不直接依赖 `gpui-operation`；transition 依赖只位于 core。 |
| `cargo test -p gpui-form --all-features --locked` | C-01 生成运行时 | core 所有者夹具套件 | 通过 |
| `cargo test -p gpui-form-gpui-component --all-features --locked` | C-02 消费者兼容性 | adapter 所有者夹具套件 | 通过 |
| `git diff --check` | 补丁完整性 | 本地工作树 | 无错误 |

**完成条件**

每个被接受的 macro input 和 diagnostic 均稳定；runtime evidence 位于 C-01/C-02，没有 dependency cycle、
legacy compatibility layer 或 macro/generated transition protocol。

### WP-205：完成 macro 所有者文档并交接

**所有者**

`crates/gpui-form-macros`

**前置条件和契约**

- `WP-204`；所有 C-01 runtime acceptance 均通过。Root 拥有 release status、index change、aggregate validation 和 downstream migration sequencing。

**文件 ID**

- `F-213`, `F-214`.

**实现顺序**

1. 仅在 source/test 证明 API 后，从英文 README/guide 中移除 preview/current-v1 status wording；保留 public example 和 target boundary statement。
2. 更新中文 README/guide 作为 semantic mirror：heading、link、code identifier、signature shape、grammar、total/partial wording、breaking-removal statement 及无通用 `Copy` 的 claim 必须与英文一致。
3. 将实际 macro file/test/command 记录为 root hub 的 deviation 或 completion update；不得独立变更 root status、plan map 或 index。

本次内部 `Transition` 决策不改变任何公开/generated 签名或调用方式，因此不会单独触发 `F-213`/`F-214`
修改，公开文档也不得新增 message、dispatch、phase 或 `Transition` 示例。`WP-205` 仍只在整体 v2 API 实现并经
root 授权后，完成原计划中的预览状态清理与中英文公开文档同步。

**失败和生命周期行为**

文档不得宣传未实现的 core/adaptor behavior。此 WP 不创建 runtime implementation。

**测试**

| R-ID | T-ID/文件 | 拟议场景 | 夹具/模拟对象 | 断言 |
| --- | --- | --- | --- | --- |
| `R-217` | `T-212` 文档审阅 | 英文/中文 macro README/guide | 渲染后的 Markdown/源码差异 | 链接、API 名、语法、示例与可用性语义一致 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `rg -n 'FormStore|store =|FormFieldId|FormEvent<' crates/gpui-form-macros/{README.md,README.zh-CN.md,docs}` | 残留文档审计 | 本地工作树 | 仅显式保留历史/移除说明 |
| `git diff --check` | Markdown 空白检查 | 本地工作树 | 无错误 |

**完成条件**

macro 文档描述一个已实现的 breaking API，并将所有跨所有者 completion reporting 委托给 root hub。

## 聚焦验证与交接

| R-ID/要求 | 所有者/WP | 自动化/手动证据 | 预期结果 | 外部前置条件 |
| --- | --- | --- | --- | --- |
| `R-200`–`R-203` 严格 grammar 和 migration diagnostic | `WP-200` | `cargo test -p gpui-form-macros --locked` | trybuild span/message 和 parser unit test 通过 | Rust toolchain/locked dependency |
| `R-204`–`R-206` 一个 state/静态 descriptor/无 entity capture | `WP-201` | macro expansion test；core `tests/derive.rs` | 精确名称、无旧 enum/accessor、runtime 显式 form call | C-01 最终 macro support |
| `R-207`–`R-210` availability/schema/stable ID | `WP-202` | macro expansion；core derive/validation test | total/partial propagation 和精确 stable path | C-01 最终 core test |
| `R-211`–`R-213` policy/lifecycle/submit delegation | `WP-203` | macro expansion；core validation/submit test | 无 macro-owned lifecycle、一个 snapshot、canonical error | C-01 最终 core test |
| `R-214`–`R-216` 所有者边界 test 分配 | `WP-204` | macro/core/adapter targeted command | test 位于实际所属的 boundary | C-01/C-02 所有者 |
| `R-217` 文档一致性 | `WP-205` | rendered/manual bilingual review 加 residual scan | 一个已实现的 API、无意外的 `Copy` commitment | source/test completion |

交接约束：

1. 在 `WP-201` 前，core owner 必须为静态 total/partial descriptor 和非泛型 `FormEvent` 暴露最终的 `C-01` macro-support API；本计划刻意不虚构 hidden constructor name。
2. core owner 必须在自己的 owner plan/file 中添加上文引用的 runtime test row。macro crate 不得将 `gpui-form` 添加为 dev dependency，因为这会创建 proc-macro/core cycle。
3. 在 `WP-205` 前，root 必须确认 C-01 和 C-02 focused validation。Jaco migration（`C-03`/`C-04`）属于 downstream，不能请求 `FormStore`/weak-entity shim。
4. 除 root `S-01`–`S-12`、`C-01`–`C-04` 和 `ERR-01`–`ERR-04` 外，不需要额外 shared ID。
