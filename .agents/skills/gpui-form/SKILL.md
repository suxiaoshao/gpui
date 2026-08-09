---
name: gpui-form
description: Use when implementing, reviewing, debugging, documenting, or integrating crates/gpui-form, gpui-form-macros, gpui-form-gpui-component, Jaco forms, or Feiwen forms. Covers the current breaking FormSchema/Form API, explicit Form ownership, typed total and dynamic paths, runtime-owned occurrence identity, precise change impact, source-aware control bindings, snapshot validation, private gpui-operation transitions, prepare/rebase, and consumer migration.
---

# GPUI Form

Use this repo-owned skill for the three Form crates and their application consumers.

## Choose the source of truth

For Issue #199 breaking work, read these contracts in order:

1. `crates/gpui-form/docs/dev/issue-199/design-draft.md`
2. `crates/gpui-form/README.md`
3. `crates/gpui-form/docs/guide.md`
4. `crates/gpui-form-macros/docs/guide.md`
5. `crates/gpui-form-gpui-component/docs/guide.md`

The confirmed design draft records the architecture contract. The current Rust source and public
documents are the implemented authority for that contract; do not reintroduce removed compatibility
surfaces such as `ControlLease`, address-hash `PathKey`, naked-revision CAS, old resolvers, or
adapter-owned event routing.

Read the affected application plan before migrating a consumer:

- `app/jaco/docs/dev/issue-199/form-breaking-api-remigration-plan.md`
- `app/feiwen/docs/dev/issue-199/form-breaking-api-remigration-plan.md`

Keep every public README and guide change mirrored in English and Chinese. Newly created
development documents are Chinese only. Preserve historical documents unless the user explicitly
changes them.

## Ownership

Classify every mutable fact before editing:

1. One `Entity<Form<M>>` owns current model, baseline, model revision, validation facts, topology,
   async validation work, control registrations/projection routing facts, and private transition
   runtimes for one editing session.
2. `FieldDef`, `ChildDef`, `ItemsDef`, `CaseDef`, `TotalPath`, and located paths contain only schema,
   typed access, and location data. They never retain a Form entity, value, subscription, or native
   control.
3. Native entities own focus, IME, selection, popup state, incomplete editor text, and native event
   subscriptions. A stateful adapter owns one non-clone `ControlBinding`.
4. Options, delegates, catalogs, capabilities, resource phases, and nonblocking hints remain in the
   application or `gpui-store`.
5. Save, fetch, query, retry, database, and provider operations remain application-owned. Form only
   validates and prepares an accepted typed snapshot.

Do not create child Form entities, a second page-owned business draft, a Form copy in Store,
Form-owned persistence, or Form-to-database access.

## Public schema and typed paths

- Derive `FormSchema` on supported monomorphic structs and enums.
- Use `#[form(child)]` for a nested schema or `Option<Child>`, `#[form(items)]` for
  `Vec<Item>`, and leaf attributes for required/validation metadata.
- Generate one reusable static descriptor per field. It never stores an entity and every operation
  receives the explicit strong Form entity.
- Make `Form::new` and `with_validator` the infallible main construction path. Identity exhaustion
  is an internal invariant failure, not a recoverable build error.
- Compose root-first with `.then(...)`; static composition is pure and needs no Form.
- Treat `RootDef<M>` as the total path for the whole draft. Use `M::ROOT.get(&form, cx)` when a
  caller intentionally needs a typed whole-model snapshot.
- A total path exposes infallible `get`, `set`, validation/error queries, and binding. `set` returns
  whether the model changed.
- A dynamic path exposes `try_get`, `try_set`, fallible validation/error queries, and fallible
  binding because its item/case/optional occurrence may retire.
- Resolve enum and optional activation explicitly. An inactive case or `None` returns `Ok(None)`;
  a wrong-session or retired starting path returns `Err(ResolveError)`.
- Once a path crosses an item, case, or optional activation boundary, it and every descendant remain
  dynamic. Rust determines the final value type and rejects a wrong `set` type at compile time.

Do not unify total and dynamic operations behind an always-fallible API, retain weak Form ownership
inside a descriptor, or navigate with string paths.

## Runtime-owned identity and topology

- Obtain `ItemPath` only from `items`/`try_items` or collection mutations.
- Use `append`, `insert_before`, `move_before`, `remove`, `replace_all`, and
  `ItemPath::move_to`; never navigate by index, business ID, serde token, or an application registry.
