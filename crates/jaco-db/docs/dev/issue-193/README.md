# jaco-db：Issue #193 conversation recency fresh schema 与排序

## Root hub and ownership

- Plan ID：`issue-193`
- Root status：`In progress`
- Root hub：[Issue #193 root hub](../../../../../docs/dev/issue-193/README.md)
- Owner directory：`crates/jaco-db`
- Owner plan：`crates/jaco-db/docs/dev/issue-193/README.md`
- Owner index：[jaco-db 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`、`S-05`、`S-08`–`S-10`、`S-15`、`S-18`–`S-19`、`D-03`、`D-09`、`C-01`–`C-02`、`ERR-01`–`ERR-03`、`DB-01`、`R-04`–`R-06`、`R-14`
- Owner-authored local IDs/ranges：`E/F/L/DB/R/T/WP-2xx`
- Assigned WP：`WP-201`
- Owns：fresh schema、typed recency persistence、append/preserve matrix、DB/sidebar query ordering与 tests
- Does not own：domain product meaning、catalog/workspace/row、relative time、runtime status、HoverCard/i18n

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-201` | 现有 schema registry 只有 0001，schema version 为 1 | `src/migrations.rs::{SCHEMA_VERSION,MIGRATIONS}` | 保持 `SCHEMA_VERSION == 1` 与唯一 0001；fresh SQL 直接扩展 |
| `E-202` | fresh open 使用既有 bootstrap | `src/store.rs::open_or_create_initial` | 本 issue 只改 fresh schema 定义 |
| `E-203` | fresh bootstrap 已有事务边界与 schema metadata 语义 | `src/migrations.rs::bootstrap_with_migrations` | 保持现有事务行为，只更新 fresh SQL 与 registry assertion |
| `E-204` | `SqlConversationRow` 与 Diesel schema没有 recency | `src/models/conversations.rs`、`src/schema.rs` | non-null column/row mapping同步增加 |
| `E-205` | entry append唯一集中在 `append_conversation_entry_with_conn` 并更新 last seq/updated_at | `src/repository.rs:230-264` | recency advance集中在同一 transaction owner |
| `E-206` | metadata、rename、pin、payload update、soft delete 都更新 conversation updated_at | `src/repository/conversations.rs` | 这些路径显式保持 recency不变 |
| `E-207` | list sidebar/no-project 按 updated_at DESC | `src/repository/conversations.rs::{list_sidebar_conversations,list_no_project_conversations}` | 改为 recency DESC + ID ASC |
| `E-208` | Diesel typed row直接解码 conversation 时间字段 | `src/models/conversations.rs`、`src/schema.rs` | recency 使用非空类型，不能提供 fallback |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-201` | 唯一 0001 fresh schema 直接声明 `DateTime NOT NULL` recency column，并建立 active-recency index | root `D-09`、`E-201`–`E-204` | 不安装默认值，不从其他时间字段推导 recency，不重建 table |
| `D-202` | SQL row uses non-null `OffsetDateTime`；conversion直接传递 stored value，不 fallback 到 `updated_at` | root `ERR-03`、`E-208` | domain C-01保持 strict，坏数据不会被隐藏 |
| `D-203` | `SCHEMA_VERSION` 保持 1，registry 只保留唯一 0001；fresh bootstrap 沿用现有事务边界 | `E-201`、`E-203` | 不增加新的 schema version 或打开路径 |
| `D-206` | append helper是唯一 recency advance owner；所有成功 append kinds均算 activity，已有 entry payload update不算 | root `DB-01`、`E-205`–`E-206` | 不按消息文本/kind另建规则 |
| `D-207` | DB lists按 recency DESC、id ASC；NULL在 strict validation前阻断，query不提供 COALESCE fallback | `E-207`–`E-208` | deterministic order和坏数据可见性 |

## Owner-local target design

### File and ownership tree

```text
crates/jaco-db/
├── src/
│   ├── migrations.rs                       # F-201 [Modify] fresh 0001 SQL + registry assertions
│   ├── schema.rs                           # F-202 [Modify] non-null recency Diesel column
│   ├── models/conversations.rs             # F-203 [Modify] SQL rows + strict conversion
│   ├── records/conversations.rs            # F-204 [Modify] EntryAdvanced recency contract
│   ├── repository.rs                       # F-205 [Modify] append advances recency
│   ├── repository/conversations.rs         # F-206 [Modify] inserts/list order/preserve paths
│   └── tests/
│       ├── bootstrap.rs                    # F-207 [Modify] fresh schema bootstrap
│       ├── schema.rs                       # F-208 [Modify] column/index/invariant
│       ├── catalog.rs                      # F-209 [Modify] create/append/order
│       ├── projects.rs                     # F-210 [Modify] rename/pin/archive preserve
│       └── agent.rs                        # F-211 [Modify] payload/run append semantics
└── docs/dev/
    ├── README.md                           # F-215 [Modify] owner index
    └── issue-193/README.md                 # F-216 [Add] this plan
```

只修改现有 0001 fresh schema SQL 与对应 typed consumers；不新增 schema version、SQL 文件或打开路径。No table/file is moved/deleted；no manifest、dependency、generated artifact or lockfile change。

### L-201：Fresh schema registry

```rust
pub(crate) struct Migration {
    pub(crate) name: &'static str,
    pub(crate) sql: &'static str,
}

pub(crate) const SCHEMA_VERSION: i32 = 1;

pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "0001_create_fresh_schema",
        sql: CREATE_FRESH_SCHEMA_SQL,
    },
];
```

Invariants：registry 只有唯一 `0001`；`SCHEMA_VERSION == 1`。Tests assert the registry rather than silently inferring it。

### DB-201：Fresh conversations schema

```sql
-- inside the existing 0001_create_fresh_schema conversations table
recency_at DateTime NOT NULL,

