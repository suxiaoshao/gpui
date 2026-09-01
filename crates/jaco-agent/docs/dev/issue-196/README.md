# jaco-agent：Issue #196 OpenRouter assistant image ingestion

## Root hub and ownership

- Plan ID：`issue-196`
- Root status：`Implemented locally`；动画首帧校验与root `T-11`基准已完成，发布仍受`RG-01/T-07/T-10`约束
- Root hub：[Issue #196 root plan](../../../../../docs/dev/issue-196/README.md)
- Owner：`crates/jaco-agent`
- Owner index：[jaco-agent 开发计划](../README.md)
- Assigned WPs：`WP-201`、`WP-202`
- Root contracts consumed：`D-01`–`D-13`、`C-01`–`C-04`、`DB-01`、`G-01`、`ST-01`、`ERR-01`–`ERR-05`、`DEP-01`–`DEP-02`、`RG-01`、`R-17`、`T-11`
- Owner-local IDs：`E/D/F/L/ST/ERR/DEP/R/T/WP-2xx`
- Owns：OpenRouter capability/request policy、locked Rig capture、bounded materialization、DB port、runtime publication/error/history tests
- Does not own：SQLite implementation/schema、ambient app data dir、startup orphan sweep、GPUI UI、credential UI或live gate decision

## Owner implementation result（2026-09-01）

- `WP-201/WP-202` 已实现；runtime `70/70`、generated `8/8`、artifacts `11/11`、providers `49/49` 通过。
- 动画摄取已移除GIF/WebP后续帧像素解码，改为分配前校验逻辑画布并只解码首帧；`cargo test -p jaco-agent artifacts --locked`为`11/11`。首帧有效但后续帧损坏的GIF/WebP现在按root `D-13`持久化，损坏可能到展示阶段才暴露。
- Root `T-11` release基准在2048×2048/1000帧样本上记录：23,039-byte GIF从全帧平均5.006s降至4ms，230,044-byte WebP从7.978s降至7–9ms，3,877,039-byte全画布GIF从9.037s降至5–6ms；CPU/RSS未测。
- 本轮修订后的`cargo test -p jaco-agent artifacts --locked`（`11/11`）与`cargo clippy -p jaco-agent --all-targets --all-features --locked -- -D warnings`通过；上一代码状态的package check也已通过。
- 生产 HTTPS downloader 的真实 redirect/status/timeout/body 联调未执行；root 继续把它与真实 OpenRouter E2E 作为发布前缺口记录。

## Owner evidence

| E-ID | Current fact | Evidence | Consequence |
| --- | --- | --- | --- |
| `E-201` | OpenRouter model response只解析input_modalities；capability mapper不设置image_generation；plain`Vec`不能接受explicit null | `src/providers.rs::{OpenRouterModelArchitecture,fetch_openrouter_models}`、`providers/capabilities.rs` | C-01必须扩展query/nullable shape/mapping |
| `E-202` | runtime只合并reasoning与provider tools；current merge返回Option且同key后值覆盖；stream selection直接读取capabilities.streaming | `src/runtime.rs:294-310`、`runtime/reasoning.rs::merge_additional_params` | existing merge后增加Result-returning generated-key guard与effective transport helper，不修改snapshot |
| `E-203` | non-stream wrapper保留局部provider step，inner completion成功后finish step；finish会清`last_provider_step_id`，再把response交给Rig | `src/persistence/model.rs::CompletionModel::completion`、`persistence/provider_step.rs::finish_provider_step_with_continuation` | artifact capture放在finish之后并显式携带局部step ID，不能重读current slot |
| `E-204` | hook按content逐项持久化Text/Reasoning，其他variant包括Image被忽略 | `src/persistence/tool_hook.rs::persist_assistant_content` | generated mode guard避免双写；normal mode保留 |
| `E-205` | locked Rig将legacy OpenRouterdata URI/URL转成markedAssistantContent::Image，并不在request history回放 | `rig-core-0.42.0/providers/openrouter/completion.rs` | 只消费normalized marker/source，不建raw parser |
| `E-206` | locked streaming delta没有images | `rig-core-0.42.0/providers/openai/completion/streaming.rs` | image mode必须non-stream，Unknown不是fallback |
| `E-207` | `AgentPersistence`只有单entry append；Direct adapter覆盖runtime tests | `src/persistence/port.rs` | 增加root C-03同形async method并实现Direct |
| `E-208` | `PersistenceContext`持有run/conversation/provider/model/settings/cancel/events/final_entry | `src/persistence.rs`、`persistence/conversation_entries.rs` | artifact orchestration与pending failure留在同一authority |
| `E-209` | current execution会先分支max-steps/cancel，PromptError映射`prompt_error`，outer error映射`runtime_error` | `src/runtime.rs` execution/finalization branches | stable artifact error需要统一terminal classifier与first-wins pending payload |
| `E-210` | Assistant Message history始终构造`RigMessage::assistant(content_text)`，image-only变成空text | `src/runtime/history.rs::conversation_entry_to_rig_message_with_options` | empty assistant必须返回None，mixed只回放text |
| `E-211` | agent已有base64/reqwest/sha2/url；没有image decoder，Tokio未直接启用fs/io-util/net/rt | `Cargo.toml` | 只执行DEP-01/02 |
| `E-212` | provider step response snapshot当前保存完整`response.raw` | `src/persistence/provider_step.rs::finish_provider_step_with_continuation` | generated image locator/bytes必须在写DB前redact/fallback summary |

## Owner-local decisions

| D-ID | Decision | Root authority / evidence | Consequence |
| --- | --- | --- | --- |
| `D-201` | OpenRouter models URL显式加`output_modalities=text,image`；output字段使用nullable array并按root known/unknown规则映射 | root `D-03/C-01`；`E-201` | 普通text目录与image models同时可见，null/未来值不破坏整个fetch |
| `D-202` | 新pure helper产生modalities/effective streaming；existing reasoning/tools merge后，再用可失败的checked helper插入generated keys | root `D-04/C-01`；`E-202` | request snapshot与real request同源，duplicate modalities不能被覆盖 |
| `D-203` | `ManagedArtifactStore`接收app计算的concrete conversation directory；不读dirs/path globals、不持有DB | root `D-05/D-08`；`E-208` | 文件policy与runtime persistence仍可独立测试 |
| `D-204` | generated-mode response由completion wrapper在step Completed后用局部保存的显式step ID单次投影；hook只guard该mode，tool hooks仍保持 | root `D-05/C-04`；`E-203/E-204` | 不恢复/重读current step；normal provider/stream/tool路径零结构迁移 |
| `D-205` | provider step先Completed并写safe response snapshot；统一terminal classifier按cancel > pending artifact > max-steps > prompt/runtime选择结果 | root `D-05/D-09`；`E-203/E-209/E-212` | provider success、local failure可分别审计，artifact identity不被外层覆盖 |
| `D-206` | response projection按source order形成Reasoning entries与maximal Text/Image Assistant Message runs；有Image+ToolCall立即失败 | root `C-04/R-14` | 不重排tool protocol，Text/Image/Text保持同entry |
| `D-207` | all-image preparation完成并rename后才调用一个C-03batch；返回commit驱动event/step/final ID | root `D-06/D-08`；`E-207/E-208` | partial DB rows与非authoritative publication不可发生 |
| `D-208` | batch error使用conversation timeline读回本批attachment IDs；只有证明无row时才立即清final和写fallback | root `G-01` | commit uncertainty不造成broken DB reference或duplicate text |
| `D-209` | downloader使用explicit public-IP/DNS-pin policy；实际编码文件大小、magic与逻辑画布为authority，decoder只解码首帧 | root `C-02/D-10/D-13` | no raw URL trust、header-only validation或全帧预解码 |
| `D-210` | provider raw有images时必须redact exact slots；shape不可信时降级为safe summary | root `D-10`；`E-212` | SQLite不重复存base64/remote locator |
| `D-211` | image-only history返回None，mixed用non-empty text；不加载attachment bytes进入assistant history | root `D-11/R-13`；`E-205/E-210` | follow-up符合OpenRouter response-only contract |
| `D-212` | limits为production constant；tests可注入更小limits和fake resolver/transport，但production policy不可替换 | root `C-02` | deterministic boundary tests且无public security bypass |

## File and ownership tree

```text
crates/jaco-agent/
├── Cargo.toml                              # F-201 [Modify] DEP-01/DEP-02 only
├── src/
│   ├── lib.rs                              # F-202 [Modify] export ManagedArtifactStore
│   ├── artifacts.rs                        # F-203 [Add] source policy, downloader, decoder, staged files, tests
│   ├── providers.rs                        # F-204 [Modify] OpenRouter query/output modalities + fixtures
│   ├── providers/capabilities.rs           # F-205 [Modify] output capability mapping/tests
│   ├── runtime.rs                          # F-206 [Modify] request/stream mode, store injection, pending failure consumption
│   ├── runtime/history.rs                  # F-207 [Modify] assistant image history skip/tests
│   ├── runtime/tests.rs                    # F-208 [Modify] completion/non-stream/error/cancel/publication integration
│   ├── persistence.rs                      # F-209 [Modify] store/mode/pending failure context
│   ├── persistence/model.rs                # F-210 [Modify] post-step generated response capture
│   ├── persistence/port.rs                 # F-211 [Modify] batch port + Direct adapter
│   ├── persistence/provider_step.rs        # F-212 [Modify] safe response-body override/redaction tests
│   ├── persistence/conversation_entries.rs # F-213 [Modify] batch commit projection/publication
│   └── persistence/tool_hook.rs             # F-214 [Modify] generated-mode guard/regression tests
└── docs/dev/
    ├── README.md                           # F-215 [Modify] owner index
    └── issue-196/README.md                 # F-216 [Add] this plan
```

Explicit unchanged：

```text
crates/jaco-agent/src/{mcp/**,providers/openai/**,tools/**,skills.rs}
crates/jaco-core/**
Cargo.toml (workspace root)
```

`Cargo.lock`由`F-201/DEP-201`产生一处预期metadata变化：现有`jaco-agent.dependencies`列表增加`"image"`；不得出现新package或其他version/source/checksum/dependency edge变化。

## L-201：Capability and effective transport

Target wire shape：

```rust
struct OpenRouterModelArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    output_modalities: Option<Vec<String>>,
}
```

`fetch_openrouter_models` uses URL query builder, not string concatenation：

```text
/models?output_modalities=text,image
```

Mapping：

- missing/null/empty，或trim后没有任何known `text`/`image`：keep conservative `text_output=true,image_generation=false`。
- 至少有一个known value：unknown values被忽略，`text_output = contains_trimmed_ascii_ci("text")`，`image_generation = contains_trimmed_ascii_ci("image")`。
- reserialized normalized model metadata必须包含该nullable output field；never infer from model name。Tests覆盖missing/null/empty/unknown-only/text+unknown/image+unknown。

Runtime helpers：

```rust
fn openrouter_output_params(settings: &RunSettingsSnapshot) -> Option<Value>;
fn uses_generated_image_completion(settings: &RunSettingsSnapshot) -> bool;
fn merge_generated_output_params(
    existing: Option<Value>,
    generated: Option<Value>,
) -> Result<Option<Value>, AgentRuntimeError>;
```

- generated predicate严格等于provider kind OpenRouter + image_generation。
- params为`{"modalities":["image","text"]}`；`text_output=false`时`["image"]`。
- runtime先调用existing`merge_additional_params`取得reasoning/provider-tools baseline，再调用`merge_generated_output_params`；generated=None原样返回baseline。
- checked helper要求existing/generated若存在都为JSON object，并拒绝existing已包含generated的任一top-level key（首版即`modalities`）；在provider request/step创建前返回`AgentRuntimeError::Invariant`，不得last-write-wins。它不改变existing reasoning/tools彼此的当前merge语义。
- `use_streaming = capabilities.streaming && !generated_mode`；stored capability不被mutate。

## L-202：Managed artifact types

Public construction boundary：

```rust
#[derive(Clone)]
pub struct ManagedArtifactStore {
    conversation_id: ConversationId,
    conversation_dir: Arc<PathBuf>,
    limits: GeneratedImageLimits,
}

impl ManagedArtifactStore {
    pub fn new(conversation_id: ConversationId, conversation_dir: PathBuf) -> Self;
}
```

Production limits exactly mirrorroot C-02。Only `#[cfg(test)]` constructors may inject limits、DNS answers或body source；no public “allow private URL” flag。

Private working set：

```rust
struct GeneratedImageCandidate {
    ordinal: usize,
    source: GeneratedImageSource, // Base64 or Url only
    declared_mime: Option<String>,
}

struct PreparedGeneratedImage {
    ordinal: usize,
    attachment: NewAttachment,
    final_path: PathBuf,
}

struct PreparedGeneratedArtifacts {
    images: Vec<PreparedGeneratedImage>,
    armed: bool,
}
```

- `PreparedGeneratedArtifacts`显式async rollback；Drop只做best-effort same-path cleanup safeguard并禁止panic。
- success only after DB commit calls`disarm()`。
- attachment fields：Image/GeneratedFile、canonical MIME、safe generated display name、same final path in`path`andmetadata source、provider_id、sha256、size、width/height；external/provider_file/duration/preview均None。

## L-203：Source classification and response projection

Image marker predicate读取locked Rig `additional_params.wire_extras("openrouter")`并要求：

```text
response_only == true
source == "assistant.images"
```

Projection algorithm：

1. Enumerate full `CompletionResponse.choice` once。
2. Count markedimages before any side effect；reject any unmarkedImage/source variant。
3. If at least oneImage and anyToolCall exists，ERR-01；fallback projection retainsText/Reasoning only。
4. Reasoning emits one`NewConversationEntry` at its source position。
5. Consecutive non-emptyText/Image items form oneAssistant Message；Text/Image/Text stays oneordered content vec。
6. ToolCall with noImage flushes aText run and remains owned byexisting tool hook。
7. Everyentry usescurrent run + explicit completedprovider step ID；Image parts usepreallocated IDs。
8. LastAssistant Message becomesfinal_entry_id。

If generated mode returns noImage，the same projection persistsText/Reasoning and returnsresponse to Rig so ordinarytool processing continues。Generated hook guard prevents double persistence。

## L-204：Base64, HTTPS and image validation

### Base64

- preflight encoded length with checked arithmetic against25 MiB decoded max。
- decode usingstandard padded/unpadded acceptance already supported bybase64 engine only if complete input is valid；no whitespace stripping。
- stream/write to`.pending/{id}.part` withcreate_new；validated magic决定final extension，never form a data URI or log payload。

### HTTPS URL

Production downloader：

1. Parse with`url::Url`; enforce root C-02 syntax/port/userinfo/fragment policy。
2. Resolve using`tokio::net::lookup_host`; pass all addresses through private`is_public_artifact_ip` covering IPv4/IPv6 loopback/private/link-local/multicast/unspecified/documentation/benchmark/reserved/mapped ranges。
3. Reject if zero or any forbidden address；build reqwest client with`no_proxy()`、redirect none、no cookies/default auth and`resolve_to_addrs`pinning。
4. Send`Accept-Encoding: identity`; accept2xx only；processredirect manually最多3次并 repeat policy。
5. EnforceContent-Length if present，then`Response::chunk()`counter whilewritingstaged file；30s overall applies to thehop/download。

### Decode and metadata

- use`ImageReader::with_guessed_format` + explicitallowlist + `Limits`；先读取并校验逻辑画布尺寸/像素，再在current awaited blocking task中只解码首帧并计算SHA。
- canonical format frommagic determinesextension/MIME；declared MIME/Content-Type, when present, must map tosame format。
- first-frame decode must succeed；header/decoded dimensions必须一致，dimension和pixel math使用checked multiplication。GIF/WebP首帧之后的损坏不在摄取阶段拒绝。
- SHA-256 andsize derive from exact staged bytes that will be renamed。
- final rename target uses theapp-reserved`.jaco-generated-{attachment_id}.{ext}`grammar；display name remains`generated-image-{ordinal}.{ext}`，andunprefixed UUID/composer/legacy files areoutsidecleanup authority。
- before/after creation，reject symlink/non-directory at the immediate attachments root (`conversation_dir.parent()`)、conversation dir and`.pending`dir；also rejectpreexisting final or nonregular staged/final target。

## L-205 / ST-201：Completion capture and failure handoff

`AgentRuntime` target addition：

```rust
artifact_store: Option<ManagedArtifactStore>

pub fn with_managed_artifact_store(mut self, store: ManagedArtifactStore) -> Self;
```

Generated mode without matching conversation store is setup failure beforeprovider request。`PersistenceContext` receivesgenerated_mode/store and：

```rust
pending_run_failure: Arc<Mutex<Option<RunErrorPayload>>>
```

`set_pending_run_failure`只在slot为None时写入；`pending_run_failure`可clone读取，只有terminal run commit成功后清除。所有terminal分支必须经同一classifier，不能由某个PromptError分支提前take。

`PersistingCompletionModel::completion` generated success branch：

```text
save explicit_provider_step_id = provider_step.id
  → inner completion
  → prepare safe/redacted provider response snapshot
  → finish_provider_step_with_response_body(explicit_provider_step_id, response, safe_response_body)
  → persist_generated_completion(explicit_provider_step_id, response.choice)
  → return original response to Rig
```

`persist_generated_completion`与所有fallback batch builder都要求显式`ProviderStepId`参数；`finish_provider_step`清理current slot后禁止调用`current_provider_step_id`、恢复slot或让普通`append_item`隐式补lineage。DB-101再次验证该step已Completed且属于current run。

Artifact error branch：

1. G-01 cleanup/probe。
2. If commit absence is proven andDB writes still work，appendfallback Text/Reasoning batch。
3. Setpending run error fromERR mapping。
4. Returncompletion error only ascontrol flow；outer runtime usespending payload instead of`prompt_error`。
5. Do not fail/downgradealready Completedprovider step。

Cancel checks occur before DNS/request/chunk/write/rename/DB and immediately after awaited stages。BeforeDB commit it rolls backfiles and returnsCanceled；after knowncommit it preservesoutput and outer run becomesCanceled。

Terminal classifier固定为：

```text
if cancellation observed             → Canceled (pending artifact payload不落Error)
else if pending_run_failure is Some → Failed(exact pending payload)
else if max_steps                   → MaxSteps
else                                → existing prompt/runtime result
```

该分类发生在任何active-tool terminal update和`finish_execution`之前；inner PromptError与outer runtime error都调用同一classifier。后续tool/run finalization自身失败只作为secondary runtime diagnostic，不能把已选artifact code改成`prompt_error`/`runtime_error`；Completed provider step不进入active-step fail/cancel更新。

## L-206：Commit, publication and uncertainty

Port addition mirrorsroot C-03 exactly：

```rust
async fn append_conversation_entries_with_attachments(
    &self,
    items: Vec<NewConversationEntryBatchItem>,
) -> jaco_db::Result<ConversationCommit<AppendedConversationEntryBatch>>;
```

On success：

- disarmcleanup only after methodreturns commit。
- for each committedentry in order，emit itsreferenced AttachmentUpserted values first，thenEntryAppended。
- call existing`emit_conversation_commit_with_changes(&commit, changes)` exactly once，产生携带authoritative conversation summary的`ConversationCommitted`；push oneAgentStep perentry and matchingProviderStep OutputItemCompleted events。`ConversationTimelineChanged`仍只用于provider-step等无conversation commit的变化。
- use committedrecords to setfinal_entry_id；never reuseprecommit IDs asauthoritative event payloads。

On error：

- call existing`conversation_timeline(conversation_id)` and intersect returnedattachments withbatch IDs。
- none present：deleteall finals；fallback allowed。
- all present：keepall，no fallback/public success claim；next reload recoversfacts。
- partial：keep referenced/delete absent，setERR-05 + invariant diagnostic。
- timeline read fails：keepall，setERR-05；app startup reconciliation decides。

## L-207：Safe provider snapshot

Target API stays inside`PersistenceContext`：

```rust
pub(super) async fn finish_provider_step(
    &self,
    provider_step_id: &str,
    response: &CompletionResponse,
) -> Result<()>;

pub(super) async fn finish_provider_step_with_response_body(
    &self,
    provider_step_id: &str,
    response: &CompletionResponse,
    response_body: ProviderRawPayload,
) -> Result<()>;

async fn finish_provider_step_with_options(
    &self,
    provider_step_id: &str,
    response: &CompletionResponse,
    continuation: Option<ProviderContinuationSnapshot>,
    response_body_override: Option<ProviderRawPayload>,
) -> Result<()>;
```

- ordinary Chat path calls`finish_provider_step`，which delegates with`continuation=None,response_body_override=None` and retains existing raw behavior。
- OpenAI continuation path delegates to`finish_provider_step_with_options` with`continuation=Some(...),response_body_override=None`。
- generated OpenRouter path builds a safe`ProviderRawPayload` first and must call`finish_provider_step_with_response_body`；that wrapper delegates with`continuation=None,response_body_override=Some(safe)`。
- options implementation usesoverride verbatim whenSome，otherwise constructs the existing Rig raw payload。No new`PersistenceContext` field or constructor parameter is used for response-body override。

Generated safe payload replaces every proven`image_url.url` with a fixedmarker。

Ifraw structure、image count或slot mapping cannot be proven，persist only：

```json
{
  "providerResponse": "redacted_generated_images",
  "imageCount": 1
}
```

No original raw subtree is retained in that fallback。Tests cover all three call routes and search serialized generated provider step fordata URI payload、test URL、query and image bytes；ordinary/OpenAI snapshots remain regression-tested。

## L-208：History

Assistant Message conversion target：

```rust
let text = content_text(content);
(!text.is_empty()).then(|| RigMessage::assistant(text))
```

No attachment lookup/read occurs forAssistant entries。System/Developer/User/Tool and reasoning/tool protocol options remainunchanged。Tests coverimage-only None、mixed text-only replay andfollowing user prompt construction。

## Dependencies

| ID | Manifest edit | Source/version/features | Local use | Verification |
| --- | --- | --- | --- | --- |
| `DEP-201` | add`image` runtime dependency | exact`0.25.10`, default-features=false, gif/jpeg/png/webp | L-204 magic/header/first-frame decode/limits/dimensions | cargo tree shows sameexisting registry package，no duplicate/resolution change |
| `DEP-202` | expandexisting`tokio` features | exact`1.53.1`; addfs/io-util/net/rt | asyncstage I/O、DNS、blocking decode task | agent focused build/tests onall targets; no secondruntime |

`DEP-201`必然让`Cargo.lock`现有`jaco-agent.dependencies`增加`"image"`；`image 0.25.10` package/source/checksum与其解析依赖保持。`DEP-202`不应产生额外lock文本变化。`reqwest/base64/sha2/url/rig` declarations remain。No generated/copied docs、skills、submodule或native package synchronization。

## Owner error mapping

| Local error family | Root code | Required safe behavior |
| --- | --- | --- |
| marker/source/URL policy/MIME/magic/decode/tool-shape | `ERR-01` | retryable false，no locator/raw |
| count/byte/pixel/allocation | `ERR-02` | retryable false，no actual secret data |
| DNS/timeout/redirect/status/body | `ERR-03` | retryable true，no host/body |
| directory/stage/sync/rename | `ERR-04` | retryable true，stage + ErrorKind only |
| batch/probe/partial/unknown | `ERR-05` | retryable true，provider step remainsCompleted |

Cleanup failure is aninternal warning added to theprimary category；it does not replace theuser-visible error or claim successful deletion。

## Compatibility and rollback

| Surface | Result |
| --- | --- |
| ordinary runtime | generated predicate false follows byte-for-byte equivalent params/stream/hook paths |
| OpenRouter capability | missingoutput field preserves old conservative behavior；newfield is existing snapshot bool only |
| Rig | exact0.42.0 retained；nofork/parser/copy/upgrade |
| provider raw | only responses containinggenerated images are redacted；ordinary raw unchanged |
| AgentRuntime constructor | existing`new(persistence)` remains；builder addsoptional store |
| history | intentional fix forimage-only assistant; text/tool/reasoning behaviors preserved |
| rollback | stored core/DB artifacts remain readable/displayable；removingagent feature stops new ingestion |

## Owner requirements

| R-ID | Requirement |
| --- | --- |
| `R-201` | Models query，nullable/unknown mapping andgenerated effective transport followL-201 exactly. |
| `R-202` | Only locked Rig marked Base64/URL assistant images enterL-203/L-204. |
| `R-203` | L-204 enforces every rootC-02 network/encoded-size/header/first-frame decode rule beforefinal files become durable，并明确不保证后续动画帧完整。 |
| `R-204` | Provider step completes withsafe raw beforelocal ingestion；projection uses its explicit ID and artifact failure retainsCompleted state. |
| `R-205` | Generated projection, batch, `ConversationCommitted` publication/summary andfinal_entry followL-203/L-206. |
| `R-206` | G-01 error/cancel/uncertain outcomes leave no known broken reference orduplicate fallback. |
| `R-207` | Terminal priority is cancellation > stableERR > max-steps > prompt/runtime；stableERR preservesText/Reasoning whenDB permits. |
| `R-208` | Normal stream/text/reasoning/tool/provider behavior isunchanged. |
| `R-209` | History never replaysgenerated images or emptyassistant content. |
| `R-210` | OnlyDEP-201/202 change manifest；versions/Rig/TLS stay，lock only adds existing`image` edge to`jaco-agent`. |

## WP-201：Capability, request and runtime capture

**Prerequisites**

- `WP-101` target records/port types available。
- Root C-01/C-04/ERR catalog frozen。

**Sequence**

1. ImplementL-201 model discovery andpure transport helpers。
2. ExtendPersistenceContext withgenerated mode/store/pending failure。
3. Addbatch port totrait/Direct adapter and`ConversationCommitted` publication path。
4. Movegenerated-mode persistence intocompletion wrapper afterprovider-step completion，threading explicit local step ID；guardhook。
5. Addsafe raw override andsingle terminal classifier/pending-error priority。
6. Addcapability/request/runtime regression tests。

**Exit**

- R-201/R-204/R-205/R-207/R-208 pass without network/filesystem implementation shortcuts。

## WP-202：Artifact materialization, cleanup and history

**Prerequisites**

- `WP-101/201` complete；DEP-201/202 approved byroot plan。

**Sequence**

1. Applymanifest changes and confirm the only lock text change is`jaco-agent.dependencies += image` before code。
2. AddL-202/L-203 pure classification/projection。
3. ImplementL-204 bounded Base64/HTTPS/decode/staged pipeline。
4. ImplementG-01 cleanup guard andL-206 uncertainty probe/fallback。
5. ImplementL-208 history rule。
6. Runnegative security/failure/cancel tests，then owner aggregate tests。

**Exit**

- R-202/R-203/R-206/R-209/R-210 pass；app owner can injectstore and runreconciliation。

## Validation

| T-ID | Required test |
| --- | --- |
| `T-201` | models URLquery；text/image/image-only/missing/null/empty/unknown-only/known+unknown output mapping；normalized metadata roundtrip. |
| `T-202` | modalities afterexisting reasoning/tools merge；generated/nonobject/duplicate-key invariant；generated completion-only；normalstream called exactly once. |
| `T-203` | Rig markedBase64/URL classification；all unsupported markers/source variants andImage+ToolCall. |
| `T-204` | Base64 encoded-file/aggregate limits；validPNG/JPEG/GIF/WebP；invalid first frame/mismatch/SVG/HEIC/pixel bomb；动画GIF/WebP画布限制及首帧有效、后续帧损坏按D-13接受。 |
| `T-205` | URL syntax/IP/DNS mix/redirect/timeout/status/content-length/chunk overflow/no proxy/auth with test-only transport. |
| `T-206` | symlink/preexisting target/write/sync/rename failures cleanstage/final and createzeroDB rows. |
| `T-207` | success uses explicitCompleted step ID，createsexactmetadata/hash/app-reserved`.jaco-generated-{attachment_id}.{ext}`path/order/lineage，and emits one authoritative`ConversationCommitted` withsummary/changes/final_entry. |
| `T-208` | batchfail probe none/all/partial/unavailable drivesexactcleanup/fallback behavior. |
| `T-209` | ordinary/OpenAI/generated finish API routes；provider stepCompleted + runFailed + safeERR/Text/Reasoning；pending beatsmax-steps/prompt/outer-runtime，generated raw contains noartifact locator/data. |
| `T-210` | cancel duringdownload/write/beforeDB/afterDB and cancel+pending/max-steps races；cancel wins，no detachedwork. |
| `T-211` | image-only/mixed assistant history andnext prompt; normaltool protocol regression. |
| `T-212` | cargo tree/metadata provesonlyDEP-201/202；lock diff only adds`"image"` toexisting`jaco-agent` dependency list，no package/version/source/checksum change. |

Focused commands during implementation：

```text
cargo test -p jaco-agent providers
cargo test -p jaco-agent artifacts
cargo test -p jaco-agent runtime
cargo check -p jaco-agent --all-targets --all-features
```

Root ownsfinal fmt/workspace/clippy/live/CI gates。

## Implementer/Auditor reread

- [ ] No raw JSON or stream Unknown is used to discover images.
- [ ] Generated hook guard is exact; normal hook, tool and streaming behavior is regression-tested.
- [ ] Generated params use the checked Result-returning merge after existing reasoning/tools merge；nonobject/duplicate modalities fail invariant.
- [ ] Provider step is Completed before local ingestion and never rewritten on artifact failure.
- [ ] The explicit wrapper-local provider step ID reaches every generated/fallback entry；current step slot is never reread after completion.
- [ ] All URL hops repeat no-proxy/DNS-pin/public-IP policy and body is bounded while streaming.
- [x] Header + first-frame pixel decode、actual MIME、encoded size、dimensions、hash与final bytes都描述同一staged file；后续动画帧完整性明确留给展示解码器；`max_alloc`只作为decoder output budget，不宣称覆盖codec内部总峰值RSS。
- [ ] Files are disarmed only after authoritative DB commit; uncertainty never triggers blind deletion or duplicate fallback.
- [ ] Safe raw snapshot and ERR payload contain no URL/base64/path/body/secret.
- [ ] Generated completion calls the explicit response-body override API；ordinary and OpenAI continuation routes pass no override and preserve behavior.
- [ ] Image-only history returnsNone and mixed replays onlytext.
- [ ] One`ConversationCommitted` carries authoritative summary plus attachment/entry changes；artifact terminal priority matchesL-205 in inner and outer error paths.
- [ ] Dedicated`/images`、otherprovider/artifact kinds、new UI/config are absent.
- [ ] Manifest diff is exactlyDEP-201/202；lock only adds the existing`image` edge and resolution is unchanged.
