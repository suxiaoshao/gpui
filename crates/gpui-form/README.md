# gpui-form

`gpui-form` is an early GPUI-native form state and validation crate.

This README describes the target user-facing design. The core runtime modules,
`FormItemId(u64)` dynamic array lifecycle, validation/transform adapter traits,
component state holder types, and a GPUI-aware `#[derive(FormStore)]` path are
now implemented. The derive macro currently supports default value fields,
`input`, `number`, `select`, `combobox`, `checkbox` / `switch`, binding-backed
app component fields, explicit child-store `group` and `array` fields, plus live
`garde` validation and `garde + validify` submit pipeline generation. Field-level
`placeholder` and `mask` / `masked` attributes are applied when built-in
`InputState` values are created. Built-in `input`, `number`, `select`,
`combobox`, and bool fields use `TextInputBinding`, `NumberInputBinding`,
`SelectBinding`, `ComboboxBinding`, and `BoolBinding` for state creation,
component read, write, and focus behavior. Internally, the runtime modules are
grouped under `core`, `component`, `pipeline`, and `view`; the derive expansion
is split by generated responsibility under `gpui-form-macros/src/expand/`.

## Design Reference

The current draft borrows these ideas from the cloned reference libraries:

- TanStack Form: each field owns `value + meta`, with interaction metadata such
  as touched, blurred, dirty, validating, errors, and derived valid/pristine
  flags.
- React Hook Form: the form exposes typed field registration/state APIs such as
  set value, get field state, trigger validation, handle submit, reset, and
  field-focused errors. For dynamic arrays, `gpui-form` keeps the same stable
  runtime-identity goal, but uses a Rust `FormItemId(u64)` newtype instead of
  UUID strings or domain ids.
- Garde / Validify: validation rules should be delegated to `garde`, while
  submit-time modification should be delegated to `validify::Modify` instead of
  being reimplemented in `gpui-form`.
- GPUI / gpui-component: component state such as `InputState` and `SelectState`
  must stay owned by GPUI entities, so generated field stores should expose the
  concrete component state needed by each field.

## Validation Boundary

`gpui-form` owns form state, event triggers, field paths, error visibility,
component state, and subscriptions. `garde` owns validation rules. `validify`
owns submit-time modification through `Modify`.

Live validation is read-only. `on_change` and `on_blur` can run `validify` on a
clone to build a normalized preview, then call `garde::Validate` on that preview.
They must not rewrite `InputState` or other component state. When the user clicks
submit, `gpui-form` runs `validify::Modify`, writes the normalized value back to
the visible draft, and then runs `garde::Validate`. That write-back happens even
when submit validation fails.

## Core User Model

Users write the final submit input as an ordinary Rust struct. A derive macro
generates a matching form store type. Each generated field has a one-to-one
relationship with the target struct field. UI metadata stays in `#[form(...)]`;
validation metadata stays in `#[garde(...)]`, and modification metadata stays in
`#[modify(...)]` from `validify`.

```rust
use garde::Validate;
use gpui_form::FormStore;
use validify::Validify;

#[derive(Clone, Debug, PartialEq, FormStore, Validate, Validify)]
#[garde(allow_unvalidated)]
#[form(
    store = ConnectionFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
pub struct ConnectionInput {
    #[form(
        component = "input",
        label = "form-example-connection-name-label",
        validate(on_change, on_blur, on_submit)
    )]
    #[modify(trim)]
    #[garde(length(min = 1, max = 80))]
    pub display_name: String,

    #[form(
        component = "select",
        delegate = "ConnectionKindOptions",
        options = "ConnectionKindOptions::new()",
        label = "form-example-connection-kind-label",
        validate(on_submit)
    )]
    #[garde(skip)]
    pub kind: Option<ConnectionKind>,

    #[form(
        component = "input",
        label = "form-example-endpoint-url-label",
        validate(on_blur, on_submit)
    )]
    #[modify(trim)]
    #[garde(url)]
    pub endpoint_url: Option<String>,

    #[form(binding = "SecretRefBinding", label = "form-example-secret-label")]
    #[garde(skip)]
    pub secret_ref: Option<String>,
}
```

The macro expands to a form store shape equivalent to:

```rust
pub struct ConnectionFormStore {
    pub display_name: TextFieldStore<String>,
    pub kind: SelectFieldStore<Option<ConnectionKind>, ConnectionKindOptions>,
    pub endpoint_url: TextFieldStore<Option<String>>,
    pub secret_ref: ComponentFieldStore<Option<String>, SecretRefBinding>,
    validation: GardeAdapter<ConnectionInput>,
    transform: ValidifyTransform<ConnectionInput, ConnectionInput>,
    meta: FormMeta,
}
```

