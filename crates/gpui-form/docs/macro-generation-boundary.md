# gpui-form derive generation boundary

状态：已实施。derive 只生成 form domain glue，不生成或配置 UI component state；组件连接由调用方
使用 component-specific adapter 和 caller-owned `SubscriptionSet` 完成。

## 1. 原则

宏生成代码必须满足：

1. 只依赖 `gpui-form` core public runtime；
2. 输入 schema 只描述 value/draft/validation/group/array；
3. UI library、component entity、options、placeholder 和 subscription 不进入 schema；
4. 重复的状态算法放 runtime 类型，不在每个字段展开；
5. 生成 API 保持 typed、可发现，并能被 adapter crate 使用。

## 2. 支持的字段形态

| 字段 | 属性 | 生成 store |
| --- | --- | --- |
| typed leaf | 无 codec | `DraftFieldStore<Value, IdentityCodec<Value>>` |
| raw-draft leaf | `#[form(codec = "...")]` | `DraftFieldStore<Value, Codec>` |
| group | `#[form(group(store = "..."))]` | `FieldGroupStore<ChildStore>` |
| array | `#[form(array(store = "..."))]` | `FieldArrayStore<ChildStore>` |

示例：

```rust
#[derive(FormStore)]
#[form(store = "ConnectionFormStore")]
pub struct ConnectionInput {
    #[form(validate(on_change, on_submit))]
    pub name: String,

    pub mode: ConnectionMode,

    #[form(codec = "PortCodec", validate(on_blur, on_submit))]
    pub port: u16,

    #[form(group(store = "AuthFormStore"))]
    pub auth: AuthInput,
}
```

`group` / `array` 是 form 结构，不是 UI component。目标 parser 不再接受
`component = "group"` / `component = "array"`；旧写法 compile-fail，并提示迁移到对应结构属性。

## 3. Leaf expansion

对字段 `model: ModelId`，宏负责生成：

- `DraftFieldStore<ModelId, Codec>` 字段；
- `model_draft()` / `model_value()`；
- `set_model_draft(...)` / `set_model_value(...)`；
- `model_handle(&Entity<Self>)`；
- generated `<Form>Field` identity；
- internal typed-handle event 和 public `FormStoreEvent<<Form>Field>`；
- validation/submit path glue；
- meta aggregation glue。

概念展开：

```rust
pub struct ConnectionFormStore {
    model: DraftFieldStore<ModelId, IdentityCodec<ModelId>>,
    // form-level validation/submit state
}

impl ConnectionFormStore {
    pub fn model_handle(
        form: &Entity<Self>,
    ) -> FormFieldHandle<Self, ModelId> {
        FormFieldHandle::new(
            form.downgrade(),
            FieldPath::field("model"),
            |form| form.model.draft().clone(),
            |form, draft, cause, cx| form.set_model_draft(draft, cause, cx),
        )
    }
}
```

具体 field store 更新、parse error、dirty、baseline 和 validation trigger 算法留在 runtime；宏只传 path、codec
和 attribute policy。

## 4. Form construction and replacement

`from_value` 为每个 leaf 调用 `DraftFieldStore::new`，为 group/array 创建 child stores。它不需要 `Window` 来
创建 component entity；`reset` / `replace_from_value` 同样只需要 form context。只有 submit handler 或其他真正
需要窗口副作用的 API 才接收 `Window`。

`replace_from_value` 为 leaf 调用 `replace_baseline`，递归替换 group/array，并只为 draft mirror 实际变化的字段
发出事件。baseline/meta 变化仍通过 `cx.notify()` 使 form view 失效；宏不能触碰 adapter subscriptions 或 component
state。

## 5. Validation and submit expansion

生成的 validation/submit pipeline 只读 stores：

```text
DraftFieldStore::prepare_submit
  -> codec parse
  -> field validation
  -> form validation
  -> transform/normalize
  -> final validation report
  -> typed output
```

宏不生成“提交前从 component state 同步”的代码。mounted UI 与 unmounted/headless form 必须得到相同结果。

## 6. Generated field identity and events

