# gpui-form-gpui-component user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

## Before you start

Add the runtime, native components, and adapters:

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

The snippets use this common prelude. `ModelDelegate`, `TagDelegate`, and the
custom `SlugInput*` types later in the guide belong to the application.

```rust,ignore
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    combobox::{Combobox, ComboboxState},
    input::{Input, InputState},
    select::{Select, SelectState},
    switch::Switch,
};
use gpui_form::{DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError};
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInput,
    IntegerInputState,
};
```

The recipes below share these ordinary Rust draft types:

```rust,ignore
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ModelId(String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TagId(String);

#[derive(Clone, FormSchema)]
struct ProviderDraft {
    #[form(required)]
    name: String,
    model_id: Option<ModelId>,
    enabled: bool,
}

#[derive(Clone, FormSchema)]
struct JobDraft {
    budget: u64,
    tag_ids: Vec<TagId>,
}

#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    root: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
struct FilterNode {
    #[form(child)]
    kind: FilterNodeKind,
}

#[derive(Clone, FormSchema)]
enum FilterNodeKind {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
    limit: u64,
    model_id: Option<ModelId>,
    tag_ids: Vec<TagId>,
}
```

Create one strong form entity for each editing session. The examples use
`form`, `job_form`, and `query_form` below:

```rust,ignore
let provider_runtime = Form::try_new(ProviderDraft {
    name: String::new(),
    model_id: None,
    enabled: true,
})?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| provider_runtime);

let job_runtime = Form::try_new(JobDraft {
    budget: 1_024,
    tag_ids: Vec::new(),
})?;
let job_form: Entity<Form<JobDraft>> = cx.new(|_| job_runtime);

let query_runtime = Form::try_new(QueryDraft {
    root: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
                limit: 10,
                model_id: None,
                tag_ids: Vec::new(),
            }),
        }],
    },
})?;
let query_form: Entity<Form<QueryDraft>> = cx.new(|_| query_runtime);

let condition_node = QueryDraft::ROOT
    .then(FilterGroup::CHILDREN)
    .items(&query_form, cx)?
    .into_iter()
    .next()
    .expect("the example contains one condition");
let condition: DynamicPath<QueryDraft, FilterCondition> = condition_node
    .then(FilterNode::KIND)
    .try_case(query_form.read(cx), FilterNodeKind::CONDITION)?;
```

Use `new` when the target always exists. Pass either a root definition such as
`ProviderDraft::NAME` or an already composed `TotalPath<M, T>` directly.

A root `FieldDef<M, T>` also exposes the total-path façade directly, including
`value`, `set`, `errors`, and validation queries.

Use `try_new` with a `DynamicPath<M, T>` after the path crosses an item, enum
case, or optional child, because that target can be absent in the current form.
Resolve cases and options first with `try_case(form_entity.read(cx), case_def)`
or `try_some(form_entity.read(cx))`; adapters never receive `TopologyIndex`.

Form assigns and owns the identity of every item occurrence. Traversal and
topology operations return a typed located item path, from which the renderer
composes `condition` above. The model, page, and adapter never create or look up
an item ID. A located path is valid only for the occurrence and active case from
which Form returned it.

## Bind Input to a total path

```rust,ignore
let name_input = FormInput::new(
    &form,
    ProviderDraft::NAME,
    |window, cx| InputState::new(window, cx).placeholder("Provider name"),
    window,
    cx,
);

let element = Input::new(&name_input);
```

`InputEvent::Change` defers the typed `String` write. `InputEvent::Blur` defers
blur validation. Form commits silently call the native value setter.

Because `ProviderDraft::NAME` is total, `FormInput::new` has no
path-resolution `Result`.

## Bind Input to a dynamic path

```rust,ignore
let value: DynamicPath<QueryDraft, String> =
    condition.clone().then(FilterCondition::VALUE);

let value_input = FormInput::try_new(
    &query_form,
    value,
    |window, cx| InputState::new(window, cx).placeholder("Condition value"),
    window,
    cx,
)?;
```

Mount fails if the item has retired or the condition case is inactive. Store a
dynamic adapter in the renderer under that dynamic location's UI key. When the
renderer no longer receives that location, drop the adapter; if a new location
appears, call `try_new` for it instead of retargeting the old control. Queued
work from the retired path becomes a silent no-op.

## Bind Integer to a total path

