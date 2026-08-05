---
name: gpui-form
description: Use when implementing, reviewing, debugging, documenting, or integrating crates/gpui-form, gpui-form-macros, gpui-form-gpui-component, Jaco forms, or Feiwen forms. Covers the implemented FormSchema/Form<M> API, explicit form ownership, typed total and dynamic paths, session-bound case and optional resolution, runtime-owned item identity, private gpui-operation transitions, component adapters, validation, prepare/rebase, and breaking consumer migration.
---

# GPUI Form

Use this repo-owned skill for the three Form crates and their application
consumers. The old `FormModel`/generated `FormState` contract has been removed;
active code must use the implemented `FormSchema` + `Form<M>` API.

## Read the applicable contract

For public API or examples, read the relevant current document:

1. `crates/gpui-form/README.md`
2. `crates/gpui-form/docs/guide.md`
3. `crates/gpui-form-macros/docs/guide.md`
4. `crates/gpui-form-gpui-component/docs/guide.md`

For Issue #199 implementation decisions, read:

- `crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md`
- `app/jaco/docs/dev/issue-199/form-vnext-migration.md`
- `app/feiwen/docs/dev/issue-199/form-operation-store-migration.md`

Keep public README and guide changes mirrored in English and Chinese. Newly
created development documents are Chinese only. Preserve historical documents
unless the task explicitly changes them.

## Ownership model

Classify each mutable fact before editing:

1. One `Entity<Form<M>>` owns the current typed model, baseline, revision,
   validation report, topology, async validation tasks, and private transition
   runtime for one editing session.
2. `FieldDef`, `ChildDef`, `ItemsDef`, `CaseDef`, and located paths contain
   schema/access/location data only. They never retain form entities, values,
   subscriptions, or native controls.
3. Native entities own focus, IME, selection, popup state, incomplete editor
   text, and component subscriptions. Their adapter handles own a
   `ControlLease`.
4. Options, delegates, catalogs, capabilities, resource phase, and nonblocking
   hints remain in application state or `gpui-store`.
5. Save, fetch, query, loading, retry, database, and provider operations remain
   application-owned. Form only prepares accepted typed snapshots.

Do not create child form entities, a second page-owned business draft, a form
copy in Store, form-owned persistence, or form-to-database access.

## Schema and typed paths

- Derive `FormSchema` on monomorphic named-field structs and supported enums.
- Use `#[form(child)]` for nested schemas and `Option<Child>`;
  `#[form(items)]` for `Vec<Item>`; `#[form(required)]` and
  `#[form(validate(...))]` for leaf validation metadata.
- Use root definitions directly or compose root-first with `.then(...)`.
  Static composition is pure and does not receive a form.
- A total path exposes infallible `value`, `set`, `validate`, `errors`, and
  `bind_control` operations that receive the explicit strong form entity.
- A dynamic path exposes `try_value`, `try_set`, `try_validate`, `try_errors`,
  and `try_bind_control` because its item/case/optional boundary may retire.
- Resolve enum and optional boundaries against the current borrowed form:

  ```rust,ignore
  let payload = enum_path.try_case(form.read(cx), Enum::CASE)?;
  let child = optional_path.try_some(form.read(cx))?;
  ```

  The returned `DynamicPath` captures session, address, and incarnation without
  retaining an entity.

## Runtime-owned collection identity

- Obtain `ItemPath` only from `items`/`try_items` or collection mutations.
- Use `append`, `insert_before`, `move_before`, `remove`, `replace_all`, and
  `ItemPath::move_to`; never navigate by index, business ID, serde token, or an
  application registry.
- Use opaque `PathKey` for UI identity and maps. It converts to a GPUI
  `ElementId`; do not expose raw session/token/incarnation values.
- Same-parent reorder keeps identity. Removal/reinsertion, whole-model
  lifecycle replacement, case/optional reconstruction, and cross-parent move
  retire old paths as defined by the runtime.
- Stage topology changes before mutating the model. Allocation or anchor
  failures must leave both model and topology unchanged. A successful
  cross-parent move is one logical mutation: one revision/event/notification,
  with validation invalidation and triggers covering both source and
  destination paths.
- Never expose `TopologyIndex`, canonical addresses, session IDs, item tokens,
  epochs, or topology snapshots.

## Mutation, lifecycle, and private transitions

- Public callers use domain methods: `set`, collection mutations, `validate`,
  `replace`, `reset`, `rebase`, `rebase_if_revision`, async-validation methods,
  and `prepare`.
- Core mutations are reduced by private owned messages through
  `gpui_operation::Transition`. Do not expose Form messages, dispatch, or the
  transition module to macros, adapters, or applications.
- An equal field write is a no-op. A successful logical mutation advances the
  revision once, emits at most one `FormEvent`, and notifies at most once.
- `replace`, `reset`, and `rebase` retire old topology and async work.
  `rebase_if_revision` is the only async-save CAS merge primitive; failure
  changes nothing.
