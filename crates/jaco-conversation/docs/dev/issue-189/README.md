# jaco-conversation：hydrate Issue #189 usage 与 context facts

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)
- Owner directory：`crates/jaco-conversation`
- Owner status：`In progress`（`WP-301`、`WP-302` 均已 `Implemented`；root-level workspace/known-provider/CI gates待做）
- 消费 root IDs：`C-02`、`C-12`、`D-01`、`D-02`、`D-18`、`ST-01`、`ST-11`、`R-08`、`R-27`
- Assigned WP：`WP-301`、`WP-302`
- Owns：把DB已构造的typed collection与singular fact放入 `Conversation`
- Does not own：association、selection、query、coverage、event publication、GPUI或任何重新计算

## 证据与决定

- `E-301`：`ConversationService::load` 调用 `conversation_from_records`，当前逐字段把 `ConversationTimelineRecords` 转为 `Conversation`。
- `D-301`：新增collection原样move；service不根据runs/entries/steps再次join。
- `D-302`：load error继续只有 `ConversationError::Database`；missing usage由projection内Option表示。

## 文件与 contract

```text
crates/jaco-conversation/
├── src/lib.rs                         # F-301 [Modify] hydrate collection + service test
└── docs/dev/
    ├── README.md                      # F-302 [Add] owner index
    └── issue-189/README.md            # F-303 [Add] 本计划
```

`conversation_from_records` 的目标增加：

```rust
Conversation {
    // existing mappings unchanged
    agent_message_request_usages: records.agent_message_request_usages,
}
```

不新增public service method、error、Operation、dependency、async task或cache。

## WP-301：完成 hydration

1. 更新 `conversation_from_records`。
2. 更新empty/fixture `Conversation` constructors。
3. 增加service reopen test：构造一个final assistant message + completed step + usage，断言load结果的projection与DB timeline record完全相等。
4. 增加missing usage event fixture，断言projection保留且 `usage == None`。

| T-ID | Proposed test |
| --- | --- |
| `T-301` | `load_hydrates_agent_message_request_usage_without_reassociation` |
| `T-302` | `load_preserves_missing_usage_event_as_none` |

### Focused validation

```sh
cargo fmt
cargo test -p jaco-conversation
git diff --check
```

完成条件：`F-301` 与 `T-301`–`T-302` 通过，service没有第二套association/cache/error contract。

## 实施证据（2026-08-20）

- `conversation_from_records` 已原样 move DB 生成的 `agent_message_request_usages`，没有增加第二套 association、cache 或 error contract。
- `cargo test -p jaco-conversation`：4 passed；`cargo fmt` 与 selected-package combined strict clippy 通过。
- workspace-wide `cargo build`、`cargo test`、`cargo clippy`、known/provider 场景与三平台 CI 未执行。

## Composer extension — `WP-302`（Implemented）

本节只登记 [composer 执行文档](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md) 的 service hydration contract。

### `L-311`：Singular fact hydration

`crates/jaco-conversation/src/lib.rs::conversation_from_records` 增加：

```rust
Conversation {
    // existing mappings unchanged
    latest_context_request_usage: records.latest_context_request_usage,
}
```

约束：

- 原样move DB optional fact；不根据runs/steps/usage events再次select或assemble。
- missing usage保留为 `Some(fact { usage: None })`，不得降为 `None`。
- load error继续复用现有database error path；不增加composer专用error/cache/operation/task。
- 更新所有empty/fixture `Conversation` constructors。

| T-ID | Owner test |
| --- | --- |
| `T-311` | `load_hydrates_latest_context_request_usage_without_reselection` |
| `T-312` | `load_preserves_latest_context_request_with_missing_usage` |

```sh
cargo fmt
cargo test -p jaco-conversation composer_context
git diff --check
```

完成条件：`L-311` 与 `T-311`–`T-312` 通过，service 无第二套 selection、association 或缓存。

## Composer 实施证据（2026-08-20）

- `WP-302` 已 `Implemented`；`cargo test -p jaco-conversation`：4 passed，singular fact hydration 与 missing usage 保留回归通过。
- `cargo fmt` 与 `cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings` 通过；workspace-wide gates、known/provider 场景与三平台 CI 未执行。
- implementation commit/PR：`Pending`。