```rust,ignore
let budget_input = FormIntegerInput::new(
    &job_form,
    JobDraft::BUDGET,
    |window, cx| {
        IntegerInputState::new(window, cx)
            .min(1_024u64)
            .max(1_000_000u64)
            .step(1_024u64)
    },
    window,
    cx,
)?;

let element = IntegerInput::new(&budget_input);
```

The form value remains `u64`. The native entity owns private editor text.
Incomplete, invalid, overflowing, or out-of-range text stays native and
publishes a leased control issue; only valid typed input is deferred to the
form. Checked arithmetic never routes the integer through `String` or `f64`.

The total constructor may return a native integer-policy error, but not a
path-resolution error.

## Bind Integer to a dynamic path

```rust,ignore
let limit: DynamicPath<QueryDraft, u64> =
    condition.clone().then(FilterCondition::LIMIT);

let limit_input = FormIntegerInput::try_new(
    &query_form,
    limit,
    |window, cx| IntegerInputState::new(window, cx).min(0u64).step(1u64),
    window,
    cx,
)?;

let element = IntegerInput::new(&limit_input);
```

`FormIntegerInputBuildError` distinguishes `Resolve` from `Policy`, so callers
can handle an unavailable path separately from an invalid integer policy.

## Bind Select to a total path

`FormSelect<D>` binds `Option<D::Item::Value>` and writes only after
`SelectEvent::Confirm`:

`ModelDelegate` is an application-defined native select delegate whose item
value is `ModelId`. `provider_models` and `condition_models` below are separate
option snapshots owned by the application.

```rust,ignore
let provider_model_select = FormSelect::new(
    &form,
    ProviderDraft::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(provider_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);

let element = Select::new(&provider_model_select);
```

The adapter uses the native state's current delegate for every silent
projection. It does not retain a second delegate or value/index map.

## Bind Select to a dynamic path

```rust,ignore
let model_id: DynamicPath<QueryDraft, Option<ModelId>> =
    condition.clone().then(FilterCondition::MODEL_ID);

let condition_model_select = FormSelect::try_new(
    &query_form,
    model_id,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(condition_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Select::new(&condition_model_select);
```

`try_new` resolves the current item and case before constructing native state.
If the dynamic location later disappears, its old binding cannot write a newly
created object at the same address.

## Bind Combobox to a total path

`FormCombobox<D>` binds `Vec<D::Item::Value>` and writes on
`ComboboxEvent::Change`:

`TagDelegate` is an application-defined native combobox delegate whose item
value is `TagId`. The two option variables below are independent application
snapshots.

```rust,ignore
let job_tags = FormCombobox::new(
    &job_form,
    JobDraft::TAG_IDS,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(job_tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
);

let element = Combobox::new(&job_tags);
```

Programmatic projection uses `set_selected_values` against the native state's
current delegate.

## Bind Combobox to a dynamic path

```rust,ignore
let tag_ids: DynamicPath<QueryDraft, Vec<TagId>> =
    condition.clone().then(FilterCondition::TAG_IDS);

let condition_tags = FormCombobox::try_new(
    &query_form,
    tag_ids,
    move |window, cx| {
        ComboboxState::new(TagDelegate::new(condition_tag_options), vec![], window, cx)
            .multiple(true)
            .searchable(true)
    },
    window,
    cx,
)?;

let element = Combobox::new(&condition_tags);
```

The selected values remain typed even when the surrounding item or case is
dynamic.

## Render Checkbox and Switch

`Checkbox` and `Switch` expose no public native state entity, so use them as
controlled elements instead of creating adapter wrappers:

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.value(&form, cx);

let checkbox_form = form.clone();
let checkbox_path = enabled.clone();
let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        checkbox_path.set(&checkbox_form, *checked, cx);
    });

let switch_form = form.clone();
let switch_path = enabled.clone();
let switch = Switch::new("provider-enabled-switch")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        switch_path.set(&switch_form, *checked, cx);
    });
```

These callbacks are not emitted from another state entity's active update, so
the total path writes synchronously through the explicit strong form. For a
dynamic boolean, use `try_value` while rendering and `try_set` in the callback.

## Refresh Select or Combobox options

Options and delegates belong to the application, not `Form<M>`. Update native
items and immediately reproject the authoritative form value:

```rust,ignore
let selected_model = ProviderDraft::MODEL_ID.value(&form, cx);
provider_model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobDraft::TAG_IDS.value(&job_form, cx);
job_tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

