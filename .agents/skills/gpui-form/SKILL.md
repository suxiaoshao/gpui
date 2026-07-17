---
name: gpui-form
description: Use when implementing, reviewing, debugging, or integrating crates/gpui-form and its UI adapters. Covers pure form draft ownership, validation/transform boundaries, field handles, component value mirrors, independent component configuration, and nested groups/arrays.
---

# GPUI Form

Use this skill for `crates/gpui-form`, `gpui-form-macros`, the
`gpui-form-gpui-component` adapter, and app code that deliberately integrates
their form APIs.

## Required Reading

Before changing form runtime or derive behavior, read:

1. `crates/gpui-form/README.md`
2. `crates/gpui-form/docs/development-plan.md`
3. `crates/gpui-form/docs/external-state-synchronization-plan.md`
4. affected files under `crates/gpui-form/src/` and
   `crates/gpui-form-macros/src/expand/`

For app integration, read the app state flow first. For Jaco issue #175 also
read `app/jaco/docs/dev/issue-175/state-ownership-sync-plan.md`.

## Target Core Model

- A generated `FormStore` is the only editable draft owner. It owns raw/typed
  field drafts, group/array state, meta, validation, transform, and submit.
- The original domain value, committed config/DB snapshot, provider catalog, or
  repository is owned by the calling app. `gpui-form` does not access them.
- `gpui-form` must not own, construct, expose, or read a UI component entity.
  Legacy names such as `FormComponentBinding`, `ComponentStateOptions`, and
  `ComponentFieldStore` may appear in historical migration notes, but they are
  not part of the current API or target abstractions.
- Do not add `FormSelection`, draft revision, generic conflict payloads, or
  automatic rebase to solve component synchronization. The approved minimal
  external operation is an explicit whole-form `replace_from_value` after the
  app has decided whether to discard a dirty draft.
- Do not add a generic form↔store two-way binding or a dependency on `gpui-store`.
  The app composes committed value replacement and commit explicitly.
- Derive keeps a generated typed field enum, but whole-form observers use the
  runtime `FormStoreEvent<Field>` instead of a generated event enum per form.
  Adapter mirrors use `FormFieldHandle`/`FieldDraftEvent`; focus/blur are not
  separate public event variants.
- `required` remains form validation/meta. Dynamic rules use a pure generated
  `set_<field>_required(required, cx)` with no `Window` or component side effect.
- Nested structures use `group(store = ...)` and `array(store = ...)`; do not
  model them through a `component = ...` attribute.

## Component Integration Model

Always classify component state into three semantic channels:

1. user draft/value: owned by the form;
2. options/config/capability/disabled/placeholder: owned by the app or catalog;
3. focus/open/query/highlight/scroll/IME/tasks: owned by the component entity.

`gpui-form-gpui-component` provides component-specific bind functions between
an app-created component state and a generated `FormFieldHandle`. Each function
returns a core `SubscriptionSet`; the caller extends a controller/page-owned set
and therefore decides the binding lifetime. Do not introduce per-field binding
handles, `ComponentBindingSet`, a universal component adapter trait, or
component-specific methods on `SubscriptionSet`.

The adapter's two subscription closures capture a private shared direction
guard so a user event does not synchronously mirror back into the component
currently being updated. The adapter crate creates subscriptions but does not
own them after return. Custom components and other component libraries implement
their own `bind_* -> Result<SubscriptionSet, _>` functions from the same
`FormFieldHandle` protocol without depending on this adapter crate.

Options/configuration never pass through the form and never trigger
`replace_from_value`, dirty conflict, fallback, or submit normalization. Simple
config uses component-specific APIs. Select/combobox items must use the adapter's
component-specific config command so it replaces the delegate and reprojects the
selected item/label from the form draft without mutating the form. Submit and
validation read only form-owned draft.

## GPUI Ownership and Reentrancy

- Keep form/group/array entities owned by the form caller. Keep component state
  and one or more core `SubscriptionSet`s owned by the view/controller.
- Never subscribe to component changes generically. Only genuine user events
  publish draft changes; programmatic options/state updates do not.
- Nested groups/arrays may observe child draft/meta and refresh the generated parent;
  parent notification should occur once per effective change.
- Observers read/derive/notify. If another entity must change, schedule an explicit
  command/task after the source update scope completes.

## Validation and Persistence Boundary

- `garde` owns validation rules and `validify` owns submit-time modification;
  form runtime routes them and writes normalized submit values back to the visible
  draft according to the existing contract.
- App-specific persistence, DB/config writes, catalog refresh and conflict UI stay
  in the app. An explicit `replace_from_value` discards the existing baseline/draft.
- Never use a control/picker cache, formatted label, or form selection as the
  business value for repository requests or attachment/capability validation.

## Validation

For form changes run:

```sh
cargo fmt --all
cargo test -p gpui-form
cargo test -p gpui-form-macros
cargo check -p gpui-form
cargo clippy -p gpui-form --all-targets --all-features -- -D warnings
git diff --check
```

For app integration also run the focused app checks and the applicable GPUI
desktop smoke test. If only docs or skills changed, report that crate tests were
not needed and run `git diff --check`.
