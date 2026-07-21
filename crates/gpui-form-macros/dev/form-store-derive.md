# `FormStore` derive breaking 重构实施计划

## 1. 状态与范围

- 文档位置：`crates/gpui-form-macros/dev/form-store-derive.md`。
- 关联分支：`codex/175-jaco-shortcut-temporary-window`。
- 关联 issue：无独立 issue；这是跨 crate form 基础设施迁移，不属于 issue #175 的产品需求。
- **当前状态：目标 expansion、严格 parser 与 compile-fail harness 已实现；runtime integration、
  parser unit tests 和 trybuild tests 均已通过。** 公开契约以 [`README.md`](../README.md) 和
  [`docs/guide.md`](../docs/guide.md) 为准。`src/attributes.rs` 现在拒绝重复、互斥、空 clause、
  quoted type、bare array ID 与已删除的 component/codec/binding 等语法；`src/expand.rs` 生成单一
  typed runtime、字段/分组/数组 accessor、结构验证与 associated transform output。
- **2026-07-21 验证：**已按仓库规则通过 Cargo 添加 dev-only `trybuild 1.0.118`，由 Cargo
  同步更新同一份 `Cargo.lock`；11 组 compile-fail fixture 及其 `.stderr` 契约均通过，未手工
  改写 manifest 或 lockfile。
- **目标：**保留 `#[derive(gpui_form::FormStore)]`，从普通 Rust model 生成一个持有完整
  typed model 的 GPUI store、field identity/schema、typed `FormField` projection、validation
  glue、stable-ID traversal 与 submit transform glue。
- **非目标：**生成具体 UI component、component options、raw/string draft、codec、focus/touched/
  blurred 状态、subscription、异步提交任务、busy/retry 状态、数据库访问或持久化代码。
- **兼容策略：**这是一次有意的 breaking 重构。删除的属性和 API 直接给出迁移诊断，不保留
  alias、兼容 trait 或双轨生成代码。
- **落地前置：**`gpui-form` 的目标 `FormStore`、`ValidationAdapter`、`SubmitTransform`、
  validation runtime、revision 与 typed field transaction 必须先落地或与宏在同一提交中原子
  落地。宏不能临时复制 core lifecycle 来绕过前置依赖。
- **系统边界：**本计划只改 `crates/gpui-form-macros` 及直接验证 target API 所需的
  `crates/gpui-form` integration tests。UI adapter 与 Jaco 迁移属于各自实施计划。

## 2. 证据快照

### 2.1 当前仓库事实

| 证据位置 | 当前事实 | 与目标契约的差距 |
| --- | --- | --- |
| `src/attributes.rs` | 同一 option 可被后一个值覆盖；仍接受部分 quoted type、bare array ID、空 clause 等宽松语法 | 必须只接受唯一 canonical grammar，重复或非 canonical 输入立即报错 |
| `src/expand.rs` | store、field、schema、validation、transform 与 accessor 生成集中在一个文件 | 按解析模型和生成职责拆分，禁止在多个分支重复生命周期代码 |
| `src/expand.rs` | 自定义 store 名推导 field enum 时只去掉 `Store`，例如 `ProviderInputFormStore` 会得到错误的 `ProviderInputFormField` | field enum 始终由 model 名生成：`ModelField`；store override 只影响 store 名 |
| `src/expand.rs` | `from_value` 按 validation 配置条件生成；mount/context setter 行为与目标契约不一致 | constructor 统一走 core contract；两种 constructor 都只执行一次 `on_mount`，context setter 不隐式验证 |
| `src/expand.rs` | generated store 仍持有 `SubmitRuntime`，`Output = Model`，transform 仍含 preview/context/旧 submit 方法 | 删除 submit runtime；通过 `SubmitTransform<Model>::Output` 与唯一 `transform` 生成 submit glue |
| `src/expand.rs` | `required` 受 trigger 配置影响；nested identified array 缺少完整 `*_item_in` traversal | `required` 无条件进入 submit validation；生成四种固定 accessor |
| `src/attributes.rs` 单元测试 | 覆盖部分 parser 行为 | 缺少系统性的 duplicate、mutual-exclusion、removed-option 与 canonical-syntax 断言 |
| `tests/` | macro crate 没有 compile-fail harness | 新增 `trybuild`，把 public invalid syntax 与迁移诊断固定为编译契约 |

### 2.2 目标 core 契约

宏生成代码只依赖下列 target contract，不自行发明替代 runtime：

```rust,ignore
pub trait FormStore: EventEmitter<FormEvent<Self::Field>> + Sized + 'static {
    type Model: Clone + PartialEq + 'static;
    type Output: 'static;
    type Field: FormFieldId;
    type ValidationContext: 'static;
    type ValidationAdapter:
        ValidationAdapter<Self::Model, Context = Self::ValidationContext>;
    type SubmitTransform:
        SubmitTransform<Self::Model, Output = Self::Output>;

    fn from_value(value: Self::Model, cx: &mut Context<Self>) -> Self
    where
        Self::ValidationContext: Default;

    fn from_value_with_validation_context(
        value: Self::Model,
        context: Self::ValidationContext,
        cx: &mut Context<Self>,
    ) -> Self;

    fn value(&self) -> &Self::Model;
    fn baseline(&self) -> &Self::Model;
    fn revision(&self) -> FormRevision;
    fn validation_context(&self) -> &Self::ValidationContext;
    fn validation_report(&self) -> ValidationReport;
    fn is_dirty(&self) -> bool;
    fn is_valid(&self) -> bool;
    fn is_validating(&self) -> bool;
    fn is_validating_at(&self, path: &FieldPath) -> bool;
    fn errors_at(&self, path: &FieldPath) -> Vec<ValidationIssue>;
    fn first_error_path(&self) -> Option<FieldPath>;

    fn set_validation_context(
        &mut self,
        context: Self::ValidationContext,
        cx: &mut Context<Self>,
    );
    fn replace(&mut self, value: Self::Model, cx: &mut Context<Self>);
    fn reset(&mut self, cx: &mut Context<Self>);
    fn rebase(&mut self, value: Self::Model, cx: &mut Context<Self>);
    fn rebase_if_revision(
        &mut self,
        expected: FormRevision,
        value: Self::Model,
        cx: &mut Context<Self>,
    ) -> bool;
    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &mut Context<Self>,
    );
    fn prepare_submit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<Self::Output, SubmitError>;
}

pub trait SubmitTransform<Model>: Default + 'static {
    type Output: 'static;

    fn transform(&self, model: &Model) -> Result<Self::Output, TransformReport>;
}
```

