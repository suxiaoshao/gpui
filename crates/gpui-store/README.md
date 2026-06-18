# gpui-store

`gpui-store` is a small GPUI-native state layer. Its API is intentionally closer
to Zustand and `useSyncExternalStore` than to Redux: users update state directly,
while the library owns revision tracking, selector subscriptions, equality
checks, and external source synchronization.

Internal design notes live in
[`docs/development-plan.md`](docs/development-plan.md).

## Core Ideas

- No mandatory action enum.
- No mandatory reducer.
- No mandatory change-set type.
- State is updated with `set`, `update`, or a writable `StoreBinding`.
- `cx.notify()` only happens when the value really changed.
- `StoreSelection<T>` is read-only derived state.
- `StoreBinding<T>` is a writable selection/lens that writes back to the store.
- External data sources are user-defined through a source trait; common
  load/write/subscribe glue should be handled by the library.
- Source type is orthogonal to ownership: both `LocalStore` and `SharedStore`
  can be memory-only, file-backed, S3-backed, database-projected, or anything
  else the user implements.

## Main Types

| Type | Role |
| --- | --- |
| `SharedStore<S, Source = MemorySource>` | Shared observable store backed by a GPUI `Entity`. |
| `LocalStore<S, Source = MemorySource>` | Component-owned store with no separate entity. |
| `StoreSelection<T>` | Read-only subscribed snapshot. |
| `StoreBinding<T>` | Writable subscribed snapshot. |
| `StoreSource<S>` | User-implemented external synchronization trait. |
| `StoreSourceBuilder` | Convenience closure builder that implements `StoreSource<S>`. |
| `MemorySource` | Default source for memory-only stores. |
| `StoreUpdate` | `{ changed, revision, origin }` result for a mutation. |

## Pointer-Like Snapshots

`StoreSelection<T>` and `StoreBinding<T>` should feel like smart read pointers to their
current snapshot.

They should implement:

```rust
impl<T> Deref for StoreSelection<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}

impl<T> Deref for StoreBinding<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.get()
    }
}
```

This keeps rendering code concise:

```rust
self.model.as_ref()
self.rows.iter()
self.query.is_empty()
*self.can_submit
```

Use `*self.can_submit` for boolean snapshots. Rust method-call deref works for
methods like `iter`, `as_ref`, and `is_empty`, but `if self.can_submit` does not
coerce through `Deref`.

`StoreBinding<T>` should not implement `DerefMut`. Mutating through `&mut T` would
bypass the backing store and break equality checks, revision updates,
notifications, and external write-back.

Writes must stay explicit:

```rust
self.model.set(cx, Some(model));
self.query.update(cx, |query| {
    query.push_str(" next");
});
```

Recommended convenience traits:

| Trait | `StoreSelection<T>` | `StoreBinding<T>` | Reason |
| --- | --- | --- | --- |
| `Deref<Target = T>` | yes | yes | Read ergonomics and method-call deref. |
| `AsRef<T>` / `Borrow<T>` | yes | yes | Interop with APIs that take borrowed values. |
| `Debug` / `Display` | when `T` supports it | when `T` supports it | Forward to the snapshot. |
| `PartialEq` / `Eq` | when `T` supports it | when `T` supports it | Compare snapshots. |
| `DerefMut` / `BorrowMut` | no | no | Would bypass store updates. |
| `Copy` | no | no | Handles own subscriptions and cached snapshots. |
| `Clone` | avoid by default | avoid by default | Cloning owner-bound subscriptions is ambiguous. |

To clone the value, clone the snapshot explicitly:

```rust
let model = self.model.get().clone();
```

If cloneable handles are needed later, they should be a separate type, such as
`StoreSelectionHandle<T>` or `StoreBindingHandle<T>`, not the owner-bound
component field.

`StoreState` should be a marker trait:

```rust
pub trait StoreState: 'static {}
```

For most state, derive `Clone + PartialEq` and let the library decide whether an
update changed anything.

```rust
#[derive(Clone, PartialEq, Default)]
struct ChatPrefs {
    model: Option<ModelKey>,
    approval: ApprovalMode,
    reasoning: ReasoningSelection,
}

impl StoreState for ChatPrefs {}
```

## Change Detection

The default API should support four levels. Most users only need the first two.

| Level | API | Cost | Use when |
| --- | --- | --- | --- |
| Field set | `store.set(cx, field, value)` / `binding.set(cx, value)` | Compare one field | Common settings and UI state |
| Whole-state update | `store.update(cx, |s| ...)` | Clone + compare `S` | Small/medium state |
| Manual changed flag | `store.update_if(cx, |s| -> bool { ... })` | User decides | Large state or custom equality |
| Optional delta | `store.update_delta(cx, ...)` | User returns delta | Partial persistence/audit paths |

