# gpui-form 开发计划

> 历史说明：本文保留第一阶段完整设计和实现记录。当前入口见
> `crates/gpui-form/docs/development-plan.md`；dynamic array 的后续设计见
> `crates/gpui-form/docs/array-design.md`；validation report 路由设计见
> `crates/gpui-form/docs/validation-routing.md`。leaf field binding 的最终架构已调整为
> `crates/gpui-form/docs/binding-architecture.md`：`gpui-form` core 不再默认依赖 `gpui-component`，
> 所有 leaf field 统一使用 Draft-aware `ComponentFieldStore<Value, Binding>`，当前文中关于内置
> `TextFieldStore` / `NumberFieldStore` / `BoolFieldStore` / `SelectFieldStore` / `ComboboxFieldStore`
> 的内容保留为第一阶段历史记录。

状态：第一阶段 crate runtime、基础 derive 宏、显式 child-store nested group、dynamic array 宏、组件 binding
和 `garde + validify` submit pipeline 已落地。宏已生成 typed field setter、clear/apply field errors、
array remove-by-id 和 values-with-id helper。runtime 顶层模块已按 `core` / `component` / `pipeline` /
`view` 分组；derive 宏展开逻辑已从单个 `expand.rs` 拆成按职责划分的子模块。字段级 `required`
元数据、generated getter/setter 和 binding 同步能力已落地；下游 app 仍需要自己把 generated
`*_required()` 接到 `gpui_component::form::field().required(...)`，并保留业务 validator。
本文档用于固定 `gpui-form` 的实现计划、边界和待确认问题；文档中标为目标形态的 API 仍可能在后续
binding macro 和 app 接入阶段调整。

## 目标

`gpui-form` 用来补齐仓库内表单缺少统一校验、输入时反馈和提交时校验的问题。第一阶段目标曾是提供
GPUI 原生、类型安全、能和 `gpui-component` 状态模型配合的表单状态层；后续目标已调整为 core
只提供 UI-library agnostic 的表单状态和 binding contract，`gpui-component` 适配进入独立 crate。

核心目标：

- 原始业务数据结构和表单运行时状态分离，编辑过程不直接修改 domain/input struct。
- 用户定义最终提交用的业务结构体，derive 宏生成一份一一对应的 form store。
- 每个字段 store 持有字段值、交互元信息、错误、校验状态，以及对应 `gpui-component` 的组件状态。
- 支持输入时、blur 时、提交时校验，并能控制错误何时对用户可见。
- 所有组件都通过统一 binding 接入；`Input`、`Select`、`Combobox` 等只是 crate 提供的预置 binding。
- 所有监听、组件状态和未来可能引入的异步任务都必须有明确所有者，不使用随手 `.detach()` 隐藏生命周期问题。

## 非目标

- 不在 `gpui-form` 内置任何 app 业务规则。
- 不在 `gpui-form` 内访问数据库、keychain、app runtime 或配置文件。
- 不持久化 draft、错误状态或提交历史。
- 不替换 `gpui-component` 的输入、选择器、表单布局等通用组件。
- 不重复实现 `garde` / `validify` 已覆盖的专业校验和 normalize/sanitize 规则库。
- 第一阶段不做 async validator；只保留后续扩展点。
- 第一阶段不接入 `gpui-store`；除非后续某个 app 的全局状态管理确实需要。

## 已确定设计

- domain struct 是最终提交输入；generated form store 是编辑期 draft state。
- submit 成功前，不把字段变更写回原始 domain struct 或数据库。
- derive 宏必须放在独立 proc-macro crate：`crates/gpui-form-macros`。
- 第一阶段历史中，`crates/gpui-form` 同时负责运行时类型、组件适配和宏 re-export。
- 第一阶段历史中，crate 是 GPUI-aware 并直接依赖 `gpui` 和 `gpui-component`；后续目标已改为拆出
  `gpui-component` adapter crate，见 `binding-architecture.md`。
- 第一阶段支持嵌套 struct、数组字段和动态字段列表。
- 字段组件统一由 `binding = "..."` 决定 state、事件订阅、value 读写和 focus 行为；`component = "input"` /
  `"select"` / `"combobox"` / `"checkbox"` / `"switch"` / `"number"` 只是对内置 binding 的语法糖。
- 不再设计独立的 `custom` 组件分支；app 自己提供的组件和 crate 内置组件都实现同一个
  `FormComponentBinding<Value>`。
- `placeholder`、`label`、`description`、`help`、`search_placeholder`、`disabled`、`mask` 等 UI 选项来自字段
  `#[form(...)]` 属性，并通过 `ComponentStateOptions` 传给 binding 的 `new_state`，由 binding 自行应用到
  组件 state。
- `required` 需要成为字段级元数据：默认 `false`，用户可通过 `#[form(required)]` 或
  `#[form(required = true/false)]` 指定。它第一阶段只表达 UI/语义 marker，不自动生成 validation rule。
- 动态数组 row 必须按业务语义建模。不能复用一个泛型 row 再由父字段动态覆盖 child placeholder；
  如果不同数组字段的 placeholder、校验规则或必填语义不同，就拆成不同 row input/store。
- nested struct 使用 `component = "group"` 和显式 `store = "ChildFormStore"`；父 group 持有 child
  `Entity<ChildFormStore>`，缓存 child draft/meta，并保存 child observe subscription。
- dynamic array 使用 `component = "array"` 和显式 `store = "ChildFormStore"`；字段类型必须是 `Vec<Item>`。
  生成的 append/insert/remove/remove_id/move/swap/replace/reset/values_with_id helpers 接受
  `Context<ParentStore>`，负责创建 child entity 并把 observe subscription 保存到对应 `FieldArrayItem`。
- `gpui-form` 不沉淀 array row handle / row action UI 模板；add/remove action、按钮、图标和布局属于接入 app。
- 具体 app 的迁移顺序和接入细节不属于 `gpui-form` crate 文档，应放到对应 app 的开发文档中。
- cross-field validation 的错误归属到具体字段；没有自然字段归属的全局提交错误才进入 form-level errors。
- `gpui-form` 负责表单状态机、触发时机、字段路径、错误可见性、组件 state 和 subscriptions。
- `garde` 负责专业校验规则；`validify` 只负责 normalize/sanitize，不使用 `validify` 的 validation 结果。
- 第一阶段提供 `garde` validation adapter 和 `validify` submit transform adapter。
- live validation 可以在 clone 上运行 `validify::Modify` 形成 validation preview，再用 `garde::Validate`
  校验 preview；这个过程不写回组件 state。
- 用户点击 submit 后，无论校验成功还是失败，都先把 `validify` 规范化后的值写回表单 draft 和组件 state，
  再用 `garde` 对规范化后的值做 submit validation。
- built-in 字段 store 持有对应组件状态，例如 `Entity<InputState>`、`Entity<SelectState<_>>`、
  `Entity<ComboboxState<_>>`。
- `Checkbox`、`Switch` 这类 controlled component 没有独立 state entity，使用字段 store 中的 bool
  draft value 作为 UI state。
- 字段、字段组、动态数组和表单必须保存 `Subscription`，避免监听脱离表单生命周期。
- 后续如果引入 async validator 或 async options source，对应 `Task` 也必须保存在字段或表单中。
- `.detach()` 不用于字段、字段组、动态数组、表单、选项加载、校验和提交任务；只有明确 app 生命周期永久存在的全局监听才允许例外。
- i18n 输出 message key + params，不在 crate 内硬编码 app 文案。
- icon 输出 semantic kind，不在 crate 内引用 app-local `IconName` 或新增 app asset。

## 已确认范围

- `FormItemId` 使用表单生命周期内单调递增的 `u64` newtype；它只表达运行时 row identity，
  不使用 UUID 或业务 ID，也不承诺跨 session 稳定。
- 动态数组第一阶段支持 add/remove/reorder/reset；暂不支持虚拟列表和跨数组拖拽。
- app 接入顺序不在本 crate 文档中固定；具体迁移计划放在对应 app 文档中。
- validation 固定使用 `garde`；normalize/sanitize 固定使用 `validify::Modify`。
- submit 被点击后，无论成功还是失败，normalized value 都要写回表单 draft 和对应组件 state。

## Required 字段支持计划

当前状态：

- `FieldAttributes` 和 `ComponentStateOptions` 已有 `required` 字段。
- `FieldCore`、`FieldGroupStore`、`FieldArrayStore`、内置 field store、`FormField` 和 `AnyFormField`
  已暴露 required 元数据。
- derive 宏已解析 `#[form(required)]` / `#[form(required = true/false)]`，并生成
  `<field>_required()` / `set_<field>_required(required, window, cx)` helper。
- `FormComponentBinding::set_required(...)` 已提供默认 no-op，供 app 自定义组件同步 required 语义。
- 未知 `#[form(...)]` 字段参数已改为编译时报错，避免拼写错误被静默忽略。
- required 仍只表达 UI/语义 marker，不自动产生 validation error；下游 app 需要继续按业务规则写 validator。

### API 和语义

目标属性：

```rust
#[form(component = "input", label = "prompt-field-name", required)]
pub name: String,

#[form(component = "input", label = "provider-field-base-url", required = false)]
pub base_url: String,
```

语义：

