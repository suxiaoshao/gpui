# gpui-store

`gpui-store` is a small GPUI-native state layer. Its API is intentionally closer
to Zustand and `useSyncExternalStore` than to Redux: users update state directly
when the store is memory-only, while backend-backed stores make commit and
projection boundaries explicit.

Internal design notes live in
[`docs/development-plan.md`](docs/development-plan.md).

## Core Ideas

- No mandatory action enum.
- No mandatory reducer.
- No mandatory change-set type.
- Memory-only state is updated with `set`, `update`, or a writable
  `StoreBinding`.
- Backend-committed state is updated with `try_set`, `try_update`, or a
  committed `StoreBinding` that returns the backend error.
- Read-only projections are refreshed with committed snapshots, not local
  generic setters.
- `cx.notify()` only happens when the value really changed.
- `StoreSelection<T>` is read-only derived state.
- `observe_select` / `observe_select_in` subscribe to selected values for side
  effects without observing unrelated store changes.
- `StoreBinding<T>` is a writable selection/lens that writes back to the store.
- External data is user-defined through backend traits; common
  load/commit/subscribe glue should be handled by the library.
- Backend type is orthogonal to ownership: both `LocalStore` and `SharedStore`
  can be memory-only, file-backed, S3-backed, database-projected, or anything
  else the user implements.
- Backend capability is type-level: a projection backend should not expose
  `set`, `update`, `bind`, `try_set`, `try_update`, or writable bindings.

## Main Types

| Type | Role |
| --- | --- |
| `SharedStore<S, Backend = MemoryBackend>` | Shared observable store backed by a GPUI `Entity`. |
| `LocalStore<S, Backend = MemoryBackend>` | Component-owned store with no separate entity. |
| `StoreSelection<T>` | Read-only subscribed snapshot. |
| `StoreBinding<T, Error = Infallible>` | Writable subscribed snapshot; exposes infallible or fallible write methods by error type. |
| `StoreBackend<S>` | User-implemented external load/subscribe/reconcile trait. |
| `StoreCommitBackend<S>` | Optional capability trait for backends that can commit local draft state. |
| `StoreBackendBuilder` | Convenience closure builder that implements `StoreBackend<S>`. |
| `MemoryBackend` | Default backend for memory-only stores. |
| `StoreUpdate` | `{ changed, revision, origin }` result for a mutation. |

## Owned Snapshot Reads

`StoreSelection<T>` and `StoreBinding<T>` hold subscribed snapshots. Store
notifications can replace the current snapshot, so the public API should not
hand out long-lived references into replaceable storage. Instead, reads are
explicit:

```rust
let model = self.model.snapshot(); // Rc<T>, safe to keep as an old snapshot

self.model.read(|model| {
    // Short borrow scoped to this closure.
});

let model = self.model.cloned(); // T: Clone
```

This keeps React-like stale snapshots safe: a component can hold an old `Rc<T>`
while later store notifications install a newer snapshot.

Rendering code should choose either a held snapshot or a scoped read:

```rust
self.model.read(|model| {
    model
        .as_ref()
        .map(|model| model.display_label())
        .unwrap_or_else(|| "No model".into())
})

let rows = self.rows.snapshot();
for row in rows.iter() {
    render_row(row);
}

if self.can_submit.read(|can_submit| *can_submit) {
    render_submit_button();
}
```

`StoreSelection<T>` and `StoreBinding<T>` should not implement `Deref`,
`AsRef<T>`, or `Borrow<T>`. Those traits all expose `&T` without making the
caller hold the `Rc<T>` snapshot that keeps the value alive. `StoreBinding<T>`
also should not implement `DerefMut`; mutating through `&mut T` would bypass the
backing store and break equality checks, revision updates, notifications, and
backend commit.

Memory-only writes stay explicit:

```rust
self.model.set(cx, Some(model));
self.query.update(cx, |query| {
    query.push_str(" next");
});
```

Committed backend writes use fallible methods:

```rust
self.model.try_set(cx, Some(model))?;
self.query.try_update(cx, |query| {
    query.push_str(" next");
})?;
```

Recommended read surface:

