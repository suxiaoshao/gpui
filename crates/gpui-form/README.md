# gpui-form

`gpui-form` is a form-state library for GPUI applications. It owns form draft,
dirty/touched/error metadata, validation, transformation, and submission. UI
components are connected by adapter crates and remain owned by the application.

> Status: the pure draft/handle API and caller-owned component adapter contract
> are implemented. The workspace has migrated off the legacy state-owning
> binding surface; new code should use the API shown here.

## Why this crate exists

A GPUI form usually needs more than a collection of component states:

- invalid raw input must survive long enough to display and validate;
- dirty state must compare against a stable baseline;
- nested groups and dynamic arrays need one validation path;
- programmatic replacement must be distinguishable from user editing;
- submit must read one authoritative snapshot;
- component options and focus state must not become business data.

`gpui-form` provides that form-domain layer without depending on a component
library or an application store.

## State ownership

The public model has three deliberately separate channels:

| State | Owner | Examples |
| --- | --- | --- |
| Form draft | `gpui-form` | raw text, typed selection, dirty, touched, errors |
| Component configuration | application | items, capabilities, disabled, placeholder, mask |
| Component interaction | component entity | focus, open, query, highlight, scroll, IME, tasks |

The text or selection cached inside a component is only a UI mirror. Validation,
transformation, and submit read the form draft, never the component entity.

## Quick start

Define the domain input and derive its form store:

```rust
use gpui_form::{FieldChangeCause, FormStore};

#[derive(Clone, Debug, PartialEq)]
pub enum ProviderKind {
    OpenAi,
    Ollama,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(
    store = "ProviderFormStore",
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
pub struct ProviderInput {
    #[form(validate(on_change, on_blur, on_submit))]
    pub name: String,

    pub kind: Option<ProviderKind>,

    #[form(codec = "PortCodec", validate(on_blur, on_submit))]
    pub port: u16,
}
```

`name` and `kind` use `IdentityCodec<Value>` because their UI draft is already
the domain type. `PortCodec` converts the component-friendly `String` draft into
`u16`.

Create the generated form entity from a committed value:

```rust
let form = cx.new(|cx| {
    ProviderFormStore::from_value(
        ProviderInput {
            name: "Local".into(),
            kind: Some(ProviderKind::Ollama),
            port: 11434,
        },
        window,
        cx,
    )
});
```

The generated store owns every field draft and its baseline. A component adapter
receives a typed field handle; it does not receive the whole form:

```rust
let name = ProviderFormStore::name_handle(&form);
let kind = ProviderFormStore::kind_handle(&form);
let port = ProviderFormStore::port_handle(&form);
```

## Field codecs

A codec defines the form-owned draft representation for one domain value:

```rust
pub trait FieldCodec<Value>: 'static
where
    Value: Clone + PartialEq + 'static,
{
    type Draft: Clone + PartialEq + 'static;

    fn draft_from_value(value: &Value) -> Self::Draft;

    fn parse(draft: &Self::Draft) -> Result<Value, FieldCodecError>;
}
```

`FieldCodecError` contains only a stable error code, message key, and params.
The field runtime adds the current field path, validation trigger, and internal
validation source. Codecs therefore remain pure, context-free, and easy to unit
test.

Use the implicit `IdentityCodec<Value>` for checkboxes, typed selections, and
other fields whose UI draft is already the domain value. Use an explicit codec
when the user must be allowed to hold an intermediate representation:

```text
text input   String draft -> String / Option<String>
number input String draft -> integer / float
custom input AppDraft     -> DomainValue
```

Parsing belongs to the form boundary. A number component may display `"-"` or
`"1."`; the form keeps that raw draft and reports a parse error instead of
forcing the component to invent a valid domain value.

## Generated API

For a leaf field named `model`, the derive generates typed accessors equivalent
to:

