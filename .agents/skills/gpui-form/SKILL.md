---
name: gpui-form
description: Use when implementing, reviewing, debugging, documenting, or integrating crates/gpui-form, gpui-form-macros, gpui-form-gpui-component, or Jaco forms. Covers typed form-owned values, revision-safe rebasing, owning bound controls, validation, exact integer controls, persistence boundaries, and nested groups/arrays.
---

# GPUI Form

Use this repo-owned skill for the three form crates and application code that
integrates them. Do not edit `.agents/skills/gpui/SKILL.md` for form-specific
guidance; that file is copied from upstream GPUI material.

## Required reading

Read the English public contract before changing an API; use the Chinese file as
the mirrored reference when needed:

1. `crates/gpui-form/README.md`
2. `crates/gpui-form/docs/guide.md`
3. `crates/gpui-form-macros/docs/guide.md`
4. `crates/gpui-form-gpui-component/docs/guide.md`

Read the applicable Chinese implementation plan before implementation:

- core: `crates/gpui-form/dev/typed-form-store.md`
- derive: `crates/gpui-form-macros/dev/form-store-derive.md`
- bound controls: `crates/gpui-form-gpui-component/dev/typed-bound-controls.md`
- Jaco migration: `app/jaco/docs/dev/gpui-form-migration.md`

The public API is the typed store plus owning-control architecture in those
documents. Do not infer the target design from transitional source code and do
not reintroduce deleted draft/codec/`bind_*`/submit-runtime compatibility APIs.

## Ownership model

Classify every mutable fact before editing:

1. current typed business value, baseline, `FormRevision`, validation
   report/generations, and already-started async-validation tasks: one generated
   form store containing exactly one `FormRuntime`; validation adapters and
   submit transforms are associated types constructed through `Default`, not
   additional stored instances;
2. component focus, IME, selection, popup/query/highlight, incomplete editor
   text, and binding subscriptions: the concrete owning bound control;
3. options, delegates, catalogs, capabilities, disabled state, placeholder, and
   nonblocking hints: application store/controller plus native component state;
4. persistence/database/provider calls, save `Task`, loading, retry, and
   notifications: application page/controller/service.

Do not create a second page/component business value, raw form draft, automatic
catalog fallback, form-to-database access, form-owned `FocusHandle`, form-owned
persistence state, or page-level `show_error` mirror.

## Typed form contract

- `#[derive(FormStore)]` generates one GPUI store containing exactly one
  `FormRuntime` with the complete typed model, baseline, monotonic revision, and
  validation runtime. The validation adapter and submit transform are associated
  types constructed through `Default`; the store does not retain either value.
- Leaf fields keep their declared Rust types. Integers, booleans, enums, options,
  and collections never travel through a form-owned `String` or codec.
- `FormField<Form, T>` is the leaf read/write/bind identity. Same-value writes are
  no-ops; successful business-value changes advance `FormRevision` once.
- `replace` changes current value, `reset` restores baseline, and `rebase`
  installs current plus baseline. Whole-form changes cancel affected async
  validation, clear stale reports, advance revision, and silently reproject all
  mounted controls.
- `rebase_if_revision(expected, value, cx) -> bool` is the only async-save merge
  primitive. A failed comparison has no side effects: no current/baseline/report,
  task, revision, or control-projection change.
- Nested values use generated group projections; dynamic arrays use caller-owned
  stable IDs through `array(id = "...")`. IDs are immutable, unique within the
  array, and never silently repaired or reused. Do not create child form entities.

## Bound control contract

- A bound control is an owning newtype created and bound in one constructor call
  from a generated `FormField<T>`.
- Its data fields are exactly `subscriptions: Vec<Subscription>` first and
  `state: Entity<State>` second; implement `Deref<Target = Entity<State>>` so
  callers use the upstream component API directly.
- Do not store `ControlAttachment`, form snapshots, options, focus/blur flags,
  error-visibility flags, or a second value in the wrapper.