CREATE INDEX idx_conversations_active_recency
ON conversations(status, recency_at DESC, id ASC);
```

```rust
diesel::table! {
    conversations (id) {
        // existing columns
        updated_at -> TimestamptzSqlite,
        recency_at -> TimestamptzSqlite,
        // existing columns
    }
}
```

- `recency_at` 是 fresh schema 的必填列；不安装 DEFAULT，所有 new insert path 都提供显式值。
- active-recency index 与 column 同属 0001 schema；不新增第二个 schema step。

### L-202 / DB-202：SQL row and strict domain conversion

```rust
pub(crate) struct SqlConversationRow {
    // existing fields
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) recency_at: OffsetDateTime,
    // existing fields
}

pub(crate) struct SqlNewConversationRow {
    // existing fields
    pub(crate) updated_at: OffsetDateTime,
    pub(crate) recency_at: OffsetDateTime,
    // existing fields
}
```

```rust
ConversationSummary {
    // existing mapping
    updated_at: row.updated_at,
    recency_at: row.recency_at,
    // existing mapping
}
```

Diesel 的非空 row 类型直接传递 recency；没有 nullable mapping、fallback 或额外数据库打开分支。

### L-204 / DB-204：Create and append mutation contract

Every `SqlNewConversationRow` production construction uses one `now`：

```rust
created_at: now,
updated_at: now,
recency_at: now,
```

`append_conversation_entry_with_conn` updates in the same transaction：

```rust
.set((
    conversations::last_entry_seq.eq(seq),
    conversations::updated_at.eq(now),
    conversations::recency_at.eq(now),
))
```

`ConversationIndexDelta::EntryAdvanced` target shape：

```rust
EntryAdvanced {
    id: ConversationId,
    last_entry_seq: i32,
    updated_at: OffsetDateTime,
    recency_at: OffsetDateTime,
}
```

Create-with-first-entry relies on the same append helper；a failed attachment/entry/run transaction rolls back conversation and recency together。

### DB-205：Preserve and order queries

- `update_conversation_entry_payload` updates entry/conversation `updated_at` only。
- metadata/settings、rename、pin、single/batch archive/delete update existing bookkeeping fields and `updated_at` only。
- `list_sidebar_conversations` and `list_no_project_conversations`：

```rust
.order((
    conversations::recency_at.desc(),
    conversations::id.asc(),
))
```

- Repository get/timeline/RETURNING select includes recency through `SqlConversationRow::as_select()`。
- Project ordering continues to use project fields and is outside this contract。

## Owner-local requirements

| R-ID | Requirement |
| --- | --- |
| `R-201` | Fresh stores use `SCHEMA_VERSION == 1` and the single 0001 schema. |
| `R-202` | The 0001 conversations table has a non-null, decodable recency; domain conversion never invents a fallback. |
| `R-203` | Create/append advance and every root DB-01 preserve operation behaves exactly, including failed transactions. |
| `R-204` | Active/scratch sidebar lists sort recency DESC then ID ASC. |
| `R-205` | SQL/domain recency remains non-null in every producer, row, record, and summary mapping. |
| `R-206` | Manifest, lockfile and unrelated schema/data remain unchanged. |

## WP-201：Deliver fresh schema and recency persistence

**Owner**

`crates/jaco-db`。

**Prerequisites and contracts**

- `WP-101` complete。
- Root `D-03`、`D-09`、`C-01`–`C-02`、`DB-01`、`R-04`–`R-06`。

**File IDs**

- `F-201`–`F-211`

**Implementation sequence**

1. Add L-201/DB-201 and update schema/SQL row strict mapping under L-202。
2. Set explicit recency on every create path and advance it only in append helper；extend `EntryAdvanced`。
3. Audit every conversation UPDATE against root DB-01, leaving preserve paths free of recency assignments。
4. Change both sidebar list orders and add deterministic tie-break。
5. Add focused fresh-schema、mutation 与 order tests；run focused gates and unchanged-file audit。

**Tests**

| Root R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-201` | `T-201` `tests/bootstrap.rs` | fresh DB bootstrap | `SCHEMA_VERSION == 1`；registry has only 0001；recency column/index present |
| `R-202` | `T-202` `tests/schema.rs` | fresh schema column/index and non-null mapping | column is NOT NULL；active index exists；typed row decodes |
| `R-203` | `T-203` `tests/catalog.rs` | create empty/create with first entry/append | exact recency initialization and advance |
| `R-203` | `T-204` `tests/agent.rs` | update existing entry payload/status | updated_at advances，recency preserved |
| `R-203` | `T-205` `tests/projects.rs` | metadata/rename/pin/single+batch archive | each preserves recency |
| `R-204` | `T-206` `tests/catalog.rs` | updated_at order conflicts with recency and equal-recency IDs | recency DESC、ID ASC |

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco-db bootstrap
cargo test -p jaco-db schema
cargo test -p jaco-db catalog
cargo test -p jaco-db projects
cargo test -p jaco-db agent
cargo test -p jaco-db
cargo clippy -p jaco-db --all-targets --all-features -- -D warnings
git diff --check
```

Unchanged scope audit：

```sh
git diff --exit-code -- \
  crates/jaco-db/Cargo.toml \
  Cargo.toml \
  Cargo.lock
```

Implementation review additionally verifies the `0001_create_fresh_schema` SQL contains the non-null recency column and active-recency index。

**Done condition**

T-201–T-206 prove fresh schema, non-null invariant, mutation, and order contracts；no manifest/lockfile or unrelated data diff。

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Production/schema diff | fresh 0001、typed rows、writes、delta 与 list order 已实现 |
| Tests/commands | `cargo test -p jaco-db --no-fail-fast`：82 passed；clippy、fmt、diff check 通过 |
| Delivered local IDs | `D/L/DB/R/T/WP-2xx` scoped implementation complete |
| Fresh schema evidence | `SCHEMA_VERSION == 1`；单一 0001；recency NOT NULL 与 active index tests 通过 |
| Deviations | `None` |
