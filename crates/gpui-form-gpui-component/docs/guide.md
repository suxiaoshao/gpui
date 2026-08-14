# gpui-form-gpui-component user guide

[English](guide.md) | [简体中文](guide.zh-CN.md)

## Before you start

Add the Form runtime, gpui-component, and this adapter crate:

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

The snippets below use the following imports. `ModelDelegate`, `TagDelegate`,
and `SlugInputState` are application types.

```rust,ignore
use gpui::{Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    combobox::{Combobox, ComboboxState},
    input::{Input, InputState},
    select::{Select, SelectState},
    switch::Switch,
};
use gpui_form::{DynamicPath, Form, FormSchema, ResolveError};
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInput,
    IntegerInputState,
};
```

The examples use ordinary typed drafts. Schema annotations describe nesting once;
call sites do not use string paths or application-managed item IDs.

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
    filters: FilterGroup,
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

## Create and locate a form

Create one strong `Entity<Form<M>>` per editing session. `Form::new` is
infallible. Static fields are total paths: use `get` and `set` directly.

```rust,ignore
let form = cx.new(|_| Form::new(ProviderDraft {
    name: String::new(),
    model_id: None,
    enabled: true,
}));

let name: String = ProviderDraft::NAME.get(&form, cx);
let changed: bool = ProviderDraft::NAME.set(&form, "Local provider".into(), cx);
```

Collection items, active enum cases, and `Option::Some` values are dynamic
locations. Form creates their identities. Enumerate them from the Form, then
resolve a case or optional boundary against that same session:

```rust,ignore
let job_form = cx.new(|_| Form::new(JobDraft {
    budget: 1_024,
    tag_ids: Vec::new(),
}));
let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode {
            kind: FilterNodeKind::Condition(FilterCondition {
                value: String::new(),
                limit: 10,
                model_id: None,
                tag_ids: Vec::new(),
            }),
        }],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.items(&query_form, cx).into_iter().next().unwrap();
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&query_form, cx)?
    .expect("the example starts with a condition");
let value: DynamicPath<QueryDraft, String> =
    condition.clone().then(FilterCondition::VALUE);
let current = value.try_get(&query_form, cx)?;
```

`Ok(None)` means the current enum case or optional value is not active. A
`ResolveError` means the dynamic starting point is no longer usable, such as
after removal or replacement. Do not turn either condition into an index lookup
or a business ID lookup.

## Bind Input

Pass a total path to `FormInput::new`:

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

`InputEvent::Change` defers a typed write. `InputEvent::Blur` requests the
configured blur validation. Relevant Form value changes are silently projected
back to the input; a write from this input is not echoed to itself.

Bind a resolved dynamic path with `try_new`:

```rust,ignore
let value = condition.clone().then(FilterCondition::VALUE);
let value_input = FormInput::try_new(
    &query_form,
    value,
    |window, cx| InputState::new(window, cx).placeholder("Condition value"),
    window,
    cx,
)?;
```

Keep a dynamic adapter under its dynamic `PathKey` in the renderer. If that
location retires, drop its adapter. If a later model change creates another
condition at the same schema position, create a new adapter; never retarget the
old one.

## Bind integer input

`FormIntegerInput` keeps incomplete or invalid editor text in its native state.
Only a valid typed integer is written to the Form.

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

The constructor can reject an invalid native integer policy. It does not turn a
total path into a resolution error. For a dynamic integer path, use
`FormIntegerInput::try_new`; its build error distinguishes an unavailable path
from an invalid integer policy.

```rust,ignore
let limit = condition.clone().then(FilterCondition::LIMIT);
let limit_input = FormIntegerInput::try_new(
    &query_form,
    limit,
    |window, cx| IntegerInputState::new(window, cx).min(0u64).step(1u64),
    window,
    cx,
)?;
```

## Bind Select and Combobox

`FormSelect<D>` binds `Option<D::Item::Value>` and writes after
`SelectEvent::Confirm`:

```rust,ignore
let model_select = FormSelect::new(
    &form,
    ProviderDraft::MODEL_ID,
    move |window, cx| {
        SelectState::new(ModelDelegate::new(provider_models), None, window, cx)
            .searchable(true)
    },
    window,
    cx,
);

let element = Select::new(&model_select);
```

`FormCombobox<D>` binds `Vec<D::Item::Value>` and writes on
`ComboboxEvent::Change`:

```rust,ignore
let tags = FormCombobox::new(
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

let element = Combobox::new(&tags);
```

Use the equivalent `try_new` constructor after resolving a dynamic `model_id`
or `tag_ids` path. Selections stay typed even when their enclosing item or case
is dynamic.

## Render Checkbox and Switch

`Checkbox` and `Switch` have no state entity, so render them as controlled
elements. A total-path callback can write through the explicit Form directly:

```rust,ignore
let enabled_path = ProviderDraft::ENABLED;
let checked = enabled_path.get(&form, cx);
let form_for_change = form.clone();

let checkbox = Checkbox::new("provider-enabled-checkbox")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        enabled_path.set(&form_for_change, *checked, cx);
    });

let switch_form = form.clone();
let switch_path = ProviderDraft::ENABLED;
let switch = Switch::new("provider-enabled-switch")
    .checked(checked)
    .on_click(move |checked, _window, cx| {
        switch_path.set(&switch_form, *checked, cx);
    });
```

For a dynamic boolean, use `try_get` while rendering and `try_set` in the
callback. A callback emitted from another state entity must defer its write;
use a stateful adapter for that case.

## Refresh options without changing the Form

Delegates, catalogs, and option snapshots belong to the application. After
replacing native items, reproject the Form's authoritative selection using the
native state's current delegate:

```rust,ignore
let selected_model = ProviderDraft::MODEL_ID.get(&form, cx);
model_select.update(cx, |state, cx| {
    state.set_items(ModelDelegate::new(next_models), window, cx);
    match selected_model.as_ref() {
        Some(value) => state.set_selected_value(value, window, cx),
        None => state.set_selected_index(None, window, cx),
    }
});

let selected_tags = JobDraft::TAG_IDS.get(&job_form, cx);
tags.update(cx, |state, cx| {
    state.set_items(TagDelegate::new(next_tags), window, cx);
    state.set_selected_values(&selected_tags, window, cx);
});
```

An option refresh must not select a fallback, mutate Form data, start
validation, or persist configuration. If a dynamic location no longer resolves,
tear down its adapter instead of selecting a replacement.

Form suppresses the immediate self-echo after a Combobox commits a selection.
It does not change `gpui-component`'s own collection-selection semantics:
`set_selected_values` must still resolve all committed values from its source,
including while a search filter is active. That behavior is tracked separately
in [gpui-component#2652](https://github.com/longbridge/gpui-component/issues/2652).

## Render validation feedback

Form owns validation facts; the page owns visibility, localization, layout, and
focus decisions. Total and dynamic paths keep their different failure modes:

```rust,ignore
let errors = ProviderDraft::NAME.errors(&form, cx);

let value = condition.clone().then(FilterCondition::VALUE);
match value.try_errors(&query_form, cx) {
    Ok(errors) => render_errors(errors),
    Err(error) => teardown_missing_control(error),
}
```

Native editor issues, such as incomplete integer text, remain associated with
that control. A valid edit clears its own obsolete editor issue. Validation-only
changes do not reset a native value or erase unrelated editor state.

Observe the Form only when page-owned rendering needs it:

```rust,ignore
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

Built-in adapters and custom bindings synchronize independently of this
observer. Do not subscribe each adapter to `FormEvent`.

## Connect a custom component

### Stateless controlled element

If a component has no separate state entity, use the same read-in-render and
write-in-callback pattern as `Checkbox` and `Switch`.

### Stateful adapter

For a native state entity, the core binding owns Form-to-control projection.
The adapter owns the native entity, its native event subscriptions, and one
non-`Clone` `ControlBinding`. Native callbacks capture the cloneable typed
`ControlWriter`.

```rust,ignore
use std::ops::Deref;
use gpui_form::{
    ControlBinding, ControlProjection, ControlWriter, Form, FormSchema,
    IntoTotalPath,
};

pub struct FormSlugInput {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
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
        let initial = path.get(form, cx);
        let state = cx.new(|state_cx| build(initial, window, state_cx));

        let (binding, writer): (ControlBinding, ControlWriter<M, String>) =
            path.bind_control_in(
                form,
                &state,
                |state, projection, window, cx| match projection {
                    ControlProjection::Value(value) => {
                        state.set_value_silently(value, window, cx);
                    }
                    ControlProjection::Retired => {
                        state.set_retired(window, cx);
                    }
                },
                window,
                cx,
            );

        let native_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SlugInputEvent, window, cx| match event {
                SlugInputEvent::Change(value) => {
                    writer.defer_set(value.clone(), window, cx);
                }
                SlugInputEvent::Blur => writer.defer_blur(window, cx),
            },
        );

        Self {
            subscriptions: vec![native_subscription],
            _binding: binding,
            state,
        }
    }
}
```

The silent setter must not emit a native `Change` event. `ControlProjection` is
exhaustive: `Value` updates the state, and `Retired` marks the dynamic control
unavailable until the renderer removes it. The adapter does not hold a form
entity, subscribe to `FormEvent`, clone the binding, identify a control, or
implement a local direction flag.

For a dynamic path, read with `try_get`, call `try_bind_control_in`, and return
`Result<Self, ResolveError>`. The method returns a `ControlBinding` and a
`ControlWriter` only when that dynamic location is still active. Dropping the
adapter drops its binding, so later native callbacks cannot change the Form.

## Related documentation

- [gpui-form README](../../gpui-form/README.md)
- [gpui-form guide](../../gpui-form/docs/guide.md)
- [gpui-form-macros guide](../../gpui-form-macros/docs/guide.md)