`Delta` is an advanced optimization. It should not be part of the default
learning path.

## Example: Component-Local Store

Use `LocalStore<S>` when the state is private to one component and does not need
to be observed by other components. The default source is `MemorySource`, so this is
just component memory state.

```rust
use gpui::{Context, Render};
use gpui_store::{LocalStore, StoreState};

#[derive(Clone, PartialEq, Default)]
struct ComposerState {
    text: String,
    is_expanded: bool,
}

impl StoreState for ComposerState {}

struct Composer {
    state: LocalStore<ComposerState>,
}

impl Composer {
    fn new() -> Self {
        Self {
            state: LocalStore::new(ComposerState::default()),
        }
    }

    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.state.set(cx, |s| &mut s.text, text);
    }

    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |s| {
            s.is_expanded = !s.is_expanded;
        });
    }
}
```

`LocalStore` calls the owner component's `cx.notify()` only when the selected
field or whole state actually changed. If a source is attached later, its
subscription and background tasks live with the owner component, not with a
separate store entity.

## Example: Shared Global Store

Use `SharedStore<S>` when multiple components need to read or write the same
state. A `SharedStore` owns its own GPUI entity, so its subscriptions and source
tasks can outlive any single component that reads it.

```rust
use gpui::{App, Context};
use gpui_store::{SharedStore, StoreBinding, StoreSelection, StoreState};

#[derive(Clone, PartialEq, Default)]
struct ChatPrefs {
    model: Option<ModelKey>,
    approval: ApprovalMode,
    reasoning: ReasoningSelection,
}

impl StoreState for ChatPrefs {}

fn init(cx: &mut App) {
    SharedStore::install_global(cx, ChatPrefs::default());
}

struct ChatForm {
    model: StoreBinding<Option<ModelKey>>,
    approval: StoreBinding<ApprovalMode>,
    can_submit: StoreSelection<bool>,
}

impl ChatForm {
    fn new(cx: &mut Context<Self>) -> Self {
        let prefs = SharedStore::<ChatPrefs>::global(cx);

        Self {
            model: prefs.bind(
                cx,
                |s| s.model.clone(),
                |s, model| s.model = model,
            ),
            approval: prefs.bind(
                cx,
                |s| s.approval,
                |s, approval| s.approval = approval,
            ),
            can_submit: prefs.select(cx, |s| {
                s.model.is_some() && s.approval.allows_submit()
            }),
        }
    }

    fn select_model(&mut self, model: ModelKey, cx: &mut Context<Self>) {
        self.model.set(cx, Some(model));
    }

    fn set_approval(&mut self, approval: ApprovalMode, cx: &mut Context<Self>) {
        self.approval.set(cx, approval);
    }

    fn render_model_label(&self) -> SharedString {
        self.model
            .as_ref()
            .map(|model| model.display_label())
            .unwrap_or_else(|| "No model".into())
    }
}
```

`StoreBinding::set` writes to the backing store. It does not become a second source
of truth. The update flow is:

```text
binding.set(value)
  -> store update
  -> store compares old/new
  -> changed: revision += 1 and store notifies
  -> binding refreshes from the store
  -> owner component notifies only if the binding snapshot changed
```

## Example: Read-Only StoreSelection

Use `StoreSelection<T>` for derived data that has no meaningful inverse write.

```rust
struct PromptList {
    rows: StoreSelection<Arc<[PromptRow]>>,
    has_results: StoreSelection<bool>,
}

impl PromptList {
    fn new(cx: &mut Context<Self>, store: SharedStore<PromptState>) -> Self {
        Self {
            rows: store.select(cx, |s| {
                s.prompts
                    .iter()
                    .filter(|prompt| prompt.matches(&s.query))
                    .map(PromptRow::from)
                    .collect()
            }),
            has_results: store.select(cx, |s| {
                s.prompts.iter().any(|prompt| prompt.matches(&s.query))
            }),
        }
    }
}
```

`StoreSelection<T>` should not have `set`. A filtered list, count, boolean predicate,
or formatted label cannot be generically written back to the store.

Because `StoreSelection<T>` derefs to `T`, rendering can read it directly:

```rust
for row in self.rows.iter() {
    render_prompt_row(row);
}

if *self.has_results {
    render_results();
}
```

## Example: Store Owned By a Component

Some shared stores are not global. A parent component can own a `SharedStore`
and pass it to children.

```rust
struct SearchPane {
    store: SharedStore<SearchState>,
    query: StoreBinding<String>,
}

impl SearchPane {
    fn new(cx: &mut Context<Self>) -> Self {
        let store = SharedStore::new(cx, SearchState::default());
        let query = store.bind(cx, |s| s.query.clone(), |s, query| {
            s.query = query;
        });

        Self { store, query }
    }

    fn child_store(&self) -> SharedStore<SearchState> {
        self.store.clone()
    }
}
```