| API / Trait | `StoreSelection<T>` | `StoreBinding<T>` | Reason |
| --- | --- | --- | --- |
| `snapshot() -> Rc<T>` | yes | yes | Hold an immutable snapshot safely across later store updates. |
| `read(|&T| ...)` | yes | yes | Borrow current snapshot only for the closure duration. |
| `cloned() -> T` | `T: Clone` | `T: Clone` | Convenience for owned value clones. |
| `Debug` / `Display` | when `T` supports it | when `T` supports it | Forward to the current snapshot. |
| `PartialEq` / `Eq` | when `T` supports it | when `T` supports it | Compare current snapshots. |
| `Deref` / `AsRef<T>` / `Borrow<T>` | no | no | Would expose references without an owned snapshot guard. |
| `DerefMut` / `BorrowMut` | no | no | Would bypass store updates. |
| `Copy` | no | no | Handles own subscriptions and cached snapshots. |
| `Clone` | avoid by default | avoid by default | Cloning owner-bound subscriptions is ambiguous. |

To clone the value, use `cloned()`:

```rust
let model = self.model.cloned();
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
The infallible names are only available for `MemoryBackend`; backend-committed
stores use the `try_*` variants and only update the store after the backend
commit succeeds.

| Level | API | Cost | Use when |
| --- | --- | --- | --- |
| Field set | `store.set(...)` / `binding.set(...)` or `store.try_set(...)` / `binding.try_set(...)` | Compare one field | Common settings and UI state |
| Whole-state update | `store.update(...)` or `store.try_update(...)` | Clone + compare `S` | Small/medium state |
| Manual changed flag | `store.update_if(...)` or `store.try_update_if(...)` | User decides | Large state or custom equality |
| Optional delta | not implemented yet | User returns delta | Partial persistence/audit paths |

`StoreDelta` is currently only a marker trait. Delta writes are an advanced
optimization and are not part of the implemented default API.

## Example: Component-Local Store

Use `LocalStore<S>` when the state is private to one component and does not need
to be observed by other components. The default backend is `MemoryBackend`, so
this is just component memory state.

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
field or whole state actually changed. If a backend is attached later, its
subscription and background tasks live with the owner component, not with a
separate store entity.

## Example: Shared Global Store

Use `SharedStore<S>` when multiple components need to read or write the same
state. A `SharedStore` owns its own GPUI entity, so its subscriptions and backend
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
        self.model.read(|model| {
            model
                .as_ref()
                .map(|model| model.display_label())
                .unwrap_or_else(|| "No model".into())
        })
    }
}
```

For memory-only stores, `StoreBinding::set` writes to the backing store. It does
not become a second source of truth. The update flow is:

```text
binding.set(value)
  -> store update
  -> store compares old/new
  -> changed: revision += 1 and store notifies
  -> binding refreshes from the store
  -> owner component notifies only if the binding snapshot changed
```

For committed backends, the fallible write flow is different:

```text
binding.try_set(value)
  -> clone current store state into a draft
  -> setter mutates the draft
  -> unchanged: stop without backend I/O
  -> backend.commit_snapshot(draft)
  -> error: store state is unchanged and no owner is notified
  -> success: store installs the committed state or committed snapshot
  -> changed: revision += 1 and store notifies
  -> binding refreshes from the store
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

Rendering can hold an owned snapshot for the duration of the render path:

```rust
let rows = self.rows.snapshot();
for row in rows.iter() {
    render_prompt_row(row);
}