- 默认值是 `required = false`。
- `required` 是字段元数据和 UI marker，不自动创建 `FieldError`，也不绕过现有 validation pipeline。
- 字段是否为空、是否需要结合其它状态判定，仍由 `garde` 或 app-specific validator 决定。例如 secret 是否必填
  需要结合 saved secret ref 和 dirty/cleared 状态，不能由通用 `gpui-form` 静态规则决定。
- submit 时 `required` 本身不阻止提交；只有 validation adapter 或 app validator 写入错误时才阻止。
- `required` 可以是静态属性，也可以被 app 在运行时通过 generated setter 更新，用于 secret ref 是否满足、
  transport kind、动态 row sibling value 等条件式 UI。
- `required` 的接入不能只覆盖首批迁移字段。下游 app 文档需要按完整表单面维护字段验证矩阵，至少写清
  required marker、空值规则、格式/重复/引用约束、DB/config/runtime 最终保护、i18n key 和测试点。
- 宏属性 parser 遇到未知 `#[form(...)]` key 必须报错；只允许显式列出的 forward-compatible no-op
  语法在设计确认后加入，避免拼写错误被静默忽略。

下游接入文档要求：

- 不能只记录已经迁移到 `gpui-form` 的表单，也不能只记录首批 required 消费方。一个 app 侧迁移计划必须覆盖
  同一功能面中所有用户可编辑输入、即时配置 action、搜索/filter、运行态 submit guard，以及明确不迁移的页面。
- 对每个字段都要区分“UI required”、“保存前 app validator”、“DB/config/runtime 最终保护”。例如 DB
  `TEXT NOT NULL` 不自动等于 UI required；`Option<T>` config 字段也不自动等于运行态 optional。
- URL、HTTP header、env var、hotkey、enum、duplicate、FK、capability、credentials ref 等业务语义必须由
  app-specific validator 明确写入计划；`gpui-form` 只提供 field metadata、error routing 和 binding。
- 不迁移的输入也要写清理由：如果只是 search/filter、即时 action、只读 runtime status 或列表刷新，就不应为了
  统一而创建 form store。
- 如果后续新增 required marker，必须同步检查已迁移表单和未迁移候选，避免 Provider/MCP、Prompt/Shortcut、
  General、ChatForm 等表单面的验证规则漂移。

### 文件和模块结构

不新增模块文件，按现有职责修改：

| 文件 | 变更 |
| --- | --- |
| `crates/gpui-form-macros/src/attributes.rs` | `FieldAttributes` / `FieldArgs` 新增 `required: bool`；解析 `required` 和 `required = bool`；未知字段属性改为 `syn::Error`。 |
| `crates/gpui-form-macros/src/expand/fields.rs` | `component_state_options(...)` 写入 `required`；字段初始化后调用 `core_mut().set_required(required)`；setter 生成逻辑需要同步 field meta 但不触发 dirty。 |
| `crates/gpui-form-macros/src/expand/accessors.rs` | 为普通字段、binding 字段、select/combobox/bool 字段生成 `<field>_required()` 和 `set_<field>_required(required, window, cx)` helper。 |
| `crates/gpui-form-macros/src/expand/errors.rs` | 不新增 required validation；`apply_field_error(...)` 已继续和 required 字段一起使用。 |
| `crates/gpui-form/src/component/binding.rs` | `ComponentStateOptions` 新增 `required: bool`；`FormComponentBinding` 新增可选 `set_required(...)` 默认 no-op，供 app 自定义组件消费。 |
| `crates/gpui-form/src/core/field.rs` | `FieldCore<T>` 新增 `required: bool`；`FormField` / `AnyFormField` 新增 `is_required()`；`FieldCore::set_required(...)` 不改 draft、dirty 或 errors。 |
| `crates/gpui-form/src/core/group.rs` | `FieldGroupStore` 支持 `required`，用于 group-level field label marker。 |
| `crates/gpui-form/src/core/array.rs` | `FieldArrayStore` 支持 `required`，用于 array-level field label marker；不自动要求数组非空。 |
| `crates/gpui-form/src/component/fields/{input,number,select,combobox,bool,component}.rs` | 各 field store 暴露 `is_required()`，并把 runtime setter 委托给 `FieldCore`；内置 input/select 等组件不负责渲染 marker。 |
| `crates/gpui-form/src/view/render.rs` | 保留 `FormIconKind::Required` 语义枚举；第一阶段不新增 marker view，推荐直接使用 `gpui_component::form::field().required(...)`。 |
| `crates/gpui-form/tests/derive.rs` | 增加宏属性解析、默认 false、显式 true、generated getter/setter、binding state 同步测试；未知属性报错测试仍可补。 |
| `crates/gpui-form/README.md` | 已更新 required 示例，说明 required 只负责 UI/语义，validation 仍由 `garde` / app validator 承担。 |

### 自定义组件和类型结构

`ComponentStateOptions` 目标结构：

```rust
pub struct ComponentStateOptions {
    pub label_key: Option<&'static str>,
    pub description_key: Option<&'static str>,
    pub placeholder_key: Option<&'static str>,
    pub masked: bool,
    pub disabled: bool,
    pub required: bool,
}
```

`FieldCore` 目标 API：

```rust
impl<T> FieldCore<T>
where
    T: Clone + PartialEq + 'static,
{
    pub fn is_required(&self) -> bool;
    pub fn set_required(&mut self, required: bool);
}
```

`FormField` / `AnyFormField` 目标 API：

```rust
pub trait FormField {
    fn is_required(&self) -> bool;
    // existing methods...
}

pub trait AnyFormField {
    fn is_required(&self) -> bool;
    // existing methods...
}
```

生成的 app-facing helper 目标形态：

```rust
let required = form_read.name_required();
field()
    .label(form_read.name_label(cx))
    .required(required)
    .child(Input::new(form_read.name_input_state()));

form.update(cx, |form, cx| {
    form.set_url_required(
        matches!(form.transport_value(), McpTransportKind::StreamableHttp),
        window,
        cx,
    );
});
```

组件 binding 目标：

```rust
pub trait FormComponentBinding<Value>: Sized + 'static
where
    Value: Clone + PartialEq + 'static,
{
    fn set_required(
        _state: &Entity<Self::State>,
        _required: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}
```

内置 binding 第一阶段只接收 `required`，不渲染 marker；marker 由 field layout 渲染。app 自定义组件如果有
自己的 required affordance，可以在 `set_required` 里同步组件内部 state。

### 所用组件和 UI 表达

优先使用当前 `gpui-component`：

- `gpui_component::form::field().required(true)` 已存在，并会在 label 后渲染 danger 色 `*`。
- `gpui-form` 的 generated helper 应直接喂给 `field().required(form_read.<field>_required())`。
- `Input`、`Select`、`Combobox`、`Switch`、`Checkbox` 不承担 required marker；它们只负责具体控件 state。

`/Users/sushao/Documents/code/ui` 中 shadcn/ui 只作为补充参考：

- shadcn/ui 的 Field 示例在 control 上设置 HTML `required`，并用 `data-invalid` / `aria-invalid` 表达校验状态。
- GPUI 没有 DOM `required` / `aria-required`，因此不照搬 DOM 属性；只吸收“required 属于 control
  语义、invalid 属于 field/control 状态”的划分。
- 如果后续 gpui-component 增加 accessibility metadata，再把 `ComponentStateOptions.required` 传给对应控件。

### 数据流

```text
#[form(required)] attribute
  -> gpui-form-macros FieldAttributes.required
  -> ComponentStateOptions.required
  -> GeneratedFormStore::from_value(...)
  -> FieldCore.required / FieldGroupStore.required / FieldArrayStore.required
  -> generated <field>_required() or field.is_required()
  -> app render calls gpui_component::form::field().required(...)
  -> gpui-component renders danger "*" after label
```

运行时条件式 required：

```text
app state change, e.g. transport = StreamableHttp
  -> generated set_url_required(true, window, cx)
  -> FieldCore.required changes, component binding set_required no-op or syncs app component
  -> form meta unchanged except notify
  -> render updates field().required(true)
```

提交：

```text
submit
  -> validify normalize
  -> garde / app-specific validator
  -> FieldError only if validator reports error
```

`required` 不参与 dirty、pristine、touched、blurred 或 submission attempts。

### 全局数据管理

- 不新增 `Global`。
- 不新增全局 required registry。
- `required` 是打开表单 entity 内的 field metadata；跨 view 共享表单时随同一个 `Entity<GeneratedFormStore>`
  共享。
- App-level conditional required 由拥有该表单的 view/dialog 根据当前 app state 调 generated setter。

### 数据库变更

- 无数据库变更。
- 不新增 migration。
- 不持久化 required state。
- `required` 不写入 app 数据表、`config.toml`、keychain 或 `FormItemId` output。

### 数据获取方式

- 静态 required 来自 `#[form(required)]` 宏属性。
- 条件式 required 来自 app owner view/dialog 的已有状态，例如 saved secret refs、dirty/cleared secret value、
  MCP transport kind、动态 row sibling value、prompt/shortcut dialog mode、配置型 optional 字段和运行态 submit
  required 的差异。
