# API, Type, and Lifecycle Contracts

Use this reference for every new or materially changed module, type, trait implementation, service, component, repository API, or state owner.

## Module Contract

For each file, state:

- path and whether it is added, modified, moved, or deleted;
- responsibility and non-responsibilities;
- public exports and callers;
- dependencies on sibling modules or external APIs;
- why the type belongs in this crate/layer.

## Type Contract

Provide Rust-shaped declarations detailed enough to fix the design:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExampleState {
    // fields with semantic comments
}
```

Determine as applicable:

- fields and enum payloads;
- visibility;
- derives;
- generic parameters and trait bounds;
- associated types;
- serde representation and unknown-field policy;
- identity/equality/hash semantics;
- invariants and invalid states prevented by the type.

Do not invent exact upstream type paths. Verify them first or mark the work release-gated.

## Trait and Method Contract

List every required trait implementation and method signature, not only fields:

```rust
impl SomeTrait for Example {
    type Output = ConfirmedType;
    async fn execute(&self, request: Request) -> Result<Self::Output, Error>;
}

impl Example {
    pub(crate) fn new(... ) -> Self;
    pub(crate) async fn load(... ) -> Result<...>;
    fn derive_policy(&self, ... ) -> Policy;
    pub(crate) async fn shutdown(&self) -> Result<()>;
}
```

For each method, state:

- caller and call frequency;
- inputs, ownership, and borrowing;
- output and side effects;
- async/thread/context requirements;
- errors and retry classification;
- cancellation and partial-progress behavior;
- persisted and emitted state changes;
- idempotency and safe retry boundary.

## Ownership and Concurrency

Identify the single owner and lifetime of every mutable resource. Specify:

- local value, GPUI entity, shared service, global, DB, file, or provider ownership;
- synchronization primitive and why it is required;
- `Send`/`Sync`/`Clone` requirements;
- one-in-flight, ordering, and reentrancy invariants;
- task retention and cancellation;
- explicit async shutdown; never rely on async work in `Drop`;
- state reset on model/account/project/conversation changes.

## Error and Recovery State Machine

Use typed errors. Define variants and recovery actions. Cover:

- validation/request/provider/transport/storage errors;
- retryable versus terminal conditions;
- retry budget and backoff owner;
- whether output has already been surfaced;
- duplicate text/tool-call prevention;
- cancellation during connect, request, stream, tool execution, persistence, and shutdown;
- external success followed by local persistence failure.

Avoid string matching when an upstream structured status, code, or payload exists.