The generated store is draft-only. Editing the form updates field stores, not the
original `ConnectionInput`. The domain value is created only after successful
submit validation. Submit first writes the `validify`-normalized value back into
the visible draft, then validates that normalized value with `garde`.

```rust
let original = ConnectionInput {
    display_name: "Production".into(),
    kind: Some(ConnectionKind::Https),
    endpoint_url: Some("https://example.com".into()),
    secret_ref: Some("secrets.production.token".into()),
};

let form = cx.new(|cx| ConnectionFormStore::from_value(original, window, cx));

// Later, from a submit action:
match form.update(cx, |form, window, cx| form.submit(window, cx)) {
    Ok(input) => save_connection(input, cx),
    Err(report) => show_first_error(report, cx),
}
```

## Rendering with gpui-component

Generated fields expose component-specific state for `gpui-component`.
Select and combobox fields must declare the gpui-component searchable-list
delegate type explicitly, because the generated store type needs the delegate as
a Rust generic parameter. If `options = "..."` is omitted, the delegate must
implement `Default`.

```rust
#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = ChoiceFormStore)]
pub struct ChoiceInput {
    #[form(
        component = "select",
        delegate = "Vec<String>",
        options = "vec![\"primary\".to_string(), \"secondary\".to_string()]",
        searchable
    )]
    pub target: Option<String>,

    #[form(
        component = "combobox",
        delegate = "Vec<String>",
        options = "vec![\"fast\".to_string(), \"cheap\".to_string()]",
        multiple,
        searchable
    )]
    pub tags: Vec<String>,
}
```

`Option<T>` is supported by `SelectFieldValue`, and `Vec<T>` / `Option<T>` are
supported by `ComboboxFieldValue`. A required non-`Option` domain enum can opt in
by implementing the corresponding value trait.

Nested structs use an explicit generated child store:

```rust
#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = ProfileFormStore)]
pub struct ProfileInput {
    #[form(component = "input")]
    pub nickname: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = AccountFormStore)]
pub struct AccountInput {
    #[form(component = "group", store = "ProfileFormStore")]
    pub profile: ProfileInput,
}
```

The parent group stores the child `Entity<ProfileFormStore>`, caches the child
draft/meta for parent `draft()` and `meta()`, and stores the child observe
subscription in the group store.

```rust
use gpui_component::form::{field, v_form};
use gpui_component::input::Input;
use gpui_component::select::Select;

fn render_connection_form(
    form: Entity<ConnectionFormStore>,
    window: &mut Window,
    cx: &mut Context<SettingsPanel>,
) -> impl IntoElement {
    let form_read = form.read(cx);

    v_form()
        .child(
            field()
                .label(form_read.display_name.label(cx))
                .required(form_read.display_name_required())
                .description(form_read.display_name.help_text(cx))
                .child(Input::new(form_read.display_name_input_state()))
                .error(form_read.display_name.visible_error_text(cx)),
        )
        .child(
            field()
                .label(form_read.kind.label(cx))
                .required(form_read.kind_required())
                .child(Select::new(form_read.kind_select_state()))
                .error(form_read.kind.visible_error_text(cx)),
        )
        .child(
            field()
                .label(form_read.endpoint_url.label(cx))
                .child(Input::new(form_read.endpoint_url_input_state()))
                .error(form_read.endpoint_url.visible_error_text(cx)),
        )
}
```

The field remains the source of truth for form interaction state. Component state
is the UI adapter for a concrete control, not the submitted domain value.

The derive macro also generates typed accessors so app code does not need to
reach into each field store for common component handles and values:

```rust
let display_name = form_read.display_name_value();
let enabled = form_read.enabled_value();
let input = form_read.display_name_input_state();
let select = form_read.kind_select_state();
```

For programmatic changes, prefer generated field setters over reading the whole
draft, patching one field, and calling `write_draft`:

```rust
form.update(cx, |form, cx| {
    form.set_enabled_value(true, FieldChangeCause::UserInput, window, cx);
});
```

The setter updates the field draft value, writes the matching component state,
runs field-triggered validation for the supplied cause, emits the typed
`FieldChanged` event, refreshes form meta, and notifies the view.

Generated stores also expose typed external-error helpers:

```rust
form.update(cx, |form, cx| {
    form.clear_all_errors(cx);
    form.apply_field_error(ConnectionFormField::DisplayName, error, cx);
});
```

These helpers are for app-specific validation or service errors that are already
mapped to a generated field enum. They keep `FieldPath` as an internal validation
routing detail instead of making app code build string paths.

## Dynamic Array Identity

