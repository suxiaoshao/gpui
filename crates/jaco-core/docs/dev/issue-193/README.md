# jaco-core：Issue #193 conversation recency domain contract

## Root hub and ownership

- Plan ID：`issue-193`
- Root status：`In progress`
- Root hub：[Issue #193 root hub](../../../../../docs/dev/issue-193/README.md)
- Owner directory：`crates/jaco-core`
- Owner plan：`crates/jaco-core/docs/dev/issue-193/README.md`
- Owner index：[jaco-core 开发计划](../README.md)
- Root-owned IDs consumed：`S-01`、`S-03`、`S-08`、`S-10`、`S-18`–`S-19`、`D-03`、`C-01`–`C-02`、`R-04`–`R-06`、`R-14`
- Owner-authored local IDs/ranges：`E/F/L/R/T/WP-1xx`
- Assigned WP：`WP-101`
- Owns：非空 conversation recency 的 domain shape、equality/fixture contract
- Does not own：SQLite column/write timing、catalog/sidebar sorting、relative formatter、runtime status、UI/i18n

## Owner-local evidence

| E-ID | Claim | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-101` | `ConversationSummary` 是 DB `ConversationRecord` 和 app catalog 的共享 domain type | `src/domain.rs::ConversationSummary`；`crates/jaco-db/src/records/conversations.rs` | recency 的跨 owner authority 放在 summary |
| `E-102` | summary 当前只有 `created_at`、`updated_at`、archive/delete timestamps | `src/domain.rs:21-39` | 增加一项非空 instant，不复用 updated_at |
| `E-103` | domain tests/fixtures 直接构造完整 summary | `src/domain.rs` `#[cfg(test)]` constructors | source-breaking field必须一次更新 fixtures |
| `E-104` | `ConversationChange::SummaryChanged` 整体替换 summary | `src/domain.rs::Transition<ConversationChange>` | 无需新增 recency-specific change enum |

## Owner-local decisions

| D-ID | Decision | Evidence | Consequence |
| --- | --- | --- | --- |
| `D-101` | `recency_at` 是非空 `OffsetDateTime` | root `C-01`、`E-101`–`E-102` | 存储层可选表示不泄漏到 domain |
| `D-102` | recency 作为 `ConversationSummary` field 随完整 summary equality/clone/publication | `E-103`–`E-104` | 不新增 parallel DTO 或 change type |
| `D-103` | core 不定义 advance algorithm | root `DB-01` | DB 是时间写入 authority；app 只消费 |

## Owner-local target design

### File and ownership tree

```text
crates/jaco-core/
├── src/
│   └── domain.rs                     # F-101 [Modify, handwritten] strict recency field + fixtures/tests
└── docs/dev/
    ├── README.md                     # F-102 [Modify, handwritten] owner index
    └── issue-193/README.md           # F-103 [Add, handwritten] this plan
```

No module、payload enum、serde schema、manifest、generated artifact or dependency is added/moved/deleted。

### L-101：Strict conversation recency

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationSummary {
    pub id: ConversationId,
    pub project_id: ProjectId,
    pub title: String,
    pub status: ConversationStatus,
    pub pinned: bool,
    pub prompt_id: Option<PromptId>,
    pub default_provider_id: Option<ProviderId>,
    pub default_model_id: Option<ProviderModelId>,
    pub last_entry_seq: i32,
    pub metadata: ConversationMetadata,
    pub settings_snapshot: ConversationSettingsSnapshot,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub recency_at: OffsetDateTime,
    pub archived_at: Option<OffsetDateTime>,
    pub deleted_at: Option<OffsetDateTime>,
}
```

Invariants：

- `recency_at` is a real persisted instant and never `Option`/sentinel in domain code。
- `updated_at` remains independent record modification time；no alias/accessor makes them interchangeable。
- `SummaryChanged` replacement carries both values exactly；no transition derives one from the other。
- Core does not inspect title/message/payload to infer recency。

## Owner-local requirements and tests

| R-ID | Requirement |
| --- | --- |
| `R-101` | Every `ConversationSummary` constructor and fixture provides an explicit recency instant. |
| `R-102` | Clone/equality/summary transition preserve recency independently from updated_at. |
| `R-103` | No optional/sentinel/fallback recency type or recency algorithm is introduced in core. |

| Root R-ID | T-ID/file | Proposed scenario | Assertions |
| --- | --- | --- | --- |
| `R-04`–`R-05` | `T-101` `src/domain.rs` | summary uses distinct updated/recency instants | equality sees recency；values remain independent |
| `R-05` | `T-102` `src/domain.rs` | `SummaryChanged` replaces a summary with same updated_at and different recency_at | resulting conversation carries exact new recency |
| `R-14` | `T-103` crate test/build | all downstream source constructors compile after owner sequence | no missing-field workaround/default |

## WP-101：Add the strict recency contract

**Owner**

`crates/jaco-core`。

**Prerequisites and contracts**

- Root `D-03`、`C-01`–`C-02`、`R-04`–`R-06`。

**File IDs**

- `F-101`

**Implementation sequence**

1. Add `L-101.recency_at` beside `updated_at`。
2. Update every owner-local constructor/fixture with an explicit time；use distinct instants where the test should prove independence。
3. Add `T-101/T-102` without adding a default constructor or alias。
4. Hand off the compile break to `WP-201/WP-301` for producer/consumer completion。

**Failure and lifecycle behavior**

No async、persistence or error path in this owner。A missing field remains a compile-time failure until coordinated owners implement C-01。

**Focused validation**

```sh
cargo fmt --all -- --check
cargo test -p jaco-core
cargo clippy -p jaco-core --all-targets --all-features -- -D warnings
git diff --check
```

**Done condition**

`L-101` and `T-101/T-102` exist；all core fixtures provide intentional recency；no payload/API/dependency diff beyond the field contract。

## Completion evidence

| Evidence | Actual result |
| --- | --- |
| Production diff | `ConversationSummary.recency_at` 与 owner fixtures/tests 已实现 |
| Tests/commands | `cargo test -p jaco-core`：43 passed；clippy、fmt、diff check 通过 |
| Delivered local IDs | `L-101`、`T-101/T-102`、`WP-101` complete |
| Deviations | `None` |