`ValidationAdapter<Model>` 同样要求 `Default + 'static`。Generated store 只保存一个
`gpui_form::__private::FormRuntime<Model, ValidationContext>`；adapter 与 transform 只通过
associated type 指定，每次执行时分别调用 `Self::ValidationAdapter::default()` 与
`Self::SubmitTransform::default()`，不保存实例。

Core 还固定以下写入语义，宏只提供 projection/schema：equal leaf write 是 no-op；whole-form
`replace`、`reset`、`rebase` 和成功的 `rebase_if_revision` 即使值相等也推进 monotonic revision；
失败的 `rebase_if_revision` 没有任何副作用。

### 2.3 依赖证据

| 依赖 | 当前版本与用途 | 本计划决定 |
| --- | --- | --- |
| `proc-macro2` | `1.0.106`，token/span | 保持不变 |
| `quote` | `1.0.45`，生成 token stream | 保持不变 |
| `syn` | `2.0.118`，features `full`, `extra-traits` | 保持不变；继续用公开 parser API |
| `trybuild` | 当前不存在 | 新增精确 dev dependency `1.0.118`，只用于 compile-fail contract tests |
| `garde` | workspace/core 使用 `0.23.0` | 宏不新增直接 runtime 依赖；只生成 core feature-gated adapter type，并基于公开 `Path::Display` 格式映射 |
| `gpui-component`/assets | 当前 lock `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` | 先由 adapter `DEP-00` 锁到 `5b45bcb26b9343d91a123a4d5ed8a654360512e5`；macro 只消费同一 lockfile |
| Zed/GPUI crates | 当前 manifest/lock 带 `?rev=1d217ee39d381ac101b7cf49d3d22451ac1093fe` | 先由 adapter `DEP-00` 统一为无 query `https://github.com/zed-industries/zed`，lock `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` |

`trybuild 1.0.118` 的公开 API 为 `trybuild::TestCases::new()`、`compile_fail(...)` 与
`pass(...)`；crate 使用 Rust 2024 edition，`rust-version = 1.85`，低于 workspace 的 Rust
`1.92+` 基线。它没有 native、系统服务或平台 runtime 要求。

上游证据：

- <https://docs.rs/trybuild/1.0.118/trybuild/>
- <https://docs.rs/crate/trybuild/1.0.118/source/Cargo.toml>

### 2.4 明确不变的系统表面

| 系统表面 | 决定 |
| --- | --- |
| UI/component/focus/accessibility | 宏不拥有；由 adapter crate 与页面决定 |
| 数据库、持久化、网络、异步任务、shutdown | 不进入宏；`prepare_submit` 只做同步验证与 transform |
| 资源、图标、主题、用户文案 | 无变更 |
| 国际化 | `i18n = ProviderType` 只选择 Garde provider type；宏不生成 locale key、翻译文本或 observer |
| 数据迁移 | 无持久化 schema，因此不需要 migration 或 fallback |

## 3. 已冻结设计决策

1. **命名与输入类型**：只支持 named-field struct。默认生成 `ModelFormStore` 与
   `ModelField`；`#[form(store = CustomStore)]` 只覆盖 store 名，field enum 始终来自 model
   名。完整保留 visibility、lifetime、type parameter、const generic、合法的 default 与
   `where` clause；impl 中按 Rust 规则去掉 type default。
2. **唯一 runtime 与业务值**：generated store 恰好只保存一个
   `gpui_form::__private::FormRuntime<Model, ValidationContext>`。完整 typed model、baseline、
   revision、validation context 与 validation state 都由它持有；不生成 raw/string draft、codec、
   child form entity、每字段业务值副本、adapter instance 或 transform instance。
3. **严格语法**：model/field 各最多一个 `#[form(...)]`；每个 option 最多一次；空 clause、
   duplicate、unknown、removed 与非 canonical 拼写都是 compile error，禁止 last-write-wins。
4. **Validation adapter 互斥规则**：无 adapter 时禁止 `context`/`i18n`；`"garde"` 禁止
   `context`、可选 `i18n`；custom adapter 可选 `context`、禁止 `i18n`。省略 custom context
   时使用 adapter 的 associated `Context`。
5. **Constructor**：两种 constructor 都先构造唯一 runtime，再恰好执行一次 `on_mount`。
   `from_value` 由 trait 提供并要求 `ValidationContext: Default`；derive 不按配置条件生成另一套
   inherent API，也不构造或保存 adapter/transform instance。
6. **Context 更新**：`set_validation_context` 只替换 context 并 notify；调用者需要时显式选择
   trigger/scope 验证，宏不偷偷运行 dynamic/form validation。
7. **写入 transaction**：equal typed field write 整条流程 no-op。非相等写入由 core 的
   `FormField` transaction 先投影并保存 typed value、推进 revision；只清除相交的 required、
   structural 与 generated synchronous field bucket；取消并清除相交 async validation；保留
   adapter-wide form bucket 与所有 active control issue；随后运行 Change validation，最后发出一个
   typed event 并 notify 一次。宏只生成读写 projection、path 与 schema，不能为每个字段复制
   lifecycle。
8. **Required 语义**：`required` 永远进入 submit validation；`validate(...)` 只增加 mount/change/
   blur/dynamic/submit 等更早或显式 trigger，不控制 required 是否在提交时生效。
9. **Adapter/Transform 类型**：`ValidationAdapter<Model>` 与 `SubmitTransform<Model>` 都要求
   `Default + 'static`，仅作为 `FormStore` associated type。每次运行临时 `default()` 构造，store
   不保存实例。`SubmitTransform<Model>` 只有 associated `Output` 与一个 `transform`；generated
   `FormStore::Output` 精确等于 transform output。无 preview、context、model 回写或 validation
   state 副作用。
10. **Owned validation query**：generated `validation_report()` 返回 `ValidationReport`，
    `errors_at(path)` 返回 `Vec<ValidationIssue>`；不得把 runtime 内 bucket 的引用生命周期暴露给
    调用者。
11. **Nested traversal**：一个 root store；group/array 只生成 typed lens。固定生成
    `*_field`、`*_in`、`*_item`、`*_item_in`，所有 nested handle 保留 root store type。
12. **Stable ID**：`array(id = "row_id")` 只接受 `Vec<Item>`，ID 通过 `ToFormItemId` 转换。
    missing、duplicate 或不可转换 ID 是 blocking structural issue；访问器不猜测第一个 item。