- `garde` / app validator 仍是错误来源；不要从 `garde` report 反推 UI required marker。
- `gpui-form` 不决定 URL scheme、hotkey 冲突、DB 唯一性、FK 存在性、secret ref 是否有效、model
  是否 enabled 等 app 业务规则；这些规则必须由接入 app 的 validator 明确处理并映射到具体 generated field。

### Icon

- 不新增 icon asset。
- 不使用 Lucide icon 表达 required。
- UI marker 使用 gpui-component `Field::required(true)` 内建 danger 色 `*`。
- `FormIconKind::Required` 只保留为未来可选语义，不要求本阶段消费。

### i18n

- Required marker 本身是符号 `*`，不新增 `.ftl` key。
- 内置 `gpui-form` 不新增 required validation 文案。
- App-specific required errors 继续用 app locale key，例如 `provider-validation-required`、
  `mcp-validation-name-required`、`prompt-validation-name-required`、`shortcut-validation-hotkey-required`。
- 如果未来新增通用 required validator，再在 app locale 中补 `gpui-form-error-required`，但不属于本阶段。

### 新增依赖

- 不新增依赖。
- 继续使用现有 `gpui`、`gpui-component`、`garde`、`validify` 和 proc-macro 依赖。
- 不引入 shadcn/ui、React、DOM 或 accessibility helper crate。

## 文件和模块结构

第一阶段新增两个 crate。禁止新增 `mod.rs`，模块入口都使用同名 `.rs` 文件。`gpui-form` 顶层只保留
少量稳定分组：`core` 保存表单核心状态，`component` 保存组件 binding，`pipeline` 保存 validation /
transform 适配，`view` 保存渲染辅助。

```text
crates/gpui-form/
  Cargo.toml
  README.md
  docs/development-plan.md
  src/lib.rs
  src/core.rs
  src/core/form.rs
  src/core/field.rs
  src/core/meta.rs
  src/core/error.rs
  src/core/path.rs
  src/core/group.rs
  src/core/array.rs
  src/core/trigger.rs
  src/core/subscriptions.rs
  src/core/options.rs
  src/component.rs
  src/component/binding.rs
  src/component/fields.rs
  src/component/fields/input.rs
  src/component/fields/select.rs
  src/component/fields/combobox.rs
  src/component/fields/bool.rs
  src/component/fields/number.rs
  src/component/fields/component.rs
  src/pipeline.rs
  src/pipeline/validation.rs
  src/pipeline/validation/adapter.rs
  src/pipeline/validation/garde.rs
  src/pipeline/validation/report.rs
  src/pipeline/transform.rs
  src/pipeline/transform/adapter.rs
  src/pipeline/transform/validify.rs
  src/view.rs
  src/view/render.rs
  src/macro_support.rs
  src/test_support.rs

crates/gpui-form-macros/
  Cargo.toml
  src/lib.rs
  src/attributes.rs
  src/expand.rs
  src/expand/accessors.rs
  src/expand/arrays.rs
  src/expand/fields.rs
  src/expand/pipeline.rs
  src/expand/validation.rs
  src/field_kind.rs
  src/group_kind.rs
  src/array_kind.rs
```

模块职责：

| 模块 | 责任 | 公开性 |
| --- | --- | --- |
| `core/form.rs` | `FormStore` / `FormState` trait、submit/reset API 边界 | public |
| `core/field.rs` | `FormField`、`AnyFormField`、字段通用 API 和 `FieldCore` | public |
| `core/meta.rs` | `FieldMeta`、`FormMeta`、derived state | public |
| `core/error.rs` | `FieldError`、`FormError`、`FormValidationReport` | public |
| `core/path.rs` | `FieldPath`、字段路径和路径段 | public |
| `core/group.rs` | nested struct 的字段组 store、group meta、group validation routing | public |
| `core/array.rs` | 动态字段数组、`FormItemId(u64)`、add/remove/reorder/reset runtime | public |
| `core/trigger.rs` | `ValidationTrigger`、错误可见性触发来源 | public |
| `core/subscriptions.rs` | 订阅持有工具类型，确保监听随字段/form 生命周期释放 | public |
| `core/options.rs` | select/combobox options snapshot 与 mismatch 诊断 | public |
| `component/binding.rs` | `FormComponentBinding`、`ComponentStateOptions` 和 app component 接入点 | public |
| `component/fields.rs` | 内置字段 store 和 binding re-export | public |
| `component/fields/*` | 内置 gpui-component binding 实现 | public |
| `pipeline/validation.rs` | validation pipeline 入口和 re-export | public |
| `pipeline/validation/adapter.rs` | adapter trait、scope、source、mapping config | public |
| `pipeline/validation/garde.rs` | `garde::Validate` adapter 和 error 映射 | feature-gated public |
| `pipeline/validation/report.rs` | `ValidationIssue`、adapter report、field/form report 转换 | public |
| `pipeline/transform.rs` | submit-time transform pipeline 入口和 re-export | public |
| `pipeline/transform/adapter.rs` | `SubmitTransform` trait、transform context 和 write-back policy | public |
| `pipeline/transform/validify.rs` | `validify::Modify` adapter，只执行 normalize/sanitize | feature-gated public |
| `view/render.rs` | 可选的错误文本、label、help text 读取 helper | public |
| `macro_support.rs` | derive 展开后调用的稳定 runtime helper | public but hidden docs |
| `test_support.rs` | 单元测试 helper | `cfg(test)` 或 feature-gated |

宏 crate 职责边界：

| 模块 | 责任 |
| --- | --- |
| `attributes.rs` | 解析 `#[form(...)]` 属性，不负责生成 runtime 行为 |
| `field_kind.rs` / `group_kind.rs` / `array_kind.rs` | 宏属性中的枚举解析 |
| `expand.rs` | derive 主编排：解析输入 struct、构建字段模型、组织最终 token 输出 |
| `expand/fields.rs` | 字段 store 类型、字段初始化、write-back glue code |
| `expand/accessors.rs` | typed value/state/store/items 访问器、reset、focus-first-error 片段 |
| `expand/arrays.rs` | dynamic array helper 方法和 `Vec<T>` item 类型提取 |
| `expand/validation.rs` | validation report 路由到普通字段、group 和 array 的生成片段 |
| `expand/pipeline.rs` | `garde` / `validify` 字段、初始化、preview、submit 代码片段 |

设计约束：宏只负责类型安全的静态 glue code；通用状态机、组件 binding、subscription 持有、validation /
transform adapter 语义必须继续下沉到 `gpui-form` runtime，不能把 app 业务规则或可复用运行时逻辑塞进
`quote!` 里。

## 核心类型结构

### Form store

derive 宏生成的 form store 是一个具体 struct，通常作为 `Entity<GeneratedFormStore>` 被 view/dialog 持有。

目标形态：

```rust
pub struct ConnectionFormStore {
    pub display_name: TextFieldStore<String>,
    pub kind: SelectFieldStore<Option<ConnectionKind>, ConnectionKindOptions>,
    pub endpoint_url: TextFieldStore<Option<String>>,
    pub secret_ref: ComponentFieldStore<Option<String>, SecretRefBinding>,
    validation: GardeAdapter<ConnectionInput>,
    transform: ValidifyTransform<ConnectionInput, ConnectionInput>,
    meta: FormMeta,
    form_errors: Vec<FormError>,
    _subscriptions: Vec<Subscription>,
}
```

包含动态字段时，derive 宏会生成字段组和数组 store：

```rust
pub struct CommandFormStore {
    pub title: TextFieldStore<String>,
    pub mode: SelectFieldStore<Option<CommandMode>, CommandModeOptions>,
    pub arguments: FieldArrayStore<ArgumentRowFormStore>,
    pub environment: FieldArrayStore<EnvironmentRowFormStore>,
    pub headers: FieldArrayStore<HeaderRowFormStore>,
    pub secret_headers: FieldArrayStore<SecretHeaderRowFormStore>,
    validation: GardeAdapter<CommandInput>,
    transform: ValidifyTransform<CommandInput, CommandInput>,
    meta: FormMeta,
    form_errors: Vec<FormError>,
    _subscriptions: Vec<Subscription>,
}
```

`FormStore` trait 负责表单级行为：

```rust
pub trait FormStore {
    type Output;

    fn meta(&self) -> &FormMeta;
    fn reset(&mut self, window: &mut Window, cx: &mut App);
    fn validate(&mut self, trigger: ValidationTrigger, window: &mut Window, cx: &mut App)
        -> FormValidationReport;
    fn submit(&mut self, window: &mut Window, cx: &mut App) -> Result<Self::Output, FormValidationReport>;
    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App);
}
```

### Field store

每个字段 store 负责保存 draft value、default value、meta、errors、component state 和 subscriptions。

```rust
pub struct TextFieldStore<T> {
    value: T,
    default_value: T,
    meta: FieldMeta,
    errors: Vec<FieldError>,
    input_state: Entity<InputState>,
    visibility: ErrorVisibility,
    _subscriptions: Vec<Subscription>,
    revision: u64,
}
```

字段 store 不保存 `FieldPath`。字段地址由 derive macro 生成的 form store 持有，用于构造
`ValidationScope` 和把 `FormValidationReport` 路由回具体字段。

字段公共 trait：

