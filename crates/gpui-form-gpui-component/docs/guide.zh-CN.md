# gpui-form-gpui-component 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

`gpui-form-gpui-component` 把原生 `gpui-component` state entity 适配到类型化
`gpui-form` descriptor。它不会创建第二份业务值，也不拥有应用配置。

## 创建并渲染 total control

每个 generated static field 都有唯一的 schema-level definition，并以 associated `const`
descriptor 暴露，例如 `ProviderForm::NAME`、`ProviderForm::MODEL_ID` 与
`ProviderForm::ENABLED`。它们可复用且不产生 allocation：`FormField<Form, T>` 标识类型化
path，并提供静态 schema 与 field access，但绝不持有某个 form entity、value、subscription 或
每个 form 的 allocation；读、写、校验和绑定时都由调用者显式传入 form：

```rust,ignore
use gpui_component::input::{Input, InputState};
use gpui_form_gpui_component::FormInput;

let name_input = FormInput::new(
    &form,
    ProviderForm::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);

let element = Input::new(&name_input);
```

`ProviderForm::NAME` 是 total descriptor：generated model 保证其 path 存在。因此
`FormInput::new` 直接返回 `Self`，不返回 `Result`。

`FormInput` 是普通 Rust value，不是 `Entity<FormInput>`。它只保存 subscription
与原生 entity，并 deref 到后者：

```rust,ignore
pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}
```

subscriptions 必须在 entity 之前声明，以便 Rust 先释放它们。其他有状态 adapter 采用相同
布局；它们不保存 form、field、`ControlBinding`、delegate、`Config`、focus/blur flag
或 validation report。binding 细节只存在于 subscription closure。

## Partial descriptor

optional projection 或 identified array item 会产生 partial descriptor：descriptor 仍轻量且
可复用，但它在某个 `Entity<Form>` 上的 path 可能不存在。`item` 与 `within` 创建的是轻量的
located descriptor，不是新的 schema definition。使用得到的 `PartialFormField` 通过 `try_new`
绑定：

```rust,ignore
let header = ServerForm::HEADERS.item(header_id);
let header_name = HeaderRowForm::NAME.within(header);

let header_input = FormInput::try_new(
    &form,
    header_name,
    |window, cx| InputState::new(window, cx).placeholder("Header name"),
    window,
    cx,
)?;
```

`try_new` 只在 projected 或 identified path 不存在时返回 `FieldAccessError`。它不会
报告 form 已释放：调用者提供的是强 `Entity<Form>`。同样的 partial 边界使用
`try_value`、`try_set`、`try_errors` 等 `try_*` API。不能因为同一视图中另一个
descriptor 是 partial，就给 total descriptor 增加 `Result`。

## 同步与生命周期

Form 持有唯一权威的类型化值。原生 state 只持有当前 presentation projection 以及
focus、IME、selection、query、popup、highlighted item 等交互细节。

所有有状态 adapter 都遵循同一套同步规则：

1. constructor 通过显式 form 读取 descriptor，创建原生 state，静默投影初值，并安装双向 subscriptions；
2. component event 等 emitting entity 的 update 结束后，再 defer 类型化 form 写入；
3. form subscription 对每个值变化或 whole-model replacement 重新投影，包括值相等的生命周期替换，
   以及其他 path 改变当前 descriptor projection 的情况；
4. 原生 silent setter 不会再发出 user event，因此 round trip 自然终止，不需要 origin-echo suppression
   或 value read-back API。

`ControlBinding` 是低层 adapter 边界。它在有状态 control 挂载时由显式强 form 与 descriptor
创建，内部持有 deferred intent 所需的 weak form；`FormField` 本身绝不包含
`Entity<Form>` 或 `WeakEntity<Form>`。

```rust,ignore
let binding = ProviderForm::NAME.bind_control(&form, cx);
let binding = header_name.try_bind_control(&form, cx)?;
```

`ControlBinding` 可以 clone。subscription callback 捕获 clone 后，只调用
`defer_set`、`defer_blur`、`defer_set_issue` 或 `defer_clear_issue`。deferred
callback 与 async work 是仅有的其他 weak-form 边界；weak form 无法 upgrade 时，queued work
静默取消。

普通 form-to-control projection closure 可以捕获 static descriptor，或 clone/capture 轻量的
located descriptor 与 weak native entity。partial adapter 或拥有 lifecycle-scoped control issue
的 typed editor 可以额外捕获 binding，用于使自身失效，或在程序化投影成功后清除 issue。
wrapper 字段仍严格只有 subscriptions 在前、native state 在后，不能保存 descriptor 或 binding。

## 校验与错误

Adapter 只转发具体组件能够表达的事件：

| Control | 用户写入 | Blur |
| --- | --- | --- |
| `FormInput` | `InputEvent::Change` defer 类型化 `String` 写入 | `InputEvent::Blur` defer field blur validation |
| `FormIntegerInput<N>` | 合法类型化整数编辑；无效文本产生 control issue | 原生 input blur 执行 field blur validation |
| `FormSelect<D>` | `SelectEvent::Confirm(Option<Value>)` | 不支持：upstream 没有可靠的 composite final-blur |
| `FormCombobox<D>` | `ComboboxEvent::Change(Vec<Value>)` | 不支持：upstream 没有可靠的 composite final-blur |

非相等 typed field write 修改 model 与 revision，只清除相交的校验工作，并保留 active
control issue；相等写入是完整 no-op。whole-form lifecycle replacement 即使模型相等，也会让
已挂载 control 重新投影。

bound handle 不保存交互 flag 或 validation-report 副本。通过显式 form 读取数据级状态：