if self.has_results.read(|has_results| *has_results) {
    render_results();
}
```

## Example: Observe Selected Changes

Use `observe_select` when a component needs to run a side effect after a
selected value changes, but does not need a render-time snapshot field.

```rust
struct Shell {
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    fn new(config: SharedStore<AppConfig, ConfigBackend>, cx: &mut Context<Self>) -> Self {
        let theme_subscription = config.observe_select(
            cx,
            |state| state.theme.clone(),
            |this, theme, cx| {
                this.apply_theme(theme, cx);
            },
        );

        Self {
            _subscriptions: vec![theme_subscription],
        }
    }
}
```

Use `observe_select_in` when the side effect needs a `Window`, matching GPUI's
`observe_in` convention.

```rust
let language_subscription = config.observe_select_in(
    cx,
    window,
    |state| state.language.clone(),
    |this, language, window, cx| {
        this.rebuild_menu(language, window, cx);
    },
);
```

`observe_select` differs from `select`:

- `select` returns a `StoreSelection<T>` for render-time reads and notifies the
  owner when the selected snapshot changes.
- `observe_select` returns a `Subscription` for side effects and calls the
  callback only when the selected value changes.
- `observe_select` does not call the callback immediately on creation and does
  not automatically call `cx.notify()`; the callback decides whether to notify.
- Neither API fires for unrelated store changes whose selected output is equal.

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

## Backend Is Orthogonal

`LocalStore` versus `SharedStore` answers "who owns the state and who gets
notified". `StoreBackend` answers "where can the state be loaded from,
refreshed from, or reconciled with". `StoreCommitBackend` answers "can this
backend accept a local draft as a committed write". These axes should not be
coupled.

| Ownership | Backend capability | Example |
| --- | --- | --- |
| `LocalStore<Prefs, MemoryBackend>` | memory-only | Component-only draft state. |
| `LocalStore<Prefs, S3PrefsBackend>` | committed backend | One component owns preferences loaded from and committed to S3. |
| `SharedStore<Prefs, MemoryBackend>` | memory-only | Shared in-memory UI state. |
| `SharedStore<Prefs, FilePrefsBackend>` | committed backend | App-wide preferences persisted to TOML. |
| `SharedStore<Prompts, PromptProjectionBackend>` | read-only projection backend | Shared UI snapshot over committed DB rows. |

The library should provide `MemoryBackend` and closure-based backend builders,
but not hard-code file/database/S3 as special store kinds.

## Store Backend Traits

The backend traits are the real extension point. Users decide how to load,
subscribe, reconcile, and, only when supported, commit. The library owns GPUI
wiring, revision checks, weak entity upgrade, selection refresh, and owner
notification.

For a `Backend: StoreBackend<S>` store, the synchronization surface is
`load` / `subscribe` / `load_after_event` / `reconcile` plus
`store.sync_snapshot(...)`. The first four belong to the backend trait;
`sync_snapshot` belongs to the store wrapper so committed snapshots take the
same revision, equality, notification, and selector refresh path as backend
events.

The exact future/task alias is an implementation detail, but the target shape
is:

```rust
pub trait StoreBackend<S: StoreState>: 'static {
    type Snapshot: Clone + PartialEq + 'static;
    type Event: 'static;
    type Subscription: 'static;
    type Error: 'static;

    fn backend_id(&self) -> StoreBackendId;

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        Ok(None)
    }

    fn subscribe(
        &self,
        on_change: StoreBackendCallback<Self::Event>,
    ) -> StoreBackendFuture<Option<Self::Subscription>, Self::Error> {
        Ok(None)
    }

    fn load_after_event(
        &self,
        event: Self::Event,
    ) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
        self.load()
    }

    fn reconcile(&self, state: &mut S, snapshot: Self::Snapshot) -> bool;
}