```rust
pub trait FormField {
    type Value;
    type ComponentState;

    fn value(&self) -> &Self::Value;
    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause);
    fn reset(&mut self, window: &mut Window, cx: &mut App);

    fn component_state(&self) -> Entity<Self::ComponentState>;
    fn meta(&self) -> &FieldMeta;
    fn errors(&self) -> &[FieldError];
    fn visible_errors(&self) -> &[FieldError];

    fn mark_touched(&mut self);
    fn mark_blurred(&mut self);
    fn validate(&mut self, trigger: ValidationTrigger, window: &mut Window, cx: &mut App)
        -> FieldValidationReport;
    fn focus(&mut self, window: &mut Window, cx: &mut App);
}
```

`AnyFormField` 只暴露 meta、errors、focus 等 object-safe 能力，用于错误汇总和测试；不同
字段的具体 value/component state 仍保留在生成的具体 store 类型中。

derive 宏为每个 store 生成强类型字段枚举和 form-level 事件。例如
`ConnectionFormStore` 会生成：

```rust
pub enum ConnectionFormField {
    Enabled,
    DisplayName,
    SecretRef,
    EndpointUrl,
}

pub enum ConnectionFormEvent {
    FieldChanged(ConnectionFormField),
    FieldFocused(ConnectionFormField),
    FieldBlurred(ConnectionFormField),
}
```

app 需要处理 secret dirty、清空业务校验状态等副作用时，订阅 form store 的事件；不再逐个订阅
`InputState` 再反查字段 key。

### Field group and array

nested struct 不生成完整 form store，而是生成 form fragment/group。

```rust
pub trait FormFragment {
    type Output;

    fn path(&self) -> &FieldPath;
    fn meta(&self) -> &FormMeta;
    fn validate(&mut self, trigger: ValidationTrigger, window: &mut Window, cx: &mut App)
        -> FormValidationReport;
    fn output(&self) -> Result<Self::Output, FormValidationReport>;
}

pub struct FieldGroupStore<G> {
    path: FieldPath,
    fields: G,
    meta: FormMeta,
    errors: Vec<FormError>,
    _subscriptions: Vec<Subscription>,
}
```

动态数组负责稳定持有每一行的字段组、组件 state 和 subscriptions。remove/reorder 时必须 drop 被移除行的
subscriptions，reorder 不能重建仍存在行的组件 state。

```rust
pub struct FieldArrayStore<Item> {
    path: FieldPath,
    items: Vec<FieldArrayItem<Item>>,
    id_generator: FormItemIdGenerator,
    array_revision: u64,
    meta: FieldMeta,
    errors: Vec<FieldError>,
    _subscriptions: Vec<Subscription>,
}

pub struct FieldArrayItem<Item> {
    id: FormItemId,
    index: usize,
    item: Item,
}

pub struct FormItemId(u64);

pub struct FormItemIdGenerator {
    next: u64,
}

impl FormItemIdGenerator {
    pub fn generate(&mut self) -> FormItemId {
        let id = FormItemId(self.next);
        self.next = self.next.checked_add(1).expect("form item id overflowed");
        id
    }
}
```

`FieldPath` 需要支持 nested field 和 dynamic array：

```rust
pub enum FieldPathSegment {
    Field(&'static str),
    Index(usize),
    Item(FormItemId),
}
```

UI 定位优先使用 `FormItemId`，提交报告可以同时包含当前 index，方便文案显示。

参考库结论和本 crate 决策：

- React Hook Form：为每个 field array item 生成 UUID-like `id`，`fields` 返回值默认带 `id`，用户用它做
  render key；append/insert 生成新 id，remove 删除 id，swap/move 移动 id，update/replace 生成新 id。
- TanStack Form：不维护独立 item id，主要用 index/path 操作数组，同时移动 field meta，并用
  `_arrayVersion` 触发 array-mode re-render。
- `gpui-form` 决策：使用表单生命周期内单调递增的 `FormItemId(u64)` 作为动态数组 row identity，
  用它保护 GPUI component state 和 `Subscription` 生命周期；同时采用 TanStack 的 index/path 模型来做校验路径和错误定位。
- `FormItemId` 不写入 domain output，不进入数据库，不用于跨 session 稳定性。
- 初始构造、append、insert 会生成新 id；remove 会 drop item 及其 subscriptions；move/reorder 会移动 item
  及其 id；reset/replace 会重建整组 id 和 item state。

### Meta

参考 TanStack Form 和 React Hook Form 的交互状态，但不照搬 JS 表单库的 `isValid` /
`canSubmit` 存储字段。字段和表单只保存交互/生命周期事实；合法性从当前 errors/report 派生。

```rust
pub struct FieldMeta {
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_dirty: bool,
    pub is_default_value: bool,
    pub is_validating: bool,
}

pub struct FormMeta {
    pub is_dirty: bool,
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_validating: bool,
    pub last_submit_outcome: Option<SubmitOutcome>,
    pub submission_attempts: u32,
}
```

- `FieldMeta::is_pristine()` 和 `FormMeta::is_pristine()` 由 `!is_dirty` 计算。
- `is_submitting` 不进入 `FormMeta`；`FormStore::is_submitting()` 由 `SubmitRuntime.task.is_some()` 计算。
- `FormStore::can_attempt_submit()` 由 `!submit_runtime.is_submitting() && !form_meta.is_validating`
  计算，只表示运行态允许尝试提交，不表示数据合法。
- `is_valid` 不进入 meta；字段/form 合法性由 `FieldError::is_error`、`FormError::is_error` 和最终
  `FormValidationReport::is_valid()` 派生。

### Error

错误需要适配 i18n 和 UI 展示，不直接保存最终中文/英文字符串。

```rust
pub struct FieldError {
    pub path: FieldPath,
    pub trigger: ValidationTrigger,
    pub severity: ValidationSeverity,
    pub code: &'static str,
    pub message_key: &'static str,
    pub params: ErrorParams,
}

pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}
```

错误可见性不等于错误存在。字段可以已经有 `Change` 错误，但 UI 只在 touched/blurred/submitted 后显示。

## 所用 gpui-component 组件

`gpui-form` 第一阶段只做 state/binding 层，并提供少量 helper；表单布局仍优先使用
`gpui_component::form::{v_form, h_form, field}`。

| 控件类型 | gpui-component 组件 | state | 字段 store | 监听来源 |
| --- | --- | --- | --- | --- |
| 文本输入 | `Input` | `InputState` | `TextFieldStore<T>` | `InputEvent::Change/Focus/Blur/PressEnter` |
| 密码/secret | `Input` | `InputState` | `TextFieldStore<T>` | 同文本输入 |
| URL | `Input` | `InputState` | `TextFieldStore<Option<String>>` | 同文本输入 |
| 多行文本 | `Input` | `InputState` | `TextFieldStore<T>` | 同文本输入，配置 `multi_line/auto_grow` |
| code/editor | `Input` | `InputState` | `TextFieldStore<T>` | 同文本输入，配置 `code_editor` |
| 数字 | `NumberInput` | `InputState` | `NumberFieldStore<N>` | `InputEvent::Change/Blur`，从 raw input 同步 typed draft 和 dirty |
| 单选下拉 | `Select` | `SelectState<D>` | `SelectFieldStore<Value, D>` | `SelectEvent::Confirm` |
| 搜索选择 | `Combobox` | `ComboboxState<D>` | `ComboboxFieldStore<Value, D>` | `ComboboxEvent::Change/Confirm` |
| 多选 | `Combobox` | `ComboboxState<D>` | `ComboboxFieldStore<Vec<T>, D>` | `ComboboxEvent::Change/Confirm` |
| checkbox | `Checkbox` | `BoolComponentState` | `BoolFieldStore` | app click handler / binding write |
| switch | `Switch` | `BoolComponentState` | `BoolFieldStore` | app toggle handler / binding write |
| 嵌套字段组 | app 布局 + `field` | 子字段 state | `FieldGroupStore<G>` | 子字段 subscriptions |
| 动态字段列表 | app 布局 + `Button` | 每行子字段 state | `FieldArrayStore<Item>` | 每行 subscriptions |
| 任意 app 组件 | app 自己提供 | binding 定义 | `ComponentFieldStore<Value, B>` | `FormComponentBinding` |

错误展示方式：

- built-in helper 提供 `visible_error_text(cx)`、`visible_error_kind()`、`aria/description` 这类读取方法。
- view 仍使用 `gpui_component::form::field().error(...)` 或 app 自己的错误区域渲染。
- 第一阶段不提供包住所有控件的全功能 `FormFieldView`，避免和 app 布局强耦合。

## 组件 Binding 和类型

所有组件都通过 `FormComponentBinding<Value>` 接入。内置 `TextInputBinding`、`NumberInputBinding`、
`SelectBinding<T, D>`、`ComboboxBinding<T, D>`、`BoolBinding` 只是 crate 提供的 binding；
app 自己的组件也实现同一个 trait，不再有特殊的 `custom` 分支。

```rust
pub trait FormComponentBinding<Value>: 'static {
    type State: 'static;
    type Event;

    fn new_state(
        initial: &Value,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State>;

    fn read_value(state: &Entity<Self::State>, cx: &App) -> Value;

    fn write_value(
        state: &Entity<Self::State>,
        value: &Value,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    );

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        form: Entity<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static;

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool;
}
```