- Allocate a monotonically increasing, never-reused occurrence for every item, active enum case, and
  active `Some`.
- Preserve an item occurrence on same-parent reorder. Retire it on removal/reinsertion, case or Some
  reconstruction, whole-model lifecycle replacement, and cross-parent move. A cross-parent move
  returns a fresh destination `ItemPath`.
- Keep total identity stable for the whole Form session. Whole-model replace/reset/rebase retires all
  old dynamic occurrences but keeps total paths and total bindings active.
- Back public opaque `PathKey` with a session-local unique path ID plus a private real canonical
  address. Use the unique ID for `Eq`/`Hash`/`ElementId` and the address for internal path relations.
  Expose neither representation, serialization, nor stable display.
- Build identity during initial topology construction or staged topology edits. Reads, snapshots,
  validation, and `PathKey` queries must not allocate through `ensure_*` behavior.
- Complete all fallible resolution, anchor checks, and allocation before mutating model or topology.
  Failure changes nothing.

Never expose topology indexes, canonical addresses, sessions, occurrences, generations, or snapshots.

## Mutation and events

Keep value, structure, and retirement impact separate internally:

- A leaf/composite replacement affects that path, its ancestors, and still-active descendants.
- A collection append/insert/reorder changes the collection aggregate and structure, not existing
  item field values.
- Removal and replacement additionally retire the removed dynamic subtrees.
- Whole-model replace/reset/rebase affects all values and structure and retires all old dynamic
  paths.
- Validation impact is separate from value projection impact.

Publish business facts through typed `FormEvent<M>` and `ModelChange<M>`. Provide a sealed
`ChangeTarget<M>` implemented for definitions, typed paths, item paths, and `PathKey`; let callers
query a `PathImpact` with `value_changed`, `structure_changed`, `retired`, and `is_affected`.

Do not expose control origin, internal change routes, or a misleading mutually exclusive
`TopologyChanged` event. One successful logical model mutation advances revision once, emits at most
one model event, and notifies at most once. An equal ordinary model write is a model no-op.

## Stateful control binding

Use `FormInput`, `FormIntegerInput`, `FormSelect`, and `FormCombobox` for gpui-component controls.
`new` accepts a total path; `try_new` accepts a resolved dynamic path.

For a custom stateful adapter:

1. Read the initial typed value and create the native entity.
2. Call `bind_control_in` or `try_bind_control_in` with a silent projector.
3. Store the returned non-clone, non-generic `ControlBinding` as the sole lifecycle owner.
4. Capture the cloneable typed `ControlWriter<Root, T>` in native event subscriptions.
5. Handle exhaustive `ControlProjection::Value(T)` and `ControlProjection::Retired` in the projector.
6. Use `defer_set`, `defer_blur`, `defer_set_issue`, and `defer_clear_issue` through the writer.

The core binding owns Form subscription, impact filtering, source suppression, and projection
lifecycle. An adapter never manually subscribes to `FormEvent`, clones a binding, keeps a
`ControlLease`, parses path impact, or implements a direction boolean.

Required behavior:

- A control write updates Form but is not immediately projected back to its source control.
- Another binding to the same path receives the latest authoritative value.
- Unrelated, validation-only, and structure-only changes do not call native value setters.
- Programmatic value changes project to every affected control.
- Multiple external changes coalesce to the latest Form value; an older queued projection cannot
  overwrite a newer native edit.
- Dynamic retirement supersedes queued values and projects `Retired` once. Owner/Form disappearance
  drops silently.
- Whole-model lifecycle changes keep total bindings active and retire dynamic bindings.
- A valid equal writer set may clear that control's issue and publish validation-only change without
  a model event.

Keep invalid or incomplete integer text native. Publish a scoped control issue; never route typed
numeric values through application-side `f64` or ad-hoc parsing.

## Validation and submission

- Give `Validator<M>` one snapshot-bound `ValidationRequest`; expose `request.model()` and use that
  same request for item, case, optional, and value resolution. Do not separately pass a potentially
  mismatched model.
- Use `ValidationTrigger::External` for catalog/dependency changes; do not use the old `Dynamic`
  name.
- Run submit validation by default. Run mount/change/blur only when schema metadata explicitly
  selects them, and run external validation only when the owner requests it.