13. **Garde path**：使用 Garde 公开 `Path::Display` 字符串，不依赖 doc-hidden iterator；vector
    index 必须按本次 validated model 转成 stable item ID。unknown/out-of-bounds/duplicate/invalid
    path 返回 typed `GardePathError`，不能 lossy fallback。
14. **测试契约**：parser unit tests 验证 AST；`trybuild` 固定 compile-fail 诊断；
    `gpui-form` integration tests 验证生成 API 与 runtime。compile-fail 测试不能由 doctest 代替。
15. **删除策略**：删除 generated `SubmitRuntime` 和旧 draft/component/codec/focus/touched/blurred/
    show-error API，不保留 deprecated facade。

## 4. 目标属性语法

### 4.1 Model 属性

```text
store = StoreIdent
validation(adapter = "garde"[, i18n = ProviderType])
validation(adapter = CustomValidatorType[, context = ContextType])
transform(adapter = "validify")
transform(adapter = CustomTransformType)
```

- `StoreIdent` 是未加引号的单个 identifier。
- custom adapter、context、I18n provider 是未加引号的 Rust type path。
- 只有内建 adapter 名 `"garde"` 与 `"validify"` 使用 string literal。
- `validation(...)` 与 `transform(...)` 必须包含 `adapter`，不能为空。

### 4.2 Field 属性

```text
required
validate(on_mount, on_change, on_blur, on_dynamic, on_submit)
group
array(id = "row_id")
```

- `required`、`group` 是 bare flag；`required = true`、`group()` 无效。
- `validate(...)` 至少包含一个 trigger，且每个 trigger 唯一。
- `array(...)` 只接受一个 `id = "..."`；bare identifier、额外 option、缺失 ID 与非
  `Vec<T>` 字段无效。
- `group` 与 `array` 互斥。
- `component`、`binding`、`codec`、`state`、`focus`、`touched`、`blurred`、`show_error`、
  `group(store = ...)`、`array(store = ...)` 等旧配置全部拒绝，并指出新责任边界。

## 5. 目标实现结构

禁止新增 `mod.rs`。目标文件结构如下：

```text
crates/gpui-form-macros/
  Cargo.toml
  src/
    lib.rs
    attributes.rs
    model.rs
    expand.rs
    expand/
      field.rs
      store.rs
      validation.rs
      transform.rs
  tests/
    ui.rs
    ui/
      fail/
        *.rs
        *.stderr
```

`attributes.rs` 只负责 canonical parsing 与组合诊断；`model.rs` 把 `syn::DeriveInput` 归一化为
一次解析、供全部 expansion 共享的语义模型；`expand.rs` 只编排四个 generation module。

冻结的内部模型如下；实现必须使用这些名称与职责，不能合并回 token 分支或另建平行语义模型：

```rust,ignore
struct FormAttributes {
    store: Option<Ident>,
    validation: Option<ValidationSpec>,
    transform: Option<TransformSpec>,
}

enum ValidationSpec {
    Garde { i18n: Option<TypePath> },
    Custom { adapter: TypePath, context: Option<TypePath> },
}

enum TransformSpec {
    Validify,
    Custom(TypePath),
}

struct FieldAttributes {
    required: bool,
    triggers: Vec<ValidationTriggerName>,
    shape: FieldShape,
}

enum FieldShape {
    Leaf,
    Group,
    IdentifiedArray { item: Type, id_field: Ident },
}

struct DeriveModel<'a> {
    input: &'a DeriveInput,
    form: FormAttributes,
    fields: Vec<DeriveField<'a>>,
    store_ident: Ident,
    field_ident: Ident,
}
```

Parser 优先使用 `syn::TypePath`、`Ident`、`LitStr` 体现语法类别；不要先解析成字符串再尝试恢复
Rust type。所有 duplicate 检测必须在赋值前完成，并用重复 token 的 span 报错。

## 6. Generated API 与数据流

### 6.1 命名与 store

对 `Model` 生成：

```rust,ignore
pub enum ModelField {
    FirstField,
    SecondField,
}

pub struct ModelFormStore<Generics>
where
    /* 原 model 约束 */
{
    runtime: gpui_form::__private::FormRuntime<
        Model<Generics>,
        ValidationContextType,
    >,
}
```

这是 generated store 的唯一字段。不生成 `SubmitRuntime`，也不保存 validation adapter 或 submit
transform instance。无 validation 时选择 `NoopValidationAdapter`/
`NoValidationContext`；Garde 选择
`GardeAdapter<Model, Provider>`，默认 provider 为 `DefaultGardeI18nProvider`；custom adapter
使用声明类型。无 transform 时选择 `IdentityTransform<Model>`；`"validify"` 选择
`ValidifyTransform<Model>`；custom transform 使用声明类型。所选 adapter/transform 都通过 associated
type 表达，并分别满足 `ValidationAdapter<Model>: Default + 'static` 与
`SubmitTransform<Model>: Default + 'static`。

生成的 `FormStore` associated types 必须满足第 2.2 节 contract：

```rust,ignore
type Model = Model<Generics>;
type Output = <Self::SubmitTransform as SubmitTransform<Self::Model>>::Output;
type Field = ModelField;
type ValidationContext = /* resolved context */;
type ValidationAdapter = /* resolved adapter */;
type SubmitTransform = /* resolved transform */;
```

### 6.2 Constructor 与生命周期

`from_value_with_validation_context` 的生成顺序固定为：

1. 用 `value`、`value.clone()`、初始 revision、传入 validation context 与空 validation state
   构造唯一 `FormRuntime`；
2. 把 runtime 放入 generated store；
3. 通过 `Self::ValidationAdapter::default()` 对完整 root scope 执行一次 `on_mount`；
4. 返回 store；不提前构造 `Self::SubmitTransform`。

`from_value` 使用 `ValidationContext::default()` 委托给上述 constructor。`set_validation_context`
只替换 context 并 notify。`replace/reset/rebase/rebase_if_revision`、revision、dirty 与 validation
query 按 core contract 实现，不能在宏内增加 submit attempt、busy 或 persistence side effect。
`validation_report()` 克隆并返回 owned `ValidationReport`；`errors_at(path)` 克隆过滤后的 issue，
返回 `Vec<ValidationIssue>`。

### 6.3 Field 与 accessor

每个声明字段生成稳定 enum variant、schema 与 root accessor：

```rust,ignore
ModelFormStore::field_field(
    form: &Entity<ModelFormStore>,
) -> FormField<ModelFormStore, FieldType>;
```

对 derived child model 生成 nested accessor：

