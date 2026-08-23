# jaco-db：持久化 Issue #188 rename 与原子 project archive

## Root hub and ownership

- Plan ID：`issue-188`
- Root hub：[Issue #188 root hub](../../../../../docs/dev/issue-188/README.md)
- Owner directory：`crates/jaco-db`
- Owner plan：`crates/jaco-db/docs/dev/issue-188/README.md`
- Owner index：[jaco-db 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`、`S-05`、`S-08`–`S-10`、`S-15`、`S-18`–`S-19`、`D-02`、`D-04`–`D-05`、`C-01`–`C-02`、`ERR-01`、`ERR-04`、`R-04`–`R-06`、`R-08`、`R-10`–`R-11`
- Owner-authored local IDs/ranges：`E/F/L/DB/R/T/WP-3xx`
- Assigned WP：`WP-301`
- Owns：conversation title UPDATE、active-project batch soft-delete transaction、deterministic output、DB atomicity/error tests
- Does not own：product labels/service names、app publication/runtime/route、schema/migration、Archived/unarchive/permanent delete

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-301` | `ConversationRecord` is `ConversationSummary` and includes title/status/pinned/timestamps | `src/records/conversations.rs` | Both commands return existing record type；no DTO/model change |
| `E-302` | project rename/pin use direct UPDATE + `updated_at` + RETURNING | `src/repository/projects.rs` | Conversation rename follows established mutation style |
| `E-303` | single soft-delete uses `immediate_transaction`、running-run exists check、Deleted/deleted_at/updated_at | `src/repository/conversations.rs::soft_delete_conversation` | Batch preserves exact storage semantics and ERR-01 |
| `E-304` | sidebar query includes only Active conversations whose project is visible | `list_sidebar_conversations` | Batch only targets Active；Deleted rows disappear on publication/reload |
| `E-305` | schema already contains title/status/archived_at/deleted_at | `src/migrations.rs`、`src/schema.rs` | No schema/migration/generated-schema change |
| `E-306` | `DbError::ConversationHasActiveRun { conversation_id }` is typed | `src/error.rs` | Batch returns deterministic first conflict without new error |
| `E-307` | SQLite build enables multi-row RETURNING | `Cargo.toml` Diesel feature `returning_clauses_for_sqlite_3_35` | Batch UPDATE can return changed rows without ID bind limits |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-301` | Rename stores caller title exactly and updates `updated_at`；UI/service own trimmed nonempty precondition | `E-302`、root C-01 | No DB constraint/new validation error |
| `D-302` | Batch finds conflicts through `agent_runs JOIN conversations` inside one immediate transaction, ordered by conversation ID | `E-303`、`E-306` | deterministic ERR-01 and no per-row transaction loop |
| `D-303` | One UPDATE filters `project_id + Active`, uses one timestamp and RETURNING；sort records by ID in Rust | `E-304`、`E-307` | empty success、no bind-limit risk、stable output |
| `D-304` | Do not refactor existing single soft-delete into batch helper in this issue | bounded scope | Preserve established single-command behavior |

## Owner-local target design

### File and ownership tree

```text
crates/jaco-db/
├── src/
│   ├── repository/conversations.rs       # F-301 [Modify, handwritten] rename + batch transaction
│   └── tests/
│       ├── projects.rs                   # F-302 [Modify, handwritten] rename/filter/empty/order tests
│       └── agent.rs                      # F-303 [Modify, handwritten] active-run atomicity tests
└── docs/dev/
    ├── README.md                         # F-304 [Modify, handwritten] owner index
    └── issue-188/README.md               # F-305 [Add, handwritten] this plan
```

Explicit unchanged：`src/migrations.rs`、`src/schema.rs`、record/model modules、`Cargo.toml`、workspace manifests、`Cargo.lock`。

### Owner-local contracts

#### L-301 / DB-301：Rename conversation title

```rust
impl FreshRepository {
    pub fn rename_conversation(
        &self,
        id: &str,
        title: String,
    ) -> Result<ConversationRecord>;
}
```

Target SQL：

```sql
UPDATE conversations
SET title = ?2,
    updated_at = ?3
WHERE id = ?1
RETURNING <SqlConversationRow columns>;
```

Target Diesel：

```rust
diesel::update(conversations::table.find(id))
    .set((
        conversations::title.eq(title),
        conversations::updated_at.eq(now_string()?),
    ))
    .returning(SqlConversationRow::as_returning())
    .get_result::<SqlConversationRow>(&mut conn)?
    .try_into()
```

Invariants：

- Missing row remains Diesel NotFound -> ERR-04.
- Preserve project/status/pinned/metadata/settings/created/archived/deleted fields.
- `updated_at` changes even for equal title, matching current presentation mutations.
- One UPDATE/RETURNING statement needs no explicit transaction.

#### L-302 / DB-302：Atomic soft-delete of active project conversations

```rust
impl FreshRepository {
    pub fn soft_delete_active_project_conversations(
        &self,
        project_id: &str,
    ) -> Result<Vec<ConversationRecord>>;
}
```

Exact transaction algorithm：

```rust
let mut conn = self.conn()?;
conn.immediate_transaction(|conn| {
    let active = db_label(&ConversationStatus::Active)?;
    let deleted = db_label(&ConversationStatus::Deleted)?;

    let blocked_id = agent_runs::table
        .inner_join(conversations::table.on(
            conversations::id.eq(agent_runs::conversation_id),
        ))
        .filter(conversations::project_id.eq(project_id))
        .filter(conversations::status.eq(&active))
        .filter(agent_runs::status.eq("running"))
        .select(agent_runs::conversation_id)
        .order(agent_runs::conversation_id.asc())
        .first::<String>(conn)
        .optional()?;

    if let Some(conversation_id) = blocked_id {
        return Err(DbError::ConversationHasActiveRun { conversation_id });
    }

    let now = now_string()?;
    let rows = diesel::update(
        conversations::table
            .filter(conversations::project_id.eq(project_id))
            .filter(conversations::status.eq(&active)),
    )
    .set((
        conversations::status.eq(deleted),
        conversations::deleted_at.eq(Some(now.clone())),
        conversations::updated_at.eq(now),
    ))
    .returning(SqlConversationRow::as_returning())
    .load::<SqlConversationRow>(conn)?;

    let mut records = rows
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<ConversationRecord>>>()?;
    records.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(records)
})
```

Atomicity/data policy：

- Immediate transaction serializes precheck and UPDATE.
- Conflict query includes only Active rows in the exact project and returns lexicographically first conflicting ID.
- UPDATE touches only Active rows；already Archived/Deleted and other-project rows stay unchanged.
- Empty project/no Active rows returns `Ok(Vec::new())`.
- One `now` is written to all changed rows；RETURNING results sort `id ASC`.
- Query/conversion/time error rolls back every row；no partial Vec.
- No agent/message row or filesystem data is deleted；only status/timestamps change.

### Boundary implementations

| Root boundary/error | DB implementation |
| --- | --- |
| `C-01` | `L-301/DB-301` returns complete persisted record |
| `C-02` | existing single soft-delete + `L-302/DB-302` provide Deleted/deleted_at semantics and stable batch Vec |
| `ERR-01` | existing typed variant from deterministic conflict query；batch zero-write |
| `ERR-04` | pool/connection/Diesel/time/conversion failures propagate；statement/transaction gives no partial result |

### Database and migration design

- Final schema remains current `conversations` and `agent_runs` tables.
- Existing indexes/foreign keys are sufficient for bounded project scans；do not add an index speculatively.
- `SCHEMA_VERSION`、fresh schema SQL、`schema.rs`、legacy validation、backfill/rebuild/rollback code stay unchanged.
- Existing rows：Active eligible；Archived/Deleted history preserved；no rewrite/reinterpretation.
- Rollback is normal SQLite transaction rollback；there is no migration rollback.

## Owner-local work package

### WP-301：Implement rename and atomic project archive

**Owner**

`crates/jaco-db`。

**Prerequisites and contracts**

- Root `D-02`、`D-04`–`D-05`、`C-01`–`C-02`、`ERR-01`、`ERR-04`、`R-04`–`R-06`.

**File IDs**

- `F-301`–`F-303`

**Implementation sequence**

1. Add `L-301/DB-301` beside metadata/pin mutations.
2. Add `L-302/DB-302` beside single soft-delete；import `OptionalExtension` if needed.
3. Add rename/empty/filter/order tests to `F-302`.
4. Add active-run rollback and terminal-run tests to `F-303` using existing fixtures.
5. Run focused checks and verify unchanged-file list.

**Failure and lifecycle behavior**

- No async Task in this owner. Every failure before commit returns no success/publication signal.

**Tests**

| R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-04` | `T-301` `tests/projects.rs` | `conversation_can_be_renamed` | title/updated_at change；other fields stable；reload exact |
| `R-06` | `T-302` `tests/projects.rs` | `soft_delete_active_project_conversations_returns_empty_for_empty_project` | empty Vec |
| `R-06` | `T-303` `tests/projects.rs` | `soft_delete_active_project_conversations_only_changes_active_rows` | only active target becomes Deleted |
| `R-06` | `T-304` `tests/projects.rs` | `soft_delete_active_project_conversations_returns_ids_in_stable_order` | IDs ASC；shared command timestamp |
| `R-06`、`R-08` | `T-305` `tests/agent.rs` | `soft_delete_active_project_conversations_rolls_back_when_any_run_is_active` | exact blocked ID；all remain Active/deleted_at None |
| `R-06` | `T-306` `tests/agent.rs` | `soft_delete_active_project_conversations_succeeds_after_terminal_runs` | completed/failed/canceled allow batch |
| `R-05`、`R-10` | `T-307` existing `tests/agent.rs` | keep single active-run regression | current single behavior unchanged |

**Focused validation**

```sh
cargo fmt
cargo test -p jaco-db conversation_can_be_renamed
cargo test -p jaco-db soft_delete_active_project_conversations
cargo test -p jaco-db soft_delete_conversation_rejects_active_run
cargo test -p jaco-db
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings
git diff --check
```

Unchanged-file verification：

```sh
git diff --exit-code -- \
  crates/jaco-db/src/migrations.rs \
  crates/jaco-db/src/schema.rs \
  crates/jaco-db/Cargo.toml \
  Cargo.toml \
  Cargo.lock
```

**Done condition**

`L-301/L-302`、`DB-301/DB-302`、`T-301`–`T-307` pass；active-run rejection proves zero writes；schema/migration/manifests/lock diff is empty.

## Focused validation and handoff

| Local R-ID | T-ID/evidence | Expected result |
| --- | --- | --- |
| `R-301` exact rename persistence | `T-301` | restart-read summary matches returned title |
| `R-302` batch selection/output | `T-302`–`T-304` | Active-only、empty success、stable IDs |
| `R-303` transaction atomicity | `T-305`–`T-307` | conflict zero-write、terminal success、single regression |
| `R-304` no data-model expansion | unchanged-file diff | no schema/migration/model/dependency change |

Implementation evidence (2026-08-23): `rename_conversation` and
`soft_delete_active_project_conversations` are implemented in
`src/repository/conversations.rs`; DB tests cover rename, empty/active-only/stable-order
batch behavior, active-run rollback, and terminal-run compatibility. Focused commands
passed，and `cargo test -p jaco-db` passed all 77 tests；selected-owner and full-workspace
clippy also passed. The unchanged-file command for schema、migration、manifests and
`Cargo.lock` passed. Root hub owns manual and remote-CI completion.