pub trait StoreCommitBackend<S: StoreState>: StoreBackend<S> {
    fn commit_snapshot(
        &self,
        draft: &S,
    ) -> StoreBackendFuture<Option<StoreCommitAck<Self::Snapshot>>, Self::Error>;
}
```

Synchronization semantics are expressed by backend capabilities:

| Shape | Meaning |
| --- | --- |
| Default `MemoryBackend` | No durable/external backend; store exposes infallible `set/update/bind`. |
| `StoreBackend` with `load` | Backend hydrates initial state. |
| `StoreBackend` with `subscribe` / `load_after_event` | External backend may change independently and can push events. |
| `StoreCommitBackend` | Local draft updates are first committed to the backend; store updates only after success. |
| `StoreBackend` + manual `sync_snapshot` after repository commits | Store is a UI projection over committed external state; no generic setters are exposed. |

`backend_id` should encode backend identity when useful: `file:prefs`,
`s3:prefs`, `http:settings`, `database:prompts`, `memory:runtime`.

## SharedStore API Shape

All backend-backed stores expose read, selection, observer, and snapshot sync
APIs. `sync_snapshot` belongs to the store wrapper, not to `StoreBackend`: the
backend defines how snapshots reconcile into state, while the store owns the
revision, equality, notification, and selection refresh path.

```rust
impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState,
    Backend: StoreBackend<S>,
{
    pub fn new_with_backend(cx: &mut impl AppContext, initial: S, backend: Backend) -> Result<Self, Backend::Error>;
    pub fn install_global_with_backend(cx: &mut App, initial: S, backend: Backend) -> Result<Self, Backend::Error>;
    pub fn install_global_with_default(cx: &mut App, backend: Backend) -> Result<Self, Backend::Error>
    where
        S: Default;
    pub fn global(cx: &impl AppContext) -> Self;

    pub fn read<R>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> R) -> R;
    pub fn read_cloned<T: Clone>(&self, cx: &impl AppContext, f: impl FnOnce(&S) -> &T) -> T;
    pub fn revision(&self, cx: &impl AppContext) -> StoreRevision;
    pub fn select<Owner, T>(&self, cx: &mut Context<Owner>, select: impl Fn(&S) -> T + 'static) -> StoreSelection<T>;
    pub fn select_cloned<Owner, T>(&self, cx: &mut Context<Owner>, select: impl Fn(&S) -> &T + 'static) -> StoreSelection<T>
    where
        Owner: 'static,
        T: Clone + PartialEq + 'static;
    pub fn observe_select<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;
    pub fn observe_select_in<Owner, T>(
        &self,
        cx: &mut Context<Owner>,
        window: &mut Window,
        select: impl Fn(&S) -> T + 'static,
        observe: impl Fn(&mut Owner, &T, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where
        Owner: 'static,
        T: PartialEq + 'static;
    pub fn refresh_from_backend(&self, cx: &mut impl AppContext) -> Result<StoreUpdate, Backend::Error>;
    pub fn sync_snapshot(&self, cx: &mut impl AppContext, snapshot: Backend::Snapshot) -> Result<StoreUpdate, Backend::Error>;
}
```

Only `MemoryBackend` exposes infallible local mutation:

```rust
impl<S> SharedStore<S, MemoryBackend>
where
    S: StoreState,
{
    pub fn set<T: PartialEq>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, value: T) -> StoreUpdate;
    pub fn update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> StoreUpdate
    where
        S: Clone + PartialEq;
    pub fn update_if(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S) -> bool) -> StoreUpdate;
    pub fn bind<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T>;
}
```

Only committed backends expose fallible mutation:

```rust
impl<S, Backend> SharedStore<S, Backend>
where
    S: StoreState + Clone + PartialEq,
    Backend: StoreCommitBackend<S>,
{
    pub fn try_set<T: PartialEq>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, value: T) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S)) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_if(&self, cx: &mut impl AppContext, f: impl FnOnce(&mut S) -> bool) -> Result<StoreUpdate, Backend::Error>;
    pub fn try_update_field<T>(&self, cx: &mut impl AppContext, field: impl FnOnce(&mut S) -> &mut T, update: impl FnOnce(&mut T)) -> Result<StoreUpdate, Backend::Error>;
    pub fn bind_committed<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T, Backend::Error>;
    pub fn bind_committed_field<Owner, T>(&self, cx: &mut Context<Owner>, get: impl Fn(&S) -> T + 'static, set: impl Fn(&mut S, T) + 'static) -> StoreBinding<T, Backend::Error>;
}
```

`LocalStore` follows the same capability split: common read/sync APIs for every
backend, infallible mutation only for `MemoryBackend`, and `try_*` mutation only
for `StoreCommitBackend`.

## Convenience Helpers

The small helper methods are part of the default path, not optional sugar:

| Helper | Use when | Avoids |
| --- | --- | --- |
| `read_cloned(cx, |s| &s.field)` | A caller needs an owned clone of a field. | `read(cx, |s| s.field.clone())` at every callsite. |
| `select_cloned(cx, |s| &s.field)` | A component stores a cloned subscribed field snapshot. | Repeated `select(cx, |s| s.field.clone())` closures. |
| `observe_select` / `observe_select_in` | A side effect should run only when selected output changes. | Observing the whole store entity and filtering manually. |
| `refresh_from_backend(cx)` | A projection backend should reload its canonical snapshot after a domain command commits. | Calling repository `list_*` at every callsite and then `sync_snapshot`. |
| `sync_snapshot(cx, snapshot)` | The command already returned the committed snapshot. | Forcing the backend to reload data it already has. |
| `try_update_field(cx, |s| &mut s.field, ...)` | A committed backend owns the whole state but the UI edits one field. | Boilerplate clone/read/mutate/save/sync code in app helpers. |
| `bind_committed` / `bind_committed_field` | A form field writes through a committed backend. | Components keeping a second persistence helper next to the binding. |

`StoreBackendBuilder` also includes reconciliation helpers for common shapes:

```rust
let backend = StoreBackendBuilder::new("file:prefs")
    .load(load_prefs)
    .reconcile_replace()
    .commit_snapshot(save_prefs);