- User events defer typed field writes until the emitter update ends. Deferred
  closures capture weak entities/liveness and check them again before updating.
- A third-party adapter creates its attachment through
  `FormField::attach_control(&self, cx: &mut App) ->
  Result<ControlAttachment<Form, T>, FormFieldError>`. `ControlAttachment` is
  cloneable; all clones share one private lease/liveness record, and the control
  issue becomes inactive only after the last clone is dropped. The only public
  attachment mutation intents are
  `defer_set_user_value(value, window, cx)`, `defer_blur(window, cx)`,
  `defer_set_issue(code, message, window, cx)`, and
  `defer_clear_issue(window, cx)`. They return `()`; immediate mutation,
  weak-handle upgrade, control/source IDs, and value readback remain private to
  core. Wrappers capture an attachment inside subscriptions but never store it
  as a wrapper field.
- Every `FieldChanged` and `ModelReplaced` event silently reprojects every bound
  control, including equal-value whole-model lifecycle events, changes to other
  paths that affect a projection, and the control that originated the change.
  `RuntimeChanged` is the only event ignored by value projection. Do not add
  origin/source echo skipping, authoritative readback return values, or public
  weak-attachment machinery.
- Dropping the newtype drops subscriptions. A component can privately retain
  incomplete native editor state; if it cannot produce `T`, it reports a
  lifecycle-scoped control issue that blocks submit without changing form value.
- Form-to-control subscriptions normally capture only the typed field and the
  native state's `WeakEntity`. A typed editor that owns a lifecycle-scoped
  control draft issue may additionally capture a clone of its attachment solely
  to call `defer_clear_issue` after a successful programmatic projection; among
  the built-in adapters, only the exact-integer control uses this exception. The
  wrapper still does not store the attachment.
- Options/delegate changes remain caller-owned. Call the upstream native setter,
  then immediately read the current form value and silently reproject it through
  the updated native state; if no setter exists, rebuild the wrapper. Do not wait
  for a form value event and do not put option synchronization in form core.

For a custom component or another component library, implement core
`FormControl<T>` with a constructor closure that configures the native state.
There is no generic `Config` associated type.

## gpui-component event contract

- `InputState`: user `Change` writes the typed value; `Blur` runs the field's
  on-blur validation. Focus is read from the native focus handle, not stored in
  form state.
- `SelectState`: bind `Option<Value>` and write only from `SelectEvent::Confirm`.
- `ComboboxState`: bind `Vec<Value>` and write only from `ComboboxEvent::Change`;
  form projection calls upstream `set_selected_values` against the current
  delegate. Do not synthesize reliable blur behavior for Select/Combobox.
- Checkbox/Switch: their native typed boolean event calls
  `FormField::set_user_value` directly; no extra wrapper is required unless the
  caller needs lifecycle-owned subscriptions.
- Exact integer control: the native state owns `N`, private editor text, typed
  min/max/step, and checked arithmetic. Invalid constructor bounds return an
  error. Incomplete, syntax, overflow, and out-of-range text stays native and
  activates a control issue; only a valid `N` writes the form. Never use `f64` or
  application-side `parse::<N>()`.

## Validation

- Supported triggers are mount, change, blur, dynamic, and submit.
- Mount validation runs exactly once after initial value and validation context
  are installed.
- Store a valid new typed user value before change validation.
- `set_validation_context` only replaces context and notifies. The page that
  owns an external dependency subscription explicitly runs Dynamic validation.
- Required semantics are fixed: trimmed-empty `String`, `None`, empty `Vec`,
  `HashMap`, `BTreeMap`, `HashSet`, or `BTreeSet`, and `false` are missing.
  Numeric and enum fields have no built-in required semantics.
- A successful field write invalidates only the intersecting required,
  structural, and generated synchronous field buckets plus intersecting async
  generations. It preserves adapter-wide and lifecycle-scoped control issues.
- Required rules, custom adapters, Garde, async validation, structural issues,
  and active control issues merge into one data-level report.
