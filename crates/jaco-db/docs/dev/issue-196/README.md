# jaco-db：Issue #196 stable attachment ID 与 prelinked batch

## Root hub and ownership

- Plan ID：`issue-196`
- Root status：`Implemented locally`；agent动画首帧校验与root `T-11`基准已完成，`Done`受`RG-01/T-07/T-10`约束
- Root hub：[Issue #196 root plan](../../../../../docs/dev/issue-196/README.md)
- Owner：`crates/jaco-db`
- Owner index：[jaco-db 开发计划](../README.md)
- Assigned WP：`WP-101`
- Root-owned contracts consumed：`D-06`–`D-08`、`C-03`、`DB-01`、`G-01`、`R-06`–`R-09`、`T-04`
- Owner-local IDs：`E/D/F/L/DB/R/T/WP-1xx`
- Owns：caller-assigned attachment ID、prelinked entry/attachment batch、generated attachment index、transaction tests
- Does not own：provider/Rig、network/filesystem、runtime publication、managed-root scan、UI、schema/product decisions

## Owner implementation result（2026-08-31）

- `WP-101` 已实现；attachments `6/6`、agent `32/32`、catalog `9/9` 通过。
- package clippy `-D warnings`、source diff check 与最终 workspace 门禁通过。
- schema、migration、models、manifest 与 jaco-core 无 diff。

## Owner evidence

| E-ID | Current fact | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-101` | `NewAttachment`当前没有ID；`insert_attachment_with_conn`内部调用`new_id()` | `src/records/conversations.rs::NewAttachment`、`src/repository.rs::insert_attachment_with_conn` | stable filename/content在DB前无法引用同一ID，必须改为caller-assigned |
| `E-102` | 单entry helper在immediate transaction内先插attachments，再把parts追加到Message末尾 | `src/repository.rs::insert_attachments_into_message_item_with_conn`、`src/repository/conversations.rs::append_conversation_entry_with_attachments` | 保留composer contract，新增prelinked batch表达provider order |
| `E-103` | `append_conversation_entry_with_conn`集中分配seq并推进last_entry_seq/updated_at/recency_at | `src/repository.rs::append_conversation_entry_with_conn` | batch逐entry复用该helper，最终commit只读一次conversation |
| `E-104` | `ConversationCommit<T>`已返回value、conversation和index_delta | `src/records/conversations.rs::ConversationCommit` | 新batch沿用统一commit envelope，无新publication type |
| `E-105` | attachment schema已有kind/storage/provider/hash/size/metadata/path且只归conversation | `src/migrations.rs`、`src/schema.rs`、`src/models/conversations.rs` | schema/version/row mapping不变 |
| `E-106` | entry row已有agent_run_id/provider_step_id，payload含attachment ID | `src/models/conversations.rs`、`jaco-core::ContentPart` | root C-03 lineage可直接落地 |
| `E-107` | 当前只能按conversation列出attachments | `src/repository/conversations.rs::conversation_attachments` | app startup sweep需要只读generated-file index |
| `E-108` | create/send/user attachment transactions依赖现有auto-append helper | `src/repository/conversations.rs` call sites | 不改变它的part ordering contract，避免扩大composer迁移 |

## Owner-local decisions

| D-ID | Decision | Root authority / evidence | Consequence |
| --- | --- | --- | --- |
| `D-101` | `NewAttachment.id`为必填且所有insert paths都使用它；repository不再替调用方生成attachment ID | root `D-06`；`E-101` | app/tests所有constructor同一批更新 |
| `D-102` | 既有single/composer helper继续按input vector把parts追加到末尾；provider使用独立prelinked batch | root `C-03`；`E-102/E-108` | ordinary user message行为零变化 |
| `D-103` | batch只允许同conversation、同non-null run/step的Assistant Message/Reasoning；DB读取并验证run属于conversation、step属于run且已Completed；Message只接受Text/Image | root `C-03/DB-01` | tool/status/error、Running/foreign step不能误用此入口 |
| `D-104` | provided attachment必须由同一item的Image part恰好引用一次；batch内不允许引用existing/unprovided attachment | root `DB-01` | DB可在写入前证明完整prelinked graph |
| `D-105` | attachment insert顺序取Message content source order；entry顺序取batch order | root `R-05/R-08` | returned vectors可直接驱动publication |
| `D-106` | generated index只返回`storage_kind=GeneratedFile` records并稳定按conversation/id排序；不解释或canonicalize path | root `G-01`；`E-107` | app是filesystem trust owner |
| `D-107` | schema、Diesel row、migration registry与core payload零变化 | root `D-07`；`E-105/E-106` | no migration/rollback data work |

## File and ownership tree

```text
crates/jaco-db/
├── src/
│   ├── records/conversations.rs            # F-101 [Modify] stable ID + batch input/output records
│   ├── repository.rs                       # F-102 [Modify] caller ID insert + prelinked validation/insert helpers
│   ├── repository/conversations.rs         # F-103 [Modify] public batch + generated-file index
│   └── tests/
│       ├── attachments.rs                  # F-104 [Modify] IDs, ordering, mismatch, rollback, generated index
│       ├── agent.rs                        # F-105 [Modify] run/step lineage and batch commit
│       └── catalog.rs                      # F-106 [Modify] existing constructor/source-break regression
└── docs/dev/
    ├── README.md                           # F-107 [Modify] owner index
    └── issue-196/README.md                 # F-108 [Add] this plan
```

Explicit unchanged：

```text
crates/jaco-db/src/{migrations.rs,schema.rs,models/**}
crates/jaco-db/Cargo.toml
crates/jaco-core/**
```

`WP-101`自身不改manifest/lock；聚合分支中的预期`Cargo.lock` direct-edge metadata变化由root `DEP-01` / agent `WP-202`拥有。

## L-101：Target records

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct NewAttachment {
    pub id: AttachmentId,
    pub conversation_id: ConversationId,
    pub kind: AttachmentKind,
    pub storage_kind: AttachmentStorageKind,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub external_uri: Option<String>,
    pub provider_id: Option<ProviderId>,
    pub provider_file_id: Option<String>,
    pub sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub metadata: AttachmentMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewConversationEntryBatchItem {
    pub entry: NewConversationEntry,
    pub attachments: Vec<NewAttachment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppendedConversationEntryBatch {
    pub entries: Vec<ConversationEntryRecord>,
    pub attachments: Vec<AttachmentRecord>,
}
```

- `NewAttachment.id`不提供default/Option/compat constructor；编译器必须列出所有producer。
- batch output的attachments按entry顺序、同一Message内按content index排序；不是SQL created_at的偶然顺序。
- input/output只属于workspace-internal Rust API，不增加serde tag或database column。

## L-102：Low-level insert contract

`insert_attachment_with_conn`目标变化只有ID authority：

```rust
let row = SqlNewAttachmentRow {
    id: input.id,
    // existing mapping unchanged
};
```

既有`insert_attachment`与composer/create/send helpers继续复用它。Duplicate ID由SQLite PK返回错误；batch在SQL前还要给出deterministic invariant error。

## DB-101：Preflight validation

Public method：

```rust
pub fn append_conversation_entries_with_attachments(
    &self,
    items: Vec<NewConversationEntryBatchItem>,
) -> Result<ConversationCommit<AppendedConversationEntryBatch>>;
```

进入transaction后、第一条INSERT前完成：

1. batch非空。
2. first item提供authority conversation/run/step；run/step必须Some。
3. 每个item与authority的conversation/run/step完全相同。
4. 用existing row helpers读取authority run/step：run必须存在且`run.conversation_id == conversation_id`；step必须存在、`step.agent_run_id == run_id`且`step.status == Completed`。
5. payload只允许Assistant Message或Reasoning；Reasoning attachments必须为空。
6. Message content只允许non-empty Text与Image；空Text允许被agent projection省略后不出现。
7. 所有NewAttachment必须是Image、GeneratedFile、同conversation，ID非空且batch唯一。
8. 对每个Message按content遍历：Image ID必须存在于该item提供map、kind匹配且只出现一次。
9. 每个provided ID必须被content消费恰好一次；跨item引用、unprovided/existing reference、unused attachment全部失败。

Validation error不推进entry seq、conversation recency，也不插入任何row。

## DB-102：Transaction and return order

```text
immediate_transaction
  → DB-101 validate all items
  → for item in batch order
       → for image reference in content order
            insert provided attachment
       → append_conversation_entry_with_conn
  → conversation_commit_with_conn(last conversation, ordered output)
```

- attachment rows在引用它们的entry之前插入；foreign keys与payload linkage同一transaction提交。
- `append_conversation_entry_with_conn`是seq/recency唯一writer；不要复制其SQL。
- 第二张attachment、第二个entry或final conversation read任一失败会回滚本批全部attachment/entry与bookkeeping。
- 一个batch只返回一个`ConversationIndexDelta::EntryAdvanced`，其seq/time来自最后一个entry后的authoritative conversation。
- commit阶段错误仍按`DbError`返回；跨资源结果探测与文件补偿属于root G-01/agent owner。

## L-103：Generated attachment index

```rust
pub fn generated_file_attachments(&self) -> Result<Vec<AttachmentRecord>>;
```

- query只过滤DB label `generated_file`，稳定`.order((conversation_id.asc(), id.asc()))`。
- 返回完整typed records；不访问filesystem、不删除row、不解析path。
- app必须再检查kind/source/path/managed containment；异常record由app保留文件并报告，DB不自我修复。

## Compatibility and rollback

| Surface | Result |
| --- | --- |
| SQL/schema/migration | no change；existing DB rows load unchanged |
| NewAttachment Rust constructors | coordinated source break；DB tests在WP-101更新，app constructors在WP-301更新 |
| user composer/create/send | same auto-append ordering and transaction semantics |
| direct insert tests/helpers | caller uses`new_id()`；record ID equals input ID |
| provider batch rollback | new API only；removing implementation leaves existing APIs/data intact |
| generated index | read-only；rollback has no data effect |

## Owner requirements

| R-ID | Requirement |
| --- | --- |
| `R-101` | Every attachment insert persists the exact caller-assigned ID. |
| `R-102` | Existing composer/create/send helpers preserve their current part-append behavior. |
| `R-103` | DB-101 rejects every missing/foreign/non-Completed run/step, cross-conversation graph, duplicate, unused, unprovided, wrong-role, wrong-kind or wrong-order graph before writes. |
| `R-104` | DB-102 atomically persists ordered entries/attachments and returns authoritative records/order/index delta. |
| `R-105` | Mid-batch failures preserve prior seq/recency and leave zero batch rows. |
| `R-106` | Generated-file index is deterministic, typed and read-only. |
| `R-107` | Schema/version/models/core and `crates/jaco-db/Cargo.toml` remain byte-for-byte unchanged；WP-101 makes no lock edit. |

## WP-101：Deliver stable IDs and prelinked batch

**Prerequisites**

- Root `C-03`、`DB-01`、`D-06`–`D-08` frozen。

**Implementation sequence**

1. Add `id` to`NewAttachment` and updateDB-local constructors/tests first。
2. Change low-levelinsert to usecaller ID while preserving all other mappings。
3. Add L-101batch records and DB-101 pure/preflight validation。
4. ImplementDB-102 by reusingexisting immediate transaction andappend helper。
5. Add generated-file indexL-103。
6. Run focused tests and confirm unchanged schema/model/manifest paths。

**Exit criteria**

- R-101–R-107 pass；agent/app owners can consume the new compile-time contract without guessing。

## Validation

| T-ID | Test |
| --- | --- |
| `T-101` | `insert_attachment` returns exactly supplied ID；duplicate ID fails. |
| `T-102` | Existing user helper still appends `[Text, Image, File]` in current order. |
| `T-103` | Batch `[Reasoning, Message(Text/Image/Text)]` returns ordered entries/attachments and correct seq/recency. |
| `T-104` | Entry run/step + Image part + attachment form root C-03 lineage after timeline reload. |
| `T-105` | Empty/mixed conversation/run/step/role/kind，missing/foreign/Queued/Running/Failed/Canceled step，duplicate/unprovided/unused/cross-item IDs all roll back. |
| `T-106` | Inject second attachment/entry failure；zero rows and unchanged conversation bookkeeping. |
| `T-107` | Generated index includes onlyGeneratedFile and uses deterministic order. |
| `T-108` | Schema registry/version/SQL/models snapshots unchanged. |

Focused commands during implementation：

```text
cargo test -p jaco-db attachments
cargo test -p jaco-db agent
cargo test -p jaco-db catalog
```

Root `T-09/T-10` owns aggregate and remote gates。

## Implementer/Auditor reread

- [ ] No attachment ID is generated inside repository code after L-102.
- [ ] Existing auto-append helper was retained and tested; provider batch did not silently replace it.
- [ ] Validation completes before the first insert and checks the full batch graph.
- [ ] Batch code reuses append/commit helpers and does not duplicate recency SQL.
- [ ] Return ordering is constructed explicitly, not inferred from a later unordered query.
- [ ] Generated index performs no filesystem mutation or path trust decision.
- [ ] `migrations.rs`、`schema.rs`、`models/**`、DB manifest/core have no diff；WP-101 itself made no lock edit.
