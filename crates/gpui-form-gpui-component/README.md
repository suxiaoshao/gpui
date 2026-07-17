# gpui-form-gpui-component

`gpui-form-gpui-component` connects application-owned `gpui-component` states
to typed fields from `gpui-form`.

> Status: the caller-owned adapter API is implemented and is the only supported
> integration surface. The workspace has migrated off the legacy state-owning
> binding implementations.

## Responsibility

This crate knows how individual `gpui-component` controls expose values and
genuine user events. It creates the subscriptions needed to mirror those values,
then returns them to the caller. It does not own their lifetime.

| Concern | Owner |
| --- | --- |
| draft, dirty, touched, errors | `gpui-form` field |
| items, disabled, placeholder, capabilities | application |
| focus, open, query, highlight, scroll, IME | component state |
| subscription lifetime | caller's `gpui_form::SubscriptionSet` |
| component-specific value/event mapping | this adapter crate |

## Basic usage

Create and configure component state in the application, then extend the
controller's subscription set with the adapter result:

```rust
use gpui_component::input::InputState;
use gpui_form::SubscriptionSet;
use gpui_form_gpui_component::bind_input;

pub struct ProfileEditor {
    form: Entity<ProfileFormStore>,
    name_state: Entity<InputState>,
    subscriptions: SubscriptionSet,
}

impl ProfileEditor {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, ComponentBindError> {
        let form = cx.new(|cx| {
            ProfileFormStore::from_value(default_profile(), window, cx)
        });
        let name_state = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Display name")
        });

        let mut subscriptions = SubscriptionSet::new();
        subscriptions.extend(bind_input(
            ProfileFormStore::name_handle(&form),
            &name_state,
            window,
            cx,
        )?);

        Ok(Self {
            form,
            name_state,
            subscriptions,
        })
    }
}
```

There is no per-field binding handle. Dropping or clearing the caller-owned
`SubscriptionSet` disconnects its bindings. A controller that needs independent
mount/unmount scopes keeps one set per scope.

Binding returns `ComponentBindError` if the weak form field owner has already
been released. Each bind function builds a local set and returns it only after
initial projection and both subscriptions succeed; a failure drops the local
subscriptions and leaves the caller's set untouched.

## Supported adapters

The target crate exposes component-specific functions instead of one universal
component adapter trait:

```rust
bind_input(field, &input_state, window, cx)
bind_number(field, &input_state, window, cx)
bind_bool(field, &switch_state, window, cx)
bind_select(field, &select_state, window, cx)
bind_combobox(field, &combobox_state, window, cx)
```

Each returns `Result<SubscriptionSet, ComponentBindError>`. It knows only how
to project the field draft into that component and which events represent user
input. Component construction and configuration remain application concerns.

## Select and combobox options

Items are not form fields. Pass the field handle and component state to the
adapter-specific configuration command:

```rust
set_select_items(
    ProviderFormStore::kind_handle(&self.form),
    &self.kind_state,
    ProviderDelegate::new(kinds),
    window,
    cx,
)?;
```

The command reads the form draft, replaces the delegate, and reprojects the
matching selected item/label. It does not need a retained binding object because
the pinned `SelectState` and `ComboboxState` configuration setters do not publish
user events. If the draft is absent from the new items, the command keeps the
form draft and clears only the UI selection. Product submit policy decides
whether that unavailable draft is rejected or resolved.

Disabled state, placeholder, mask, capability flags, labels, and descriptions
are also component configuration. Update them directly without writing a form
field.

## Raw input codecs

Input and number adapters provide pure codecs for common raw draft shapes:

```rust
type OptionalTokenCodec = OptionalTextCodec;
type PortCodec = NumberCodec<u16>;
```

Plain `String` fields use `IdentityCodec<String>`. `OptionalTextCodec` explicitly
defines empty-string-to-`None`; `NumberCodec<N>` owns parsing semantics, while
min/max/step and integer/float presentation remain component configuration.

## Reentrancy and events

Each bind function creates a private shared direction guard captured by both
returned subscription closures:

```rust
enum ComponentSyncState {
    Idle,
    PublishingUserDraft,
    ApplyingFormDraft,
}
```

```text
component user event
  -> guard = PublishingUserDraft
  -> publish through FormFieldHandle
  -> form emits field event
  -> form-to-component subscription sees the guard and skips the echo

programmatic form event
  -> guard = ApplyingFormDraft
  -> update component value mirror
  -> component-to-form subscription skips any programmatic echo
```

The guard is adapter implementation state, not a public lifetime object. Its
`Rc` stays alive because both subscriptions capture it. The adapter must not
update the same GPUI entity from inside its active update scope; unconditional
deferral is not a substitute for correct event ownership.

## Custom components

Custom UI does not depend on this crate. An application or another component
library implements its own component-specific function with the same protocol:

```rust
fn bind_custom_control<Form, Owner>(
    field: FormFieldHandle<Form, CustomDraft>,
    state: &Entity<CustomState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, CustomBindError>;
```

The function installs both directions with a local guard and returns the
subscriptions. The caller owns them. No `FormComponentAdapter` trait,
`ComponentBindingSet`, or component-specific extension on `SubscriptionSet` is
needed.

## Non-goals

- no form/store binding;
- no subscription ownership in this crate;
- no catalog loading or fallback selection;
- no component construction derive attributes;
- no submit-time component reads;
- no universal adapter trait or public binding handle;
- no compatibility layer for the previous state-owning binding model; those
  implementations have been removed rather than carried forward here.

See the complete architecture and migration plan in
[`../gpui-form/docs/external-state-synchronization-plan.md`](../gpui-form/docs/external-state-synchronization-plan.md).