Dynamic array fields expose stable runtime item ids for rendering and event
ownership. The ids are generated when rows are created and are not part of the
submitted domain value.

Derived array fields use an explicit generated child store:

```rust
#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = HeaderFormStore)]
pub struct HeaderInput {
    #[form(component = "input")]
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = HeaderListFormStore)]
pub struct HeaderListInput {
    #[form(component = "array", store = "HeaderFormStore")]
    pub headers: Vec<HeaderInput>,
}
```

The generated parent store exposes helpers named after the field:

```rust
form.headers_append(value, window, cx);
form.headers_insert(index, value, window, cx)?;
form.headers_remove(index, cx)?;
form.headers_remove_id(row_id, cx);
form.headers_move(from, to, cx)?;
form.headers_swap(a, b, cx)?;
form.headers_replace(values, window, cx);
form.headers_reset_items(values, window, cx);
let rows = form.headers_values_with_id();
```

These helpers take `Context<ParentStore>` where new child rows or row
subscriptions are created, so every child `Entity<HeaderFormStore>` and observe
subscription is owned by the array item lifecycle.

```rust
pub struct FieldArrayItem<Item> {
    pub id: FormItemId,
    pub index: usize,
    pub item: Item,
}

pub struct FormItemId(u64);
```

Array operations follow the `FormItemId(u64)` runtime identity lifecycle:

- `append` / `insert` create new ids.
- `remove` / `remove_id` drop the removed item id, component state, and
  subscriptions.
- `move` / `reorder` move ids together with their items.
- `reset` / `replace` rebuild the item list and ids.
- `values_with_id` returns `FormRowValue<T>` snapshots for app validators that
  need to report row-specific errors without leaking the id into the submitted
  domain value.

Array row types should carry business meaning. Do not use one generic
`KeyValueRow` and then patch child placeholders from the parent array. If the
rows have different labels, placeholders, required rules, or normalization
rules, define separate row inputs:

```rust
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

#[derive(Clone, Debug, PartialEq, FormStore)]
#[form(store = CommandFormStore)]
pub struct CommandInput {
    #[form(component = "array", store = "EnvironmentRowFormStore")]
    pub environment: Vec<EnvironmentRowInput>,

    #[form(component = "array", store = "HeaderRowFormStore")]
    pub headers: Vec<HeaderRowInput>,

    #[form(component = "array", store = "SecretHeaderRowFormStore")]
    pub secret_headers: Vec<SecretHeaderRowInput>,
}
```

## Field Trait

Every generated field implements a shared field trait. The exact names are still
draft, but each field should support this surface:

```rust
pub trait FormField {
    type Value;
    type ComponentState;

    fn value(&self) -> &Self::Value;
    fn set_value(&mut self, value: Self::Value, cause: FieldChangeCause);
    fn reset(&mut self);

    fn component_state(&self) -> &Entity<Self::ComponentState>;

    fn meta(&self) -> FieldMeta;
    fn errors(&self) -> &[FieldError];
    fn visible_errors(&self) -> &[FieldError];
    fn visible_error_text(&self, cx: &App) -> Option<SharedString>;

    fn mark_touched(&mut self);
    fn mark_blurred(&mut self);
    fn mark_dirty(&mut self);

    fn validate(&mut self, trigger: ValidationTrigger) -> FieldValidationResult;
}
```

`FieldPath` is not a field-store property. The derive macro owns field
addresses and uses them only when creating `ValidationScope` values and routing
`FormValidationReport` errors back into concrete fields.

The derive macro also generates typed field and event enums from the store name.
For `ConnectionFormStore`, the generated types are:

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

App code should subscribe to the form entity for business side effects instead
of subscribing to each underlying `InputState`:

```rust
cx.subscribe_in(&form, window, |this, _form, event: &ConnectionFormEvent, _window, cx| {
    if let ConnectionFormEvent::FieldChanged(field) = event {
        this.handle_field_change(field.key(), cx);
    }
});
```

Suggested field metadata:

```rust
pub struct FieldMeta {
    pub is_touched: bool,
    pub is_blurred: bool,
    pub is_dirty: bool,
    pub is_pristine: bool,
    pub is_default_value: bool,
    pub is_validating: bool,
    pub is_valid: bool,
}
```

Suggested form metadata:

```rust
pub struct FormMeta {
    pub is_dirty: bool,
    pub is_pristine: bool,
    pub is_touched: bool,
    pub is_validating: bool,
    pub is_valid: bool,
    pub can_submit: bool,
    pub is_submitting: bool,
    pub is_submitted: bool,
    pub is_submit_successful: bool,
    pub submission_attempts: u32,
}
```