derive 保留一个强类型字段枚举，因为 validation/error routing、form-wide persistence observer 和 app 字段映射仍需
要稳定、可穷举的字段 identity：

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConnectionFormField {
    Name,
    Mode,
    Port,
    Auth,
}

impl ConnectionFormField {
    pub const fn key(self) -> &'static str;
    pub fn path(self) -> FieldPath;
}
```

宏不再为每个 form 生成独立的 `ConnectionFormEvent` enum。`gpui-form` runtime 提供统一页面级事件：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormStoreEvent<Field> {
    FieldChanged {
        field: Field,
        cause: FieldChangeCause,
    },
}
```

generated store 实现 `EventEmitter<FormStoreEvent<ConnectionFormField>>`。该事件供页面级 dirty/persistence/业务
分发使用，不携带 heterogeneous draft，也不要求监听者回读 component。focus/blur 不再是独立事件 variant；blur
作为 `FieldChangeCause::Blur` 进入字段变化和 validation 流程。

### Adapter field handle

field handle constructor 是 core 与 UI adapter 的唯一宏生成边界。handle 包含：

- weak form entity 和 generated field path；
- typed draft reader；
- typed draft writer；
- `FieldDraftEvent<Draft>` filtering/subscription 能力。

form entity 另外实现 internal `FormDraftEvent` emitter。宏在每个 draft setter 中写入 generated `FieldPath`、
`Rc<dyn Any>` owned draft 和 cause；`FormFieldHandle` 是唯一做 path filter/downcast 的位置。一次有效 draft 变化
同时发 internal mirror event 和 public `FormStoreEvent::FieldChanged`，两者都携带当前 update 中已经拥有的数据，
监听者不得在回调中重新 update/read 当前正在更新的 form entity。

handle 不包含：

- component entity/type；
- item/options provider；
- component disabled/required/config setter；
- label/description/placeholder；
- validation error renderer；
- repository/store binding。

## 7. Required contract

`required` 是 validation/meta，不是 component config。静态规则仍由 `#[form(required)]` 声明；每个 leaf/array
生成纯 form API，供 transport/mode 等表单内依赖动态调整规则：

```rust
pub fn model_required(&self) -> bool;

pub fn set_model_required(
    &mut self,
    required: bool,
    cx: &mut Context<Self>,
);
```

setter 在值未变化时 no-op；变化时更新 field required meta、删除该 path 已失效的 internal required error，并在字段
启用 `validate(on_dynamic)` 时以 `ValidationTrigger::Dynamic` 重算该字段，最后 refresh form meta 并
`cx.notify()`。它不接收 `Window`，不访问 component，不发 `FormDraftEvent` 或
`FormStoreEvent::FieldChanged`，因为 draft 没有变化。UI 是否显示 required affordance 仍由 app/component config
决定。

## 8. Attribute surface

目标 leaf 属性只包含 form 语义，例如：

```rust
#[form(
    codec = "PortCodec",
    default,
    required,
    validate(on_change, on_blur, on_submit)
)]
```

label、description、placeholder、masked、disabled、component construction 等 UI 属性由 app render/component
configuration 表达，不进入 derive。

## 9. Compile diagnostics

被删除的 component binding/construction 属性必须 compile-fail，并给出直接迁移提示：

```text
component state is no longer created by gpui-form;
create the state in the application, call a component-specific bind function,
and retain its subscriptions in a caller-owned gpui_form::SubscriptionSet
```

旧 `component = "group"` / `component = "array"` 使用另一条迁移提示，分别指向 `group(store = ...)` /
`array(store = ...)`；不能把结构属性误报成需要 component adapter。

未知 codec、错误 group/array store、codec draft/value 类型不匹配仍由 macro/type system 给出字段级错误。

## 10. Tests

- identity leaf expansion；
- explicit codec leaf expansion；
- group/array nesting；
- typed field handle read/write/event；
- generated field enum key/path 和 `FormStoreEvent<Field>`；
- dynamic required 不需要 `Window`、不产生 draft event，并清除失效 required error；
- raw invalid draft survives validation；
- replace emits mirror event and replaces baseline；
- deleted UI attributes、binding 和旧 component group/array 语法 compile-fail；
- generated tokens 不引用 `gpui-component` 或 component entity API。