```rust,ignore
ChildFormStore::field_in<Root>(
    parent: FormField<Root, Child>,
) -> FormField<Root, FieldType>;
```

对 identified array 生成 root 与 nested item accessor：

```rust,ignore
ModelFormStore::items_item(
    form: &Entity<ModelFormStore>,
    id: FormItemId,
) -> FormField<ModelFormStore, Item>;

ChildFormStore::items_item_in<Root>(
    parent: FormField<Root, Child>,
    id: FormItemId,
) -> FormField<Root, Item>;
```

accessor 是 cheap typed handle；重复创建不注册 subscription、不分配 child entity、不复制值。读写时按
stable path 在 root model 上投影；找不到唯一 item 时返回 `FormFieldError::ValueUnavailable`。

### 6.4 Validation 与 Garde mapping

- static schema 顺序等于字段声明顺序。
- leaf trigger 只属于该 leaf；group/array ancestor 不复制 descendant trigger。
- `required` 无条件加入 submit rule，并可按 `validate(...)` 在更早 trigger 运行。
- custom adapter 从同一 owned model snapshot 接收 trigger、scope 与 typed context。
- 每次 adapter run 临时构造 `Self::ValidationAdapter::default()`；runtime 不保存 adapter instance。
- 成功 typed write 的失效集合只包括与 path 相交的 required/structural/generated synchronous field
  bucket 和 async entry；adapter-wide form bucket 与 active control issue 必须保留。
- Garde adapter context 固定为 `<Model as garde::Validate>::Context`；macro 只选择 adapter/provider
  type，不调用或复制 Garde validation runtime。
- generated `GardePathMapper` 解析公开 `Path::Display`，先在本次 model 上 bounds-check index，
  再读取 declared ID 并递归 child schema。
- path mapping failure 转为 typed `GardePathError`，core runtime 将其记录为 blocking internal
  issue；不能丢弃错误或附着到错误的 index path。

### 6.5 Submit transform

`prepare_submit` 必须委托 core runtime，针对同一个 owned model snapshot：

1. 执行 submit validation，包括 required 与 structural checks；
2. 拒绝 validation issue 或 blocking async validation pending；
3. 构造一次 `Self::SubmitTransform::default()` 并调用一次
   `SubmitTransform<Model>::transform`；
4. 返回 associated `Output`。

宏不生成 transform preview、transform context、第二次 model read、I/O 或 current model 回写。

## 7. 编译期诊断 contract

| 非法输入 | 诊断定位 | 诊断要求 |
| --- | --- | --- |
| 第二个 model/field `#[form(...)]` | 第二个 helper attribute | 说明每个 target 只允许一个 helper |
| 重复 `store`/`validation`/`transform`/nested option | 重复 option | 不允许后值覆盖前值 |
| quoted custom type | string literal | 要求未加引号的 Rust type path |
| 未加引号或未知 built-in name | adapter value | 列出仅支持的 built-in literal |
| 空 `validation()`/`transform()`/`validate()` | 空 clause | 指出缺失的 required option/trigger |
| Garde + `context` | `context` | 指向 `#[garde(context(...))]` |
| custom/no adapter + `i18n` | `i18n` | 说明 I18n 只属于 Garde provider |
| `group` + `array` | 后出现的 shape | 说明互斥 |
| `array` 用于非 `Vec<T>` | 字段类型 | 说明 canonical `Vec<Item>` 要求 |
| `array` ID 非 string、缺失或有额外 option | 对应 token | 给出 `array(id = "field")` |
| removed draft/component/focus option | removed option | 指出由 component/adapter/page 负责，不给兼容 alias |

无法在 proc macro 中反射验证的 Rust 关系（例如 item type 是否真的有 declared ID field）由 generated
typed field access 交给编译器检查；计划不能声称宏能提前 introspect 另一个类型的字段。

## 8. 上游复用审计

| 能力 | 复用方案 | 本地适配边界 |
| --- | --- | --- |
| Rust AST/parser/span | 直接使用 `syn` | 只增加 canonical grammar 与聚合诊断，不造字符串 parser |
| Token generation | 直接使用 `quote`/`proc-macro2` | generation module 消费同一个 `DeriveModel` |
| typed field/schema/path | 复用 `gpui-form` 的 `FormField`、`FormFieldId`、`FieldPath`、schema API | 宏只生成静态 projection 与 metadata |
| write/revision/validation lifecycle | 复用 core transaction/runtime | 删除宏内重复 setter lifecycle |
| Garde validation/i18n | 复用 core `GardeAdapter` 与 provider traits | 宏只选择 type 并生成 stable path mapper |
| transform | 复用 `IdentityTransform`、`ValidifyTransform` 与 `SubmitTransform` | 不复制 Validify 逻辑 |
| compile-fail | 直接使用 `trybuild 1.0.118` | 维护 repo-local `.rs`/`.stderr` fixture |

需要删除的 interim/legacy 实现：generated `SubmitRuntime`、preview/context transform glue、条件化
constructor、draft/component/codec/focus/touched metadata、last-write-wins parser、child store expansion 与
重复 field lifecycle。

## 9. 工作包

### DEP-00：消费统一的 GPUI source 与 lockfile 门禁

这是 adapter 计划拥有的跨计划发布门禁；macro 计划只验证并消费结果，不再次选择依赖版本或改写
root source。

**前置**

- 用户已批准第 2.3 节的依赖设计选择。
- 设计批准不等于未来命令权限。实际执行联网解析、新增 `trybuild`、修改 `Cargo.lock` 或其他受仓库
  权限规则约束的命令前，必须在执行当次单独申请提权。

**证据**

- 当前 root manifest/lock 的 Zed source 带
  `?rev=1d217ee39d381ac101b7cf49d3d22451ac1093fe`，gpui-component lock 为
  `c36b0c6ae6d14c33473f6610a27c3abc584afdf9`。
- Adapter 计划已核实 gpui-component
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5` 与其 Zed lock
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`；带 query 与无 query 的 Zed Git source 会形成
  不同 Cargo source identity。

**文件**

- 本 gate 不修改 macro 文件。
- `Cargo.toml` 与 `Cargo.lock` 的唯一修改 owner 是
  `crates/gpui-form-gpui-component/dev/typed-bound-controls.md` 的 `DEP-00`。

**API 契约**

- Macro 不拥有 dependency mutation API；它只消费 adapter `DEP-00` 产出的唯一 manifest/source
  identity 与 root lockfile，任何不一致都视为未满足前置条件。