`ComponentStateOptions` 是宏参数到组件 state 的稳定传递层：

```rust
pub struct ComponentStateOptions {
    pub label: Option<FormText>,
    pub description: Option<FormText>,
    pub help: Option<FormText>,
    pub placeholder: Option<FormText>,
    pub search_placeholder: Option<FormText>,
    pub disabled: bool,
    pub required: bool,
    pub mask: Option<FormInputMask>,
}
```

规则：

- derive 宏为每个字段生成具体 binding 类型；需要动态遍历时只通过 `AnyFormField` 访问共同状态。
- binding 的事件订阅必须返回 `SubscriptionSet` 给字段、数组 item 或表单保存，不能由组件内部 `.detach()`。
- placeholder、search placeholder、mask、disabled 等只在 binding 的 `new_state` / `write_value` 中应用。
- required 只作为 field metadata 和 custom binding option；built-in binding 不渲染 required marker。
- app 不应该在 form 创建后再调用 `apply_*_placeholders` 这类 helper 去修正组件 state。
- 如果同一种视觉 row 在不同业务字段里需要不同 placeholder 或校验规则，应拆成不同 row input/store。
- number binding 是内置 binding 的特殊形态：state 仍是 `InputState`，但 generated render helper 必须使用
  `NumberInput::new(&state)`；app 不应把 `component = "number"` 的 state 渲染成普通 `Input::new(...)`。
- number dirty/default 不从 `FieldCore<N>` 的 typed value 派生，而从 raw input 文本与 raw default 文本比较得出；
  typed value 只表示最后一次成功 parse 后的 domain draft。

number 字段目标结构详见 `number-input-design.md`。核心结构为：

```rust
pub struct NumberFieldStore<N>
where
    N: NumberFieldValue,
{
    core: FieldCore<N>,
    input_state: Entity<InputState>,
    raw_default: String,
    raw_value: String,
    raw_revision: u64,
    parse_error: Option<FieldError>,
}
```

其中 `raw_value` 是 form store 处理过的 raw input draft；更新入口只能是 generated subscription、
generated setter、reset 或 submit normalize。`InputState` 是 UI entity，必须通过 binding/store 写回保持同步。

语义化动态 row 目标形态：

```rust
#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = ArgumentRowFormStore)]
pub struct ArgumentRowInput {
    #[form(component = "input", placeholder = "form-example-placeholder-argument")]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = EnvironmentRowFormStore)]
pub struct EnvironmentRowInput {
    #[form(component = "input", placeholder = "form-example-placeholder-variable")]
    pub key: String,

    #[form(component = "input", placeholder = "form-example-placeholder-value")]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = HeaderRowFormStore)]
pub struct HeaderRowInput {
    #[form(component = "input", placeholder = "form-example-placeholder-header-name")]
    pub name: String,

    #[form(component = "input", placeholder = "form-example-placeholder-header-value")]
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = SecretHeaderRowFormStore)]
pub struct SecretHeaderRowInput {
    #[form(component = "input", placeholder = "form-example-placeholder-header-name")]
    pub name: String,

    #[form(component = "input", placeholder = "form-example-placeholder-secret-ref")]
    pub secret_ref: String,
}
```

第一阶段计划提供的通用 UI helper：

| 类型 | 位置 | 用途 |
| --- | --- | --- |
| `FieldText` | `view/render.rs` | label、description、help、error 的 i18n key + params |
| `FieldErrorViewState` | `view/render.rs` | UI 渲染错误时所需的 severity、icon kind、文本 key |
| `FormErrorSummary` | `view/render.rs` | 表单级错误汇总数据，不直接绑定具体布局 |
| `BoolFieldStore` | `component/fields/bool.rs` | checkbox/switch controlled value |
| `ComponentFieldStore<Value, B>` | `component/binding.rs` | 任意 binding-backed 组件状态适配 |

动态数组的 add/remove/reorder 按钮、row action 和具体 icon 不进入 `gpui-form`。crate 只提供
`FormItemId`、array helper 和字段/错误状态；UI 由接入 app 按自己的交互和样式组织。

## 数据流

标准流程：

```text
repository/config/runtime data
  -> app 构造 domain input struct
  -> GeneratedFormStore::from_value(domain, window, cx)
  -> 每个字段、字段组、动态数组 item 创建 draft value
  -> macro 把字段 #[form(...)] UI 选项整理为 ComponentStateOptions
  -> binding::new_state 创建 component state 并应用 placeholder/disabled/mask 等 UI 选项
  -> binding::install_subscriptions 返回 SubscriptionSet，由字段或 array item 保存
  -> 用户操作 gpui-component
  -> component event 通过 Subscription 回写字段 draft value
  -> 更新 dirty/touched/blurred/revision
  -> emit typed form event 给 app 层处理业务副作用
  -> 按 trigger 构造 validation preview
  -> 在 preview 上运行 validify::Modify
  -> 调用 garde adapter 校验 preview
  -> adapter report 映射为 FieldError/FormError
  -> 聚合 FormMeta 并 notify view
  -> submit 时对 draft 运行 validify::Modify
  -> normalized value 写回表单和组件 state
  -> 调用 garde adapter 校验 normalized value
  -> 成功后把 normalized value 作为 output
  -> app repository/config command 持久化
```

输入时：

- 组件事件先更新字段 draft value。
- 字段根据 `FieldChangeCause` 更新 `is_dirty`、`is_touched`、`revision`。
- 如果配置了 `on_change`，从当前 draft clone 出 validation preview。
- validation preview 可以运行 `validify::Modify`，再交给 `garde::Validate`，保证输入时和提交时校验语义一致。
- 输入时不把 preview 写回字段或组件 state。
- 错误是否显示由 `ErrorVisibility` 决定，不是所有错误立刻显示。

number 输入时：

- generated subscription 从 `InputState` 读取当前 raw text，调用 `NumberFieldStore::sync_raw_input(...)`。
- parse 成功：更新 typed `FieldCore<N>`，清除 parse error；dirty 仍按 `raw_value != raw_default` 计算。
- parse 失败：保留上一次 typed draft，写入 `ValidationSource::Internal` parse error；如果 raw text 变化，
  字段和 form 仍必须 dirty，revision 仍必须递增。
- change validation 只在 parse 成功后运行，避免用 stale typed value 校验当前不可解析的 raw input；submit preflight
  会把 parse error 纳入 final report。

嵌套字段组：

- nested struct 被展开成 `FieldGroupStore<G>`。
- 子字段路径带完整父路径，例如 `transport.http.url`。
- group meta 从子字段聚合，group 自身可以有 cross-field validator。

动态字段数组：

- `FieldArrayStore<Item>` 管理 add/remove/remove_id/reorder/reset。
- 每个 item 拥有稳定 `FormItemId`、当前 index 和字段组。
- add 时创建新的 item 字段组、组件 state 和 subscriptions。
- remove 时 drop 对应 item，释放该行 subscriptions 和组件 state。
- reorder 时只调整 item 顺序，不重建 item 内部 state。
- app validator 需要 row identity 时读取 `field_values_with_id()` 返回的 `FormRowValue<T>` 快照；
  `FormItemId` 不进入提交输出。

blur 时：

- 字段设置 `is_blurred = true`。
- 执行 `on_blur` validator。
- 一般允许 blur 后显示该字段错误。

submit 时：

- 表单把 `SubmitRuntime.task` 作为 submit loading 的唯一事实源，并记录 `submission_attempts += 1`。
- 递归执行 `prepare_submit`，内置 number input 会重新读取当前 raw text；若 parse 失败，写入
  `ValidationSource::Internal` field error，返回 invalid preflight report，不执行 normalize。
- number preflight parse 成功时可同步 typed draft，但 dirty/default 仍从 raw input 基线计算；normalize 写回后再用
  normalized raw 文本重新计算 dirty。
- 构造 submit candidate，并调用 `validify::Modify` 产出 normalized output。
- normalized output 写回 generated form store 的字段 draft value 和对应 component state，写回原因是
  `FieldChangeCause::NormalizeOnSubmit`。
- normalize 写回期间不触发 `on_change` 校验，避免提交过程重复递归校验。
- `garde` adapter 对 normalized output 调用 `garde::Validate`。
- 执行 app-defined form validator。
- 把 adapter report 写回字段/form errors 后，用 `current_validation_report` 从当前 store 状态构造
  final report；internal field errors 会保留在 final report 中。
- 若失败：清空 `SubmitRuntime.task`，设置 `last_submit_outcome = Some(Failure)`，返回 final report 并聚焦首个
  error 字段。
- 若成功：返回 normalized `Output`，清空 `SubmitRuntime.task`，设置 `last_submit_outcome = Some(Success)`。

外部数据刷新：

- `gpui-form` 不自动覆盖 dirty form。
- 若表单 pristine，可由 app 调用 `reset_from_value(new_value)`。
- 若表单 dirty，返回或设置 `FormExternalConflict` 状态，由 app 决定提示用户、覆盖、合并或丢弃。

## 数据监听和生命周期

这是实现重点，不能依赖 `.detach()`。

第一阶段推荐结构：

