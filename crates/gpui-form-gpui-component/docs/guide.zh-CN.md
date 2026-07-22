# gpui-form-gpui-component 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

> **实现状态：**这份指南描述已经实现的公开 API。

`gpui-form-gpui-component` 把原生 `gpui-component` state entity 适配到
类型化 `gpui-form` 字段。它不会创建另一份业务值，也不拥有应用配置。

## 创建并渲染 control

传入 generated field 与创建原生 state 的闭包：

```rust,ignore
use gpui_component::input::{Input, InputState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormInput;

let name_input = FormInput::new(
    ProviderInputFormStore::name_field(&form),
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
)?;

let element = Input::new(&name_input);
```

`FormInput` 是普通 Rust value，不是 `Entity<FormInput>`。它只包含普通
subscriptions 与原生 entity，并 deref 到该 entity：

```rust,ignore
use std::ops::Deref;
use gpui::{Entity, Subscription};

pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}

impl Deref for FormInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}
```

Subscriptions 先于 entity 声明，因此 Rust 会先释放它们。其他有状态 adapter
采用相同布局。它们不保存 field、`ControlAttachment`、delegate、`Config`、focus
flag、blur flag 或 validation report；binding 细节只存在于 subscription closure。

重复调用 generated field accessor 是安全的。`FormField` 只是指向同一个 form path
的轻量类型化 handle，不会创建另一份 value 或 subscription。

## 同步与生命周期

Form 持有唯一权威的类型化值。原生 state 只持有当前 presentation projection，以及
focus、IME、selection、query、popup、highlighted item 等交互细节。

所有有状态 adapter 都遵循同一套同步规则：

1. Constructor 读取 field，创建原生 state，静默投影初值，并安装双向 subscriptions；
2. Component event 等 emitting entity 的 update 结束后，再 defer 类型化 form 写入；
3. Form subscription 响应每个 `FieldChanged` 与 `ModelReplaced` 并重新投影，包括值相等的
   whole-form lifecycle，以及其他 path 的事件导致当前 projection 变化的情况；只忽略
   `RuntimeChanged`；
4. 原生 silent setter 不会再发出 user event，因此 round trip 会自然终止，不需要
   origin-echo skip 或 value read-back API。

`FormField` 唯一公开的 attachment 创建入口是：

```rust,ignore
pub fn attach_control(
    &self,
    cx: &mut App,
) -> Result<ControlAttachment<Form, T>, FormFieldError>;
```

`ControlAttachment` 实现 `Clone`；所有 clone 共享同一个 private lease 与 liveness state。
Component-event subscription callback 捕获一个 clone，只调用它的
`defer_set_user_value`、`defer_blur`、`defer_set_issue` 或
`defer_clear_issue` 窄 intent。这四个方法是唯一公开的 mutation API；weak lifetime、
source ID 与 control ID 都留在 core 内部。

普通 form-to-control projection closure 只捕获 typed field 与 weak native entity。拥有
lifecycle-scoped control draft issue 的 typed editor 可以额外捕获同一个 attachment clone，唯一
用途是在 programmatic silent projection 成功后调用 `defer_clear_issue`；当前内置 adapter 中只有
exact integer control 使用这一例外。Bound wrapper 的字段仍严格是 subscriptions 在前、native
state entity 在后，不把 field 或 attachment 存成另一个字段。Wrapper drop 会释放 subscription
持有的所有 clone；最后一个 clone drop 后，queued intent 失效，control-scoped issue 也不再
active。

Projected 或 identified path 消失时返回 `FormFieldError::ValueUnavailable`。Callback
不会发明 fallback，也不会把 component value 保留成第二个权威来源；它会通知 owner，
由结构页面释放或重建 stale control。

## 验证与错误

Adapter 只转发具体组件能够表达的事件：

| Control | 用户写入 | Blur |
| --- | --- | --- |
| `FormInput` | `InputEvent::Change` defer 一个 `String` 写入 | `InputEvent::Blur` 执行 field blur validation |
| `FormIntegerInput<N>` | 合法的类型化整数编辑；无效文本产生 control issue | 原生 input blur 执行 field blur validation |
| `FormSelect<D>` | `SelectEvent::Confirm(Option<Value>)` | 不支持：upstream 没有可靠的 composite final-blur |
| `FormCombobox<D>` | `ComboboxEvent::Change(Vec<Value>)` | 不支持：upstream 没有可靠的 composite final-blur |