**实现流程**

1. 确认 root manifest 的所有 Zed dependencies 与 patch replacements 使用同一个无 query
   `https://github.com/zed-industries/zed` source，原 features 不变。
2. 确认同一 `Cargo.lock` 中 longbridge packages 全部锁到 `5b45bcb...`，Zed packages 全部锁到
   `1a246efd...`。
3. 执行 duplicate tree 与 macro 定向 check；任一 source 不一致都停止 FORM/MACRO 工作包。
4. 后续新增 `trybuild` 继续更新这同一份 lockfile，禁止生成第二份 lockfile 或手工编辑 package
   checksum/source。

**错误与生命周期**

- 这是构建期原子门禁，没有运行时生命周期。Manifest 与 lockfile 必须作为同一 changeset 验证。
- 出现第二份 GPUI、错误 SHA 或未锁定解析时立即停止；不得用 type conversion、patch alias、unsafe
  或兼容 facade 绕过。

**UI、数据、数据库、图标、国际化与依赖**

- UI、form data、数据库/schema、网络、icons/assets、应用 i18n：`No change`。
- Features/MSRV：`gpui_platform` 既有 features 与 Rust 1.92+ 基线不变。
- 平台：不改 bootstrap；最终由 macOS/Linux/Windows CI 覆盖。
- 依赖：只消费 adapter `DEP-00` 的 exact source/lock 结果。

**测试**

| 要求 | Fixture | 断言 |
| --- | --- | --- |
| 单一 GPUI identity | root dependency tree | 不存在第二份 `gpui`/`gpui_platform` source |
| exact lock | `Cargo.lock` | longbridge=`5b45bcb...`；Zed=`1a246efd...` |
| macro 可解析 | macro package check | dependency resolution 在 `--locked` 下成功 |

**验证**

```bash
cargo tree -d --locked
cargo check -p gpui-form-macros --all-targets --locked
```

**完成条件**

- 两条命令通过，manifest source identity 与两个 exact lock SHA 一致。
- Gate 完成后进入 core `FORM-10..60`；core 全部完成后才进入 `MACRO-10`。

### MACRO-10：依赖、严格属性 AST 与 compile-fail harness

**前置**

- DEP-00 与 core `FORM-10..60` 已完成；第 3、4、7 节契约已冻结。
- 执行依赖变更前单独申请联网与 lockfile 修改权限；本计划文本不能代替当次授权。

**证据**

- `Cargo.toml` 当前只有 `proc-macro2`、`quote`、`syn`，没有 compile-fail harness。
- `src/attributes.rs` 仍存在 last-write-wins、quoted custom type、bare array ID 与空 clause 等宽松
  路径；这些输入与第 4、7 节 canonical grammar 冲突。
- Macro crate 不能添加 `gpui-form` dev dependency，否则会形成 proc-macro/core dev cycle。

**文件**

- 修改 `crates/gpui-form-macros/Cargo.toml`、`src/attributes.rs`、`src/lib.rs`。
- 新增 `src/model.rs`、`tests/ui.rs`、`tests/ui/fail/*.rs`、`tests/ui/fail/*.stderr`。
- 由 Cargo 更新已有 root `Cargo.lock`；不手工编辑 lockfile。

**API 契约**

- Parser 只接受第 4 节 grammar；model/field 各最多一个 helper，每个 option/trigger 最多一次。
- `store` 使用 `Ident`；custom adapter/context/provider 使用 `TypePath`；array ID 使用 `LitStr`；仅
  `"garde"`/`"validify"` 是 built-in string literal。
- Parsing 只产生第 5 节冻结的 `FormAttributes`、`ValidationSpec`、`TransformSpec`、
  `FieldAttributes`、`FieldShape` 与 `DeriveModel`。

**实现流程**

1. 获得提权后，在 workspace root 执行
   `cargo add --package gpui-form-macros --dev trybuild@1.0.118`，把精确版本写入本 crate
   `[dev-dependencies]`，并让 Cargo 更新 DEP-00 已产生的同一 `Cargo.lock`；禁止另建 lockfile
   或手改 entry。依赖变更命令本身是 mutation，不冒充 `--locked` 验证；变更完成后的所有 Cargo
   验证必须使用 `--locked`。
2. 先实现 helper/option 首次 span 记录与 duplicate 聚合，再解析具体值，避免后值覆盖前值。
3. 实现 validation/context/i18n mutual exclusion、field shape 互斥与 canonical `Vec<Item>` 检查。
4. 一次构建 `DeriveModel`，所有 expansion 只消费它，不再次解释 attributes。
5. `tests/ui.rs` 只运行 parser/model 阶段即可失败的 fixtures，确保不会展开到 core symbol。

**错误与生命周期**

- 所有非法输入组合为 `syn::Error` 并定位 offending token；独立错误尽可能一次报告。
- Unknown/duplicate/removed option 不能忽略、fallback 或生成兼容 alias。
- Proc macro 无运行时资源、异步任务或 cleanup；测试生命周期只由 trybuild 子进程管理。

**UI、数据、数据库、图标、国际化与依赖**

- UI、业务数据、数据库/schema、网络、icons/assets、应用 i18n、平台代码：`No change`。
- i18n 属性只解析 provider type，不生成文案或 locale observer。
- 依赖：唯一新增项是 dev-only `trybuild 1.0.118`；`proc-macro2`/`quote`/`syn` 不变；同一 root
  lockfile 继续保留 DEP-00 exact Git source。

**测试**

| 要求 | 测试位置 | Fixture/测试名 | 可观察断言 |
| --- | --- | --- | --- |
| canonical parser | `src/attributes.rs` unit tests | `parses_canonical_model_and_field_attributes` | 任意合法顺序归一为同一 AST |
| duplicate 聚合 | `tests/ui/fail` | `duplicate_*` | `.stderr` 指向第二个 helper/option |
| type token 分类 | 同上 | `quoted_custom_type.rs` | 要求未加引号 `TypePath` |
| adapter 互斥 | 同上 | `invalid_garde_context.rs`、`invalid_custom_i18n.rs` | 说明 Garde/custom owner 边界 |
| 空 clause | 同上 | `empty_validation.rs`、`empty_transform.rs`、`empty_validate.rs` | 指出缺少 adapter/trigger |
| array grammar | 同上 | `invalid_array_id_syntax.rs`、`non_vec_array.rs` | 精确要求 `Vec<T>` 与 `id = "field"` |
| removed surface | 同上 | `removed_*_option.rs` | 指向 adapter/page 责任边界，不提供 alias |

