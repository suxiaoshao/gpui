# `FormStore` derive breaking 重构实施计划

## 1. 状态与范围

- 文档位置：`crates/gpui-form-macros/dev/form-store-derive.md`。
- 关联分支：`codex/175-jaco-shortcut-temporary-window`。
- 关联 issue：无独立 issue；这是跨 crate form 基础设施迁移，不属于 issue #175 的产品需求。
- **当前状态：2026-07-22 重新打开的 expansion/runtime integration 门禁已经关闭。** 严格 parser、
  compile-fail harness、pure nested model lens、完整路径 schema resolver、array
  container/item/item-leaf mapper 与递归 bounds 已完成，并与 core `FORM-45` 原子通过定向及 workspace
  gate。公开契约以
  [`README.md`](../README.md) 和 [`docs/guide.md`](../docs/guide.md) 为准。
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
| `src/expand.rs`（2026-07-22 review） | generated `__write_*` 仍负责 commit/invalidation/change validation/event/notify，并用 root/group/array 的 `validate_change` 决定是否运行；`*_in` 最终委托该 ancestor writer | 只生成纯 `Model` lens；唯一 lifecycle 由 outer `FormField` transaction 负责，nested leaf/whole group/whole array/item 都无条件进入一次 Change scope |
| `src/expand.rs`（2026-07-22 review） | adapter issue 通过 root field prefix 取 schema；`auth.username` 与 `rows[#id].name` 因而使用 ancestor trigger | 生成递归 `FormModelSchema`；core 对每个完整 stable path 先解析精确 owner，再按 scope/trigger 过滤 |
| `src/expand.rs` array mapper（2026-07-22 review） | 只解析 `rows[index]`，没有 `path == "rows"`；empty suffix 的 item root 虽可映射，却没有明确 trigger owner | 完整映射 `rows`/`rows[index]`/`rows[index].leaf`；direct item root 明确由直接所属 array schema 控制 |

### 2.2 目标 core 契约

宏生成代码只依赖下列 target contract，不自行发明替代 runtime：