- Preserve independent source/path/trigger buckets and invalidate completed facts only at
  intersecting scopes. Because pending async work is bound to the global `FormVersion`, cancel all
  pending work before any model revision is published.
  Attach issues to the precise typed field. Form owns facts; the page owns visibility and focus.
- Keep async validation bound to snapshot version, path occurrence, and validation generation.
  Any model revision cancels pending work because it advances the global version; completed issues
  are invalidated only at intersecting scopes. Pending work blocks `prepare`.
- Keep control identity and async generations private. Validation-only changes never project values.
- Let `prepare` return `Prepared<M>` carrying an opaque session-bound `FormVersion` and the accepted
  snapshot. `Prepared::map` preserves the version.
- Apply a canonical saved model with `rebase_if_current(version, model)`. A wrong session or stale
  version changes nothing. Do not use naked `FormRevision` as an async-save CAS token.

Persistence, retry, loading, and user notifications remain application work.

## Private transitions and transaction order

Use `gpui_operation::Transition` only for private Form revision/lifecycle/effect reduction, binding
`Active`/`Retired`/`Dropped` plus projection mailbox, and validation runtimes that genuinely need a
state machine. Do not use the predefined refresh/repair families and do not expose Form messages.

For each typed mutation:

1. Resolve against an immutable model/topology snapshot.
2. Stage the model edit, topology edit, identities, change impact, and validation impact.
3. Commit model and topology only after no recoverable failure remains.
4. Update validation/control issues and configured validation work.
5. Transition the private runtime once.
6. Route each binding to suppress, project, retire, or ignore.
7. Publish one logical event/notification and drain native projection after releasing the Form borrow.

Use lifecycle/occurrence as deferred-write freshness barriers. Use revision only for projection
ordering/coalescing; rejecting writes merely because revision advanced would reject legitimate
consecutive user input.

## Application integration

- Jaco uses local `Entity<Form<M>>` sessions. Provider, Prompt, Shortcut, ChatInput, RunSettings, and
  MCP settings consume the public API without importing private transitions. MCP runtime remains out
  of scope.
- Feiwen Query uses a recursive typed tree and `PathKey`; no business/UI counter is Form identity.
  Reconcile rows on structural impact so unaffected native controls retain identity.
- Feiwen Fetch remains a flat Form. Query/Fetch operation state belongs to application transitions.
- Catalog changes update their owner Store/native options and explicitly request external validation
  when product rules require it. They do not silently rewrite Form values.
- Keep the exact prepared query draft snapshot for display/recovery while the runner owns compiled
  execution data. Do not reconstruct dormant form variants from an executable spec.
- Never update an entity from inside its active update scope. Defer native-to-Form writes and recheck
  weak owner plus dynamic lifetime in queued work.

## Removed surfaces

Do not add compatibility wrappers or active-source uses of:

- `FormModel`, generated `*Form`/`FormState`, `FormField`, `PartialFormField`, `FormItemId`,
  `#[form(array(...))]`, child-first `within`, or user-provided Form identity;
- `Form::try_new`, `try_new_with_validator`, raw `Form::value`/`Form::baseline`, total-path `value`,
  dynamic `try_value`, old fallible case/optional resolver semantics, or naked-revision
  `rebase_if_revision`;
- address-hash-only `PathKey`, public topology tokens, public Form messages, or public control origin;
- public `ControlLease`, cloneable generic `ControlBinding`, adapter-owned Form event routing, or
  local direction guards;
- writable projection APIs, core submit transforms, Form-owned focus/touched/error visibility,
  persistence callbacks, resource/catalog state, database access, or application operation tasks.

Intentional compile-fail fixtures and historical Issue #175/#199 documents may mention removed APIs;
classify them rather than rewriting history.

## Documentation and validation

Keep public documents task-oriented: show how to construct a Form, bind/render controls, validate,
submit, use nested paths, and implement a custom adapter. Keep internal identity, change routing,
mailbox, and Transition mechanics in development documents.

For documentation/skill-only changes, check English/Chinese semantic parity, links, removed terms,
skill structure, and `git diff --check`; crate tests are not required. For implementation, run the
direct crate tests, clippy, and affected Jaco/Feiwen consumer checks. Run workspace aggregate checks
for the breaking cross-crate migration. Run Computer Use only when actual UI verification is
authorized.
