# jaco-conversation：提供 Issue #188 rename/archive service boundary

## Root hub and ownership

- Plan ID：`issue-188`
- Root hub：[Issue #188 root hub](../../../../../docs/dev/issue-188/README.md)
- Owner directory：`crates/jaco-conversation`
- Owner plan：`crates/jaco-conversation/docs/dev/issue-188/README.md`
- Owner index：[jaco-conversation 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`、`S-08`–`S-10`、`S-18`–`S-19`、`D-02`、`D-04`–`D-05`、`D-09`、`C-01`–`C-02`、`ERR-01`、`ERR-04`、`R-04`–`R-06`、`R-08`、`R-10`–`R-11`
- Owner-authored local IDs/ranges：`E/F/L/R/T/WP-2xx`
- Assigned WP：`WP-201`
- Owns：conversation mutation service names/signatures、repository delegation、DB error identity preservation、focused service tests
- Does not own：DB SQL/transaction、app publication/route/runtime/UI、schema/migration、archive recovery

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-201` | `ConversationService` is a thin synchronous wrapper over `FreshRepository` and maps `DbError` transparently into `ConversationError::Database` | `src/lib.rs` | Keep mutation boundary thin；no cache/Task/Operation |
| `E-202` | Existing service exposes `set_pinned` and unused `delete`; app currently bypasses them for mutation writes | `src/lib.rs` + workspace call-site search | Replace delete with archive and make app use service consistently |
| `E-203` | `ConversationRecord` aliases `ConversationSummary` | `jaco-db::records::conversations` | No conversion or new DTO is required |
| `E-204` | `ProjectId`/`ConversationId` are String aliases in jaco-core | `jaco-core/src/lib.rs` | Public signatures use domain aliases without new types |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-201` | Add only three product commands: rename、single archive、project archive；keep `set_pinned` unchanged | root C-01/C-02 | No share/unread/unarchive/permanent-delete API |
| `D-202` | Remove `ConversationService::delete` and add `archive` with no alias/deprecation layer | `E-202`、root `D-09` | Source-breaking only for hypothetical external users；workspace has no current caller |
| `D-203` | Preserve every `DbError` variant exactly through `ConversationError::Database` | `E-201`、root ERR-01/ERR-04 | App can match active-run error without string parsing |

## Owner-local target design

### File and ownership tree

```text
crates/jaco-conversation/
├── src/lib.rs                         # F-201 [Modify, handwritten] service API + inline tests
└── docs/dev/
    ├── README.md                      # F-202 [Modify, handwritten] owner index
    └── issue-188/README.md            # F-203 [Add, handwritten] this plan
```

No manifest、dependency、serialized type、schema、migration or generated artifact changes.

### Owner-local contracts

#### L-201：Conversation presentation mutations

`F-201` implements root C-01/C-02 exactly：

```rust
use jaco_core::{
    Conversation, ConversationId, ConversationSummary, ProjectId,
};

impl<'a> ConversationService<'a> {
    pub fn rename(
        &self,
        id: &ConversationId,
        title: String,
    ) -> Result<ConversationSummary> {
        self.repository
            .rename_conversation(id, title)
            .map_err(Into::into)
    }

    pub fn archive(&self, id: &ConversationId) -> Result<ConversationSummary> {
        self.repository
            .soft_delete_conversation(id)
            .map_err(Into::into)
    }

    pub fn archive_project_conversations(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<ConversationSummary>> {
        self.repository
            .soft_delete_active_project_conversations(project_id)
            .map_err(Into::into)
    }
}
```

Contract details：

- Methods remain synchronous because DB execution/thread ownership belongs to app `SessionDatabaseExecutor`。
- Service does not trim/validate title, re-sort outputs, catch NotFound, translate status, refresh catalog or publish changes；app provides validated input and DB provides authoritative output/order.
- `archive` is the product name；the repository name remains explicit about storage soft delete.
- Empty project archive is `Ok(Vec::new())`。
- ERR-01 and every other DB failure retain the exact `DbError` value in `ConversationError::Database`。

### Boundary implementations

| Root boundary/error | This owner implementation |
| --- | --- |
| `C-01` | `L-201::rename` delegates to DB owner `L-301` and returns the complete summary unchanged |
| `C-02` | `L-201::{archive,archive_project_conversations}` delegate to existing single soft-delete / DB owner `L-302` |
| `ERR-01` | `DbError::ConversationHasActiveRun` is wrapped transparently；no retry or partial output |
| `ERR-04` | All other DB failures are wrapped transparently；no generic service fallback |

### State, lifecycle and compatibility

- This crate owns no mutable state, Entity, Store, Task, cache, subscription or Operation for these commands.
- Returned summaries are moved directly across C-01/C-02. The app is the sole publication owner.
- Replacing `delete` with `archive` intentionally removes the old source symbol. A final `rg 'ConversationService.*delete|\.delete\('` over workspace must find no valid call site or compatibility alias.
- `ConversationError` enum is unchanged；no new validation/error variant.

## Owner-local work package

### WP-201：Deliver C-01/C-02 service API

**Owner**

`crates/jaco-conversation`。

**Prerequisites and contracts**

- Root `D-02`、`D-09`、`C-01`–`C-02`、`ERR-01`、`ERR-04`；DB `WP-301` complete.

**File IDs**

- `F-201`

**Implementation sequence**

1. Import `ProjectId` and add `L-201::rename` / `archive_project_conversations`.
2. Rename existing `delete` to `archive` and keep its exact repository delegation.
3. Add focused service tests using `FreshStore::open_or_create_initial` and existing project/conversation fixtures.
4. Confirm no caller/alias keeps the delete service name and no non-target file changes.

**Failure and lifecycle behavior**

- Service returns repository result synchronously；atomicity belongs to DB transaction；no partial Vec or error recovery is synthesized.

**Tests**

| R-ID | T-ID/file | Proposed scenario | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| `R-04` | `T-201` `src/lib.rs` | `sidebar_mutation_service_renames_conversation` | temp DB + one conversation | title persisted；returned summary exact |
| `R-05`–`R-06` | `T-202` `src/lib.rs` | `sidebar_mutation_service_archives_single_and_project_conversations` | project with two active conversations | single/batch status Deleted；stable batch IDs |
| `R-06`、`R-08` | `T-203` `src/lib.rs` | `sidebar_mutation_service_preserves_active_run_error_and_empty_batch` | running-run project + empty project | exact nested ERR-01；empty Vec success；blocked rows remain Active |

**Focused validation**

```sh
cargo fmt
cargo test -p jaco-conversation sidebar_mutation_service
cargo test -p jaco-conversation
cargo clippy -p jaco-conversation --all-targets --all-features -- -D warnings
git diff --check
```

**Done condition**

`L-201` and `T-201`–`T-203` pass；C-01/C-02 output/error identity is unchanged from repository；no `delete` alias、new state、manifest or lockfile diff.

## Focused validation and handoff

| Local R-ID | T-ID/evidence | Expected result |
| --- | --- | --- |
| `R-201` exact delegation | `T-201`–`T-203` | return values/errors exactly match repository |
| `R-202` semantic API cleanup | workspace `rg` + compile | `archive` is the sole service soft-delete name |
| `R-203` owner isolation | final diff | only `F-201` production code plus plan/index changes |

Implementation evidence (2026-08-23): `ConversationService::{rename,archive,
archive_project_conversations}` delegates directly to the repository and preserves
`ConversationError::Database` identity. `cargo test -p jaco-conversation` passed all 8
tests，including rename、single/project archive、empty-project no-op and exact active-run
error identity；selected-owner and full-workspace clippy also passed. No service delete
alias、manifest or lockfile change remains. Root hub owns manual and remote-CI completion.
