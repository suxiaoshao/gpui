# jaco-agent：Rig 0.42 与兼容依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（Rig 0.42 本地自动化通过；RMCP 2↔3 E2E 与三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)
- Owner directory：`crates/jaco-agent`
- Root-owned surfaces consumed：`S-08`、`S-09`、`S-10`、`S-17`、`S-19`
- Owner-local IDs：`F-JA-01`–`F-JA-08`、`R-JA-01`–`R-JA-06`、`T-JA-01`–`T-JA-05`、`WP-JA-01`–`WP-JA-03`
- Owns：Rig adapter、hook/persistence、history/tool conversion、OpenAI websocket adapter、RMCP 2 client 和本 crate 的 registry dependencies。
- Does not own：root workspace version declarations/lockfile、Jaco UI、database schema，或独立 RMCP 3 test server 的 implementation。

Rig `0.41.0 -> 0.42.0` 是本 owner 的 breaking migration。主 workspace 的 RMCP **不升级到 3.x**：root
将其精确约束为 `=2.2.0`，以与 Rig 0.42 的 `rmcp = "2"` feature 保持同一套 Rust types。

## 精确依赖目标

### Updated targets

| Dependency | Current | Target | Preserved features/kind | Classification | Principal local surface |
| --- | --- | --- | --- | --- | --- |
| `rig` | workspace `0.41.0` | workspace `0.42.0` | root: `agent`, `reqwest`, `rmcp`, `rustls`, `websocket`; dev adds `test-utils` | Breaking | runtime, hooks, history, tools, persistence, OpenAI adapters |
| `async-trait` | `0.1.91` | `0.1.92` | default | Compatible | MCP/persistence async traits |
| `base64` | `0.23.0` | `0.23.1` | `default-features = false`, `std` | Compatible | history attachments |
| `futures` | `0.3.33` | `0.3.34` | default | Compatible | runtime streaming/tasks |
| `globset` | `0.4.19` | `0.4.20` | default | Compatible | builtin search |
| `http` | `1.4.2` | `1.5.0` | default | Compatible | MCP header parsing |
| `ignore` | `0.4.31` | `0.4.33` | default | Compatible | filesystem/search walk |
| `similar` | `3.1.1` | `3.2.0` | default | Behavior migration | default Myers 输出变化；显式 `RawMyers` 保持 tool diff |
| `thiserror` | `2.0.19` | `2.0.20` | default | Compatible | `AgentRuntimeError` and MCP errors |
| `time` | `0.3.54` | `0.3.55` | `formatting`, `parsing`, `serde` | Compatible | persistence timestamps/continuations |

### Explicit pin and retained targets

| Dependency | Issue #205 target | Features/kind | Reason |
| --- | --- | --- | --- |
| `rmcp` | root exact pin `=2.2.0` | `auth`, `client`, `macros`, `transport-child-process`, `transport-streamable-http-client-reqwest` | Rig 0.42 remains on RMCP 2 types; reject 3.x in main graph |
| `async-stream` | `0.3.6` | runtime | no candidate in refreshed snapshot |
| `dirs` | `6.0.0` | runtime | retained |
| `grep-matcher` | `0.1.9` | runtime | retained |
| `grep-regex` | `0.1.14` | runtime | retained |
| `grep-searcher` | `0.1.17` | runtime | retained |
| `hex` | `0.4.3` | runtime | retained |
| `reqwest` | `0.13.4` | `json` | retained |
| `serde` | `1.0.229` | `derive` | retained |
| `serde_json` | `1.0.151` | runtime | retained |
| `sha2` | `0.11.0` | runtime | retained |
| `tokio` | `1.53.1` | runtime: `process`, `sync`, `time`; dev: `macros`, `rt`, `sync`, `time` | retained |
| `tokio-util` | `0.7.19` | runtime | retained |
| `tracing` | `0.1.44` | runtime | retained |
| `url` | `2.5.8` | runtime | retained |
| `tempfile` | `3.27.0` | dev | retained |

`jaco-core`、`jaco-db`、`gpui-operation` 继续使用 workspace path declarations。本 owner 不添加 direct
RMCP 3、provider SDK 或第二个 HTTP/TLS runtime。

## Breaking migration map