```rust
impl ProviderFormStore {
    pub fn model_draft(&self) -> ModelDraft;
    pub fn model_value(&self) -> ModelValue;

    pub fn set_model_draft(
        &mut self,
        draft: ModelDraft,
        cause: FieldChangeCause,
        cx: &mut Context<Self>,
    );

    pub fn set_model_value(
        &mut self,
        value: ModelValue,
        cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    pub fn model_handle(
        form: &Entity<Self>,
    ) -> FormFieldHandle<Self, ModelDraft>;

    pub fn model_required(&self) -> bool;

    pub fn set_model_required(
        &mut self,
        required: bool,
        cx: &mut Context<Self>,
    );
}
```

`FormFieldHandle<Form, Draft>` is a small, cloneable adapter boundary. It holds a
weak form entity, generated field path, and typed read/write/event-filtering
functions. Its public operations are `draft`, `set_user_draft`, explicit-cause
`set_draft`, and `subscribe_in` for `FieldDraftEvent<Draft>`. An adapter does not
subscribe to the whole form and guess which field changed, or re-read the form
inside an event callback. The handle cannot create a component, change component
configuration, query a catalog, or submit the form.

Use `FieldChangeCause::UserInput` for genuine user edits. Programmatic setters use an
explicit non-user cause so validation and metadata can apply the intended
policy.

The derive also keeps a typed `ProviderFormField` enum for stable field identity.
Whole-form observers receive the library-provided
`FormStoreEvent<ProviderFormField>`, whose `FieldChanged` payload contains the
field and `FieldChangeCause`. The derive does not generate a separate public
event enum for every form, and focus/blur are not separate event variants.

`required` belongs to form validation. A generated `set_<field>_required` is a
pure form command: it takes no `Window`, never configures a component, and does
not emit a draft-change event. Applications configure any visual required
affordance separately.

## Connecting UI components

The application creates and configures component state, then asks an adapter
crate to mirror its value with a field handle:

```rust
use gpui_component::{input::InputState, select::SelectState};
use gpui_form::SubscriptionSet;
use gpui_form_gpui_component::{bind_input, bind_select};

pub struct ProviderEditor {
    form: Entity<ProviderFormStore>,
    name_state: Entity<InputState>,
    kind_state: Entity<SelectState<ProviderDelegate>>,
    subscriptions: SubscriptionSet,
}

impl ProviderEditor {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, ComponentBindError> {
        let form = cx.new(|cx| {
            ProviderFormStore::from_value(default_provider(), window, cx)
        });

        let name_state = cx.new(|cx| InputState::new(window, cx));
        let kind_state = cx.new(|cx| {
            SelectState::new(
                ProviderDelegate::new(load_provider_kinds()),
                None,
                window,
                cx,
            )
        });

        let mut subscriptions = SubscriptionSet::new();
        subscriptions.extend(bind_input(
            ProviderFormStore::name_handle(&form),
            &name_state,
            window,
            cx,
        )?);
        subscriptions.extend(bind_select(
            ProviderFormStore::kind_handle(&form),
            &kind_state,
            window,
            cx,
        )?);

        Ok(Self {
            form,
            name_state,
            kind_state,
            subscriptions,
        })
    }
}
```

The adapter returns subscriptions; the caller decides their lifetime by storing
them in the core, component-library-neutral `SubscriptionSet`. The field handle
is only the typed connection point; it does not own a component or a
subscription. Use separate sets when different mounted subtrees need independent
teardown.

The adapter has only two responsibilities:

1. publish genuine component user events into the form field;
2. mirror programmatic form changes back into the component.

It uses an explicit direction guard so one synchronous event cannot re-enter an
entity already being updated.

`SubscriptionSet` is not tied to `gpui-component`. A custom control or another
component library can provide its own `bind_*` function that accepts a
`FormFieldHandle` and component entity and returns `Result<SubscriptionSet, _>`.
No core adapter trait or dependency on `gpui-form-gpui-component` is required.

## Dynamic options and component configuration

Options are application data, not form data. For select/combobox items, use the
adapter's component-specific config command:

```rust
fn refresh_provider_kinds(
    &mut self,
    kinds: Vec<ProviderKind>,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Result<(), ComponentBindError> {
    set_select_items(
        ProviderFormStore::kind_handle(&self.form),
        &self.kind_state,
        ProviderDelegate::new(kinds),
        window,
        cx,
    )
}
```