```rust,ignore
let field = ProviderForm::NAME;
let is_validating = field.is_validating(&form, cx);
let error = field.errors(&form, cx).into_iter().next();
let required = field.schema().is_required();
```

partial descriptor 应使用对应 `try_*` 方法并处理 `FieldAccessError`。submit 失败后由
当前页面选择需要 focus 的可见 control；form 与 adapter 都不拥有该选择。

## Select 与 Combobox

`FormSelect<D>` 精确绑定 `Option<D::Item::Value>`：

```rust,ignore
let model_select = FormSelect::new(
    &form,
    ProviderForm::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);
```

用户确认把 `Option<Value>` 直接 defer 给 form。投影使用原生 state 当前 delegate 并保持
silent。adapter 不保存 delegate，也不提供 adapter-specific item updater。

`FormCombobox<D>` 精确绑定 `Vec<D::Item::Value>`：

```rust,ignore
let tags = FormCombobox::new(
    &form,
    JobForm::TAG_IDS,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
);
```

`ComboboxEvent::Change(values)` 把 `values` defer 到 form。每次 form update 使用当前
delegate 调用 upstream `set_selected_values`，因此 captured delegate 或 value/index map
不会过期。

## 精确整数输入

`FormIntegerInput<N>` 把标准 signed/unsigned integer primitive 绑定到
`IntegerInputState<N>`。原生 state 持有类型化 `N`、私有 editor text 和类型化
min/max/step policy：

```rust,ignore
let budget = FormIntegerInput::new(
    &form,
    JobForm::BUDGET,
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;
```

对于 total descriptor，这个 `Result` 只可能是 `IntegerInputPolicyError`：
`step <= 0` 返回 `NonPositiveStep`，`min > max` 返回 `ReversedRange`。partial integer
descriptor 使用 `try_new`；其 build error 是
`FormIntegerInputBuildError::{Field, Policy}`，分别保留 `FieldAccessError` 或
`IntegerInputPolicyError`，不会抹掉实际原因。

不完整、语法无效、溢出或超范围的 editor text 留在 native state 并发布
lifecycle-scoped control issue；只有合法 `N` 才 defer form write。increment/decrement 使用
带类型边界的 checked arithmetic，不使用 `f64`、不 clamp overflow，也不会丢失超过 `2^53`
的值。

## 无状态布尔 element

Upstream `Checkbox` 与 `Switch` 是没有公开 state entity 的 `RenderOnce` element，不能
制造假的 wrapper。将其渲染为 controlled element，并显式把 form 传给 descriptor：

```rust,ignore
let enabled = ProviderForm::ENABLED.value(&self.form, cx);

let checkbox_field = ProviderForm::ENABLED;
let checkbox_form = self.form.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        checkbox_field.set(&checkbox_form, *checked, cx);
    });

let switch_form = self.form.clone();
let switch_field = ProviderForm::ENABLED;
let switch = Switch::new("provider-enabled-switch")
    .checked(enabled)
    .on_click(move |checked, _window, cx| {
        switch_field.set(&switch_form, *checked, cx);
    });
```

这些 callback 并非从 component-state entity update 中发出，因此可直接写该 total field。
partial descriptor 则使用 `try_value` 与 `try_set`。这些 element 没有公开 focus handle，
所以无法提供原生 blur validation。

## 修改 options 与组件配置

Options、delegate、placeholder、disabled state、size、accessibility、focus 与 catalog refresh
都属于应用。原生 state 配置放在构造闭包中，或通过 deref entity 修改；只属于 element 的
presentation 在 render 时配置。

替换 items 后，显式读取 form 并立即使用 native setter 重投影当前值，或者替换整个 bound
handle：

```rust,ignore
let selected_model = ProviderForm::MODEL_ID.value(&form, cx);
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobForm::TAG_IDS.value(&form, cx);
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

native item update 与值投影必须作为一次 refresh 连续完成。修改 options 本身不写 form，
所以不保证产生值变化事件。adapter 不会选择 fallback、修改 form data、持久化配置或因为
item refresh 自动开始 dynamic validation。

## 实现其他有状态 adapter

没有 `FormControl` trait。第三方 adapter 应按原生 state 暴露 inherent `new` 与
`try_new`。调用者将 static associated descriptor 或轻量的 located descriptor 与显式
`&Entity<Form>` 一起传入：

```rust,ignore
pub struct FormDateInput {
    subscriptions: Vec<Subscription>,
    state: Entity<DateInputState>,
}

impl FormDateInput {
    pub fn new<Form>(
        form: &Entity<Form>,
        field: FormField<Form, Date>,
        cx: &mut App,
    ) -> Self
    where
        Form: FormState,
    {
        let binding = field.bind_control(form, cx);
        // 创建 state，并在 subscriptions 中 capture `binding`。
    }

    pub fn try_new<Form>(
        form: &Entity<Form>,
        field: PartialFormField<Form, Date>,
        cx: &mut App,
    ) -> Result<Self, FieldAccessError>
    where
        Form: FormState,
    {
        let binding = field.try_bind_control(form, cx)?;
        // 创建 state，并在 subscriptions 中 capture `binding`。
    }
}
```

示例省略了组件特定的构造参数。真实 handle 仍只包含 `Vec<Subscription>` 与
`Entity<State>`；binding 只 capture 在 subscriptions 内，用其 deferred intent 完成
component-to-form 写入，并静默重投影 form 变化。不能增加 adapter `Config`、
descriptor/binding 字段、focus mirror、delegate copy、origin-echo skip、authoritative
read-back API 或公开 source/control ID。

## 相关文档

- [gpui-form 使用指南](../../gpui-form/docs/guide.zh-CN.md)
- [gpui-form-macros 使用指南](../../gpui-form-macros/docs/guide.zh-CN.md)