`ComboboxEvent::Confirm` 会被明确忽略：`Change` 已经在每次 toggle 时提交当前 selection，
同时监听两者会把同一 selection 写入两次。

一次非相等 typed field write 先修改 model 与 revision，然后只清除和写入 path 相交的
required、structural、generated synchronous field bucket，以及相交的 async validation。
Adapter-wide issue 与 active control issue 都保留。随后执行 Change validation 并发出
`FieldChanged`。相等 field write 是完整 no-op。Whole-form lifecycle 使用
`ModelReplaced`；即使 replacement model 比较相等，mounted control 仍会重新投影。

Bound handle 不保存 `focused`、`blurred`、`touched`、`show_error` 或 validation
report 副本。数据级状态通过 generated field 读取：

```rust,ignore
use gpui_form::FormFieldId as _;

let field = ProviderInputFormStore::name_field(&form);
let is_validating = field.is_validating(cx)?;
let error = field.errors(cx)?.into_iter().next();
let required = ProviderInputField::Name.schema().is_required();
```

同一字段被渲染多次时，所有实例都会读取同一份数据级 issue。Submit 失败后由当前页面
选择需要 focus 的可见 control；form 与 adapter 都不拥有这个选择。

## Select

`FormSelect<D>` 只绑定 `Option<D::Item::Value>`。使用应用拥有的 items 与配置创建
state：

```rust,ignore
use gpui_component::select::{Select, SelectState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormSelect;

let model_select = FormSelect::new(
    ProviderInputFormStore::model_id_field(&form),
    move |window, cx| {
        SelectState::new(ModelDelegate::new(models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Select::new(&model_select);
```

用户确认时，event 中的 `Option<Value>` 会直接 defer 到 form。Form projection 对
`Some` 调用 `set_selected_value`，对 `None` 调用 `set_selected_index(None)`。两者
都使用原生 state 的当前 delegate 解析，并且不会发出 user event。

Adapter 不保存 delegate，也不提供 adapter-specific item updater。无法解析的
`Some(value)` 只会清空原生 selection；typed form value 保持不变，交给应用拥有的
dynamic validation 处理。

## Combobox

`FormCombobox<D>` 只绑定 `Vec<D::Item::Value>`：

```rust,ignore
use gpui_component::combobox::{Combobox, ComboboxState};
use gpui_form::FormControl as _;
use gpui_form_gpui_component::FormCombobox;

let tags = FormCombobox::new(
    JobInputFormStore::tag_ids_field(&form),
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Combobox::new(&tags);
```

`ComboboxEvent::Change(values)` 把 `values` defer 到 form。每次 form change 都调用
upstream `ComboboxState::set_selected_values`。该方法使用当前 delegate 解析 value，
忽略无法解析的 value，保留输入顺序，更新 committed selection 与 snapshot，并且不
发出 `ComboboxEvent`。因此不会存在过期的 captured delegate 或 value/index mapping。

## 精确整数输入

`FormIntegerInput<N>` 把标准 signed/unsigned integer primitive 绑定到
`IntegerInputState<N>`。原生 state 持有类型化 `N`、私有文本 editor，以及类型化
min、max、step policy：

```rust,ignore
use gpui_form::FormControl as _;
use gpui_form_gpui_component::{
    FormIntegerInput, IntegerInput, IntegerInputState,
};

let budget = FormIntegerInput::new(
    JobInputFormStore::budget_field(&form),
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;

let element = IntegerInput::new(&budget);
```

Wrapper 会在安装 subscriptions 之前验证构造 policy：

- `step <= 0` 返回 `IntegerInputPolicyError::NonPositiveStep`；
- `min > max` 返回 `IntegerInputPolicyError::ReversedRange`。

Editor change 会被分类为 `Incomplete`、`InvalidSyntax`、`Overflow` 或
`OutOfRange { min, max }`。这些状态会保留 raw text，发布 lifecycle-scoped
validation issue，并且不写 form。合法编辑会清除 issue，并 defer 类型化 `N`；随后
产生的 form event 会把规范文本静默投影到所有实例。程序化 form write 是权威写入：只有
field read、weak-entity upgrade 与 silent projection 都成功后，它才会替换 stale raw text
并清除旧 editor issue；投影失败不得清除该 issue。应用写入的值是否违反业务范围，仍由
model 的 business validation 负责。

Increment/decrement 使用带类型边界的 `checked_add` 与 `checked_sub`。它们不会使用
`f64`、不会 clamp overflow，也不会丢失大于 `2^53` 的值。Blur 执行 field blur
validation；无效文本保留在输入框中等待修正。