| Rig 0.42 change | Current local use | Required owner edit | Requirement |
| --- | --- | --- | --- |
| content collections move from `OneOrMany<T>` to `Vec<T>` | `src/tools.rs:8,368`; `src/runtime/history.rs:12,148-170,218-224`; `src/providers/openai/websocket.rs:8,832-847`; tests | remove `OneOrMany` imports/construction/return types; preserve explicit non-empty validation only where Jaco has a domain invariant; do not add a compatibility wrapper | `R-JA-01` |
| hook events add model-turn/event identity | `src/persistence/tool_hook.rs:568-991` | adapt `AgentHook` method signatures and associate completion/tool events with the same provider step; do not infer identity from arrival order when the new ID is available | `R-JA-02` |
| assistant content wire representation becomes tagged | request snapshot serialization at `src/persistence/provider_step.rs:42-53`; content matching in `tool_hook.rs`/`history.rs` | update Rig-facing serde expectations while keeping Jaco `ConversationEntryPayload` and provider snapshot schema stable; add old-record/current-record fixture coverage | `R-JA-03` |
| normalized raw/response identity and model-turn finish | `src/error.rs`; `src/persistence/{model,provider_step,tool_hook}.rs`; `src/runtime.rs`; `src/providers/openai/websocket.rs` | retain the local public-event decoder, route its raw choices through Rig `normalize_stream`/`StreamingCompletionResponse`, then delete private response-id probes and duplicate completion side channels in favor of identity/raw/`ModelTurnFinished` | `R-JA-04` |
| Rig RMCP feature still requires RMCP 2 | `src/mcp.rs`; `src/mcp/connector.rs` | keep direct/root RMCP at `=2.2.0`, compile one RMCP type universe, and validate protocol interoperability with the independent RMCP 3 server | `R-JA-05` |

## Owner-local 目标与文件

```text
crates/jaco-agent/
├── Cargo.toml                                      # F-JA-01 [Modify] compatible direct targets; rig/rmcp stay workspace-owned
├── src/tools.rs + src/runtime/history.rs           # F-JA-02 [Modify] Vec content/tool/history conversion
├── src/runtime.rs + src/runtime/**/*.rs            # F-JA-03 [Modify/verify] stream/non-stream runtime and tests
├── src/persistence/tool_hook.rs                     # F-JA-04 [Modify] Rig 0.42 hook events and identity
├── src/persistence/{model,provider_step}.rs         # F-JA-05 [Modify/verify] completion snapshots and response identity
├── src/providers/openai.rs                          # F-JA-06 [Verify; modify if required] provider construction/capabilities
├── src/providers/openai/websocket.rs                # F-JA-07 [Modify] Vec history, errors, response continuation
└── src/mcp.rs + src/mcp/connector.rs                # F-JA-08 [Verify] pinned RMCP 2 auth/transports/tools
```

- `R-JA-01`：history/tool conversion keeps ordering, rejects domain-invalid empty user content, and preserves tool-call/result pairing.
- `R-JA-02`：streaming/non-streaming hook events persist each assistant/tool output exactly once against the correct provider step/model turn.
- `R-JA-03`：Jaco-owned DB/domain payload schema remains stable; existing request/response snapshots remain readable or receive an explicit tested boundary conversion, never a database schema rewrite hidden in this WP.
- `R-JA-04`：OpenAI response ID/reasoning continuation, previous-response fallback, cancellation and retryable provider-error classification retain current behavior. Exact 0.42 WebSocket `session.completion()` does not populate `raw`, so blocking and streaming both use the retained `send + next_event` decoder followed by public `RawStreamingChoice -> normalize_stream -> StreamingCompletionResponse`; do not lose reasoning context or copy Rig private accumulators.
- `R-JA-05`：main workspace resolves exactly one `rmcp 2.2.0`; no RMCP 3 package appears in the Jaco graph.
- `R-JA-06`：compatible search/filesystem/time/base64 updates do not alter path filtering, attachment encoding, diff text or timestamps；filesystem unified diff 显式选择 `Algorithm::RawMyers` 并覆盖复杂/重复行 snapshot。

## Owner-local Work Packages

### WP-JA-01：更新 compatible direct dependencies

1. 在 `F-JA-01` 写入表中的九个 owner-local compatible target；保留全部 features 和 retained entries。
2. 运行 search/filesystem/history focused tests，确认 `R-JA-06`；不把 leaf update failure与 Rig migration混为 fallback。
3. 由 root owner 使用 Cargo 更新 workspace lockfile。

完成条件：manifest 精确匹配 target table，leaf APIs 无行为偏差。

### WP-JA-02：迁移 Rig 0.42 runtime、hook 与 persistence

