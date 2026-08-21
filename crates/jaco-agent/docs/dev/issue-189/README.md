# jaco-agent：Issue #189 provider discovery、cost finalization 与 live publication

## 根计划与 owner 边界

- Plan ID：`issue-189`
- Root hub：[Issue #189](../../../../../docs/dev/issue-189/README.md)
- 执行文档：[Agent 消息单次请求用量](../../../../../docs/dev/issue-189/agent-message-request-usage-plan.md)、[Composer context occupancy](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md)、[Settings 请求费用统计](../../../../../docs/dev/issue-189/settings-usage-cost-analytics-plan.md)
- Owner directory：`crates/jaco-agent`
- Owner status：`Implemented`（`WP-401`、`WP-402`、`WP-403`均已实施）
- 消费 root IDs：既有IDs，以及`C-81`–`C-83`、`R-81`–`R-88`
- Assigned WP：`WP-401`、`WP-402`、`WP-403`
- Owns：既有publication/capability discovery；models.dev exact model pricing与现有usage completion计价接线
- Does not own：DB schema/aggregation、core arithmetic、provider-reported/raw response、GPUI、Settings呈现、本地手工迁移或#194 manual editor

## 证据与决定

- `E-401`：provider step完成时只知道step/usage，final entry可能尚未确定。
- `E-402`：`PersistenceContext::finish_run` 与 `AgentRuntime::finish_agent_run_with_observer` 都从 `finish_agent_run` commit构造 `ConversationCommitted.changes`。
- `E-403`：Jaco runtime已通过FIFO publication task消费 `ConversationCommitted`，无需新event/channel。
- `D-401`：两个finalization producer都使用 `FinishedAgentRun.request_usage`，不得从context中重建。
- `D-402`：ordered changes固定为RunStatusChanged、可选EntryAppended、可选AgentMessageRequestUsageChanged。
- `D-403`：ProviderStepChanged保持原样；不提前发布request usage。

## 文件与 ownership tree

```text
crates/jaco-agent/
├── src/
│   ├── persistence.rs                 # F-401 [Modify] PersistenceContext finalization publication
│   ├── runtime.rs                     # F-402 [Modify] AgentRuntime finalization publication
│   └── runtime/tests.rs               # F-403 [Modify] observer content/order/live-reload tests
└── docs/dev/
    ├── README.md                      # F-404 [Modify] owner index
    └── issue-189/README.md            # F-405 [Add] 本计划
```

不修改 `AgentRuntimeEvent` enum、`AgentPersistence` trait signature、provider completion mapping、Rig adapters、Tasks、channels或shutdown。

## Boundary implementation

### L-401：PersistenceContext finalization changes

`PersistenceContext::finish_run` 在DB commit成功后、`FinishCommitted` consume value前，clone可选 `request_usage` 并构造：

```rust
let mut changes = vec![ConversationChange::RunStatusChanged {
    run: Box::new(commit.value.run.clone()),
}];
if commit.value.appended_final_entry {
    changes.push(ConversationChange::EntryAppended {
        entry: Box::new(commit.value.final_entry.clone()),
    });
}
if let Some(request_usage) = commit.value.request_usage.clone() {
    changes.push(ConversationChange::AgentMessageRequestUsageChanged {
        request_usage: Box::new(request_usage),
    });
}
```

然后沿现有 `emit_conversation_commit_with_changes` 发布。Commit失败无event；observer缺失时持久化仍成功，reload恢复。

### L-402：AgentRuntime finalization changes

`AgentRuntime::finish_agent_run_with_observer` 使用与 `L-401` 完全相同的change builder/order。为避免两处漂移，新增crate-private纯helper：

```rust
fn finished_agent_run_changes(finished: &FinishedAgentRun) -> Vec<ConversationChange>;
```

`L-401`/`L-402` 都调用该helper；helper不query、不记录、不发布，只clone DB authoritative values。

### ST-401：Live publication

- **Authority：** DB `FinishedAgentRun.request_usage`
- **Owner/lifetime：** 当前run finalization call；不存入agent field
- **Publication：** 既有 `AgentRuntimeEvent::ConversationCommitted`
- **Ordering：** root `C-03`
- **Failure：** DB失败无change；observer/channel lifecycle沿用现有规则
- **Cancellation：** canceled/failed run只有DB返回eligible projection时才发布；agent不按run status过滤或补造
- **Recovery：** startup/no-observer path依赖下一次Conversation reload

## WP-401：完成 live publication

1. 增加 `L-402` helper并让两条finalization路径调用。
2. 删除两处重复的手写changes构造，避免后续variant遗漏。
3. 扩展observer fixture覆盖normal final entry、tool-loop final step、missing usage、final status/error、no observer。
4. 断言provider-step completion event不含request usage，run finalization commit才包含。
5. 断言change order与DB projection identity/content。

| T-ID | Proposed test |
| --- | --- |
| `T-401` | `finished_agent_run_changes_publish_request_usage_after_run_and_entry` |
| `T-402` | `tool_loop_publishes_only_final_entry_request_usage` |
| `T-403` | `provider_step_completion_does_not_publish_message_request_usage_early` |
| `T-404` | `final_error_or_status_without_step_publishes_no_request_usage` |
| `T-405` | `missing_usage_event_publishes_unavailable_projection` |
| `T-406` | `no_observer_finalization_relies_on_reload_without_changing_persistence` |

### Focused validation

```sh
cargo fmt
cargo test -p jaco-agent request_usage
cargo test -p jaco-agent runtime
git diff --check
```

完成条件：`L-401`–`L-402`、`ST-401` 与 `T-401`–`T-406` 通过，无新event、query、Task或provider adapter逻辑。

## 实施证据（2026-08-20）

- 两条 finalization 路径已统一调用 `finished_agent_run_changes`，按 run、可选 entry、可选 request usage 的顺序发布 DB authoritative projection；provider-step completion未提前绑定。
- `cargo test -p jaco-agent run_finalization_publishes_request_usage_after_run_status`：1 passed；`cargo test -p jaco-agent`：128 passed。
- `cargo fmt` 与 selected-package combined strict clippy 通过；workspace-wide `cargo build`、`cargo test`、`cargo clippy`、known/provider 场景与三平台 CI 未执行；未新增 event、query、Task、channel 或 provider adapter logic。

## Composer extension — `WP-402`（Implemented）

本节登记 [composer 执行文档](../../../../../docs/dev/issue-189/composer-context-occupancy-plan.md) 的 provider discovery 与 live publication contract。

### Owner-local 文件与边界

```text
crates/jaco-agent/src/
├── providers.rs                      # F-411 [Modify] response字段解析/传递与fixtures
├── providers/capabilities.rs         # F-414 [Modify] typed capability mappings/tests
├── providers/openai.rs               # F-415 [Modify] capability fixture fallout only
├── persistence.rs                    # F-412 [Modify] ordered composer change publication
└── runtime/{lifecycle.rs,reasoning.rs,tests.rs}
                                        # F-413 [Modify] capability fixtures + lifecycle/reload tests
```

- 不新增通用或family-wide静态model默认表、heuristic fallback、API request或依赖。
- 不从provider name、非exact model ID或家族前缀推断context window。
- manual provenance只由core/db fixture覆盖；agent不激活unfinished editor或CRUD。

### `L-411`：Authoritative discovery mapping

`providers.rs` 只解析并把response已有字段传入 `providers/capabilities.rs`；后者是 `ModelCapabilitiesSnapshot` 的唯一mapping owner，只接受权威正整数：

| Source | Field | Provenance |
| --- | --- | --- |
| Rig `Model` | `context_length` | `ApiDiscovered` / `rig model listing` |
| [Gemini Models API](https://ai.google.dev/api/models) | `inputTokenLimit` | `ApiDiscovered` / `/v1beta/models` |
| [OpenRouter Models API](https://openrouter.ai/docs/api/api-reference/models/get-models) | `context_length` | `OpenRouterNormalized` |
| [Ollama Show API](https://docs.ollama.com/api-reference/show-model-details) | `details.context_length` or `model_info.*.context_length` | `ApiDiscovered` / `/api/show` |
| [OpenAI Models 文档](https://developers.openai.com/api/docs/models) | exact GPT-5.6 model ID，listing缺失且默认/显式 `api.openai.com[/v1]` 端点 | `OfficialDocs` / `1_050_000` |

Ollama algorithm：优先收集typed `details.context_length`，并兼容 `model_info` 的exact key或suffix `*.context_length`；过滤0/非整数后仅当distinct positive values集合大小为1才known。多个来源给同值合法，冲突值保持unknown并由fixture固定。

### `L-412`：Composer live change

扩展既有 `finished_agent_run_changes`：在run、可选entry、可选message usage之后，clone DB `FinishedAgentRun.context_request_usage` 为 `ConversationContextRequestUsageChanged`。

- provider-step completion不提前发布。
- failed/canceled/no-step或DB返回None不发布。
- partial/unreported/missing usage的 `Some(fact)` 必须发布，从而清除旧known摘要。
- 两条finalization producer继续调用同一helper；不新增event/channel/task。

### Tests 与验证

| T-ID | Owner test |
| --- | --- |
| `T-411` | `rig_gemini_and_openrouter_map_positive_context_windows_with_provenance` |
| `T-412` | `ollama_context_window_requires_one_distinct_positive_value` |
| `T-413` | `finished_agent_run_changes_publish_context_request_after_message_usage` |
| `T-414` | `partial_unreported_and_missing_usage_publish_context_request_change` |
| `T-415` | `failed_canceled_and_provider_step_completion_do_not_publish_context_change` |
| `T-416` | exact GPT-5.6 IDs在官方端点使用官方值；discovered/manual优先，compatible endpoint、非exact/provider mismatch保持unknown |

```sh
cargo fmt
cargo test -p jaco-agent context_window
cargo test -p jaco-agent composer_context
git diff --check
```

完成条件：root `C-11` discovery与`C-13` publication、`T-411`–`T-416`通过，并保持WP-401 event order tests全绿。

## Composer 实施证据（2026-08-20）

- `WP-402` 已 `Implemented`；`cargo test -p jaco-agent`：131 passed，`cargo test -p jaco-agent composer_context`：2 passed。
- capability 回归覆盖 provider discovery、invalid-value、exact OpenAI GPT-5.6 official-doc profile、official/compatible endpoint 与 discovered/manual precedence；`cargo fmt` 与 `cargo clippy -p jaco -p jaco-agent -p jaco-db --all-targets --all-features -- -D warnings` 通过。
- provider mapping自动化覆盖exact GPT-5.6 official-doc capability；app不再对旧缓存做读取时补全。workspace-wide gates、现场provider refresh/新请求与三平台 CI 未执行；implementation commit/PR：`Pending`。

## Cost extension — `WP-403`（Implemented）

本节登记[Settings 请求费用统计计划](../../../../../docs/dev/issue-189/settings-usage-cost-analytics-plan.md)的agent owner contract。message/composer publication、provider-step lifecycle和现有usage completion顺序保持不变。

### Owner-local文件与边界

```text
crates/jaco-agent/
├── Cargo.toml                     # workspace serde_json arbitrary_precision feature；无新package
└── src/
    ├── providers.rs               # Fetch Models exact price merge
    ├── providers/models_dev.rs    # [Add] fixed catalog client/parser
    └── persistence/provider_step.rs # 现有成功usage completion调用core estimator
```

最终实现如需在已有provider success helper附近放置薄adapter，可以使用当前owner模块；不得为费用新增runtime、streaming或tool-call state machine。

### `L-421`：models.dev Fetch Models

- 仅built-in official OpenAI、Anthropic、Gemini/Google、OpenRouter、DeepSeek、Mistral endpoint eligible；Ollama、custom base/custom OpenAI-compatible与manual model不请求或不附加目录价。
- 固定`https://models.dev/api.json`，无auth/query；10秒总超时、32MiB响应上限，不记录body、provider key或用户内容。
- 在workspace现有`serde_json`启用`arbitrary_precision`；价格词法直接进入core fixed-point parser，不使用`as_f64`。
- 只接受top-level provider key、payload provider ID和Jaco固定mapping一致，以及model map key、payload model ID和Fetch Models exact model ID一致的条目；无family/bare/display-name/cross-provider回退。
- 只解析base input/output/cache_read/cache_write与input threshold tiers；experimental modes、reasoning专价、账单附加项和provider-reported amount全部忽略。
- provider listing与models.dev network/decode成功后才返回priced model batch；失败沿用现有Fetch Models错误且DB零mutation。这是有意的all-or-nothing边界，避免临时catalog故障清空已有price；成功catalog没有exact价格仍是合法`pricing=None`。
- pricing snapshot携带core typed official route key、models.dev provider/model ID和fetched_at；不进入RunSettings、shortcut或conversation。
- 不新增startup、TTL、polling或background refresh。

### `L-422`：复用现有usage completion计价

- 每个当前已经持久化`ProviderUsageSnapshot`的成功completion路径，从对应`ProviderStepRecord.pricing_snapshot`读取frozen price并调用core `estimate_request_cost`。
- 结果作为`Option<UsdNanoAmount>`交给现有`CompleteProviderStep`；price missing、all-zero usage、underflow或overflow为None，不新增reason enum、审计列或日志协议。
- estimator不读取provider raw response、provider-reported cost或completion时的current catalog。
- 不移动、不增加也不删除provider-step terminalization、CompletionCall、tool hook、invalid-call、cancel/error或usage-event边界。现有路径此前没有usage event时，本计划不另行制造费用event。
- DB写失败沿用现有completion rollback/error语义；估算不可用不得让成功provider request失败。

### Tests与验证

| T-ID | Owner test |
| --- | --- |
| `T-481` | provider mapping与official/custom endpoint eligibility |
| `T-482` | provider/model三方exact identity、missing/unpriced与无family/cross-provider fallback |
| `T-483` | timeout/status/body cap/decode/invalid price使Fetch Models零mutation |
| `T-484` | arbitrary-precision decimal、base/cache/tier parse与unsupported字段忽略 |
| `T-485` | existing non-streaming success usage调用同一estimator并写amount/None |
| `T-486` | existing streaming success usage调用同一estimator；event/step顺序与旧fixture完全一致 |
| `T-487` | catalog refresh后旧step继续使用frozen price，新step使用新price |
| `T-488` | custom/unmatched/all-zero/underflow/overflow不阻断completion且cost None |
| `T-489` | tool/invalid/cancel/error现有生命周期回归；费用实现没有新增completion event |
| `T-490` | provider credentials、raw response和prompt不进入models.dev请求或日志 |

```sh
cargo fmt
cargo test -p jaco-agent pricing
cargo clippy -p jaco-agent --all-targets --all-features -- -D warnings
git diff --check -- crates/jaco-agent
```

完成条件：root `C-81`–`C-83`、agent-owned `R-81`–`R-88`与`T-481`–`T-490`通过；models.dev exact merge和现有usage completion计价完成；没有provider-reported/raw evidence、runtime state、completion lifecycle或后台刷新变化；无新package。

### Cost 实施证据（2026-08-21）

- models.dev constrained client/exact merge已接入现有Fetch Models；现有non-streaming与streaming成功completion读取step frozen price并复用core estimator。
- `cargo test -p jaco-agent pricing`：7 passed；`cargo test -p jaco-agent`：138 passed；strict clippy与scoped diff check通过。