This keeps the lifetime explicit and avoids installing global state just because
two components need to cooperate.

## Source Is Orthogonal

`LocalStore` versus `SharedStore` answers "who owns the state and who gets
notified". `StoreSource` answers "where can the state be synchronized from or
to". These axes should not be coupled.

| Ownership | Source | Example |
| --- | --- | --- |
| `LocalStore<Prefs, MemorySource>` | none | Component-only draft state. |
| `LocalStore<Prefs, S3Source>` | remote object | One component owns preferences loaded from S3. |
| `SharedStore<Prefs, MemorySource>` | none | Shared in-memory UI state. |
| `SharedStore<Prefs, FileSource>` | file | App-wide preferences persisted to TOML. |
| `SharedStore<Prompts, DatabaseProjection>` | database projection | Shared UI snapshot over committed DB rows. |

The library should provide `MemorySource` and closure-based source builders, but
not hard-code file/database/S3 as special store kinds.

## Store Source Trait

The source trait is the real extension point. Users decide how to load,
subscribe, reconcile, and write. The library owns GPUI wiring, revision checks,
write-back scheduling, weak entity upgrade, and owner notification.

The exact future/task alias is an implementation detail, but the target shape
is:

```rust
pub trait StoreSource<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    fn source_id(&self) -> StoreSourceId;
    fn policy(&self) -> StoreSourcePolicy;

    fn load(&self) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error>;

    fn subscribe(
        &self,
        on_change: StoreSourceCallback<Self::Event>,
    ) -> StoreSourceFuture<Option<Self::Subscription>, Self::Error>;

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error>;

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;

    fn write_snapshot(
        &self,
        state: &S,
    ) -> StoreSourceFuture<Option<StoreSourceWriteAck<Self::Snapshot>>, Self::Error>;
}
```

`StoreSourcePolicy` describes synchronization semantics, not backend type:

| Policy | Meaning |
| --- | --- |
| `MemoryOnly` | No durable/external source. |
| `StoreBacked` | Source hydrates initial state; local store changes write back. |
| `ExternalBacked` | External source may change independently and can push events. |
| `Projection` | Store is a UI projection over committed external state. |

`source_id` should encode backend identity when useful: `file:prefs`,
`s3:prefs`, `http:settings`, `database:prompts`, `memory:runtime`.

## Example: File-Backed Preferences

Common file-backed state should not require users to implement a trait by hand.
Use the closure builder, which itself implements `StoreSource<S>`.

```rust
use gpui::App;
use gpui_store::{StoreSourceBuilder, SharedStore};

fn install_prefs(path: PathBuf, cx: &mut App) -> anyhow::Result<SharedStore<ChatPrefs>> {
    let source = StoreSourceBuilder::store_backed("file:chat-prefs")
        .load({
            let path = path.clone();
            move || load_toml::<ChatPrefs>(&path).map(Some)
        })
        .reconcile(|state, snapshot| {
            if *state == snapshot {
                return false;
            }

            *state = snapshot;
            true
        })
        .write_snapshot(move |state| {
            save_toml(&path, state)?;
            Ok(None)
        });

    SharedStore::install_global_from_source(cx, ChatPrefs::default(), source)
}
```

Default behavior:

- Initial load calls `load`.
- External events are optional; no `subscribe` means no live watching.
- `reconcile` mutates the store state and returns whether it changed.
- The store only bumps revision and notifies when `reconcile` or local updates
  report a real change.
- `write_snapshot` runs only after local changed updates.

## Example: S3-Backed Local Store

S3 should not require a new built-in source kind. The app implements the trait
and uses it with either `LocalStore` or `SharedStore`.

```rust
use gpui::Context;
use gpui_store::{
    LocalStore, StoreSource, StoreSourceFuture, StoreSourceId, StoreSourcePolicy,
    StoreSourceWriteAck,
};

struct S3PrefsSource {
    bucket: String,
    key: String,
    client: S3Client,
}

impl StoreSource<ChatPrefs> for S3PrefsSource {
    type Snapshot = ChatPrefs;
    type Event = S3ObjectVersion;
    type Subscription = S3Watcher;
    type Error = anyhow::Error;

    fn source_id(&self) -> StoreSourceId {
        StoreSourceId::new(format!("s3:{}/{}", self.bucket, self.key))
    }

    fn policy(&self) -> StoreSourcePolicy {
        StoreSourcePolicy::ExternalBacked
    }

    fn load(&self) -> StoreSourceFuture<Option<Self::Snapshot>, Self::Error> {
        self.client
            .load_toml(self.bucket.clone(), self.key.clone())
            .map(Some)
    }

    fn reconcile(&self, state: &mut ChatPrefs, snapshot: ChatPrefs) -> bool {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    }

    fn write_snapshot(
        &self,
        state: &ChatPrefs,
    ) -> StoreSourceFuture<Option<StoreSourceWriteAck<ChatPrefs>>, Self::Error> {
        self.client
            .save_toml(self.bucket.clone(), self.key.clone(), state.clone())?;
        Ok(None)
    }
}

struct PrefsPane {
    prefs: LocalStore<ChatPrefs, S3PrefsSource>,
}

impl PrefsPane {
    fn new(source: S3PrefsSource, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        Ok(Self {
            prefs: LocalStore::with_source(cx, ChatPrefs::default(), source)?,
        })
    }
}
```