1. root 先将 Rig 更新到 `0.42.0`，然后按 migration map 修改 `F-JA-02`–`F-JA-07`；删除所有生产代码中的 `rig::OneOrMany` 残留。
2. 将 hook 的新 identity 贯穿 completion call/response、tool call/result、stream finish 到现有 provider-step persistence；保持 cancellation、approval 与 max-tool guard 的原子性。
3. 以 Jaco domain/persistence types 为稳定边界，更新 Rig 0.42 tagged content 的转换和 fixtures；不得把 Rig wire enum 直接提升为新的 DB schema。
4. 保留本地 WebSocket event decoder，将其输出迁到 0.42 公开 `RawStreamingChoice`，并经公开
   `normalize_stream` 与 `StreamingCompletionResponse::stream` 收束；terminal raw 由前者捕获，tool fragment 的
   internal correlation ID 由后者的共享 accumulator mint。不要调用丢 `raw` 的 0.42 unary helper，也不要复制
   私有 `RawChoiceAccumulator` / `ResponsesAdapter`。
5. 上述公开链路通过 raw/tool/reasoning parity 后，使用 Rig 0.42 `identity/raw/ModelTurnFinished`，删除
   `__jaco_response_id` 注入与 probe、blocking/streaming 两套 attempt-complete side channel、runtime
   `final_raw_response`、stream-finish hook 和本地 `tool_call_internal_ids` minting。若 parity 不成立，停止这些
   删除并记录 blocker，不保留半迁移的双 commit path；transport failure 必须 evict failed session。
6. `ResponseIncomplete` 按 finish reason 保存 partial output/usage，`Unknown` frame 进入 raw audit 但不进入
   assistant aggregate。
7. 更新 inline mocks/tests 的 Vec response/content construction，分别覆盖 streaming、non-streaming、tool-bearing turn、tool-free final turn、cancel/error 和 persistence failure。
8. 将 filesystem diff 配置为 `RawMyers`，补复杂/重复行 unified-diff snapshot；不在依赖升级中启用新的
   `TextMerge`/`WhitespaceMode` 产品行为。

完成条件：`R-JA-01`–`R-JA-04` 全部有测试证据，源码无旧 Rig API 残留，既有 Jaco public/domain/schema contract 不变。

### WP-JA-03：固定 RMCP 2 并验证边界

1. root manifest 将 RMCP 精确约束为 `=2.2.0`；`F-JA-01` 继续 `rmcp.workspace = true`，`F-JA-08` 使用同一 RMCP 2 types。
2. 检查 dependency graph：Rig 0.42 RMCP feature 与 direct RMCP 合并为单一 2.2.0 resolution；出现 RMCP 3 或第二个 RMCP 2 version 即失败。
3. 在独立 RMCP 3.1.4 test server 完成后，用 Jaco RMCP 2 client 运行 handshake、OAuth discovery/token/refresh、tool listing/call mixed-version scenario；server implementation 仍由 tool owner 负责。

完成条件：`R-JA-05` 成立，unit/integration tests 与 mixed-version scenario 通过；不要为了消费测试服务而把主 workspace 升至 RMCP 3。

## Focused Validation 与 handoff

| T-ID | Command/scenario | Expected evidence |
| --- | --- | --- |
| `T-JA-01` | `cargo test -p jaco-agent --all-features --locked` | runtime、hooks、persistence、history、tools、MCP 与 OpenAI adapter tests 通过 |
| `T-JA-02` | `cargo clippy -p jaco-agent --all-targets --all-features --locked -- -D warnings` | production/mocks 无 warning 或旧 API workaround |
| `T-JA-03` | `cargo tree -p jaco-agent -i rmcp@2.2.0 -e features --locked` plus `cargo tree -p jaco-agent --duplicates --locked` | 唯一 RMCP 2.2.0；Rig/direct features 合流；无 RMCP 3 |
| `T-JA-04` | residual scan for `OneOrMany`, `__jaco_response_id`, `tool_call_internal_ids`, `final_raw_response`, old finish hook/signatures and obsolete error-field probes | live WebSocket event decoder/pool 保留并走 public normalize chain；旧 identity/internal-id/finish side channel 为零 |
| `T-JA-05` | Jaco RMCP 2 client ↔ standalone RMCP 3.1.4 server mixed-version scenario | initialize handshake、OAuth discovery/authorization/token/refresh、`tools/list`、`echo` call 均成功；token 不写入日志 |

`app/jaco` consumer regression、standalone server commands、workspace aggregate gates 和最终 lockfile evidence 由各自 owner/root 记录。