`set_select_items` reads the authoritative form draft, replaces the delegate,
and reprojects the component's selected item/label from the new items. If the
draft is absent, it clears only the UI selection and exposes an unavailable
presentation; it does not change the form or choose a business fallback.

Simple configuration such as disabled state, placeholder, capabilities, or
masking can be updated directly through the component API. These changes must
not write a form field. The final submit resolver decides whether an unavailable
draft is rejected or resolved.

This distinction prevents a catalog refresh from silently changing user input.

## Reading and replacing values

Read drafts and metadata through the form entity:

```rust
let dirty = form.read(cx).meta().is_dirty;
let kind = form.read(cx).kind_draft().clone();
```

There are only two whole-form replacement operations:

```rust
form.update(cx, |form, cx| form.reset(window, cx));

form.update(cx, |form, cx| {
    form.replace_from_value(new_committed_value, cx);
});
```

`replace_from_value` deliberately discards the current draft, replaces the
baseline, clears errors and submit metadata, and emits field events so mounted
adapters update their mirrors. The caller must decide whether discarding a dirty
draft is acceptable:

```rust
if !form.read(cx).meta().is_dirty {
    form.update(cx, |form, cx| {
        form.replace_from_value(reloaded_value, cx);
    });
}
```

The core library does not implement automatic store binding, incoming-value
conflicts, revision counters, or catalog rebase.

## Validation and submit

Validation is triggered from form events and reads form-owned draft/value state.
The final submit path is:

```text
form draft
  -> codec parse
  -> field/form validation
  -> transform or normalize
  -> final validation report
  -> typed output / submit handler
```

Submit never asks a component for its current text or selection:

```rust
form.update(cx, |form, cx| {
    form.submit(window, cx)
});
```

Parse and validation failures remain in field/form metadata. An async submit
task is owned by the form submit runtime and is not part of component state.

## Groups and arrays

Nested groups and dynamic arrays remain explicit form structures:

```rust
#[derive(Clone, PartialEq, FormStore)]
#[form(store = "ServerFormStore")]
pub struct ServerInput {
    #[form(group(store = "TlsFormStore"))]
    pub tls: TlsInput,

    #[form(array(store = "HeaderFormStore"))]
    pub headers: Vec<HeaderInput>,
}
```

Groups own nested form stores. Arrays own stable row identities plus structural
operations such as append, remove, move, swap, and replace. Neither abstraction
owns UI entities. Whole-form replacement can rebase rows when the incoming array
has the same length; a structural length change is reported as an internal
validation error and should instead use the array's explicit row operations.

## What belongs outside gpui-form

Keep these concerns in the application or a dedicated adapter:

- creation and rendering of GPUI component entities;
- select/combobox items and catalog reloads;
- disabled/read-only/capability policy;
- placeholder, label, description, mask, and localization;
- repository loading and persistence;
- product-specific fallback and submit resolution;
- focus, picker visibility, query, highlight, scroll, and IME state.

This is intentional: changing any of those concerns should not require changing
the form schema or core form runtime.

## Crates

| Crate | Responsibility |
| --- | --- |
| `gpui-form` | pure form draft, metadata, validation, transform, submit, field handles |
| `gpui-form-macros` | derive field stores, accessors, field identity, handles, groups, and arrays |
| `gpui-form-gpui-component` | bind app-created `gpui-component` states to field handles |

See [`docs/README.md`](docs/README.md) for architecture, migration, and feature
plans. The authoritative component-separation plan is
[`docs/external-state-synchronization-plan.md`](docs/external-state-synchronization-plan.md).

## Migration status

The migration is complete. The public surface contains no generic
component-state associated type, component-construction derive attribute,
generated component-state accessor, or compatibility binding implementation.
All workspace callers, including provider secrets and MCP dynamic rows, create
their component state explicitly and own adapter subscriptions at the page or
controller boundary.

## Validation commands

```bash
cargo fmt --all
cargo test -p gpui-form
cargo test -p gpui-form-macros
cargo test -p gpui-form-gpui-component
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component \
  --all-targets --all-features -- -D warnings
```