## Validation Triggers

Fields can be configured from `#[form(...)]` for different validation causes.
Validation rules are supplied by `garde`; submit-time modification is supplied
by `validify::Modify`.

```rust
#[derive(Clone, Debug, PartialEq, FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = ShortcutFormStore, validation(adapter = "garde"))]
pub struct ShortcutInput {
    #[form(component = "input", validate(on_change, on_blur, on_submit))]
    #[garde(length(min = 1, max = 80))]
    pub title: String,

    #[form(component = "select", validate(on_submit))]
    #[garde(skip)]
    pub action: ShortcutAction,
}
```

Target trigger model:

```rust
pub enum ValidationTrigger {
    Mount,
    Change,
    Blur,
    Submit,
    Dynamic,
}
```

Errors should preserve their source trigger and adapter source so UI can choose
between "show all errors" and "show the error produced by this interaction".

```rust
pub struct FieldError {
    pub path: FieldPath,
    pub trigger: ValidationTrigger,
    pub source: ValidationSource,
    pub code: &'static str,
    pub message_key: &'static str,
    pub params: ErrorParams,
}

pub enum ValidationSource {
    Garde,
    App(&'static str),
    Internal,
}
```

## Component Binding

Users can provide their own component state by implementing the same binding
trait used by the built-in components. `component = "input"` and
`component = "select"` are shorthand for built-in bindings; app components use
`binding = "..."`.

```rust
pub trait FormComponentBinding<Value>: 'static {
    type State: 'static;
    type Event: 'static;

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
    fn set_disabled(
        state: &Entity<Self::State>,
        disabled: bool,
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

`ComponentStateOptions` is the field attribute payload passed into bindings:

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

Example:

```rust
pub struct SecretRefBinding;

pub struct SecretRefState {
    input: Entity<InputState>,
    reveal: bool,
}

impl FormComponentBinding<Option<String>> for SecretRefBinding {
    type State = SecretRefState;
    type Event = InputEvent;

    fn new_state(
        initial: &Option<String>,
        options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let placeholder = options
            .placeholder_key
            .map(|key| gpui_form::resolve_form_text(key, cx));

        cx.new(|cx| SecretRefState {
            input: cx.new(|cx| {
                let mut input =
                    InputState::new(window, cx).default_value(initial.clone().unwrap_or_default());
                if let Some(placeholder) = placeholder {
                    input = input.placeholder(placeholder);
                }
                if options.masked {
                    input = input.masked(true);
                }
                input
            }),
            reveal: false,
        })
    }

    fn read_value(state: &Entity<Self::State>, cx: &App) -> Option<String> {
        let value = state.read(cx).input.read(cx).value();
        (!value.is_empty()).then_some(value)
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &Option<String>,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        let input = state.read(cx).input.clone();
        input.update(cx, |input, cx| {
            input.set_value(value.clone().unwrap_or_default(), window, cx);
        });
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        form: Entity<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let input = state.read(cx).input.clone();
        SubscriptionSet::from(cx.subscribe_in(&input, window, move |_, _, event, window, cx| {
            let _ = (&form, event, window, cx);
        }))
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let input = state.read(cx).input.clone();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
        true
    }
}
```

Then the target struct can opt into it:

```rust
#[derive(Clone, Debug, PartialEq, FormStore)]
pub struct CommandInput {
    #[form(binding = "CommandArgsBinding")]
    pub args: Vec<String>,
}
```

Field UI options are passed to the binding through `ComponentStateOptions`.
Post-creation placeholder patching helpers should not be needed: the row field
declares the placeholder and the binding applies it when creating its state.

## Manual FormState Implementation

The derive macro should not be the only path. Advanced users can implement a
form state trait by hand for forms that are generated dynamically or backed by
non-standard controls.

The first implementation assumes the normalized output has the same shape as the
domain input. App-specific repository command conversion should happen after
`submit` returns `Ok(input)`.

```rust
pub trait FormState {
    type Draft;
    type Output;
    type Validation: ValidationAdapter<Self::Draft>;
    type Transform: SubmitTransform<Self::Draft, Self::Output>;

    fn meta(&self) -> FormMeta;
    fn draft(&self) -> Self::Draft;
    fn validation(&self) -> &Self::Validation;
    fn transform(&self) -> &Self::Transform;
    fn write_normalized_output(&mut self, output: Self::Output, window: &mut Window, cx: &mut App);
    fn field_paths(&self) -> &[FieldPath];
    fn field(&self, path: &FieldPath) -> Option<&dyn AnyFormField>;
    fn field_mut(&mut self, path: &FieldPath) -> Option<&mut dyn AnyFormField>;