- The page owns subscriptions that trigger external validation. The form owns
  every async-validation `Task` after it is started, with scope and generation
  checks; cancel or supersede older generations. Stale completion returns false
  and cannot replace newer state.
- Every active async validation is blocking for `prepare_submit`. Nonblocking
  remote hints must stay application-owned and outside form validation.
- External libraries integrate through verified adapters. Map their stable paths
  and messages into generated `FieldPath`; do not leak provider rendering,
  persistence, or focus into form core.

## Submit and persistence

- `prepare_submit` performs synchronous submit validation, rejects structural or
  control issues, rejects pending async validation, and applies the configured
  transform exactly once to the same owned model version.
- Core has no `SubmitRuntime`, busy flag, attempts, persistence callback, retry,
  or notification API. `SubmitError` does not have `Busy`.
- The page captures `FormRevision` and prepared output in one form update, owns
  the save task/loading/retry, and calls `rebase_if_revision` after persistence.
- On persistence failure, do not rebase. On stale success, failed CAS has no form
  side effects; preserve newer edits and dirty state.
- Never assemble persistence data by reading controls or by combining multiple
  form/catalog snapshots.

## Options and external stores

Options/catalog changes are configuration changes, not form value changes:

1. mutate the application catalog store through its owner;
2. update the current native control items/delegate;
3. immediately read the current form value and silently reproject it through the
   updated native state, or rebuild the wrapper if the upstream API cannot update
   configuration in place;
4. retain unavailable typed values and explicitly run Dynamic validation.

Never select the first item, rewrite/rebase/persist a form field, reload a
database, or create an implicit fallback as a side effect of an options refresh.

## Focus and errors

The form reports `FieldPath` and validation status but owns no focus target or
interaction flags. The active page/dialog chooses which visible native control to
focus because one form field may be rendered by multiple controls.

Required metadata comes from generated schema. Render validation reports directly
with application localization; do not derive visibility from a second
`show_error`, touched, blurred, or submission-attempt value.

## GPUI reentrancy

- Never update an entity from inside that entity's active update scope.
- Separate component/delegate updates, deferred form writes, and owner
  route/focus changes.
- Capture weak entities in deferred/async callbacks and recheck lifetime before
  update; never detach lifecycle-critical work.
- Picker/list delegate callbacks emit intent; owners apply cross-entity changes.
- Do not hide reentrancy with fallback branches, nested-update retries, or
  `RefCell` error swallowing.

## Dependency identity

`gpui`, `gpui_platform`, `gpui_macros`, and gpui-component's transitive GPUI must
resolve from one identical Cargo Git source. A root `?rev=` source and upstream's
unqualified source are different crate identities even at the same commit. Follow
the adapter implementation plan: use one unqualified manifest source, pin the
exact commit in `Cargo.lock`, and validate with `--locked` plus `cargo tree -d`.

## Legacy deletion gate

After workspace migration, active source must not contain:

- `DraftFieldStore`, `FieldCodec`, raw `_draft` accessors;
- `FormFieldHandle`, `FormDraftEvent`, type-erased draft events;
- core `SubscriptionSet` or free `bind_input`/`bind_number`/`bind_select`/
  `bind_combobox`/`bind_bool`;
- `FieldChangeSource`, origin echo-skip/readback APIs, wrapper-stored attachment;
- form-owned focus/touched/blurred/error visibility;
- `SubmitRuntime`, `submit_runtime`, `SubmitError::Busy`, form-owned save tasks;
- Jaco `FormTextResolver`, application integer parsing, catalog fallback, or
  database reads from form/control paths.

Do not add compatibility wrappers that recreate these boundaries.

## Validation commands

For implementation changes run:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --all-features --locked
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
cargo tree -d --locked
git diff --check
```

For Jaco integration also run `cargo test -p jaco --all-features --locked`, the
workspace clippy target, and the Computer Use smoke in the Jaco migration plan
when the user authorizes runtime UI validation. For docs/skill-only changes,
validate links, English/Chinese semantic parity, residual terms, skill structure,
and `git diff --check`; crate tests are not required.
