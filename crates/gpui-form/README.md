# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` gives a GPUI editing page one typed Rust draft, validation, and a
snapshot that can safely be saved. A `Form<M>` is an editing session, not a
second application store: the page owns persistence, loading, and presentation;
the form owns the current draft, baseline, validation facts, and session-local
locations for dynamic fields.

## Add the crates

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

The component crate is optional, but is the usual way to connect standard
`gpui-component` inputs. The examples below use these imports:

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    checkbox::Checkbox,
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    Form, FormEvent, FormSchema, Prepared, ValidationMessage,
    ValidationItemPath, ValidationRequest, ValidationSink, ValidationTrigger, Validator,
};
use gpui_form_gpui_component::{FormInput, FormIntegerInput, IntegerInputState};
```

## A complete small form

### Describe the draft

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_blur, on_submit))]
    name: String,

    #[form(validate(on_submit))]
    retry_limit: u32,

    enabled: bool,
}
```

`FormSchema` creates reusable static descriptors such as
`ProviderDraft::NAME`. A descriptor contains schema metadata and typed access;
it never retains a form entity, value, subscription, or native control. The
root descriptor is also a total path, so its `get` and `set` operations cannot
fail.

### Create an editing session

The constructors are infallible. Give the form an initial draft, then optionally
attach a validator for this editing session:

```rust,ignore
let form: Entity<Form<ProviderDraft>> = cx.new(|_| {
    Form::new(ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    })
    .with_validator(ProviderValidator::new(reserved_names))
});
```

The same schema can have multiple independent sessions with different validator
data. Updating an application catalog does not rewrite the form; the catalog
owner explicitly asks for validation when that external fact matters.

### Write a validator

Validation receives one self-consistent snapshot. Read the model through the
request, and attach an issue to its precise typed path:

```rust,ignore
struct ProviderValidator {
    reserved_names: Arc<HashSet<String>>,
}

impl ProviderValidator {
    fn new(reserved_names: Arc<HashSet<String>>) -> Self {
        Self { reserved_names }
    }
}

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        let model = request.model();

        if request.includes(&ProviderDraft::NAME)
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-reserved",
                ValidationMessage::key("provider-name-reserved"),
            );
        }
    }
}
```

Business validation runs on `Submit` by default. Schema-declared `Mount`,
`Change`, and `Blur` rules opt into their corresponding triggers. Use
`ValidationTrigger::External` when a catalog or another external dependency
changes; it is not related to a `DynamicPath`.

### Connect controls and redraw the page

Use the built-in adapters for ordinary controls. They subscribe to the form and
handle both directions of synchronization themselves:

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

impl ProviderPage {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let form = cx.new(|_| Form::new(ProviderDraft {
            name: String::new(),
            retry_limit: 3,
            enabled: true,
        }));

        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );
        let retry_limit_input = FormIntegerInput::new(
            &form,
            ProviderDraft::RETRY_LIMIT,
            |window, cx| IntegerInputState::new(window, cx).min(0u32).max(10u32),
            window,
            cx,
        )?;

        // This redraws this page. It is not required to keep controls in sync.
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input, retry_limit_input })
    }
}
```

`FormInput`, `FormIntegerInput`, `FormSelect`, and `FormCombobox` own their
binding and native subscriptions. A page observer is only for rendering page
state such as errors, dirty status, or button availability.

For a non-stateful callback, use the same descriptor explicitly with the form:

```rust,ignore
let enabled = ProviderDraft::ENABLED;
let checked = enabled.get(&self.form, cx);
let form = self.form.clone();

Checkbox::new("provider-enabled")
    .checked(checked)
    .on_click(move |checked, _, cx| {
        enabled.set(&form, *checked, cx);
    });
```

## Validate, save, and conditionally rebase

`prepare` runs submit validation for one snapshot. On success it returns a
`Prepared<M>` that carries both the value and a session-bound `FormVersion`:

```rust,ignore
struct SaveProvider(ProviderDraft);

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self(draft)
    }
}

let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;

let (version, request) = prepared
    .map(SaveProvider::from)
    .into_parts();
self.start_save(version, request, cx);

// In the page-owned async completion callback:
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_saved_model, cx)
});
if !applied {
    self.show_saved_while_editing_notice(cx);
}
```

`Prepared::map` preserves the same version. `rebase_if_current` changes nothing
when the user has edited this session since preparation, or when the version
belongs to another session. Saving, retries, notifications, and error display
remain application responsibilities.

## Nested and dynamic data

Mark nested schemas with `#[form(child)]` and structured collections with
`#[form(items)]`. This complete recursive model contains no Form-only ID:

```rust,ignore
#[derive(Clone, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    title: String,
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, FormSchema)]
struct FilterCondition {
    value: String,
}

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        title: "All articles".into(),
        children: Vec::new(),
    },
}));

let title = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::TITLE);
let value: String = title.get(&query_form, cx);
title.set(&query_form, "Recent articles".into(), cx);
```

Items, enum cases, and `Option::Some` locations are dynamic. Form creates their
identity; callers neither create IDs nor navigate by array index. Resolve a case
or optional payload against the current form. An inactive case/option is
`Ok(None)`; a retired starting location is `Err(ResolveError)`:

```rust,ignore
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
let node = children.append(
    &query_form,
    FilterNode::Condition(FilterCondition { value: String::new() }),
    cx,
)?;
let key = node.key(); // stable UI identity while this item remains active

let condition = node
    .case(FilterNode::CONDITION)
    .resolve(&query_form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: String = value.try_get(&query_form, cx)?;
    value.try_set(&query_form, "Rust".into(), cx)?;
}
```

Enumerate existing items with `children.items(&query_form, cx)`. Every returned
`ItemPath` is typed and carries its current Form-owned location.

Same-parent reordering preserves an item path. Removal, replacement,
case/optional reconstruction, whole-form replacement, and cross-parent moves
retire the affected dynamic paths; they never revive at a matching-looking
location.

## Observe semantic changes when needed

Most pages only need `cx.observe` for rendering. A tree reconciler or another
cross-field owner can subscribe to semantic form events and ask whether its
typed target was affected:

```rust,ignore
let subscription = cx.subscribe(&form, |_, _, event, cx| {
    if let FormEvent::ModelChanged(change) = event {
        let children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroup::CHILDREN);
        let impact = change.impact(&children);

        if impact.structure_changed() {
            cx.notify(); // re-enumerate rows
        } else if impact.value_changed() {
            cx.notify(); // reread an existing value
        }
    }
});
```

`PathImpact` also reports retirement. Validation-only changes arrive separately
as `FormEvent::ValidationChanged`; they do not mean that a native control value
must be set again.

## Learn more

- [User guide](docs/guide.md): lifecycle operations, validation, submission,
  recursive collections, and event handling.
- [Macro guide](../gpui-form-macros/docs/guide.md): schema declarations, enum
  cases, and compile-time diagnostics.
- [Component adapter guide](../gpui-form-gpui-component/docs/guide.md):
  built-in controls and the custom-control binding API.