```rust,ignore
pub trait FormStore: EventEmitter<FormEvent<Self::Field>> + Sized + 'static {
    type Model:
        Clone + PartialEq + StructuralValidate + FormModelSchema + 'static;
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
    #[doc(hidden)]
    fn __validate_snapshot(
        &mut self,
        snapshot: &Self::Model,
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

Core 还固定以下写入语义，宏只提供纯 `Model` projection、完整路径 schema resolver 与 Garde path
mapper：equal leaf write 是 no-op；whole-form
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
   `FormField` transaction clone candidate、通过纯 model lens 投影并保存 typed value、推进 revision；只清除相交的 required、
   structural 与 generated synchronous field bucket；取消并清除相交 async validation；保留
   adapter-wide form bucket 与所有 active control issue；随后无条件运行一次 Change scope，最后发出一个
   typed event 并 notify 一次。宏只生成读写 projection、path 与 schema，不能为每个字段复制
   lifecycle，也不能根据 root/ancestor schema 提前 gate Change validation。
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
    `*_field`、`*_in`、`*_item`、`*_item_in`，所有 nested handle 保留 root store type。Raw read/write
    closure 只接收 `&Model`/`&mut Model`，不能接收 generated store、runtime 或 `Context`。
12. **Stable ID**：`array(id = "row_id")` 只接受 `Vec<Item>`，其中 Item 必须是编译器可直接做字段
    访问的名义类型 path；允许 `Row<T>` 这类字段布局已知的泛型名义类型，但拒绝裸 model type
    parameter `T`、`T::Item`/`<T as Trait>::Item` 等无法表达命名字段约束的 item。ID 通过直接字段
    access + `ToFormItemId` 转换；不为支持裸泛型另增 identity trait 或第二套 accessor grammar。
    missing、duplicate 或不可转换 ID 是 blocking structural issue；访问器不猜测第一个 item。
    identified-item lens 写回前再次确认 candidate ID 等于捕获 ID，因此整项替换和 ID leaf mutation
    都返回 `ItemIdentityChanged` 且完整 no-op。Runtime 不保存 retired-ID history；在同一 form
    session 内 `(array path, stable ID)` 是名义 identity，whole-array/reset/rebase 中相同 ID 表示
    同一 logical item，新的 logical item 必须由 caller/model 分配新 ID。
13. **Garde path**：使用 Garde 公开 `Path::Display` 字符串，不依赖 doc-hidden iterator；vector
    index 必须按本次 validated model 转成 stable item ID。unknown/out-of-bounds/duplicate/invalid
    path 返回 typed `GardePathError`，不能 lossy fallback。Array mapper 必须区分 container、direct
    item root 与 item descendant，不能只实现带 index 的分支。
14. **完整路径 schema**：derive 为每个 model 生成 doc-hidden `FormModelSchema`。Group 递归 child；
    array 用当前 model 确认 stable ID 唯一。`rows` 与 direct `rows[#id]` 由 `rows` schema 控制，
    `rows[#id].leaf` 递归 leaf schema；嵌套 item root 只归最近直接 array。Macro 不生成 root-prefix
    filter，Garde mapper 与 schema resolver 是两个独立阶段。
15. **测试契约**：parser unit tests 验证 AST；`trybuild` 固定 compile-fail 诊断；
    `gpui-form` integration tests 验证生成 API 与 runtime。compile-fail 测试不能由 doctest 代替。
16. **删除策略**：删除 generated `SubmitRuntime` 和旧 draft/component/codec/focus/touched/blurred/
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
      schema.rs
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
一次解析、供全部 expansion 共享的语义模型；`expand.rs` 只编排五个 generation module。

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

Root accessor 交给 `FormField` 的 closure 必须是纯 model lens，概念签名固定为：

```rust,ignore
read: Fn(&Model) -> Option<FieldType>;
write: Fn(&mut Model, FieldType) -> Result<(), FormFieldError>;
```

它只能读取/修改传入的 candidate model，不能接收 generated store、`FormRuntime`、revision、
validation state 或 `Context<Store>`，也不能 commit、emit 或 notify。`*_in` 只组合 parent model
lens；`*_item`/`*_item_in` 使用 core identified-item lens。后者在 write-back 前检查 replacement ID
仍等于捕获 ID，因此继续组合出来的 ID leaf accessor 也不能改变 identity。

### 6.4 Validation 与 Garde mapping

- static schema 顺序等于字段声明顺序。
- leaf trigger 只属于该 leaf；group/array ancestor 不复制 descendant trigger。
- derive 为 model 生成 `FormModelSchema::schema_at_path`，逐 segment 匹配完整 stable path：group
  递归 child；array 在当前 model 中确认 item ID 有效且唯一，再处理 item root 或递归 item child。
- `#[form(group)]` child 必须生成 `StructuralValidate + FormModelSchema` where-bound；启用 Garde
  mapper 时再要求 `GardePathMapper`。`#[form(array)]` 的具体/名义 Item 生成相同递归 bounds；ID
  field type 无法由 parent derive 写成独立 where predicate，因此 generated direct field access 使用
  field span 并通过 `ToFormItemId::to_form_item_id(&item.row_id)` 在类型检查期强制。Bare generic/
  associated item 在 derive model 阶段直接拒绝。不得引入额外 identity trait 或依赖未声明的能力。
- exact `rows` 与 direct `rows[#id]` 使用 `rows` array schema；`rows[#id].name` 使用 `name` leaf
  schema。递归必须逐级闭合：`settings.rows[#rid](.name)`、
  `sections[#sid].auth(.username)`、nested container `sections[#sid].rows` 以及
  `sections[#sid].rows[#rid](.name)` 都解析到各自 exact container/item-root/leaf；nested item root
  使用直接所属 `rows` schema。该规则不得实现成任意 ancestor prefix，item 后仍有字段时必须递归。
- `required` 无条件加入 submit rule，并可按 `validate(...)` 在更早 trigger 运行。
- custom adapter 从同一 owned model snapshot 接收 trigger、scope 与 typed context。
- 每次 adapter run 临时构造 `Self::ValidationAdapter::default()`；runtime 不保存 adapter instance。
- 成功 typed write 的失效集合只包括与 path 相交的 required/structural/generated synchronous field
  bucket 和 async entry；adapter-wide form bucket 与 active control issue 必须保留。宏不得生成
  `validate_change` gate；每个 non-equal write 是否参与 Change 由 core transaction 统一发起。
- Garde adapter context 固定为 `<Model as garde::Validate>::Context`；macro 只选择 adapter/provider
  type，不调用或复制 Garde validation runtime。
- generated `GardePathMapper` 只负责把公开 `Path::Display` 转成 stable `FieldPath`：array 必须先处理
  exact container `path == "rows"`，再处理 `rows[index]`；empty suffix 返回 direct item root，带
  `.child` 的 suffix 递归 item 的 mapper。相同算法必须递归覆盖 group 内 array、array item 内
  group/nested array 和双层 index；每个 index 都使用本次 snapshot 对应层级的当前 model 转成 stable
  ID。它不读取或过滤 schema trigger。
- core 收到 mapped/custom adapter issue 后，先调用本次 model 的完整 schema resolver，再按 scope
  与精确 owner trigger 规范化；macro 不生成 `RootField::ALL + starts_with` filter closure。
- path mapping failure 转为 typed `GardePathError`，core runtime 将其记录为 blocking internal
  issue；不能丢弃错误或附着到错误的 index path。
- schema resolution failure 由 core 转成 `form_schema_path_resolution` blocking internal issue；合法
  direct item root 不能走该失败路径。

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
| `array` item 是裸 type parameter/associated type | item type | 说明命名 ID field 无法由该类型表达，要求具体/名义 item path |
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

**状态**

- `[历史基线已完成]`。本工作包只拥有命名、单 runtime、associated types、typed accessor 形状与
  stable-ID lookup/reorder 基线。当前 expansion 仍生成带 lifecycle 的 writer，identified write-back
  仍未保护 identity；pure candidate lens、唯一 core transaction 与 `ItemIdentityChanged` 统一由
  MACRO-35/FORM-45 修正，本工作包不得作为这些行为的完成证据。

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
- 历史 accessor 已提供 typed read/write surface；raw writer 当前仍接收 generated store/context 并
  复制 commit/validation/event/notify lifecycle。删除这些能力并改成纯 `Model` candidate lens 只归
  MACRO-35。

**实现流程**

1. 从 model 名生成 `ModelField`，从 default/override 独立生成 store ident。
2. 传播 visibility、lifetimes、type/const generics、合法 defaults 与 `where`；impl 按 Rust 规则移除
   type defaults。
3. 生成唯一 runtime field 和 core trait delegation；owned query 从 runtime 克隆 snapshot。
4. 按声明顺序生成 enum/schema/root leaf accessors，再生成 group/identified-array namespace
   accessors。
5. 生成 root/group/identified-array accessor 的历史接线；generated setter 中的 commit、
   invalidation、trigger gate、event/notify 删除与 pure model lens 迁移由 MACRO-35 独占。

**错误与生命周期**

- Unsupported input shape 在 derive target span 编译失败；不生成部分 store。
- Array read/write 必须找到恰好一个 stable ID；0/多项返回 `ValueUnavailable`，structural traversal
  产生 blocking issue，绝不选第一个。
- 本历史包只保证 lookup 恰好一个 item；identified write-back 的 replacement stable-ID check、
  declared ID getter 传递与 `ItemIdentityChanged` complete no-op 由 MACRO-35/FORM-45 实现。
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
- Generated struct 只有一个 runtime；owned queries、泛型、四种 accessor 与 lookup/reorder failure
  测试通过。
- Pure generated lens、无 writer lifecycle 与 immutable identified path 只能由 MACRO-35 的 residual
  scan 和 integration tests 关闭。
- 源码不存在 generated child form entity、draft field store、adapter/transform instance 或
  `SubmitRuntime`。

### MACRO-30：validation、Garde stable path 与 submit transform

**状态**

- `[历史基线已完成]`。本工作包记录 2026-07-21 已落地的 validation/transform、indexed Garde
  stable-path 与 structural traversal；完整 `FormModelSchema`、array container/direct-item-root
  mapping、nested trigger normalization 和递归 bounds 统一由 MACRO-35/FORM-45 纠偏。本工作包不得
  再作为这些新增行为的实现 owner 或验收证据。

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
  保留 adapter-wide form bucket 和 active control issue，再由 core 无条件执行一次 Change scope；宏不
  按 root/ancestor schema gate。
- 历史 expansion 保留 root schema/trigger 与 structural traversal 基线；完整 `FormModelSchema` 和
  core exact-owner normalization 由 MACRO-35 替换现有 prefix filter。
- `prepare_submit` 对一个 owned snapshot 完成 Submit validation、issue/pending gate 与一次 transform。

**实现流程**

1. 根据属性唯一解析 no-op/custom/Garde validation associated types 与 typed context。
2. 生成 mount/context/required/trigger/schema glue，所有运行都委托 core runtime；禁止 generated
   setter 自行清 report或决定是否运行 Change。
3. 生成历史 `GardePathMapper` indexed stable-path 基线；array container、direct item root 与 item
   leaf 三形态的完整映射和 mapper/schema 分层由 MACRO-35 完成。
4. 生成历史 root field schema 与 structural traversal；递归 `FormModelSchema`、child/item bounds 和
   `resolver -> scope -> trigger` normalization 由 MACRO-35 完成。
5. 根据属性唯一解析 identity/custom/Validify transform associated type；从 associated `Output`
   推导 `FormStore::Output`。
6. `prepare_submit` 直接委托 core；不生成第二次 model read、preview/context、I/O 或 model 回写。

**错误与生命周期**

- Garde unknown field、malformed/out-of-bounds index、invalid/duplicate ID 返回 typed
  `GardePathError`，由 core 记录为 blocking internal issue；不 panic、不丢弃、不退回 index path。
- 已映射但无法按 model schema 解析的路径由 core 转 blocking internal issue；合法 array container 与
  direct item root 必须成功，不能被误分类。
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

- 历史 No-op/custom/Garde indexed mapping 与 identity/custom/Validify 组合通过 feature-aware tests；
  nested schema 和 array 三形态 mapper 只能由 MACRO-35 新增测试关闭。
- Store/runtime 不保存 adapter/transform instance；owned validation query 与精确 bucket invalidation
  保持 core 契约。
- 源码不存在 preview、transform context、`transform_on_submit`、第二份 submit state 或 lossy Garde
  path fallback。

### MACRO-35：纠正 nested lens、完整路径 schema 与 array Garde mapping

**状态**

- `[已完成]`。2026-07-22 已与 core `FORM-45` 在同一 changeset 完成。Generated root/nested accessor
  只组合 pure model lens，不再生成 writer lifecycle；递归 `FormModelSchema` 与 Garde mapper 完整覆盖
  group、array container、direct item root、item leaf 和交叉嵌套，stable-ID mutation 与不满足递归
  bounds 的类型在正确字段 span 被拒绝。
- 验证证据：macro unit/trybuild、core corrective integration tests、generated-writer/root-prefix residual
  scan 和 workspace build/test/clippy 全部通过。

**前置**

- MACRO-20/MACRO-30 的单 runtime、canonical parser、structural traversal、Garde adapter 与 transform
  基础保持可用。
- Core `FORM-45` 先提供纯 model lens constructor、`FormModelSchema`/`FormSchemaPathError`、
  `ItemIdentityChanged` 和不发事件的 validation pass；macro 不复制这些实现作临时过渡。
- 英文/中文 Guide 已冻结 core owns transaction、nested leaf owns trigger、stable ID immutable；本
  工作包补充 array direct-item-root 的明确 owner 后，由 MACRO-40 首次同步公开措辞，FORM-70 只做
  最终一致性审计与状态翻转。

**证据**

- 当前 generated `__write_*` 同时 commit、invalidate、按声明字段的 `validate_change` gate、emit 与
  notify；`ChildFormStore::leaf_in(parent)` 最终调用 ancestor writer，leaf `on_change` 会被绕过。
- 当前 adapter filter 只迭代 root field 并用 `path.starts_with(root_path)` 读取 schema；nested
  group/array/item leaf 使用 ancestor trigger，Submit/Blur/Change 都可能丢失合法 issue。
- 当前 array mapper 没有 `path == array_name` 分支，合法 container rule 变成 `UnknownField`；Garde
  Vec + item 类型级 custom rule 又会合法产生 `rows[index]`，不能把其 stable item root 当非法路径。
- 当前 `identified_item` slot replacement 未核对 replacement ID；generated ID leaf accessor 可改变
  handle 的寻址身份。

**文件**

- 修改：`src/model.rs`、`src/expand.rs`。
- 按第 5 节目标结构新增/修改：`src/expand/field.rs`、`src/expand/schema.rs`、
  `src/expand/validation.rs`；禁止新增 `mod.rs`。
- 修改：`crates/gpui-form/tests/derive.rs`、`crates/gpui-form/tests/form_store.rs`、
  `crates/gpui-form/tests/validation.rs`、`crates/gpui-form/tests/submit.rs`。
- 原子配套由 core `FORM-45` 修改 `crates/gpui-form/Cargo.toml`，并新增
  `crates/gpui-form/tests/ui.rs` 与 recursive-bound `.rs/.stderr` fixtures；可引用 core trait 的
  compile-fail contract 放在 core crate，macro crate 自己的 trybuild 只覆盖 parser/model 阶段错误。

**API 契约**

- Root accessor 只生成
  `Fn(&Model) -> Option<T>` / `Fn(&mut Model, T) -> Result<(), FormFieldError>`；nested accessor 只组合
  lens。Generated closure 无法访问 store/runtime/context，因而无法执行 lifecycle。
- 每个 derived model 实现 doc-hidden `FormModelSchema`。完整 path owner 矩阵固定为：
  `auth -> auth`、`auth.username -> username`、`rows -> rows`、`rows[#id] -> rows`、
  `rows[#id].name -> name`、`settings.rows[#rid](.name)`、`sections[#sid].auth(.username)`、
  `sections[#sid].rows -> rows`、`sections[#sid].rows[#rid](.name)`；交叉嵌套递归，nested container
  使用自己声明的 array schema，nested item root 只使用最近直接 array schema，nested leaf 使用
  item model 声明的 leaf schema。
- 对 `#[form(group)]` child 生成 `StructuralValidate + FormModelSchema` bound，对
  `#[form(array)]` 的具体/名义 item 生成相同递归 bound；启用 Garde mapper 时两者再要求
  `GardePathMapper`。Generic group child 显式进入 generated where-clause；array 允许 `Row<T>` 等
  名义 path，但 model 阶段拒绝裸 `T`/associated item。ID field 的 `ToFormItemId` 由 field-spanned
  generated direct access 强制，因为 parent derive 无法命名该字段类型。
- `GardePathMapper` 只做 display path 到 stable path：container、item root、item leaf 三种形态都必须
  支持；内建 Garde adapter 返回完整 mapped report，不预先按 scope/trigger 丢 issue；
  schema/scope/trigger normalization 完全由 core 负责。
- Macro 只把 declared ID getter 传给 core identified-item lens；整项与 ID leaf mutation 都由同一
  identity check 拒绝，不生成第二套保护。Macro 不生成 identity history；同一 stable ID 始终表示
  同一名义 item，“新 logical item 使用新 ID”是 caller/model 契约。
- `FormStore`、accessor、validation、submit 的既有 public 签名不因本工作包增加兼容 overload。

**实现流程**

1. 把 root accessor 生成改为纯 candidate-model read/write；删除 generated `__write_*` 中的
   commit、runtime invalidation、`validate_change` 条件、event 与 notify，所有调用统一进入 outer
   `FormField` transaction。
2. 保持 `*_in` 通过 `project` 组合、`*_item`/`*_item_in` 通过 `identified_item` 组合；确认 ID leaf
   write 会回到 item lens 的 immutable-ID check，而不是直接写 vector slot。
3. Derive model 先拒绝 array 的裸 generic/associated item，保留具体/名义 path；不增加 identity
   trait。随后在独立 schema expansion 中按字段声明生成递归 matcher。Group exact 结束返回 group schema，
   有 child 则递归；array exact 结束返回 array schema，`Item(id)` 先验证当前 model 中恰好一个匹配，
   direct item root 返回 array schema，有 child 则递归 item model。Whole-array 中相同 ID 被解释为同一
   名义 item；macro 不生成 retired-ID bookkeeping。
4. Schema matcher 对 unknown field、意外 segment、missing/duplicate item、Projection、空 path 和
   trailing segment 返回结构化 `FormSchemaPathError`；不得 panic、选择第一个 match 或退回
   root/最近 ancestor。Stable path 已经是 `FormItemId`，不可转换 model ID 只归 structural
   validation，Garde index 指向它时归 mapper 的 `InvalidItemId`，不生成不可达 resolver variant。
5. 在 Garde mapper 的 array 分支先生成 `path == name`；再解析 `name[index]`，empty suffix 返回
   stable item root，`.child` suffix 只递归 child mapper。映射阶段禁止查询 trigger 或调用 schema
   resolver。
6. 删除 `RootField::ALL + starts_with` adapter filter closure。Generated adapter 只返回 stable report；
   core 按 `resolver -> scope -> exact trigger` 规范化，path failure 转 blocking internal issue。
7. 运行完整 group/array matrix 和 residual scan；若仍存在 generated lifecycle 或 prefix filter，不得
   用补一个特例分支关闭 review comment。

**错误与生命周期**

- Lens 返回错误发生在 cloned candidate 上；missing/duplicate item 为 `ValueUnavailable`，ID
  invalid/different mutation 为 `ItemIdentityChanged`，两者都不改变 model/revision/issues/tasks/
  event/notify。
- Garde mapping error 与 stable schema resolution error 是两个 typed boundary；前者使用
  `garde_path_mapping`，后者使用 `form_schema_path_resolution`，不得互相吞掉或重复记录原 issue；
  两类 `ValidationSource::Internal` issue 无条件保留，不接受 scope/trigger 过滤。
- 每个 non-equal nested write 只有一个 revision、一次 hidden Change pass、一个 `FieldChanged` 和一次
  notify；equal/error write 完整 no-op，不产生额外 `RuntimeChanged`。

**UI、数据、数据库、图标、国际化与依赖**

- UI/component/focus/subscription、数据库/schema、网络、icons/assets、应用 locale、平台：
  `No change`。
- 数据：不改变 model/persistence shape；只收紧 generated lens、stable identity 与 validation
  metadata traversal。
- 依赖：macro crate 不新增 crate/feature；core `FORM-45` 给 `gpui-form` 增加已在同一 lockfile 中
  锁定的 dev-only `trybuild 1.0.118`，不新增 runtime dependency，继续使用既有 Garde 0.23.0 与
  同一 GPUI identity。

**测试**

| 要求 | 测试文件 | 测试名 | 断言 |
| --- | --- | --- | --- |
| nested group write | `gpui-form/tests/form_store.rs` | `nested_group_leaf_write_uses_leaf_transaction` | parent 无 Change、leaf 有；commit/validate/event/notify 各一次 |
| whole group write | 同上 | `generated_whole_group_write_uses_one_scoped_transaction` | exact group、subtree、ancestors 参与，sibling group 不参与 |
| root/whole array writes | 同上 | `whole_array_and_item_writes_validate_selected_subtrees` | root array、item root 与 leaf 分别覆盖正确 subtree/ancestors，不运行 sibling |
| group/array cross nesting | 同上 | `generated_lenses_preserve_complete_cross_nested_paths` | array-in-group、group-in-array、nested array 的 path/validation_path/leaf trigger 均正确 |
| cross-nested resolver matrix | `gpui-form/tests/validation.rs` | `generated_cross_nested_paths_resolve_every_level` | `settings.rows[#rid](.name)`、`sections[#sid].auth(.username)`、`sections[#sid].rows[#rid](.name)` 的每一级 schema owner 都有断言 |
| immutable stable ID | `gpui-form/tests/form_store.rs` | `generated_item_and_id_leaf_writes_cannot_change_identity` | 不同有效 ID 与不可转换 ID 两组 fixture；整项/ID leaf 都返回 ItemIdentityChanged 且完整 no-op |
| whole-array identity | 同上 | `generated_whole_array_id_change_is_remove_and_insert` | `a -> b` 后旧 handle 不可用、新 handle 可读、removed descendant state 清除/取消；重复/不可转换 ID 阻止提交 |
| stable ID nominal contract | 同上 | `generated_same_id_remains_the_same_nominal_item` | whole-array/reset/rebase 保留 ID，或 remove 后重新出现同一 ID 时，原 handle 都按同一名义 item 读取；无 runtime identity history，caller 不得把它当新 item |
| whole-array reorder | 同上 | `generated_identified_handle_survives_whole_array_reorder` | 保留 ID 重排后既有 handle 继续读取同一 logical item，不退回 index identity |
| reset restores baseline ID | 同上 | `generated_reset_restores_removed_baseline_item_handle` | 暂时移除 baseline ID 后 handle 不可用；reset 后同一 handle 恢复可读并保持名义 identity |
| rebase/CAS ID sets | 同上 | `generated_rebase_and_cas_replace_item_sets` | 成功 rebase/rebase_if_revision 后旧 handle 不可用、新 handle 可读、旧后代状态清除/取消；失败 CAS 完整 no-op |
| exact schema matrix | `gpui-form/tests/validation.rs` | `generated_schema_resolver_covers_group_array_item_and_leaf` | container/item root/item leaf/交叉嵌套分别命中明确 owner |
| nested exact containers | 同上 | `generated_schema_resolver_covers_nested_group_and_array_containers` | `settings.auth`、`settings.rows`、`sections[#sid].auth`、`sections[#sid].rows` 命中各自声明 schema |
| item-root scope | 同上 | `generated_item_leaf_scope_includes_own_item_and_array_only` | item leaf run 保留同 item-root + owning array，排除 sibling item-root/leaf |
| recursive bounds expansion | `src/expand/schema.rs` unit tests | `recursive_schema_bounds_are_generated_at_field_spans` | generic group 与具体/名义 array item 含 StructuralValidate/FormModelSchema；Garde expansion 另含 GardePathMapper；ID access 使用 field span + UFCS |
| recursive bounds integration | `gpui-form/tests/derive.rs` | `generic_group_and_nominal_array_items_resolve_nested_schema` | generic group 与 `Row<T>` 名义 array item 的 derive、访问和 schema resolution 均编译运行 |
| recursive bounds compile-fail | `gpui-form/tests/ui/fail` | `recursive_*_bounds.rs` / `array_id_to_form_item_id.rs` | core trybuild 固定缺少 StructuralValidate/FormModelSchema/GardePathMapper/ToFormItemId 的字段 span 诊断 |
| unsupported generic array | `tests/ui/fail` | `generic_array_item.rs` / `associated_array_item.rs` | 裸 type parameter/associated item 在 derive model 阶段给出字段定位诊断，不展开到 core symbol |
| 五种 nested trigger | `gpui-form/tests/validation.rs` | `nested_adapter_issues_use_leaf_schema_for_all_triggers` | Mount/Change/Blur/Dynamic/Submit 不使用 root prefix |
| Garde array container | 同上 | `garde_array_container_rule_maps_without_index` | `rows` 不再 UnknownField，使用 rows schema |
| nested Garde array container | 同上 | `garde_nested_array_container_maps_to_exact_schema` | `sections[index].rows -> sections[#sid].rows`，使用内层 rows schema，不回退 sections |
| cross-nested Garde paths | 同上 | `garde_cross_nested_paths_map_every_level` | `settings.rows[index](.name)`、`sections[index].auth.username`、`sections[index].rows[index](.name)` 都映射完整 stable path |
| double-index reorder | 同上 | `garde_nested_reorder_maps_outer_and_inner_stable_ids` | outer/inner 同时重排后两个 index 都映射当前 snapshot 的正确 logical item/leaf |
| Garde item root | 同上 | `garde_item_custom_rule_maps_to_direct_array_schema` | `rows[index] -> rows[#id]`，最近 direct array owner |
| reorder/item leaf | 同上 | `garde_reordered_item_leaf_uses_stable_id_and_leaf_schema` | reorder 后映射 logical item，leaf trigger 不继承 array |
| nested item roots | 同上 | `nested_item_roots_use_their_direct_array_owners` | outer/direct/nested item root 各自使用最近 array schema，不扩散父/兄弟 item |
| nested structural IDs | 同上 | `generated_nested_array_structural_issues_use_exact_paths` | group 内与 item 内 nested array 的 duplicate/unconvertible ID 落在准确内层 container，blocking 且不污染 sibling |
| mapping failure scope | 同上 | `garde_mapping_failure_survives_field_scope` | malformed/out-of-bounds/invalid ID 在 Field scope 仍保留为 blocking Internal |
| nested submit | `gpui-form/tests/submit.rs` | `nested_submit_issue_blocks_before_transform` | root/ancestor 无 Submit、leaf 有 Submit 时仍拒绝 output |
| one transaction | `gpui-form/tests/form_store.rs` | `generated_nested_write_has_one_core_transaction` | 无 ancestor transaction、无双 event/notify；equal write no-op |

**验证**

```bash
cargo fmt --all --check
cargo test -p gpui-form --all-features --locked --test form_store
cargo test -p gpui-form --all-features --locked --test validation
cargo test -p gpui-form --all-features --locked --test derive
cargo test -p gpui-form --all-features --locked --test submit
cargo test -p gpui-form --all-features --locked --test ui
cargo test -p gpui-form-macros --locked
cargo clippy -p gpui-form -p gpui-form-macros --all-targets --all-features --locked -- -D warnings
rg -n '__write_|validate_change|RootField::ALL|starts_with\(' crates/gpui-form-macros/src/expand.rs crates/gpui-form-macros/src/expand
git diff --check
```

**完成条件**

- group、array container、direct item root、item leaf、交叉嵌套与 nested array 都由完整 path 的明确
  schema owner 控制；不存在 root-prefix 或未声明 ancestor fallback。
- Generated accessor 只生成纯 model lens；identified stable ID 不可改变；每次 nested write 只经过
  一个 core transaction。
- Garde container/item/item-leaf 映射和 resolver 分层有端到端测试；上述 residual scan 在 active
  expansion 中无命中，core `FORM-45` 同时完成后才可解决两条 review comment。
- Generic group、nominal array item、Garde mapper 与 ID field 的递归 bounds 同时有成功 integration
  与字段 span compile-fail 证据；macro parser fixture 不承担无法引用 core contract 的验证。

### MACRO-40：删除旧表面、同步双语公开文档并交接

**状态**

- `[已完成]`。既有 legacy cleanup 保持完成；direct-item-root ownership、stable identity error 与新
  递归 bounds 已同步到英文默认 README/Guide 及中文镜像，core `FORM-70` 已完成最终本地一致性与
  workspace gate。

**前置**

- MACRO-10 至 MACRO-30 的既有迁移已完成；2026-07-22 corrective `MACRO-35` 与 core `FORM-45`
  必须原子完成并通过各自 locked 验证。
- 历史 adapter `CORE-GATE`、后续 control/Jaco 迁移已经完成并保留其证据；本 corrective 工作包
  完成后直接把新增证据交给 core `FORM-70`，不重跑 adapter/Jaco 迁移。

**证据**

- 第 2.1、3、7、8 节已经固定旧 parser、draft/component/focus、submit runtime 与 child-store
  surface 的删除列表。
- 英文 README/Guide 是默认入口；中文文件是语义镜像。两者代码/API 名必须逐项一致。

**文件**

- `[历史基线已完成，不在 corrective MACRO-40 修改]`：
  `crates/gpui-form-macros/src/**` 的 legacy branch/helper/export 清理。
- 修改 `README.md`、`README.zh-CN.md`、`docs/guide.md`、`docs/guide.zh-CN.md`、
  `dev/form-store-derive.md`。

**API 契约**

- 最终只公开第 4 节属性 grammar 和第 6 节 generated API。
- README 保持 crate 介绍与最小完整示例；Guide 解释完整 grammar、单 runtime/associated type、owned
  query、nested traversal、validation/transform 与诊断。
- 中文文档只翻译叙述，所有 Rust snippet、类型、方法、attribute 与返回签名和英文一致。

**实现流程**

1. 先只读复核历史 legacy residual gate；removed-option diagnostic 与对应 negative fixture 保留。
   若 MACRO-35 引入新的 active-source legacy 命中，返回 MACRO-35 修复，MACRO-40 不接管源码清理。
2. 逐段比对英文/中文 README 与 Guide；默认链接指向英文，中文页面提供 reciprocal link。Guide
   明确 array container/direct-item-root/item-leaf schema ownership、stable ID mutation error 与
   Garde 三种 path，并明确“不复用”是 caller/model 的会话期名义 identity 契约、runtime 不保存
   ID history；README 只保留项目介绍和最短示例。
3. 为英文 Guide 的公开 snippet 建立 compile/integration coverage；中文复用相同 API，不另造示例。
4. 运行 residual scan 并逐条分类。只有明确的 removed-option diagnostic、negative fixture 或解释删除
   边界的文档命中可保留。
5. corrective FORM-45/MACRO-35 通过后完成文档纠偏并把证据交给 FORM-70；README/Guide 和本计划
   在这个中间节点仍保持“尚未完成”。FORM-70 独立复核既有 downstream 证据与 workspace gate 后
   统一翻转最终状态；该翻转已于 2026-07-22 完成，2026-07-21 的旧状态证据没有被用于关闭新门禁。

**错误与生命周期**

- 删除 legacy API 后的编译失败必须由 targeted migration diagnostic 或 Rust missing-symbol error
  直接暴露；不增加 deprecated shim。
- 文档状态不得早于 core `FORM-70`；本次在 FORM-70 gate 通过后才翻转。若后续 residual 无法分类，
  应恢复“尚未实现”并停止 handoff。
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
- 公开内容已与 target implementation 一致；新增证据已交给并通过 core `FORM-70`，最终状态已经
  翻转，不重跑已完成的 adapter `CORE-GATE`、control 或 Jaco 工作包。

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

当前 corrective gate 在既有迁移链之后原子执行：

```text
core FORM-45 <-> macro MACRO-35
  -> macro MACRO-40（只同步纠偏文档；在此中间节点保持未完成状态）
  -> 重跑 core FORM-70（已完成最终状态翻转）
```

它不要求重做 adapter/Jaco 迁移，但旧的 MACRO-40/FORM-70 通过记录不能替代新增 nested
group/array tests。这里的 MACRO-40 是 corrective 文档纠偏，不再沿用历史主链中的
`MACRO-40 -> CORE-GATE` handoff；它只把新增 evidence 交给最终 FORM-70。

## 11. 跨包验证

### 11.1 定向验证

MACRO-10..40 与 corrective MACRO-35 只执行各工作包列出的 macro/core 定向命令。下面的完整跨包矩阵由最后的 core
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
| pure nested model lens + single transaction | 不适用 | 必须 | 无 generated `__write_*` lifecycle/ancestor trigger gate |
| complete schema owner resolution | shape/ID 字段错误 | 必须 | 无 root-prefix/最近 ancestor fallback |
| array container/item-root/item-leaf mapping | array 配置错误 | 必须 | container 不丢失；item root 只归 direct array；leaf 递归 |
| immutable identified-item ID | ID 字段类型错误 | 必须 | 整项/ID leaf mutation 均无法 commit |
| constructor/context/mount | 属性组合错误 | 必须 | 无 conditional constructor |
| 精确 write invalidation | 不适用 | 必须 | 无全量清 adapter/control bucket |
| required/validation/Garde path | trigger/path 配置错误 | 必须 | 无 lossy mapping |
| associated transform output | adapter syntax 错误 | 必须 | 无 preview/context/SubmitRuntime |

## 12. 执行交接审计

- 已冻结 public naming、属性 grammar、内部语义模型、mutual exclusion、constructor、单 runtime、
  associated adapter/transform、owned query、validation invalidation、traversal 与删除列表；无待选方案。
- Parser unit tests、locked trybuild compile-fail gate 与 target expansion/runtime integration 均已完成；
  2026-07-22 nested group/array review 由 `MACRO-35`/`FORM-45` 的新增证据关闭本地门禁。
- `trybuild = "1.0.118"` 是唯一新增依赖且仅为 dev dependency；manifest 与 DEP-00 产生的同一
  `Cargo.lock` 均由 Cargo 更新，没有第二份 lockfile 或手工编辑。
- Macro 不拥有 UI state、subscription、focus、请求生命周期或持久化，不需要数据库 migration、
  async shutdown 或平台权限方案。
- Runtime correctness 由 `gpui-form` integration tests 证明；parser compile-fail 由 trybuild 证明；
  两类测试不能互相替代。
- [x] MACRO-35 已实施：pure model lens、完整 schema resolver、array container/item/item-leaf mapper、
  stable ID immutable、递归 bounds 与单 transaction tests 全部通过。
- 如果实现发现 core contract 与第 2.2 节不一致，停止 expansion 工作，先更新并重新审阅英文/中文
  Guide 与 core/macro 实施计划，不增加临时适配层。