let prompts = StoreBackendBuilder::new("database:prompts")
    .load(list_prompts)
    .reconcile_field(|state: &mut PromptState| &mut state.prompts);
```

## Example: File-Backed Preferences

Common file-backed state should not require users to implement every trait by
hand. Use the closure builder, which implements `StoreBackend<S>` and returns a
commit-capable backend after `commit_snapshot` is attached.

```rust
use gpui::App;
use gpui_store::{SharedStore, StoreBackendBuilder, StoreCommitAck};

fn install_prefs(path: PathBuf, cx: &mut App) -> anyhow::Result<()> {
    let backend = StoreBackendBuilder::new("file:chat-prefs")
        .load({
            let path = path.clone();
            move || load_toml::<ChatPrefs>(&path).map(Some)
        })
        .reconcile_replace()
        .commit_snapshot(move |draft| {
            save_toml(&path, draft)?;
            Ok(Some(StoreCommitAck::without_snapshot()))
        });

    SharedStore::install_global_with_backend(cx, ChatPrefs::default(), backend)?;
    Ok(())
}
```

Default behavior:

- Initial load calls `load`.
- External events are optional; no `subscribe` means no live watching.
- `reconcile` mutates the store state from backend snapshots and returns whether
  it changed.
- `try_update` clones the current state into a draft and commits that draft
  before changing the store.
- If commit fails, the store remains unchanged and nothing is notified.
- If commit returns a snapshot, the store reconciles from that committed
  snapshot. Otherwise, it installs the committed draft.

## Example: S3-Backed Local Store

S3 should not require a new built-in backend kind. The app implements the traits
and uses it with either `LocalStore` or `SharedStore`.

```rust
use gpui::Context;
use gpui_store::{
    LocalStore, StoreBackend, StoreBackendFuture, StoreBackendId, StoreCommitAck,
    StoreCommitBackend,
};

struct S3PrefsBackend {
    bucket: String,
    key: String,
    client: S3Client,
}

impl StoreBackend<ChatPrefs> for S3PrefsBackend {
    type Snapshot = ChatPrefs;
    type Event = S3ObjectVersion;
    type Subscription = S3Watcher;
    type Error = anyhow::Error;

    fn backend_id(&self) -> StoreBackendId {
        StoreBackendId::new(format!("s3:{}/{}", self.bucket, self.key))
    }

