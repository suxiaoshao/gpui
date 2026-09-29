# Form Internal Invariants

Public construction, paths and bindings are documented in the [guide](../../../../crates/gpui-form/docs/guide.md). These are the non-obvious constraints for changing their implementation.

## Identity and mutation

Descriptors contain schema/location data without retaining Form or native entities. Total paths remain valid for the session; item/case/optional paths carry runtime occurrence identity and can retire.

Occurrence IDs are never reused. Same-parent reorder preserves identity; reconstruction, removal/reinsertion, cross-parent move and whole-model replacement retire affected dynamic occurrences. Whole-model lifecycle changes preserve total paths and bindings. Read operations do not allocate topology identity.

Resolve and stage all recoverable work before mutating model/topology. A failed mutation changes nothing. A successful logical mutation advances revision once and emits at most one model event and notification. Equal model writes are no-ops; validation changes may still publish independently.

## Bindings

A custom stateful adapter retains one non-clone `ControlBinding` and captures its typed writer in native event subscriptions. The binding owns Form observation, impact filtering, source suppression and retirement. Defer native-to-Form writes to avoid re-entering an active native entity update.

User input does not immediately project back to its source control. Programmatic changes reach affected controls; unrelated and validation-only changes do not call value setters. Coalesce queued projections to the latest value; retirement overrides queued values. Preserve total bindings across reset/rebase and retire dynamic bindings. Keep incomplete numeric text native with a scoped control issue.

## Validation and saves

Validation uses one snapshot-bound request for model and path resolution. Pending work is version-bound and cancelled before a new model revision is published; completed facts invalidate only at intersecting scopes. Pending validation blocks preparation.

A prepared snapshot carries a session-bound version. Rebase a saved model only when that version is still current. Lifecycle/occurrence checks protect deferred writes; a revision advance alone must not reject consecutive valid user input.

Private transition machinery belongs inside Form. Commit model/topology, update validation, route bindings, then publish and drain native projection after releasing the Form borrow. Applications consume the public API and own persistence and notifications.