    fn validate(&mut self, trigger: ValidationTrigger) -> FormValidationReport;
    fn submit(&mut self, window: &mut Window, cx: &mut App) -> Result<Self::Output, FormValidationReport>;
    fn reset(&mut self, window: &mut Window, cx: &mut App);
    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App) -> bool;
}
```

Manual example:

```rust
pub struct DynamicHeaderForm {
    rows: Vec<HeaderRowField>,
    meta: FormMeta,
    validation: GardeAdapter<Vec<HeaderRowInput>>,
    transform: ValidifyTransform<Vec<HeaderRowInput>, Vec<HeaderRowInput>>,
}

impl FormState for DynamicHeaderForm {
    type Draft = Vec<HeaderRowInput>;
    type Output = Vec<HeaderRowInput>;
    type Validation = GardeAdapter<Vec<HeaderRowInput>>;
    type Transform = ValidifyTransform<Vec<HeaderRowInput>, Vec<HeaderRowInput>>;

    fn meta(&self) -> FormMeta {
        self.meta.clone()
    }

    fn draft(&self) -> Self::Draft {
        todo!("build draft from row field values")
    }

    fn validation(&self) -> &Self::Validation {
        &self.validation
    }

    fn transform(&self) -> &Self::Transform {
        &self.transform
    }

    fn write_normalized_output(&mut self, output: Self::Output, window: &mut Window, cx: &mut App) {
        todo!("write normalized row values back into row fields and component states")
    }

    fn field_paths(&self) -> &[FieldPath] {
        todo!("return stable row paths")
    }

    fn field(&self, path: &FieldPath) -> Option<&dyn AnyFormField> {
        todo!("lookup row field")
    }

    fn field_mut(&mut self, path: &FieldPath) -> Option<&mut dyn AnyFormField> {
        todo!("lookup row field")
    }

    fn validate(&mut self, trigger: ValidationTrigger) -> FormValidationReport {
        let preview = self
            .transform
            .preview(&self.draft(), &TransformContext::default())
            .expect("preview transform should not fail");

        self.validation
            .validate(&preview, trigger, ValidationScope::Form, &ValidationContext::default())
            .into_form_report()
    }

    fn submit(&mut self, window: &mut Window, cx: &mut App) -> Result<Self::Output, FormValidationReport> {
        let normalized = self
            .transform
            .transform_on_submit(&self.draft(), &TransformContext::default())
            .map_err(TransformReport::into_form_report)?;

        self.write_normalized_output(normalized.clone(), window, cx);

        self.validation
            .validate(&normalized, ValidationTrigger::Submit, ValidationScope::Form, &ValidationContext::default())
            .into_result()?;

        Ok(normalized)
    }

    fn reset(&mut self, window: &mut Window, cx: &mut App) {
        todo!("reset rows and component states")
    }

    fn focus_first_error(&mut self, window: &mut Window, cx: &mut App) -> bool {
        todo!("focus first invalid row")
    }
}
```

## Submit Flow

The submit path validates all fields, focuses the first visible error, and only
then returns the final domain input. Submit also writes normalized values back
to the form draft before returning either `Ok` or `Err`.

```rust
fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let result = self.connection_form.update(cx, |form, cx| {
        form.submit(window, cx)
    });

    match result {
        Ok(input) => {
            self.repository.save_connection(input);
        }
        Err(report) => {
            self.connection_form.update(cx, |form, cx| {
                form.focus_first_error(window, cx);
                form.mark_submit_attempted(report);
            });
        }
    }
}
```

## Attribute Sketch

The derive macro should keep GPUI field configuration near the domain input
field, while validation libraries keep their own validation attributes.

```rust
#[derive(FormStore, garde::Validate, validify::Validify)]
#[garde(allow_unvalidated)]
#[form(
    store = ConnectionFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
pub struct ConnectionInput {
    #[form(component = "input")]
    #[form(label = "form-example-connection-name-label")]
    #[form(description = "form-example-connection-name-description")]
    #[form(placeholder = "form-example-connection-name-placeholder")]
    #[form(validate(on_change, on_blur, on_submit))]
    #[modify(trim)]
    #[garde(length(min = 1, max = 80))]
    pub display_name: String,

    #[form(
        component = "select",
        delegate = "ConnectionKindOptions",
        options = "ConnectionKindOptions::new()",
        searchable
    )]
    #[garde(skip)]
    pub kind: Option<ConnectionKind>,
}
```

Open questions for the macro:

- Whether labels and descriptions are always i18n keys.
- Whether stale validation reports should be explicitly cleared when dynamic
  arrays are reordered before the next validation run.