- 生成的 form store 是一个 `Entity<GeneratedFormStore>`。
- 字段 store 是 form store 的普通字段，不单独成为 `Entity`。
- 每个字段 store 内部可以持有 `Entity<InputState>`、`Entity<SelectState<_>>` 等组件 state。
- 组件 state 的事件订阅在 `GeneratedFormStore::new/from_value` 中安装。
- 订阅回调通过 form entity 更新对应字段 store。
- 返回的 `Subscription` 存入字段 store 或 form store 的 `_subscriptions`。
- 表单被 drop 时，字段和 subscriptions 一起 drop，监听自动取消。
- 动态数组 item 被 remove 时，该 item 的 subscriptions 必须随 item drop，不影响其他行。
- 动态数组 reorder 时，不重新安装仍存在 item 的 subscriptions。
- submit normalize 写回使用显式 `FieldChangeCause::NormalizeOnSubmit`。
- normalize 写回组件 state 时，form store 设置短生命周期的 `is_normalizing_on_submit` 或等价 guard；
  组件事件订阅看到该 cause/guard 时只同步必要 state，不触发 `on_change` validation，避免递归提交或重复校验。

示意：

```text
SettingsPanel
  owns Entity<ConnectionFormStore>

ConnectionFormStore
  owns TextFieldStore.display_name
    owns Entity<InputState>
    owns Vec<Subscription>
  owns SelectFieldStore.kind
    owns Entity<SelectState<ConnectionKind>>
    owns Vec<Subscription>

CommandFormStore
  owns FieldArrayStore.environment
    owns FieldArrayItem<FormItemId(1)>
      owns Entity<EnvironmentRowFormStore>
        owns TextFieldStore.key
        owns TextFieldStore.value
```

异步任务：

- 第一阶段不做 async validator，字段校验不需要 `validation_task`。
- `gpui-form` 的 submit 只做同步校验和 output 构造，不持有 app 的保存任务。
- 后续如果引入 async validator，字段级异步校验任务必须保存在 field store：`validation_task: Option<Task<()>>`。
- 后续如果引入 async options source，options load task 必须保存在 field store 或 array item store。
- 新任务替换旧任务时，旧任务通过 drop 取消。
- 异步校验结果必须带 `revision`，只允许写回仍匹配当前字段 revision 的结果。
- UI 线程更新必须通过 foreground task 或 entity update，不从 background task 直接改 entity。

明确禁止：

- 在字段监听中调用 `.detach()`。
- 在字段组或动态数组 item 的监听中调用 `.detach()`。
- 在选项加载中调用 `.detach()`。
- 在 validator 中把 task 脱离字段或表单生命周期。
- 在 render 里创建新的 subscription。
- 在一个 entity update 内嵌套更新同一个 entity。

## 全局数据管理

第一阶段不设计全局表单注册表。

- 表单状态由打开表单的 view/dialog 持有。
- `gpui-form` 不使用 `Global` 存储所有活跃表单。
- `gpui-form` 不主动接入 `gpui-store`。
- dirty/unsaved guard 由 app owner view 根据 `form.meta().is_dirty` 接入。
- 需要跨 view 共享同一表单时，由 app 显式传递同一个 `Entity<GeneratedFormStore>`。
- 表单只作为 view/dialog 局部编辑状态；成功 submit 后再写入 app 现有数据源。

后续如果需要全局协调：

- app 可以实现自己的 unsaved changes registry。
- `gpui-store` 可作为 app-level 数据源，但不应成为表单字段本身的必选依赖。

## 数据库变更

`gpui-form` 第一阶段没有数据库变更。

- 不新增 migration。
- 不新增 schema。
- 不保存 draft。
- 不保存 validation error。
- 不直接调用 Diesel、SQLite 或 repository。
- app 只有在 `form.submit(...) -> Ok(output)` 后，才调用自己的 repository/config command。

接入某个 app 时，如果表单暴露出已有数据模型需要变更，migration 属于该 app 的开发计划，不属于
`gpui-form` crate 本身。

## 数据获取方式

初始数据：

- 由 app 从 repository/config/runtime 获取。
- app 构造 domain input struct。
- `GeneratedFormStore::from_value(value, window, cx)` 创建表单 draft。

选项数据：

- 第一阶段支持静态 options snapshot 和 app-driven update。
- select/combobox 字段通过 `delegate = "DelegateType"` 确定 `SelectState<D>` / `ComboboxState<D>` 的
  具体类型；通过 `options = "..."` 提供初始 delegate 构造表达式。
- 如果没有配置 `options = "..."`，delegate 类型必须实现 `Default`。
- `SelectFieldStore<Value, D>` 使用 `SelectFieldValue` 把 domain value 映射到 `Option<D::Item::Value>`；
  当前内置支持 `Option<T>`。非 `Option` 的 required enum 需要业务类型显式实现该 trait。
- `ComboboxFieldStore<Value, D>` 使用 `ComboboxFieldValue` 把 domain value 映射到
  `Vec<D::Item::Value>`；当前内置支持 `Vec<T>` 和 `Option<T>`。
- combobox 由于 gpui-component 当前只暴露按 index 写回选择，field store 会保存一份 delegate snapshot
  用于 normalize/reset 时按 value 查找 index。
- app 数据源变化时，显式调用 `field.set_items(...)` 或 form-level generated helper。
- 如果当前 value 不在新 options 中，字段进入 `OptionMismatch` 错误或 warning，具体 severity 待接入点确认。

动态数组数据：

- 初始数组从 domain `Vec<T>` 生成 `FieldArrayStore<Item>`.
- submit 成功后按当前 item 顺序输出 `Vec<T>`.
- remove/reorder 只影响 draft，不在 submit 前修改原始 domain `Vec<T>`.
- 如果 domain item 有业务 ID，输出时保留业务 ID；`FormItemId` 只用于表单运行时。
- 数组 item 类型必须表达业务语义。不同用途的数组字段使用独立 row input/store，字段名、placeholder、
  校验规则都在 row 内部声明，不由父数组字段运行时改写 child state。

后续阶段再考虑 `OptionsSource`：

```rust
pub trait OptionsSource<Item>: 'static {
    fn load(&self, window: &mut Window, cx: &mut App) -> Task<Result<Vec<Item>, FormError>>;
    fn subscribe(
        &self,
        callback: OptionsCallback<Item>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Subscription>;
}
```

`OptionsSource` 一旦进入实现，也必须把 load task 和 subscribe 返回值保存到字段或表单中。

## 校验和提交变换模型

`gpui-form` 不实现大型规则库。核心只定义 trigger、scope、report、path 映射、submit transform 和
adapter pipeline。具体 validation 交给 `garde`，normalize/sanitize 交给 `validify::Modify`。
第一阶段要求 normalized output 和 validation input 是同一个 domain input 类型；app-specific repository
command 转换在 `submit -> Ok(input)` 之后由 app 完成。

触发类型：

```rust
pub enum ValidationTrigger {
    Mount,
    Change,
    Blur,
    Submit,
    Dynamic,
}
```

validation scope：

```rust
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}
```

validation adapter trait：

```rust
pub trait ValidationAdapter<Draft>: 'static {
    fn validate(
        &self,
        draft: &Draft,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: &ValidationContext,
    ) -> ValidationAdapterReport;
}
```

submit transform trait：

```rust
pub trait SubmitTransform<Draft, Output>: 'static {
    fn preview(
        &self,
        draft: &Draft,
        context: &TransformContext,
    ) -> Result<Output, TransformReport>;

    fn transform_on_submit(
        &self,
        draft: &Draft,
        context: &TransformContext,
    ) -> Result<Output, TransformReport>;
}
```

cross-field validation：

- 第一阶段支持字段组和表单级 cross-field validator。
- cross-field 错误必须落到具体字段路径，例如 `headers[3].value` 或 `transport.http.url`。
- 如果确实没有具体字段归属，才允许进入 `form_errors`，用于提交失败或外部服务错误。
- third-party adapter 无法表达的 app 状态规则通过 app-defined form validator 补充，结果仍必须映射到具体
  `FieldPath`。

adapter 报告：

```rust
pub struct ValidationIssue {
    pub path: FieldPath,
    pub source: ValidationSource,
    pub trigger: ValidationTrigger,
    pub severity: ValidationSeverity,
    pub code: Cow<'static, str>,
    pub message_key: Cow<'static, str>,
    pub params: ErrorParams,
}

pub enum ValidationSource {
    Garde,
    App(&'static str),
    Internal,
}
```

`garde` validation adapter：

- 输入结构体实现 `garde::Validate`。
- `on_change`、`on_blur`、`on_submit` 都调用只读 validate。
- 支持 `required`、`length`、`range`、`url`、`email`、`pattern`、`dive`、`custom` 等由 `garde`
  提供的规则。
- `garde` 不负责 normalize/sanitize。

`validify` submit transform：

- 输入结构体实现 `validify::Modify`。实践上可以通过 `#[derive(validify::Validify)]` 生成 `Modify`
  实现，但 `gpui-form` 只调用 `Modify::modify`，不使用 `validify::Validate` 或 `Validify::validify`
  的校验结果。
- `preview` 在 clone 上调用 `Modify::modify`，用于 live validation 的只读校验输入。
- `transform_on_submit` 在 clone 上调用 `Modify::modify`，返回 normalized output。
- submit pipeline 会把 normalized output 写回表单 draft 和 component state，无论后续 `garde` 校验是否通过。