    fn load(&self) -> StoreBackendFuture<Option<Self::Snapshot>, Self::Error> {
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
}

impl StoreCommitBackend<ChatPrefs> for S3PrefsBackend {
    fn commit_snapshot(
        &self,
        draft: &ChatPrefs,
    ) -> StoreBackendFuture<Option<StoreCommitAck<ChatPrefs>>, Self::Error> {
        self.client
            .save_toml(self.bucket.clone(), self.key.clone(), draft.clone())?;
        Ok(None)
    }
}

struct PrefsPane {
    prefs: LocalStore<ChatPrefs, S3PrefsBackend>,
}

impl PrefsPane {
    fn new(backend: S3PrefsBackend, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        Ok(Self {
            prefs: LocalStore::with_backend(cx, ChatPrefs::default(), backend)?,
        })
    }
}
```

The same `S3PrefsBackend` can be passed to `SharedStore::install_global_with_backend`
when multiple components need the same S3-backed state.

For component-local live backend events, the library needs an accessor that tells
it where the `LocalStore` field lives on the owner component:

```rust
struct PrefsPane {
    prefs: LocalStore<ChatPrefs, S3PrefsBackend>,
}

impl PrefsPane {
    fn new(backend: S3PrefsBackend, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let mut prefs = LocalStore::with_backend(cx, ChatPrefs::default(), backend)?;
        prefs.subscribe(cx, |pane: &mut PrefsPane| &mut pane.prefs)?;
        Ok(Self { prefs })
    }
}
```

## Example: File Watching

If live reload is needed, add a subscription backend. The watcher details remain
user controlled.

```rust
let backend = StoreBackendBuilder::new("file:settings")
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

The library should own callback wiring, weak entity upgrade, state change checks,
and owner notification. The user owns the file watcher implementation.

## Example: Database Projection

Database state should usually be synchronized from committed snapshots instead
of treating every UI command as a generic store write. A database projection
implements `StoreBackend`, not `StoreCommitBackend`, so `set`, `try_set`, and
writable bindings are not available.

```rust
#[derive(Clone, PartialEq, Default)]
struct PromptState {
    rows: Vec<PromptRecord>,
}

impl StoreState for PromptState {}

fn install_prompts(
    repository: PromptRepository,
    cx: &mut App,
) -> anyhow::Result<()> {
    let backend = StoreBackendBuilder::new("database:prompts")
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

    SharedStore::install_global_with_backend(cx, PromptState::default(), backend)?;
    Ok(())
}

fn create_prompt<Backend>(
    prompts: &SharedStore<PromptState, Backend>,
    repository: &PromptRepository,
    name: String,
    cx: &mut App,
) -> anyhow::Result<()>
where
    Backend: StoreBackend<PromptState, Snapshot = Vec<PromptRecord>>,
{
    let committed_rows = repository.create_prompt(name)?;
    prompts.sync_snapshot(cx, committed_rows)?;
    Ok(())
}
```

The database transaction result is the source of truth. The store is a stable
UI snapshot over that committed data.

## Example: Advanced Delta

`StoreDelta` currently exists only as a marker trait. A future delta API may be
useful when snapshot comparison is not enough, or persistence must know exactly
which part changed.

```rust
#[derive(Clone, Default)]
struct PrefsDelta {
    model_changed: bool,
    approval_changed: bool,
}

// Possible future shape, not implemented today:
store.update_delta(cx, |state| {
    state.model = next_model;
    PrefsDelta {
        model_changed: true,
        ..PrefsDelta::default()
    }
});

backend.write_delta(|state, delta| {
    if delta.model_changed {
        save_model_choice(&state.model)?;
    }
    Ok(())
});
```

This would be the escape hatch, not the default shape.

## Rules

- Prefer memory `StoreBinding<T>` for writable in-memory fields.
- Prefer committed `StoreBinding<T, E>` or `try_*` store methods when a backend
  must accept the write before UI state changes.
- Prefer `StoreSelection<T>` for derived read-only data.
- Prefer `observe_select` / `observe_select_in` for side effects that should run
  only when a derived value changes.
- Prefer `LocalStore<S>` for component-private state.
- Prefer `SharedStore<S>` for shared observable state.
- Prefer `MemoryBackend` for memory-only state.
- Implement `StoreBackend<S>` for custom load/reconcile/subscribe behavior,
  including S3/HTTP/keychain/database projections.
- Implement `StoreCommitBackend<S>` only when generic local draft updates are a
  valid way to write to the backend.
- Use `StoreBackendBuilder` only as a convenience closure adapter for common cases.
- Do not mirror an entire database into one store.
- Do not expose generic setters for database projections; write through domain
  repository commands and refresh with committed snapshots.
- Do not observe a whole store when the side effect only depends on one selected
  field.
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