The same `S3PrefsSource` can be passed to `SharedStore::install_global_from_source`
when multiple components need the same S3-backed state.

For component-local live source events, the library needs an accessor that tells
it where the `LocalStore` field lives on the owner component:

```rust
struct PrefsPane {
    prefs: LocalStore<ChatPrefs, S3PrefsSource>,
}

impl PrefsPane {
    fn new(source: S3PrefsSource, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let mut prefs = LocalStore::with_source(cx, ChatPrefs::default(), source)?;
        prefs.subscribe(cx, |pane: &mut PrefsPane| &mut pane.prefs)?;
        Ok(Self { prefs })
    }
}
```

## Example: File Watching

If live reload is needed, add a subscription source. The watcher details remain
user controlled.

```rust
let source = StoreSourceBuilder::external_backed("file:settings")
    .load(move || load_toml::<Settings>(&path).map(Some))
    .subscribe(move |on_change| {
        watch_file(path.clone(), move || on_change(()))
    })
    .reconcile(|state, snapshot| {
        if *state == snapshot {
            return false;
        }

        *state = snapshot;
        true
    });
```

The library should own callback wiring, weak entity upgrade, snapshot equality
checks, and owner notification. The user owns the file watcher implementation.

## Example: Database Projection

Database state should usually be synchronized from committed snapshots instead
of treating every UI command as a generic store write.

```rust
#[derive(Clone, PartialEq, Default)]
struct PromptState {
    rows: Vec<PromptRecord>,
}

impl StoreState for PromptState {}

fn install_prompts(repository: PromptRepository, cx: &mut App) -> SharedStore<PromptState> {
    let source = StoreSourceBuilder::projection("database:prompts")
        .load({
            let repository = repository.clone();
            move || repository.list_prompts().map(Some)
        })
        .reconcile(|state, rows| {
            if state.rows == rows {
                return false;
            }

            state.rows = rows;
            true
        });

    SharedStore::install_global_from_source(cx, PromptState::default(), source)
}

fn create_prompt(
    prompts: &SharedStore<PromptState>,
    repository: &PromptRepository,
    name: String,
    cx: &mut App,
) -> anyhow::Result<()> {
    let committed_rows = repository.create_prompt(name)?;
    prompts.sync_snapshot(cx, committed_rows)?;
    Ok(())
}
```

The database transaction result is the source of truth. The store is a stable
UI snapshot over that committed data.

## Example: Advanced Delta

Only use a delta when snapshot comparison is not enough, or persistence must know
exactly which part changed.

```rust
#[derive(Clone, Default)]
struct PrefsDelta {
    model_changed: bool,
    approval_changed: bool,
}

store.update_delta(cx, |state| {
    state.model = next_model;
    PrefsDelta {
        model_changed: true,
        ..PrefsDelta::default()
    }
});

source.write_delta(|state, delta| {
    if delta.model_changed {
        save_model_choice(&state.model)?;
    }
    Ok(())
});
```

This is the escape hatch, not the default shape.

## Rules

- Prefer `StoreBinding<T>` for writable fields.
- Prefer `StoreSelection<T>` for derived read-only data.
- Prefer `LocalStore<S>` for component-private state.
- Prefer `SharedStore<S>` for shared observable state.
- Prefer `MemorySource` for memory-only state.
- Implement `StoreSource<S>` for custom persistence or external sync, including
  S3/HTTP/keychain/database projections.
- Use `StoreSourceBuilder` only as a convenience closure adapter for common cases.
- Do not mirror an entire database into one store.
- Do not read external entities from render callbacks; render from stored
  snapshots.
- Avoid nested entity updates; let bindings and selections refresh through
  subscriptions.

## Non-Goals

`gpui-store` should not provide:

- Redux-style action/reducer/middleware registries.
- Time travel.
- Automatic render-time dependency collection.
- Database migrations.
- Application-specific persistence.