**验证**

```bash
cargo test -p gpui-form-macros --locked
cargo tree -p gpui-form-macros --edges dev --locked
cargo tree -d --locked
git diff --check
```

**完成条件**

- Parser unit tests 与全部 `.stderr` fixture 通过，offending span 稳定。
- Dependency tree 只新增预期 trybuild dev chain，且仍只有一份 GPUI source。
- `Cargo.lock` 由 Cargo 更新并与 DEP-00 同属一份 root lockfile。

### MACRO-20：field enum、单一 runtime store 与完整 accessor

**前置**

- MACRO-10 完成；core `FormStore`、`FormRuntime`、`FormField` transaction 与 revision API 已通过
  FORM 验证。

**证据**

- 当前 `src/expand.rs` 将 store/field/schema/accessor 生命周期集中生成，并把 custom store 名错误
  传播到 field enum。
- 当前生成代码仍存在分散 value/baseline/revision/validation/submit 字段与 child-store 迁移表面；
  第 2.2、6.1 节冻结为唯一 `FormRuntime` 与 stateless typed lens。

**文件**

- 修改 `src/expand.rs`。
- 新增 `src/expand/field.rs`、`src/expand/store.rs`。
- 修改 `crates/gpui-form/tests/derive.rs`，由 core integration test 编译并运行生成代码。

**API 契约**

```rust,ignore
pub struct ModelFormStore<Generics> {
    runtime: gpui_form::__private::FormRuntime<
        Model<Generics>,
        ValidationContextType,
    >,
}
```

- Generated store 恰好一个字段；不得保存 adapter、transform、`SubmitRuntime` 或 child entity。
- Associated types 精确为 `Model`、`Output`、`Field`、`ValidationContext`、
  `ValidationAdapter`、`SubmitTransform`；`Output` 从 transform associated output 推导。
- Query 签名精确为 `validation_report() -> ValidationReport`、
  `errors_at(path) -> Vec<ValidationIssue>`，不暴露 runtime 引用。
- 固定生成 `*_field`、`*_in`、`*_item`、`*_item_in`；nested return 保留 `Root` store generic。

**实现流程**

1. 从 model 名生成 `ModelField`，从 default/override 独立生成 store ident。
2. 传播 visibility、lifetimes、type/const generics、合法 defaults 与 `where`；impl 按 Rust 规则移除
   type defaults。
3. 生成唯一 runtime field 和 core trait delegation；owned query 从 runtime 克隆 snapshot。
4. 按声明顺序生成 enum/schema/root leaf accessors，再生成 group/identified-array namespace
   accessors。
5. 所有 write closure 只投影 typed root model 并调用 core transaction；不得在 generated setter
   复制 invalidation、validation、event 或 notify 顺序。

**错误与生命周期**

- Unsupported input shape 在 derive target span 编译失败；不生成部分 store。
- Array read/write 必须找到恰好一个 stable ID；0/多项返回 `ValueUnavailable`，structural traversal
  产生 blocking issue，绝不选第一个。
- Accessor 是无订阅、无 entity、无业务值副本的 cheap handle；重复创建和多组件消费没有额外
  lifecycle。

**UI、数据、数据库、图标、国际化与依赖**

- UI/component/focus/subscription：宏不生成，`No change`。
- 数据：唯一 root typed model 在 runtime；不改变领域类型或持久化 schema。
- 数据库/schema、网络、icons/assets、应用 i18n、平台代码：`No change`。
- 依赖：只消费 DEP-00 lock 与 core public/doc-hidden macro support；不新增 crate/feature。

**测试**

| 要求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| 命名 | `gpui-form/tests/derive.rs` | `derive_generates_model_named_field_enum` | custom store model | store override 不改变 `ModelField` |
| 泛型 | 同上 | `derive_preserves_visibility_and_generics` | lifetime/type/const/default/where | declaration 与 impl 可编译 |
| 单 runtime | 同上 | `derived_store_contains_only_core_runtime_state` | default/custom adapter+transform | 行为证明 value/context/report 全委托；源码 residual 无实例字段 |
| owned query | 同上 | `derived_validation_queries_return_owned_values` | 取 report/errors 后再次 update form | owned snapshot 可继续持有且值不被后续借用限制 |
| schema 顺序 | 同上 | `schema_order_matches_declaration_order` | 多字段 model | enum/path/schema 顺序稳定 |
| typed access | 同上 | `field_accessors_preserve_rust_types` | String/u64/enum | 无 String codec/draft |
| nested API | 同上 | `nested_accessors_keep_root_store_type` | group + identified array | 四种 accessor 返回 `FormField<Root,T>` |
| stateless handle | 同上 | `repeated_accessors_are_stateless_handles` | 同字段多个 handle | 无 child entity/subscription/value copy |
| stable ID | 同上 | `identified_array_handles_survive_reorder` | reorder fixture | 始终定位 logical item |

**验证**

```bash
cargo test -p gpui-form --test derive --all-features --locked
cargo check -p gpui-form-macros --all-targets --locked
cargo tree -d --locked
git diff --check
```

**完成条件**

- 生成 API 与第 6.1 至 6.3 节、英文/中文 Guide 完全一致。
- Generated struct 只有一个 runtime；owned queries、泛型、四种 accessor 与 stable-ID failure 测试通过。
- 源码不存在 generated child form entity、draft field store、adapter/transform instance 或
  `SubmitRuntime`。

### MACRO-30：validation、Garde stable path 与 submit transform

**前置**

- MACRO-20 完成；core validation/transform target contract 与 optional `garde`/`validify` features
  已通过 FORM 验证。

**证据**

- 当前生成逻辑的 required/trigger、mount/context、Garde index path 与 preview/context transform
  仍属于旧契约。
- Core 已冻结 adapter-wide、field bucket、async 与 control issue 的独立 ownership；宏不能把它们
  合并为单一 report 或在 setter 中全部清空。

**文件**

- 新增 `src/expand/validation.rs`、`src/expand/transform.rs`。
- 修改 `src/expand.rs`。
- 修改 `crates/gpui-form/tests/validation.rs`、`crates/gpui-form/tests/submit.rs`。

**API 契约**

- `ValidationAdapter<Model>: Default + 'static`；generated store 只设置 associated type，每次 run
  临时调用 `Self::ValidationAdapter::default()`。
- `SubmitTransform<Model>: Default + 'static`；只有 associated `Output` 和
  `transform(&self, &Model)`，仅在 validation/pending 通过后临时 default 并调用一次。