- Bindings capture the topology epoch that created them. Deferred callbacks
  from an older topology or lifecycle must become no-ops even if their address
  would happen to resolve again.

## Validation and submission

- `Validator<M>` receives the model plus a snapshot-bound
  `ValidationRequest`; emit typed issues through `ValidationSink`.
- Validation supports mount, change, blur, dynamic, and submit triggers.
  Preserve independent source/path/trigger buckets and replace only the bucket
  targeted by the current validation pass. Mutations invalidate every
  intersecting stale bucket before running the applicable triggers. Attach
  issues to the precise field.
- Snapshot resolvers enumerate runtime item paths and resolve enum/optional
  payloads against that same snapshot; validators never reconstruct paths from
  indexes or reread the live form.
- Garde positional reports must map through the same topology snapshot.
  Invalid or inactive adapter paths become blocking internal issues rather than
  being dropped or attached elsewhere.
- Start remote checks with `start_async_validation` for total paths or
  `start_dynamic_async_validation` for a located dynamic path. Intersecting
  writes and topology/lifecycle changes cancel stale work; pending validation
  blocks `prepare`.
- A `ControlLease` is a weak live marker, not a second owner. Whole-model and
  subtree lifecycle retirement revoke affected leases immediately; stale
  reports and queued callbacks cannot republish their issues.
- `prepare` runs submit validation and returns `Prepared<M>`. Capture the
  revision before `Prepared::map`; let the application own I/O and apply the
  canonical saved model with `rebase_if_revision`.

## Component adapters and custom controls

- Use `FormInput`, `FormIntegerInput`, `FormSelect`, and `FormCombobox` for
  gpui-component controls. `new` accepts a total path; `try_new` accepts a
  resolved dynamic path.
- Keep a stateful adapter as a plain Rust handle containing subscriptions, a
  `ControlLease`, and the native entity. Dropping it must retire queued binding
  callbacks and control issues.
- Create a binding with `bind_control`/`try_bind_control`, immediately retain
  `binding.lease()`, and capture binding clones only in deferred native event
  callbacks.
- Use `defer_set`, `defer_blur`, `defer_set_issue`, and `defer_clear_issue`.
  Silently project Form events back to the native entity; a dynamic projection
  that no longer resolves is ignored and its control is torn down.
- Adapter projection reacts only to model events. Validation-only notifications
  must not overwrite native editor state or clear that adapter's leased control
  issue.
- Stateless controlled elements may read `value` and call `set` directly when
  the callback is not inside another entity's active update.
- Keep invalid/incomplete integer text native and publish a leased control
  issue. Never route typed numeric values through application-side `f64` or
  ad-hoc parsing.

## Application integration

- Jaco forms use local `Entity<Form<M>>` sessions. Provider, Prompt, Shortcut,
  ChatInput, RunSettings, and MCP settings consume the new API without importing
  Form's private transitions. MCP runtime is outside this migration.
- Feiwen Query uses a recursive typed tree and runtime `PathKey`; no business or
  UI counter is a form identity. Fetch uses a flat Form; Query/Fetch run state
  belongs to their private application transitions.
- Catalog changes update their owner Store and native option projection without
  rewriting Form values. The application explicitly requests dynamic
  validation when product rules require it.
- Preserve selected catalog values that are absent from the current catalog as
  explicit disabled projections with a nonblocking hint; never silently erase
  them. Reconcile recursive native rows by `PathKey` so unaffected controls keep
  their native identity.
- Feiwen Query keeps the exact prepared `QueryDraft` snapshot for display and
  recovery while the runner owns the compiled `QuerySpec`. Do not reconstruct a
  draft from an executable spec because dormant variant operands would be lost.
- Never update an entity from inside its active update scope. Defer
  component-to-form writes and recheck weak/dynamic lifetime in queued work.

## Removed surfaces

Do not add compatibility wrappers or active-source uses of:

- `FormModel`, generated `*Form`/`FormState`, `FormField`,
  `PartialFormField`, `FormItemId`, `#[form(array(...))]`, or child-first
  `within`;
- writable `project_value`, core `SubmitTransform`/`prepare_submit`/
  `PreparedSubmit`, or `validify-transform`/`form-pipeline` features;
  application-owned domain helpers may still use similar names;
- descriptor-owned form entities, `FormReleased`, public control IDs, public
  Form messages, or application-visible topology tokens;
- form-owned focus/touched/error visibility, save task, resource/catalog state,
  persistence callback, or database access.

Intentional compile-fail fixtures and historical Issue #175/#199 documents may
mention removed APIs; classify them rather than rewriting history.

## Validation commands

Run only the checks required by the touched owners, format Rust changes once,
and finish with `git diff --check`:

```sh
cargo fmt --all
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
git diff --check
```

For consumer migrations, add the directly affected Jaco or Feiwen tests and
clippy scope. Run a workspace aggregate check for breaking cross-crate changes.
Run Computer Use only when actual UI verification is authorized.
