# jaco-core：Issue #189 usage 与 context domain contract

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)
- Owner directory：`crates/jaco-core`
- Owner status：`In progress`（`WP-101`、`WP-102` 均已 `Implemented`；root-level workspace/known-provider/CI gates待做）
- 消费 root IDs：`C-01`–`C-03`、`C-11`、`C-12`、`D-01`、`D-03`、`D-04`、`D-08`、`D-11`、`D-12`、`D-18`、`D-22`、`R-01`–`R-08`、`R-21`、`R-23`、`R-27`
- Assigned WP：`WP-101`、`WP-102`
- Owns：usage coverage/cache纯函数、消息 projection、context capability snapshot、latest context request fact、Conversation collection/change/effect/transition
- Does not own：DB association/selection、provider discovery mapping、agent publication、GPUI、Settings aggregate或#194 manual editor

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
- `cargo test -p jaco-core`：31 passed；`cargo fmt` 与 selected-package combined strict clippy 通过。
- workspace-wide `cargo build`、`cargo test`、`cargo clippy`、known/provider 场景与三平台 CI 未执行。
- `ProviderUsageSnapshot` persisted usage JSON、schema 与 Cargo files 未修改；context capability 由已实施的 `WP-102` 负责。

## Composer extension — `WP-102`（Implemented）

本节只登记 [composer 执行文档](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md) 的 core owner contract；上面的 `WP-101` 实施证据保持不变。

### Owner-local 文件与边界

```text
crates/jaco-core/src/
├── payloads/capabilities.rs          # F-111 [Modify] ContextWindowCapabilitySnapshot
├── payloads/resources.rs             # F-112 [Modify] optional ModelCapabilitiesSnapshot.context_window
├── capabilities.rs                   # F-114 [Modify] conservative default is unknown
└── domain.rs                         # F-113 [Modify] latest fact/change/effect/transition
```

- 不修改 ID aliases、`ProviderUsageSnapshot` persisted representation、Cargo files 或数据库类型。
- 不把 percentage、current composer choice、provider/model label 或 unknown reason放进core。
- `CapabilitySourceSnapshot` 继续作为唯一 provenance enum，不新增平行 source contract。

### `L-111`：Context-window capability

按 root `C-11` 增加带 `NonZeroU64` tokens 与 `CapabilitySourceSnapshot` source 的 snapshot，并在 `ModelCapabilitiesSnapshot` 增加 serde-default optional field。

Owner invariants：

1. unknown 只有 `None`；0 不可构造为 known。
2. 旧 JSON 缺字段可读，unknown 重写时省略字段。
3. discovered 与 `Manual` provenance 的 positive fixture 走同一个 serde/run-settings path。
4. `conservative_model_capabilities` 显式写入 `context_window: None`；全部struct-literal fixtures同步unknown。
5. `ConversationSettingsSnapshot` / `RunSettingsSnapshot` 通过现有 `model_capabilities` 字段携带值；不新增 parallel field。

### `L-112`：Conversation singular fact

按 root `C-12` 增加 `ConversationContextRequestUsage` 与 `Conversation.latest_context_request_usage`，以及对应 change/effect。

- fact 保存 run/step/provider/model identity、step seq、step/run completed time 与 optional normalized usage。
- same step 幂等 replace；不同 step 按 `(provider_step_completed_at, agent_run_completed_at, provider_step_seq, provider_step_id)` 只接受更新值。
- 受现有Transition固定output约束，accepted、duplicate与ignored late change都返回context effect；late change不修改state，app通过同步当前fact并等值去重避免notify。
- 更新全部 `Conversation` constructors/fixtures，禁止从 `agent_message_request_usages` 推导该字段。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-111` | `model_capabilities_old_json_defaults_context_window_to_unknown` |
| `T-112` | `context_window_discovered_and_manual_snapshots_round_trip` |
| `T-113` | `context_request_usage_change_replaces_same_step_idempotently` |
| `T-114` | `context_request_usage_change_ignores_older_step_and_returns_replay_safe_effect` |

```sh
cargo fmt
cargo test -p jaco-core context_window
cargo test -p jaco-core context_request_usage
git diff --check
```

完成条件：root `C-11`/`C-12` 的 core 部分、`T-111`–`T-114` 通过，旧 capability JSON 与 WP-101 usage tests 无回归。

## Composer 实施证据（2026-08-20）

- `WP-102` 已 `Implemented`；`cargo test -p jaco-core`：31 passed，覆盖旧 capability JSON、discovered/Manual round trip 与 context request transition。
- `cargo fmt` 与 `cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings` 通过；workspace-wide build/test/clippy、known/provider 场景与三平台 CI 未执行。
- implementation commit/PR：`Pending`。