- `from_value_with_validation_context` 安装唯一 runtime 后恰好一次 Mount/Form validation；
  `from_value` 只委托，不产生第二次 mount；context setter 不隐式验证。
- 成功 typed write 只清相交 required/structural/generated synchronous field buckets 与相交 async；
  保留 adapter-wide form bucket 和 active control issue，再执行 Change validation。
- `prepare_submit` 对一个 owned snapshot 完成 Submit validation、issue/pending gate 与一次 transform。

**实现流程**

1. 根据属性唯一解析 no-op/custom/Garde validation associated types 与 typed context。
2. 生成 mount/context/required/trigger/schema glue，所有运行都委托 core runtime；禁止 generated
   setter 自行清 report。
3. 生成 `GardePathMapper`：解析公开 display path，用本次 model bounds-check index，转换 stable ID，
   再递归 child schema。
4. 根据属性唯一解析 identity/custom/Validify transform associated type；从 associated `Output`
   推导 `FormStore::Output`。
5. `prepare_submit` 直接委托 core；不生成第二次 model read、preview/context、I/O 或 model 回写。

**错误与生命周期**

- Garde unknown field、malformed/out-of-bounds index、invalid/duplicate ID 返回 typed
  `GardePathError`，由 core 记录为 blocking internal issue；不 panic、不丢弃、不退回 index path。
- Transform failure 返回 `SubmitError::Transform`，不写 validation bucket、不改变 value/baseline/
  revision。
- Adapter/transform 是每次调用的临时值，无跨 run cleanup；runtime 依赖只能来自 validation context。
- Form-owned async task/generation 的取消与 stale completion 由 core 管理；宏只生成 schema/path，
  不持有 task。

**UI、数据、数据库、图标、国际化与依赖**

- UI/focus/component/subscription：`No change`；Blur 由具体 control 显式调用 core API。
- 数据：一个 typed model snapshot；无 draft、submit state 或 persistence side effect。
- 数据库/schema、网络、icons/assets、平台代码：`No change`。
- i18n：只选择 `GardeI18nProvider` associated type；不生成 Fluent key、翻译、locale observer，
  handler 不跨 await。
- 依赖：不新增 macro runtime dependency；只消费 core 的 feature-gated Garde/Validify 类型与 DEP-00
  lock。

**测试**

| 要求 | 测试文件 | 测试名 | Fixture | 断言 |
| --- | --- | --- | --- | --- |
| mount 一次 | `gpui-form/tests/validation.rs` | `constructors_run_mount_validation_once` | default/custom context recording adapter | 两条 constructor path 各一次 |
| context owner | 同上 | `context_replacement_does_not_validate_implicitly` | report + new context | 只 notify，显式 validate 后才替换 report |
| required submit | 同上 | `required_always_runs_on_submit` | 无 `on_submit` 空字段 | 仍产生 required issue |
| 精确失效 | 同上 | `typed_write_preserves_adapter_and_control_issue_buckets` | 全 bucket + pending fixture | 只清相交三类 sync/async；adapter/control 保留 |
| Change 顺序 | 同上 | `change_validation_runs_from_typed_write` | recording adapter/event | value/revision → invalidation → validate → event/notify |
| custom context | 同上 | `derive_uses_custom_adapter_associated_context` | implicit/explicit context | associated equality 与 runtime context 正确 |
| Garde i18n/path | 同上 | `garde_indices_map_to_current_stable_ids` | reorder + custom provider | issue 跟随 logical item；localized message 保留 |
| mapping failure | 同上 | `garde_mapping_failures_are_blocking` | 五类 path error | 每类转 internal blocking issue |
| one snapshot | `gpui-form/tests/submit.rs` | `prepare_submit_validates_and_transforms_one_snapshot` | recording adapter/transform | 二者观察同一值，transform 一次 |
| associated output | 同上 | `custom_transform_returns_associated_output` | `Output != Model` | 静态类型与值正确 |
| failure purity | 同上 | `transform_failure_does_not_mutate_form` | failing transform | value/baseline/revision/report 不变 |

**验证**

```bash
cargo test -p gpui-form --test validation --all-features --locked
cargo test -p gpui-form --test submit --all-features --locked
cargo test -p gpui-form-macros --locked
cargo clippy -p gpui-form-macros -p gpui-form --all-targets --all-features --locked -- -D warnings
cargo tree -d --locked
git diff --check
```

**完成条件**

- No-op/custom/Garde 与 identity/custom/Validify 组合通过 feature-aware tests。
- Store/runtime 不保存 adapter/transform instance；owned validation query 与精确 bucket invalidation
  保持 core 契约。
- 源码不存在 preview、transform context、`transform_on_submit`、第二份 submit state 或 lossy Garde
  path fallback。

### MACRO-40：删除旧表面、同步双语公开文档并交接

**前置**

- MACRO-10 至 MACRO-30 全部完成并通过各自 locked 验证。
- 本工作包完成后，精确 handoff 到 adapter `CORE-GATE`；Jaco 不在此时越过 adapter 直接迁移。

**证据**

- 第 2.1、3、7、8 节已经固定旧 parser、draft/component/focus、submit runtime 与 child-store
  surface 的删除列表。
- 英文 README/Guide 是默认入口；中文文件是语义镜像。两者代码/API 名必须逐项一致。

**文件**

- 清理 `crates/gpui-form-macros/src/**` 的 legacy branch/helper/export。
- 修改 `README.md`、`README.zh-CN.md`、`docs/guide.md`、`docs/guide.zh-CN.md`、
  `dev/form-store-derive.md`。

**API 契约**

- 最终只公开第 4 节属性 grammar 和第 6 节 generated API。
- README 保持 crate 介绍与最小完整示例；Guide 解释完整 grammar、单 runtime/associated type、owned
  query、nested traversal、validation/transform 与诊断。
- 中文文档只翻译叙述，所有 Rust snippet、类型、方法、attribute 与返回签名和英文一致。

**实现流程**

1. 删除所有无调用者 legacy parser field、expansion branch、helper/export；removed-option diagnostic 与
   对应 compile-fail fixture 保留。
2. 逐段比对英文/中文 README 与 Guide；默认链接指向英文，中文页面提供 reciprocal link。
3. 为英文 Guide 的公开 snippet 建立 compile/integration coverage；中文复用相同 API，不另造示例。
4. 运行 residual scan 并逐条分类。只有明确的 removed-option diagnostic、negative fixture 或解释删除
   边界的文档命中可保留。