Adapter 发出稳定 message key 与字符串参数，翻译由应用负责：

- `gpui-form-error-integer-incomplete`；
- `gpui-form-error-integer-invalid`；
- `gpui-form-error-integer-overflow`；
- `gpui-form-error-integer-min`，参数为 `min`；
- `gpui-form-error-integer-max`，参数为 `max`；
- `gpui-form-error-integer-range`，参数为 `min` 与 `max`。

## 无状态布尔 element

Upstream `Checkbox` 与 `Switch` 是没有公开 state entity 的 `RenderOnce` element，
因此不会增加假的 `FormBool` wrapper。直接把它们渲染为 controlled element，并把
用户值写入 `FormField<bool>`：

```rust,ignore
use gpui_component::{checkbox::Checkbox, switch::Switch};

let enabled_field = ProviderInputFormStore::enabled_field(&self.form);
let enabled = enabled_field
    .value(cx)
    .expect("ProviderPage 在 render 期间持有 form");

let checkbox_field = enabled_field.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .label("Enabled with checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });

let switch = Switch::new("provider-enabled-switch")
    .label("Enabled with switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        enabled_field
            .set_user_value(*checked, cx)
            .expect("element 挂载期间 ProviderPage 持有 form");
    });
```

这些 element callback 并不是从 component-state entity update 中发出，因此可以直接
写 field。页面对 form 的 observation 会重新渲染两个 controlled value。Change 与
submit validation 正常工作；这些 element 没有公开 focus handle，因此无法提供原生
blur validation。

只有当 render 在结构上确定拥有 form 与 path 时才适合使用这里的 `expect`。Projected
或 dynamic path 在正常流程中可能消失时，应使用 `?` 或显式错误处理。

## 修改 options 与组件配置

Options、delegate、placeholder、disabled state、size、accessibility、focus 与
catalog refresh 都属于应用。原生 state 配置放在构造闭包中，或通过 dereferenced
entity 修改；只属于 element 的 presentation 在 render 时配置。

替换 items 后，使用原生 setter 显式重新投影当前 form value，或者替换整个 bound
handle：

```rust,ignore
use gpui_form::{FormFieldId as _, ValidationScope, ValidationTrigger};

let selected_model =
    ProviderInputFormStore::model_id_field(&form).value(cx)?;
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags =
    ProviderInputFormStore::tag_ids_field(&form).value(cx)?;
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});

form.update(cx, |form, cx| {
    form.validate(
        ValidationTrigger::Dynamic,
        ValidationScope::Field(ProviderInputField::ModelId.path()),
        cx,
    );
});
```

Native item update 与当前 form value 的重投影必须作为一次 refresh 立即连续完成。不能等待后续
form value event：修改 options 本身不会写 form，因此不保证产生 `FieldChanged`。

Adapter 不会因为 item refresh 而选择 fallback、修改 form data、持久化配置或自动执行
dynamic validation。直接调用原生 setter 只改变 presentation projection；业务写入使用
`FormField::set`、`replace`、`reset` 或 `rebase`。

## 实现其他有状态 adapter

Core `FormControl<T>` 统一一次构造与绑定，但不统一 component configuration：

```rust,ignore
use std::ops::Deref;
use gpui::{Context, Entity, Window};
use gpui_form::{FormField, FormStore};

pub trait FormControl<T>: Deref<Target = Entity<Self::State>> + Sized
where
    T: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Error;

    fn new<Form, Owner, Build>(
        field: FormField<Form, T>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State;
}
```

实现返回普通 handle，字段只包含 `Vec<Subscription>` 与 `Entity<State>`。Field 与
attachment 数据 capture 到 subscriptions；component-to-form write 使用 attachment 的
deferred intent；每个 `FieldChanged` 与 `ModelReplaced` 都必须静默回投影，只忽略
`RuntimeChanged`。普通 projection closure 只捕获 field 与 weak native entity；拥有
lifecycle-scoped control draft issue 的 typed editor 可以额外捕获同一个 attachment clone，而且
唯一用途是在 programmatic projection 成功后清除该 issue；当前内置 adapter 中只有 exact integer
control 使用这一例外。临时 editor 数据必须留在原生 state，weak lifetime 处理留在 core。不要
增加 adapter `Config`、field/attachment 字段、focus 镜像、delegate 副本、origin-echo skip、
authoritative-value read-back API 或公开的 source/control ID。

## 相关文档

- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-macros 使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)
- [实施计划](../dev/typed-bound-controls.md)
