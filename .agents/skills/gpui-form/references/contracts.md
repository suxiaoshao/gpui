# Form Internal Invariants

Public construction, paths and bindings are documented in the [guide](../../../../crates/gpui-form/docs/guide.md). These are the non-obvious constraints for changing their implementation.

## Identity and mutation

Descriptors contain schema/location data without retaining Form or native entities. Total paths remain valid for the session; item/case/optional paths carry runtime occurrence identity and can retire.

Occurrence IDs are never reused. Same-parent reorder preserves identity; reconstruction, removal/reinsertion, cross-parent move and whole-model replacement retire affected dynamic occurrences. Whole-model lifecycle changes preserve total paths and bindings. Read operations do not allocate topology identity.

Resolve and stage all recoverable work before mutating model/topology. A failed mutation changes nothing. A successful logical mutation advances revision once and emits at most one model event and notification. Equal model writes are no-ops; validation changes may still publish independently. Total set now returns Result so retiring unresolved dynamic drafts is a recoverable refusal. Use explicit discard_drafts before deliberate structural deletion.

## Bindings

A custom stateful adapter retains one non-clone `ControlBinding` and captures its typed writer in native event subscriptions. The binding owns Form observation, impact filtering, source suppression and retirement. Defer native-to-Form writes to avoid re-entering an active native entity update.

User input does not immediately project back to its source control. Programmatic changes reach affected controls; unrelated and validation-only changes do not call value setters. Coalesce queued projections to the latest value; retirement overrides queued values. Preserve total bindings across reset/rebase and retire dynamic bindings. Form owns unresolved typed display drafts under stable EditorKey plus path identity; unmount never deletes accepted drafts. Successful parsing clears only the source editor. Integer constructors require explicit keys; different editors need different keys. Mapping/display type mismatch must be rejected before restoration.

## Validation and saves

Validation uses one snapshot-bound request for model and path resolution. Pending work is version-bound and cancelled before a new model revision is published; completed facts invalidate only at intersecting scopes. Pending validation blocks preparation.

Rules have stable owner IDs, explicit input dependencies and optional occurrence identity; result replacement is per owner across triggers, including empty results. Started rules recheck on dependency changes; drafts block dependent rules. Required and Garde have deterministic owners; synchronous replacement cannot clear async/control owners.

Native callbacks synchronously reserve shared edit tickets before defer. Pending tickets block prepare and their epoch invalidates old save versions even before model/draft admission; binding/window destruction settles tickets without deleting accepted drafts.

A prepared snapshot carries a session-bound version. Rebase a saved model only when that version is still current. Lifecycle/occurrence checks protect deferred writes; a revision advance alone must not reject consecutive valid user input.

Private transition machinery belongs inside Form. Commit model/topology, update validation, route bindings, then publish and drain native projection after releasing the Form borrow. Applications consume the public API and own persistence and notifications.

## Draft lifecycle, baseline and mapping

replace/reset/rebase require DraftPolicy and return Result. Preserve rejects pending admission or retirement of dynamic drafts atomically; Discard explicitly abandons covered drafts/tickets. Scalar writes retain unresolved drafts. rebase_preserving_edits stages one B/L/R merge plus remote baseline, preserving local conflicts and draft text. Direct child leaves merge recursively; opaque fields, Vec, Option and enum/case structures are conservative whole units by default. A declared field merge hook returns FieldMerge candidates within the same transaction and always retains the original remote baseline. Any intersecting unresolved draft skips that hook and preserves the entire local field and its identities; remote != baseline reports a field conflict. Without drafts, only a changed candidate retires that field's dynamic descendants. Hooks follow existing traversal boundaries and do not reconcile occurrence identities. Never infer external occurrence correspondence by index. Path baseline reads never allocate; unmatched current occurrences return None.

Model, baseline, topology and draft content changes advance one editing revision. Combined model/draft changes share one revision and notify; error-message-only changes do not advance it. ModelChanged requires an actual model change; DraftChanged and SessionChanged cover other session changes.

ControlMapping parses with storage, baseline and the application context snapshot captured before enqueue. Refresh only reprojects clean editors, never reparses drafts or writes domain values. Applications own inheritance and persistence. Native controls retain IME/cursor/selection/focus ownership.