5. downstream adapter/Jaco 全部迁移且 core `FORM-70` 的 workspace gate 通过后，统一把
   README/Guide 状态改为“已实现”；该状态同步已于 2026-07-21 完成。

**错误与生命周期**

- 删除 legacy API 后的编译失败必须由 targeted migration diagnostic 或 Rust missing-symbol error
  直接暴露；不增加 deprecated shim。
- 文档状态不得早于 core `FORM-70`；若 residual 无法分类，保持“尚未实现”并停止 handoff。
- 本工作包不创建运行时资源、异步任务或 persistence lifecycle。

**UI、数据、数据库、图标、国际化与依赖**

- UI、form data、数据库/schema、网络、icons/assets、平台代码：`No change`。
- i18n：只维护英文默认与中文镜像文档；不修改应用 locale bundle。
- 依赖：不再新增或更新 package，只使用 MACRO-10 后的同一 locked dependency graph。

**测试**

| 要求 | 测试/审计 | Fixture | 断言 |
| --- | --- | --- | --- |
| 英文 snippet | core integration tests | README/Guide models | 公开 API 编译且行为符合示例 |
| 中文镜像 | 文档 diff checklist | 两套 README/Guide | heading/代码块/API/链接一一对应 |
| removed syntax | trybuild | `removed_*_option.rs` | 仍给 targeted diagnostic |
| legacy residual | `rg` 分类 | macro src/tests/docs | 只剩 negative fixture/删除说明 |
| 单 runtime residual | `rg` 分类 | generated expansion | 无 adapter/transform/submit instance field |

**验证**

```bash
rg -n 'SubmitRuntime|transform_on_submit|preview|DraftFieldStore|FormFieldHandle|FocusHandle|touched|blurred|show_error' \
  crates/gpui-form-macros/src crates/gpui-form-macros/tests
rg -n 'validation_adapter\s*:|submit_transform\s*:|component|binding|codec|group\s*\(\s*store|array\s*\([^)]*store' \
  crates/gpui-form-macros/src crates/gpui-form-macros/tests
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form --all-features --locked
cargo tree -d --locked
git diff --check
```

**完成条件**

- 公开文档、generated API、fixtures 与源码只描述一个 target contract，双语镜像一致。
- 无 deprecated/compatibility surface；所有 residual 已逐条分类。
- 公开内容已与 target implementation 一致，状态仍明确标为尚未完成，并可以进入 adapter
  `CORE-GATE`；最终状态切换留给 core `FORM-70`。

## 10. 工作包依赖与落地顺序

```text
adapter DEP-00
  -> core FORM-10..60
  -> macro MACRO-10 -> MACRO-20 -> MACRO-30 -> MACRO-40
  -> adapter CORE-GATE -> CONTROL-10 -> CONTROL-20 + CONTROL-30
  -> Jaco JACO-FORM-10..60
  -> adapter CONTROL-40 -> CONTROL-50
  -> Jaco JACO-FORM-70
  -> core FORM-70
```

必须按上述跨计划顺序在同一 breaking migration 中落地。不得先删 legacy expansion 后长期留下下游
不可编译状态，不得保留运行时双轨，也不得让 Jaco 越过 adapter `CORE-GATE` 直接消费未完成 API。
Macro 工作包只等待 core `FORM-10..60`，不能等待最终 `FORM-70`；`FORM-70` 只在 Jaco 收口后执行。

## 11. 跨包验证

### 11.1 定向验证

MACRO-10..40 只执行各工作包列出的 macro/core 定向命令。下面的完整跨包矩阵由最后的 core
`FORM-70` 在 JACO-FORM-70 之后执行；它不是 MACRO-40 的前置或完成条件：

```bash
cargo fmt --all --check
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p jaco --all-features --locked
cargo clippy -p gpui-form-macros -p gpui-form -p gpui-form-gpui-component \
  --all-targets --all-features --locked -- -D warnings
cargo tree -p gpui-form-macros --edges dev --locked
cargo tree -d --locked
git diff --check
```

### 11.2 Workspace/CI gate

合入 `main` 前执行仓库 CI 等价基线：

```bash
cargo build --workspace --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo tree -d --locked
```

CI 覆盖 macOS、Linux、Windows。宏本身没有 UI/manual QA surface；实际控件交互由
`gpui-form-gpui-component` 与 Jaco 迁移计划验证，不能把 GUI 验证伪装成 macro 完成条件。

### 11.3 验收矩阵

| Contract | Parser/compile-fail | Core integration | Residual audit |
| --- | --- | --- | --- |
| canonical grammar/duplicate/mutual exclusion | 必须 | 不适用 | 无兼容 parser |
| naming/generics/visibility | 基础非法输入 | 必须 | 无 store/field naming 分叉 |
| 单 `FormRuntime` + associated adapter/transform | 属性类型错误 | 必须 | 无实例字段/第二 runtime |
| owned validation query | 不适用 | 必须 | 无 `&ValidationReport`/`Vec<&ValidationIssue>` |
| typed leaf/group/array access | shape 错误 | 必须 | 无 draft/child entity |
| constructor/context/mount | 属性组合错误 | 必须 | 无 conditional constructor |
| 精确 write invalidation | 不适用 | 必须 | 无全量清 adapter/control bucket |
| required/validation/Garde path | trigger/path 配置错误 | 必须 | 无 lossy mapping |
| associated transform output | adapter syntax 错误 | 必须 | 无 preview/context/SubmitRuntime |

## 12. 执行交接审计

- 已冻结 public naming、属性 grammar、内部语义模型、mutual exclusion、constructor、单 runtime、
  associated adapter/transform、owned query、validation invalidation、traversal 与删除列表；无待选方案。
- Target implementation、parser unit tests、runtime integration 与 locked trybuild compile-fail
  gate 均已完成；README/Guide 已切换为实现态公开文档。
- `trybuild = "1.0.118"` 是唯一新增依赖且仅为 dev dependency；manifest 与 DEP-00 产生的同一
  `Cargo.lock` 均由 Cargo 更新，没有第二份 lockfile 或手工编辑。
- Macro 不拥有 UI state、subscription、focus、请求生命周期或持久化，不需要数据库 migration、
  async shutdown 或平台权限方案。
- Runtime correctness 由 `gpui-form` integration tests 证明；parser compile-fail 由 trybuild 证明；
  两类测试不能互相替代。
- 如果实现发现 core contract 与第 2.2 节不一致，停止 expansion 工作，先更新并重新审阅英文/中文
  Guide 与 core/macro 实施计划，不增加临时适配层。
