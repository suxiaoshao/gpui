# jaco-core：Agent message request usage domain contract

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)
- Owner directory：`crates/jaco-core`
- Owner status：`In progress`
- 消费 root IDs：`C-01`–`C-03`、`D-01`、`D-03`、`D-04`、`D-08`、`R-01`–`R-08`
- Assigned WP：`WP-101`
- Owns：usage coverage/cache纯函数、消息 projection、Conversation collection/change/effect/transition
- Does not own：DB association、agent publication、GPUI、composer context、Settings aggregate

## Owner-local 证据与决定

- `E-101`：`src/payloads/capabilities.rs::ProviderUsageSnapshot` 已保存六个 normalized token fields与opaque metadata。
- `E-102`：`src/domain.rs::Conversation`、`ConversationChange`、`ConversationEffect` 是 reload/live 共同的 domain authority与transition contract。
- `D-101`：coverage/cache rate是无状态纯函数，不新增 persisted字段或第二个 usage type。
- `D-102`：`AgentMessageRequestUsage` 是非序列化 domain projection；唯一 producer为jaco-db。

## 文件与 ownership tree

```text
crates/jaco-core/
├── src/
│   ├── payloads/capabilities.rs       # F-101 [Modify] C-01 coverage/cache rate
│   └── domain.rs                      # F-102 [Modify] C-02/C-03 projection、collection、change/effect/transition
└── docs/dev/
    ├── README.md                      # F-103 [Add] owner index
    └── issue-189/README.md            # F-104 [Add] 本计划
```

不修改 `Cargo.toml`、ID aliases、provider capability、run settings、context-window类型或serialization。

## Owner-local contracts

### L-101：Provider usage classification

`F-101` 按 root `C-01` 增加：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUsageCoverage {
    Unreported,
    Partial,
    Reported,
}

impl ProviderUsageSnapshot {
    pub fn coverage(&self) -> ProviderUsageCoverage;
    pub fn cache_hit_rate(&self, provider_kind: &str) -> Option<f64>;
}
```

- coverage只读六个 numeric字段，不读metadata。
- cache denominator/provider allowlist、checked sum、zero/unknown与不clamp规则完全遵循root `C-01`。
- 不修改 `ProviderUsageSnapshot` 字段、derive或serde表示。

### L-102：Message request projection

`F-102` 增加root `C-02` 的exact struct，并给 `Conversation` 增加 `agent_message_request_usages`。

- projection字段公开，便于DB producer与app consumer；不提供从松散parts构造的public helper，association只能由DB owner执行。
- `Conversation` clone/partial equality自然包含collection。

### L-103：Conversation transition

`F-102` 增加root `C-03` variants。

- change按 `conversation_entry_id + provider_step_id` 查找并replace，否则push。
- sort key由当前 `Conversation.entries` 的 seq派生；entry缺失时以 entry ID deterministic fallback，禁止按timestamp关联。
- effect返回 `agent_run_id`，使app只重测一个row。

## WP-101：实现 core contract

1. 在 `F-101` 增加 `L-101` 与table-driven tests。
2. 在 `F-102` 增加 `L-102`/`L-103`。
3. 更新现有 `Conversation` test fixtures的空collection。
4. 增加两个不同消息的upsert/order、same-key replace与sibling不变测试。

### Tests

| T-ID | Proposed test |
| --- | --- |
| `T-101` | `usage_snapshot_classifies_all_zero_as_unreported` |
| `T-102` | `usage_snapshot_classifies_detail_without_total_as_partial` |
| `T-103` | `cache_hit_rate_uses_inclusive_input_provider_denominator` |
| `T-104` | `cache_hit_rate_uses_anthropic_total_input_denominator` |
| `T-105` | `cache_hit_rate_is_unknown_for_zero_unsupported_and_overflow` |
| `T-106` | `request_usage_change_upserts_only_matching_message_and_preserves_order` |

### Focused validation

```sh
cargo fmt
cargo test -p jaco-core usage
git diff --check
```

完成条件：root `C-01`–`C-03` 的core部分与 `T-101`–`T-106` 全部通过，无serde/schema/context能力改动。

## 实施证据（2026-08-20）

- 已修改 `src/payloads/capabilities.rs` 与 `src/domain.rs`：coverage/cache rate、typed projection、Conversation collection/change/effect/transition及回归测试已落地。
- `cargo test -p jaco-core --no-fail-fast`：26 passed。
- workspace `cargo build`、`cargo test`、strict clippy、`cargo fmt` 与 `git diff --check` 通过。
- 未修改 persisted usage JSON、serde representation、schema、context capability 或 Cargo files。