If the native API cannot update items and silently reproject in place, rebuild
the adapter. An options refresh never selects a fallback, writes form data,
starts validation, or persists configuration implicitly.

For a dynamic path, call `try_value`; if it no longer resolves, tear down the
adapter instead of choosing a replacement value.

## Handle errors

Keep total and dynamic failures distinct:

```rust,ignore
// Total path: no ResolveError.
let errors = ProviderDraft::NAME.errors(&form, cx);

// Dynamic path: current availability is checked.
let condition_value = condition.clone().then(FilterCondition::VALUE);
match condition_value.try_errors(&query_form, cx) {
    Ok(errors) => render_errors(errors),
    Err(error) => teardown_missing_control(error),
}
```

- `ResolveError` reports a missing item, inactive case, retired path, wrong
  session, or another dynamic resolution failure.
- Item identity is runtime-owned and opaque. A located path carries the
  occurrence and freshness selected by Form; models and adapters never manage
  item IDs or choose among items.
- Integer-policy errors remain distinguishable from resolution errors.
- A leased control issue represents native editor state, not a second form
  value.
- The page decides when to show errors and which visible control to focus.

## Connect your own component

### Controlled elements need no adapter

If your component is rendered directly and has no separate state entity, read
the typed value while rendering and write it from the callback:

```rust,ignore
let enabled_path = ProviderDraft::ENABLED;
let enabled = enabled_path.value(&form, cx);
let form_for_change = form.clone();

TogglePill::new("provider-enabled")
    .selected(enabled)
    .on_change(move |enabled, _window, cx| {
        enabled_path.set(&form_for_change, enabled, cx);
    });
```

Use `try_value` and `try_set` instead when the component points through an
item, case, or optional payload.

### Wrap a stateful component once

Callers should get the same ergonomics as the built-in adapters:

```rust,ignore
let slug_input = FormSlugInput::new(
    &form,
    ProviderDraft::NAME,
    |initial, window, cx| SlugInputState::new(initial, window, cx),
    window,
    cx,
);

let element = SlugInput::new(&slug_input);
```

An adapter author has four jobs: read the initial typed value, defer native
change and blur events into the form, silently project form commits back into
the native state, and keep the control lease plus subscriptions alive.

```rust,ignore
use std::ops::Deref;

pub struct FormSlugInput {
    subscriptions: Vec<Subscription>,
    _lease: ControlLease,
    state: Entity<SlugInputState>,
}

impl Deref for FormSlugInput {
    type Target = Entity<SlugInputState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl FormSlugInput {
    pub fn new<M, P, Owner>(
        form: &Entity<Form<M>>,
        path: P,
        build: impl FnOnce(String, &mut Window, &mut Context<SlugInputState>)
            -> SlugInputState,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        M: FormSchema,
        P: IntoTotalPath<M, String>,
        Owner: 'static,
    {
        let path = path.into_total_path();
        let initial = path.value(form, cx);
        let state = cx.new(|state_cx| build(initial, window, state_cx));
        let binding = path.bind_control(form, cx);
        let lease = binding.lease();

        let native_binding = binding.clone();
        let native_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SlugInputEvent, window, cx| match event {
                SlugInputEvent::Change(value) => {
                    native_binding.defer_set(value.clone(), window, cx);
                }
                SlugInputEvent::Blur => {
                    native_binding.defer_blur(window, cx);
                }
            },
        );

        let weak_form = form.downgrade();
        let weak_state = state.downgrade();
        let form_subscription = cx.subscribe_in(
            form,
            window,
            move |_, _, _: &FormEvent, window, cx| {
                let (Some(form), Some(state)) =
                    (weak_form.upgrade(), weak_state.upgrade())
                else { return };
                let value = path.value(&form, cx);
                state.update(cx, |state, cx| {
                    state.set_value_silent(value, window, cx);
                });
            },
        );

        Self {
            subscriptions: vec![native_subscription, form_subscription],
            _lease: lease,
            state,
        }
    }
}
```

For a dynamic path, use `try_value` and `try_bind_control`, return
`Result<Self, ResolveError>`, and ignore a form projection after the path has
retired. The `ControlLease` is required: dropping the adapter retires queued
binding callbacks and its control issue. The owning handle stores neither the
strong form nor an authoritative value.

## Related documentation

- [gpui-form guide](../../gpui-form/docs/guide.md)
- [gpui-form-macros guide](../../gpui-form-macros/docs/guide.md)