保留的 internal validation：

- 字段路径存在性、adapter path 映射失败、动态数组 item identity 冲突等内部错误。
- 组件 state 和 domain value 转换失败。
- third-party adapter 无法表达的极少量运行时 invariant。

第三方校验库适配：

- `garde` validation adapter 和 `validify` submit transform 是第一阶段 adapter。
- 依赖通过 feature flags 隔离，核心状态机不绑定到某个库。
- validation adapter 只能产出 `ValidationIssue`，不能决定 UI 生命周期、错误可见性或组件状态。
- transform adapter 只能产出 normalized output，不能自行提交保存，也不能绕过 `garde` 校验。

## i18n

`gpui-form` 输出结构化文本，不直接输出最终文案。

```rust
pub enum FormText {
    Key(&'static str),
    Literal(&'static str),
    Message {
        key: &'static str,
        params: ErrorParams,
    },
}

pub trait FormTextResolver: 'static {
    fn resolve(&self, text: &FormText, cx: &App) -> SharedString;
}
```

规则：

- field label、description、help、placeholder、search_placeholder 使用 derive 属性里的 `FormText`。
- `gpui-form` 不依赖 app-local `I18n`；默认 resolver 可以把 key 当 literal 返回，接入 app 负责提供
  `FormTextResolver` 把 Fluent key 转成最终文案。
- `ComponentStateOptions` 只保存 `FormText`，binding 在 `new_state` 时通过 resolver 或 app-provided helper
  写入 `InputState` / `SelectState` 等组件状态。
- validation error 使用 adapter 产出的 `message_key + params`。
- `garde` 的 rule name、code、message 映射为稳定 Fluent key；app 可以提供 override map。
- `validify` transform 本身通常不产生校验错误；如果 transform adapter 无法把 normalized output 写回某个字段，
  使用 internal error key。
- crate 可定义稳定的通用 key 名称，例如 `gpui-form-error-required`，但具体 `.ftl` 文件由接入 app 提供。
- app 可以 override 内置错误 key。
- `gpui-form` 第一阶段不新增 app-local locale 文件。

接入 app 时需要在对应 `locales/{en-US,zh-CN}/main.ftl` 补齐实际文案。

## Icon

`gpui-form` 不直接依赖 app-local `IconName`，只输出语义。

```rust
pub enum FormIconKind {
    Error,
    Warning,
    Success,
    Info,
    Loading,
    Required,
    AddItem,
    RemoveItem,
    ReorderItem,
}
```

建议映射：

| `FormIconKind` | 建议 Lucide 语义 | 由谁决定 |
| --- | --- | --- |
| `Error` | `CircleAlert` | app 或 gpui-component |
| `Warning` | `TriangleAlert` | app 或 gpui-component |
| `Success` | `CircleCheck` | app 或 gpui-component |
| `Info` | `Info` | app 或 gpui-component |
| `Loading` | `LoaderCircle` / spinner | app 或 gpui-component |
| `Required` | 不强制 icon，可用文本 marker | app |
| `AddItem` | `Plus` | app |
| `RemoveItem` | `Trash2` | app |
| `ReorderItem` | `GripVertical` | app |

`gpui-form` 不新增 SVG asset，不改 app asset set。

## 新增依赖

计划依赖必须在进入实现前确认版本和用途。当前建议：

| crate | 版本/来源 | 用途 | 阶段 |
| --- | --- | --- | --- |
| `gpui` | workspace | `Entity`、`Context`、`Subscription`，以及后续 async task 扩展 | 第一阶段 |
| `gpui-component` | workspace | `InputState`、`SelectState`、`ComboboxState` 和表单组件适配 | 第一阶段 |
| `gpui-form-macros` | path | derive 宏 | 第一阶段 |
| `garde` | `0.23.0` optional, `default-features = false`, `features = ["derive", "url", "email", "pattern"]` | validation-only adapter | 第一阶段 |
| `validify` | `2.0.0` optional | submit-time normalize/sanitize via `Modify` | 第一阶段 |
| `syn` | `2.0.118` | proc-macro 解析 | 第一阶段宏 crate |
| `quote` | `1.0.45` | proc-macro 代码生成 | 第一阶段宏 crate |
| `proc-macro2` | `1.0.106` | proc-macro token 类型 | 第一阶段宏 crate |
| `thiserror` | `2.0.18` | 只有 public error 需要 `Error` impl 时再加 | 待确认 |

暂不新增：

- `serde`：除非 app 数据层本来需要；`gpui-form` 不依赖 validify payload。
- `validator`：先不做，避免同时支持三个校验库导致 adapter surface 发散。
- `gpui-store`：后续 app-level integration。

feature 计划：

```toml
[features]
default = []
garde-adapter = ["dep:garde"]
validify-transform = ["dep:validify"]
form-pipeline = ["garde-adapter", "validify-transform"]
```

## 迁移和接入计划

阶段 0：设计固定

- 完善 README 用户示例。
- 完善本文档中的模块结构、核心类型和待确认问题。
- 固定 `FormItemId` 为表单生命周期内单调递增的 `u64` newtype runtime identity。
- 固定动态数组 add/remove/reorder/reset API。
- 固定 submit pipeline：`validify::Modify` -> write back -> `garde::Validate`。

阶段 1：crate 骨架

- 新增 `gpui-form-macros` crate。
- `gpui-form` re-export derive 宏。
- 建立模块文件，不实现复杂行为。
- 补最小 compile tests。

阶段 2：同步字段和基础校验

- 实现 `TextFieldStore`、`BoolFieldStore`、`NumberFieldStore`。
- 实现 `FieldGroupStore`、`FieldArrayStore`、`FormItemId`。
- 实现 `FieldMeta`、`FormMeta`、`FieldError`、`ValidationTrigger`。
- 实现 `ValidationAdapter`、`ValidationIssue`、`ValidationAdapterReport` 和 submit report。
- 单元测试覆盖 dirty/touched/blurred/error visibility/submit/nested group/dynamic array。

阶段 3：组件状态和订阅

- 定义 `FormComponentBinding<Value>` 和 `ComponentStateOptions`。
- 接入 `InputState`、`SelectState`、`ComboboxState` 等内置 binding。
- `component = "input"` / `"select"` / `"combobox"` 等只作为内置 binding 的属性语法糖。
- 所有 `Subscription` 存入字段、字段组、动态数组 item 或表单。
- 验证 drop 表单后监听取消。
- 验证 remove dynamic item 后只取消该行监听。
- 验证 reorder dynamic item 不重建仍存在行的 state。
- 验证不使用 `.detach()`。
- 验证 `placeholder`、`search_placeholder`、`disabled`、`mask` 等字段属性由 binding `new_state`
  应用，不需要 app 后置修正组件 state。

阶段 4：derive 宏

- 解析 `#[form(...)]` 属性。
- 生成 form store、field store、field group、field array 初始化和 submit output 构造。
- 生成 `garde` validation adapter 初始化、`validify` submit transform 初始化，以及 draft/output 写回映射代码。
- 生成 binding state 创建和订阅安装代码。
- compile-fail tests 覆盖不支持字段类型、缺失 binding、非法属性、数组 item 无法生成 form group。

阶段 4.5：Required 字段语义

- 解析 `#[form(required)]` 和 `#[form(required = bool)]`，默认 false。
- 未知 field attribute 改为编译期错误，避免 `required` 拼写错误被静默忽略。
- `FieldCore`、`FieldGroupStore`、`FieldArrayStore` 保存 required 元数据。
- `ComponentStateOptions` 把 required 传给 binding；内置 binding 第一阶段 no-op。
- 宏生成 `<field>_required()` 和 `set_<field>_required(required, window, cx)`。
- README 示例改为真实 API；验证 `field().required(...)` 与 gpui-component Form 组合。

阶段 5：下游基础表单接入验证

- 选择一个下游 app 的基础表单作为 API smoke test；具体 app 名称、字段、文案和保存流程写入该 app 的开发文档。
- 验证 `validify::Modify` submit-time trim / optional 空值转换，以及 `garde::Validate` validation。
- 验证 `gpui_component::form`、`Input`、`Select` 等组件能通过 binding 正常接入。
- 验证 required helper 能驱动 `gpui_component::form::field().required(...)`，且不自动改变 validation 结果。
- 验证下游 Provider/MCP 这类已迁移表单也能消费 required，不把 required 只限制在后续 Prompt/Shortcut 表单。
- 验证 submit 成功后由 app 自己调用 repository/config command，`gpui-form` 不直接持久化。
- 不在 `gpui-form` 中引入数据库变更。

阶段 6：下游动态数组接入验证

- 选择一个下游 app 的动态数组表单作为 API smoke test；具体业务 row 类型写入该 app 的开发文档。
- 覆盖嵌套字段组、动态 row、cross-field validation。
- 覆盖动态 row 的条件式 required，例如 header row 只填一侧时另一侧必填；空 row 不能被静态 required
  marker 误报。
- 验证不同语义的动态 row 拆成独立 input/store，row 内部字段直接声明 placeholder 和校验规则。
- 验证 `validify::Modify` 规范化动态 row 的 key/value，使用 `garde::Validate` 做字段和 cross-field validation。
- 验证动态字段 add/remove/reorder 的 i18n 文案和 icon 由接入 app 负责。
- 验证 remove/reorder 时 subscriptions 和组件 state 生命周期正确。

阶段 7：后续扩展

- async validator。
- options async source。
- `validator` adapter，如果后续确实有需求。
- `gpui-store` integration。

## 验证计划

每阶段至少执行：

- `cargo fmt`
- `cargo check -p gpui-form`
- `cargo test -p gpui-form`
- `cargo test -p gpui-form --features form-pipeline`

新增 macro crate 后追加：

- `cargo check -p gpui-form-macros`
- macro compile/pass-fail tests
- required 属性测试：默认 false、显式 true/false、unknown attribute compile error、generated getter/setter、
  `ComponentStateOptions.required` 传递、required 不自动触发 validation。

接入 app 后追加：

- 对应 app 的 targeted `cargo check`
- 覆盖被接入表单的单元测试或交互测试
- 如涉及 UI 行为，使用实际 app smoke test 验证错误展示、submit 拦截和 dirty 状态

## 当前结论

当前设计方向可行，已确认：

- async validation 第一阶段不做。
- nested field group、dynamic field array 第一阶段要做。
- built-in component 隐式决定 state 类型。
- 具体 app 接入计划不写入本 crate 文档；本 crate 只保留通用 API 和验证要求。
- cross-field validation 错误落到具体字段。
- validation 交给 `garde` adapter，normalize/sanitize 交给 `validify::Modify` transform，`gpui-form` 不重复实现专业规则库。
- submit 被点击后，normalized output 会写回表单 draft 和组件 state，无论后续校验成功还是失败。
- `FormItemId` 使用表单生命周期内单调递增的 `u64` newtype runtime identity，不使用业务 ID 或 UUID。
- 组件接入统一抽象为 `FormComponentBinding<Value>`；`custom` 不再作为特殊组件种类，所有内置组件和 app
  自定义组件走同一套 binding 生命周期。
- 字段级 UI 选项通过 `ComponentStateOptions` 传给 binding 的 `new_state`；placeholder 由 row 内部字段声明，
  不由父数组字段动态覆盖 child component state。
- 动态数组行要拆成语义化 row input/store，不能依赖父数组字段动态覆盖 child component state。
- required 字段能力已补齐：`#[form(required)]` 默认 false、生成 required getter/setter、使用
  gpui-component `field().required(...)` 渲染 marker，并保持 validation 仍由 `garde` / app validator 负责。

## 当前实现进度

已完成：

- `crates/gpui-form` 加入 workspace，并实现运行时模块入口。
- `crates/gpui-form-macros` 加入 workspace，并 re-export `#[derive(FormStore)]`。
- `crates/gpui-form` runtime 文件已从一级平铺整理为 `core` / `component` / `pipeline` / `view`
  分组；`src/lib.rs` 只保留新的分组模块和常用类型 re-export，不保留旧平铺模块 alias。
- `crates/gpui-form-macros/src/expand.rs` 已拆分为主编排文件和 `expand/accessors.rs`、
  `expand/arrays.rs`、`expand/fields.rs`、`expand/pipeline.rs`、`expand/validation.rs`，避免把字段
  初始化、访问器、数组 helper、validation routing 和 submit pipeline 全部混在单个文件里。
- `FormItemId(u64)`、`FormItemIdGenerator`、`FieldArrayStore` 已实现 add/insert/remove/remove_id/move/swap/reset/replace。
- `FieldPath`、`FieldMeta`、`FormMeta`、`FieldError`、`FormValidationReport`、`ValidationTrigger` 已实现。
- `ValidationAdapter`、`GardeAdapter`、`SubmitTransform`、`ValidifyTransform` 已实现；`validify` 只调用
  `Modify::modify`，不使用 validify validation result。
- `TextFieldStore`、`NumberFieldStore`、`SelectFieldStore`、`ComboboxFieldStore`、`BoolFieldStore`、
  `ComponentFieldStore` 已作为 runtime holder 类型实现。
- `SubscriptionSet` 已实现，用于字段、字段组、动态数组和表单保存 `Subscription`。
- derive 宏已支持 named struct 生成 `from_value(value, window, cx)`、`draft`、`write_draft`、`field_paths`
  和 `FormStore` impl。
- derive 宏已支持默认 value 字段、`input`、`number`、`select`、`combobox`、`checkbox` / `switch`、
  `group`、`array`、`binding` 字段。
- 旧 `component = "custom"` / `state = "..."` 接入点已移除；app 自己提供的组件通过
  `#[form(binding = "...")]` 和 `FormComponentBinding<Value>` 接入。
- derive 宏会通过 `TextInputBinding` / `NumberInputBinding` 为 `input` / `number` 字段创建
  `InputState`，通过 `SelectBinding` / `ComboboxBinding` 为 `select` / `combobox` 字段创建
  `SelectState<D>` / `ComboboxState<D>`，通过 `BoolBinding` 为 bool 字段创建 `BoolComponentState`；
  component event 订阅会保存到字段 store。`NumberInputBinding` 的 render helper 目标是
  `NumberInput::new(&state)`，不是普通 `Input::new(&state)`。
- derive 宏已解析 `placeholder`、`mask` / `masked` 字段属性；`input` / `number` 字段创建 `InputState`
  时会通过 `FormTextResolver` 解析 placeholder，并在 builder 阶段应用 masked state。
- derive 宏已为字段生成类型化访问器：`field_value()`、`field_input_state()`、`field_select_state()`、
  `field_combobox_state()`、`field_state()`、`field_store()`、`field_items()`，避免 app 为每个字段重复手写
  component state getter。
- derive 宏已为普通字段生成 `set_field_value(value, cause, window, cx)`，用于 app action 直接修改单字段；
  setter 会同步字段 draft、对应 component state、字段级 validation trigger、typed event、form meta 和 notify。
- derive 宏已生成 `clear_all_errors(cx)`、`clear_field_errors(field, cx)`、`apply_field_error(field, error, cx)`；
  app-specific validation 或服务错误只需要映射到 generated field enum，不需要手写每个字段的
  `mark_touched` / `set_errors`。
- derive 宏已支持 `validate(on_change, on_blur, on_submit)` 的实时校验触发；字段级 live validation 使用
  `ValidationScope::Field`，只把当前 scope 的错误写回字段 store。
- derive 宏已支持 `#[form(validation(adapter = "garde"), transform(adapter = "validify"))]`，submit 时先运行
  `validify::Modify` 并写回 draft/component state，再运行 `garde::Validate`。
- submit validation report 已写回字段错误和 form-level errors，`focus_first_error` 可以基于字段错误聚焦。
- nested group validation report 会把 parent report 中的 `group.child` 路径 strip 成 child 相对路径后写入
  child store；不会把 parent root field 与 child 相对 field 同名的错误 merge 到 child group。
- dynamic array 宏已生成 `field_append` / `field_insert` / `field_remove` / `field_remove_id` /
  `field_move` / `field_swap` / `field_replace` / `field_reset_items` / `field_values_with_id` helpers；
  append/insert/replace/reset 负责创建 child store entity，并把 child observe subscription 保存到对应
  `FieldArrayItem`。
- dynamic array submit validation report 会按当前 index 把 `array[index].child` 路径 strip 成 child 相对路径后
  写入对应 child store；reorder 移动 item id 和 child state，不重建仍存在行。
- submit normalize 写回已使用 `FieldChangeCause::NormalizeOnSubmit` 和 `is_normalizing_on_submit` guard。
- derive 宏已接入 app component binding 的 `FormComponentBinding::install_subscriptions`，返回的订阅会保存到字段 store。
- derive 宏已接入 app component binding 的事件同步：custom binding 可通过
  `FormComponentBinding::event_kind(...)` 把自定义事件映射为 `Change` / `Focus` / `Blur`，由宏统一同步 draft、
  meta、validation trigger 和 typed form event。
- required 字段支持已实现：`#[form(required)]` / `#[form(required = bool)]`、`FieldCore` /
  `FieldGroupStore` / `FieldArrayStore` required 元数据、`ComponentStateOptions.required`、
  `<field>_required()`、`set_<field>_required(...)` 和 binding `set_required(...)` 都已落地。
- 已补测试覆盖 u64 item id 生命周期、字段路径、字段 meta 更新、组件字段宏展开、订阅保存、select/combobox
  事件同步、bool state 写回、nested group child draft/meta 同步、dynamic array child draft/meta 同步、
  array append/remove/remove_id/reorder id 生命周期、field setter、clear/apply errors、nested/array
  validation 错误分发、normalize 写回、binding 组件订阅安装、live validation scope、submit 错误写回和
  `validify -> garde` submit 顺序。

待补：

- 第一阶段 required 只表达字段元数据和 UI marker，不自动生成 required validation；如后续需要通用 required
  validator，需要另行设计 `gpui-form-error-required`、adapter 触发时机和 app locale 接入方式。
- `NumberInputBinding` 还需要按 `number-input-design.md` 完成 raw input dirty/default 修复：invalid raw edit
  必须让 field/form dirty，parse 成功但 typed value 不变的 raw edit 也必须按 raw 文本判断 dirty。
