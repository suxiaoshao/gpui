# Issue #196：Jaco provider 生成图片持久化与展示

## 状态与范围

- 状态：`Implemented locally`；本地动画首帧校验与性能基准已完成，`Done` 仍受真实 OpenRouter legacy Chat assistant-image E2E、手工 UI 与远程 CI 发布门约束
- 关联 issue：[#196](https://github.com/suxiaoshao/gpui/issues/196)
- Parent：[#159](https://github.com/suxiaoshao/gpui/issues/159)
- Plan ID：`issue-196`
- 根索引：[Workspace development plans](../README.md)
- 分支：`codex/196-jaco-provider-generated-artifacts`
- 基线：`main` / `origin/main@eef94c958a9c02e28e3e6553a702520fe024e777`
- 受影响 owner：`crates/jaco-db`、`crates/jaco-agent`、`app/jaco`
- 明确保持无 diff：`crates/jaco-core`、`crates/jaco-conversation`、所有其他 app/crate、bundle/workflow
- 最近证据刷新：2026-09-01

### 高影响变更摘要

| 审计门 | 结果 | Canonical IDs |
| --- | --- | --- |
| Workspace/crate topology and ownership | `[Cross-owner]` DB 提供预关联批次事务，agent 负责 OpenRouter/Rig 输出摄取，Jaco 注入当前数据库的 managed directory 并执行启动清扫 | `D-05`–`D-08`、`C-02`–`C-04`、`WP-101/201/202/301` |
| Public or cross-owner contracts | `[Modify] [Breaking]` workspace-internal `NewAttachment` 改为调用方预分配 ID；新增批次 entry+attachment repository/port contract | `D-06`、`C-03`、`DB-01`、`WP-101/201/301` |
| Global/shared authority | `None`；现有 `ConversationRuntimeStore`、`AgentRuntime`、`PersistenceContext` 和 database session 继续分别持有原有状态，不新增 Store/Global/Operation | `D-05`、`ST-01`、`WP-201/301` |
| Persistence, data, configuration, or credentials | `[Modify]` 生成图片转为 `GeneratedFile` + attachment row + assistant entry；schema/version 不变，OpenRouter secret 生命周期不变 | `C-02`–`C-04`、`DB-01`、`G-01`、`ERR-01`–`ERR-05` |
| Runtime, concurrency, performance, or shutdown | `[Modify]` OpenRouter image mode 强制 completion；每个 run 内串行摄取，编码文件大小约束下载/落盘/哈希工作，画布与首帧边界约束摄取解码，复用 cancellation；启动 recovery 清理崩溃 orphan | `D-04`–`D-05`、`D-08`–`D-10`、`D-13`、`ST-01`、`WP-201/202/301` |
| Security, privacy, or external access | `[Security-sensitive]` URL 下载仅允许经 DNS 固定的公网 HTTPS；禁止 proxy/credential/隐式 redirect；raw response 中图片 bytes/URL 必须脱敏 | `D-08`–`D-10`、`C-02`、`ERR-01`–`ERR-04` |
| Dependencies, toolchains, generated, or vendored artifacts | `[Modify]` `jaco-agent` 直接复用已锁定 `image 0.25.10`，并给现有 Tokio 开启 `fs/io-util/net/rt`；`Cargo.lock` 预期只在 `jaco-agent.dependencies` 增加已有 `image` edge，package/version/source/checksum/resolution graph 不变 | `D-12`、`DEP-01`–`DEP-02`、`S-17`、`WP-202` |
| Platform, packaging, CI, or release | `[Release-gated]` 不改 packaging/CI；当前官方已迁移到 `/images`，用户确认的 legacy Chat `message.images` 必须用真实 OpenRouter 在发布前验证 | `D-02`、`RG-01`、`T-08`、`WP-001` |
| User-visible defaults or removals | `[Modify]` 被标记为 image generation 的 OpenRouter 模型自动请求图片且不流式；普通模型仍按既有 streaming capability 运行 | `D-01`–`D-04`、`C-01`、`R-01`–`R-04` |
| Breaking change / migration | `[Breaking, workspace-internal]` `NewAttachment` 构造者必须传 stable ID；没有 serde、SQL、旧数据或 mixed-version migration | `D-06`–`D-07`、`C-03`、`WP-101/301` |

## 目标

让 OpenRouter Chat Completions 在 assistant response 中返回的图片成为 Jaco 受管的持久化会话内容。图片经过来源、网络、编码文件大小、MIME、签名、画布尺寸和首帧解码校验后写入当前数据库对应的 managed attachment directory，并与 assistant entry、provider step、agent run 一起形成可重载的 lineage。现有 Issue #195 timeline renderer 直接展示这些 `GeneratedFile` 图片并提供 preview/save actions。

目标成功路径：

```text
OpenRouter model discovery
  → ModelCapabilitiesSnapshot.image_generation
  → Chat Completion modalities=[image,text] + non-stream
  → Rig 0.42 AssistantContent::Image(Base64 | Url)
  → bounded validation/download + staged file
  → SQLite batch: assistant entries + attachment rows
  → one runtime publication
  → existing ordered timeline / preview / save / restart reload
```

## 非目标

- OpenRouter 当前专用 `POST /api/v1/images`、Rig `ImageGenerationModel` 或新的 image-generation composer mode。
- Gemini/OpenAI/Anthropic/Ollama 或其他 provider 的生成图片；audio、video、document、provider-hosted file。
- MCP/local tool result image、tool-produced artifact、remote MCP resource、provider file download。
- provider response 中同时出现 `Image` 与 `ToolCall` 的混合 step；首版将其作为不支持的 response shape 失败并保留 text/reasoning。
- 将生成图片回放为下一轮 assistant image input；后续 history 只回放非空 assistant text。
- 图片编辑、mask、seed、aspect ratio、resolution、quality、format、数量或 stream controls。
- 在 SQLite 保存 base64、remote URL、provider auth header/cookie，或把这些值展示到 UI/log。
- 新 attachment schema、run/step 外键复制、backfill、旧库升级、schema version 或 migration。
- 新 timeline component、image preview、file action、icon、asset、Fluent key、Store/Global/Operation。
- 清扫用户附件、任意 data directory 文件或不符合本计划 generated filename grammar 的文件。

## 用户已确认决定

- 2026-08-31：首条 route 只覆盖 OpenRouter assistant image output；file/audio/MCP/local tool/provider-hosted artifacts 后续单独处理。
- 2026-08-31：OpenRouter 模型的 `image_generation == true` 时自动加入 image-output 参数并强制 non-stream；不增加 composer 开关；普通模型继续 streaming。
- 2026-08-31：Base64 与 URL 图片都立即固化到 Jaco managed storage；任何 decode/download/validation/write 失败都令 run 失败，保留已产生 text/error context，并清理本轮 staged file/未提交 DB records。
- 2026-08-31：计划继续采用上述 Chat assistant-image 路线；当前官方专用 `/images` 只作为已知替代方案和发布风险记录，不在本 issue 中静默替换用户选择。
- 2026-09-01：动画图片在摄取阶段只解码首帧像素；继续执行单图 25 MiB、单响应 100 MiB 的实际编码文件大小限制，并保留 magic/MIME、画布尺寸、像素、decoder output allocation 与首帧解码校验。由此产生的行为后果是：后续损坏帧可能到展示阶段才暴露。

产品范围已经封闭。安全数值、事务补偿和模块位置由当前仓库约束与本计划固定，实施者无需再选择。

## 计划映射

| Scope | 文档 | Owns | Assigned IDs/WPs |
| --- | --- | --- | --- |
| Root hub | 本文档 | 状态、范围、用户决定、S/C/DB/G/ST/ERR/DEP/RG、跨 owner 顺序与聚合验收 | `E-01`–`E-13`、`D-01`–`D-13`、`C-01`–`C-04`、`DB-01`、`G-01`、`ST-01`、`ERR-01`–`ERR-05`、`DEP-01`–`DEP-02`、`RG-01`、`R-01`–`R-17`、`T-01`–`T-11`、`WP-001` |
| `crates/jaco-db` | [owner plan](../../../crates/jaco-db/docs/dev/issue-196/README.md) | stable attachment ID、预关联 batch transaction、generated index 与 DB tests | `E/D/F/L/DB/R/T/WP-1xx` |
| `crates/jaco-agent` | [owner plan](../../../crates/jaco-agent/docs/dev/issue-196/README.md) | capability/request、Rig capture、artifact ingestion、runtime errors/history/publication | `E/D/F/L/ST/ERR/DEP/R/T/WP-2xx` |
| `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-196/README.md) | database-target managed dir 注入、DB port adapter、startup reconciliation 与 integration tests | `E/D/F/L/ST/G/R/T/WP-3xx` |

## Applicability

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定或 negative reason | Owning section/WP |
| --- | --- | --- | --- | --- | --- |
| `S-01` | Workspace, files, modules, and owner boundaries | Applicable | provider/runtime 位于 agent，DB transaction 位于 jaco-db，managed data root 位于 Jaco database target | 保持三个 owner；新增 agent `artifacts.rs` 与 app-local reconciliation module，不新增 crate | `D-05`–`D-08`、`WP-101/201/202/301` |
| `S-02` | GPUI components, layout, interaction, and accessibility | No change | #195 已按 `ContentPart` 顺序渲染 Assistant Image，并提供 preview/save | 只发布既有 Image/Attachment shape；不改 renderer、focus、actions 或 accessibility | `D-11`、`R-11`、`WP-301` |
| `S-03` | Entity, Store, Global, identity, and projections | No change | `ConversationRuntimeStore` 已持有 active run，database session 持有 target lease/executor | 不复制 artifact state；stable identity 使用预分配 attachment ID | `D-06`–`D-07`、`ST-01` |
| `S-04` | Actions, events, subscriptions, focus, and windows | Applicable | Agent observer 已发布 conversation changes，#195 renderer消费 AttachmentUpserted/EntryAppended | 一个 batch commit 发一批 authoritative changes；无新 UI action/subscription | `C-04`、`WP-201/301` |
| `S-05` | Async tasks, concurrency, cancellation, and shutdown | Applicable | run 已有 cancellation token、retained task 和 startup recovery | 同一 response 串行摄取；DNS/download/write/DB 前后检查取消；动画在 blocking task 中只解码首帧像素；不 detach；recovery 后清 orphan | `D-05`、`D-08`–`D-10`、`D-13`、`ST-01`、`WP-202/301` |
| `S-06` | Data acquisition and Operation state | Applicable, no new Operation | provider model fetch 与 runtime recovery 已有 owner | 扩展 OpenRouter models fetch；artifact cleanup 纳入既有 recovery，不创建 gpui-operation phase | `C-01`、`C-04`、`WP-201/301` |
| `S-07` | Forms and editable state | N/A | 本 issue 没有新 control/draft/form | 不改 ChatForm、settings capability form 或 gpui-form | `S-07` |
| `S-08` | Cross-crate, provider, Rig, MCP, platform, and external contracts | Applicable | locked Rig 可规范化 legacy `message.images`；官方当前主路线已转 `/images` | 锁定 legacy Chat + Rig adapter contract，真实 E2E 为 release gate；MCP/tool/platform不变 | `D-02`–`D-06`、`C-01`–`C-03`、`RG-01` |
| `S-09` | Error identity, propagation, recovery, and error UI | Applicable | 当前 hook error 会被外层覆盖为 `prompt_error` | 增加 typed pending run failure，按 ERR catalog失败；取消保持 Canceled；现有 Error entry UI显示 | `D-09`、`ERR-01`–`ERR-05`、`WP-201/202` |
| `S-10` | Database, persistence, and schema | Applicable, schema no change | attachment、entry、run、step表已足够；现有 single-entry helper 会把附件追加到末尾 | 新增预关联批次事务，保持旧 composer helper；schema/version/serde不变 | `D-06`–`D-08`、`C-03`、`DB-01`、`WP-101` |
| `S-11` | Generated, synchronized, copied, or vendored content | Applicable | managed attachment directory是Jaco-owned runtime storage；DEP-01会同步Cargo lock的direct edge | 新runtime文件遵守G-01；只接受DEP-01精确lock metadata diff，不生成/同步其他repo artifact或vendored source | `G-01`、`DEP-01`、`WP-202/301` |
| `S-12` | Icons and assets | No change | #195 已有 Image/File actions与assets | 不增 icon、SVG、runtime asset或 app icon | `D-11` |
| `S-13` | Fluent i18n and bundle localization | No change | run error已有通用 timeline 展示，图片卡片文案已存在 | stable safe error message沿用当前 runtime error contract；不新增 locale/bundle string | `ERR-01`–`ERR-05` |
| `S-14` | Security, privacy, and credentials | Applicable | URL可能命中内网，raw response可能含完整 base64/URL；OpenRouter credential由 secret store持有 | 公网 HTTPS + DNS pin + no proxy/auth/implicit redirect；raw image locator脱敏；日志不含 path/URL/raw/key | `D-08`–`D-10`、`C-02`、`G-01` |
| `S-15` | Observability and diagnostics | Applicable | 当前 runtime使用 run/step events与 tracing | 只记录 run/step/attachment/ordinal/stage/category/count；不 dump response/artifact/locator | `ERR-01`–`ERR-05`、`C-04` |
| `S-16` | Packaging, platform behavior, and CI/release | Applicable, packaging no change | image/managed attachment已有三平台路径；官方 Chat response无稳定 images schema | 现有 macOS/Linux/Windows CI + manual restart；真实 OpenRouter gate阻止 Done | `RG-01`、`T-07`–`T-10`、`WP-001` |
| `S-17` | Dependencies, frameworks, Git sources, and toolchains | Applicable | image已由 app锁定；agent已有 base64/reqwest/sha2/url/Tokio但缺直接 decoder和所需Tokio features | 只执行 DEP-01/02；lock仅增加`jaco-agent → image`已有-package edge，版本/source/checksum/resolution graph不变 | `D-12`、`DEP-01`–`DEP-02`、`WP-202` |
| `S-18` | Owner documentation, indexes, and ADRs | Applicable | repo使用 root hub + same-ID owner plans | 新增四份计划并更新四个 index；合同留在计划，无独立 ADR | `WP-001` |
| `S-19` | Validation and completion evidence | Applicable | 行为跨 provider、network、filesystem、DB、runtime与restart | focused pure/DB/runtime/app tests → workspace gates → manual reload → real API → remote CI | `T-01`–`T-10`、`WP-001` |

## Evidence registry

| E-ID | Classification | Claim | Evidence | Plan consequence |
| --- | --- | --- | --- | --- |
| `E-01` | Current requirement | Issue #196 要求 provider-generated artifact 有 durable record、timeline展示、run/step linkage、reload、MIME/size failure与cleanup | [GitHub Issue #196](https://github.com/suxiaoshao/gpui/issues/196)，2026-08-31读取 | `R-01`–`R-16` |
| `E-02` | User decision | 首版 route、自动 non-stream 与失败语义已由用户确认 | 本轮对话，2026-08-31 | `D-01`–`D-04`、`D-09` |
| `E-03` | Current fact | core已有 Image/GeneratedFile/metadata；entry已有run/step；attachment已有provider/hash/size/path | `crates/jaco-core/src/{domain.rs,payloads/foundation.rs}` | core/schema不增加 lineage字段；使用 C-03 |
| `E-04` | Current fact | `persist_assistant_content` 当前只处理 Text/Reasoning并丢弃 Image；stream parser没有 image branch | `crates/jaco-agent/src/persistence/tool_hook.rs`、`runtime.rs` | generated mode在 completion wrapper捕获 normalized response |
| `E-05` | Current fact | DB已有单entry+attachments事务，但内部生成attachment ID并把parts追加到末尾 | `crates/jaco-db/src/repository.rs`、`repository/conversations.rs` | 增加 stable-ID prelinked batch，不用现有 helper表达 provider order |
| `E-06` | Current fact | #195 已交付 ordered Assistant Image projection、GeneratedFile containment、preview/save/reload | `app/jaco/src/components/chat/detail/{attachments.rs,attachment_access.rs,message.rs}` | UI消费现有形状，零 renderer diff |
| `E-07` | Upstream fact | Models API默认只返回text models；`output_modalities=text,image`返回text和image模型，model含`architecture.output_modalities` | [OpenRouter Models documentation](https://openrouter.ai/docs/guides/overview/models)，2026-08-31 | C-01扩展fetch/query与capability mapping |
| `E-08` | Upstream fact | Chat API仍接受`modalities`/`image_config`，但当前response reference没有承诺`message.images` | [OpenRouter Chat Completions API](https://openrouter.ai/docs/api/api-reference/chat/create-a-chat-completion)，2026-08-31 | 请求合同可实现，response兼容性进入 RG-01 |
| `E-09` | Upstream fact | OpenRouter官方SDK 2.10.0已从legacy Chat `modalities + message.images`迁移到专用`/images` | [OpenRouter AI SDK provider changelog](https://github.com/OpenRouterTeam/ai-sdk-provider/blob/main/CHANGELOG.md#2100)，2026-08-31 | `/images`明确排除；legacy route必须真实验证且失败时重开决策 |
| `E-10` | Locked dependency fact | Rig 0.42 OpenRouter non-stream normalizer把data URI/URL映射为marked AssistantContent::Image，history不回放这些图片；stream adapter没有images | locked `rig-core-0.42.0/src/providers/openrouter/completion.rs`、`openai/completion/streaming.rs` | 复用normalized type，禁止依赖raw/stream提取；实现history parity |
| `E-11` | Current fact | app已有canonical managed directory、atomic-ish user attachment prep、database-target lease和interrupted-run recovery | `app/jaco/src/features/conversation/attachments.rs`、`database.rs`、`features/conversation/runtime.rs` | app注入concrete conversation dir并在既有recovery后清扫 |
| `E-12` | Release-gated | 官方没有legacy Chat output字节上限、URL有效期或`message.images`稳定保证；本轮没有真实OpenRouter credential/E2E证据 | `E-08`–`E-10`与本轮环境 | 固定应用安全限额；RG-01在Done前必须满足 |
| `E-13` | User decision + local benchmark | 用户确认移除动画全帧预解码，并询问以总文件大小作为主要资源边界 | 本轮对话，2026-09-01；本机release基准：2048×2048/1000帧、23,039-byte GIF从全帧平均5.006s降至首帧4ms；230,044-byte WebP从7.978s降至7–9ms；3,877,039-byte全画布GIF从9.037s降至5–6ms | `D-13`、`R-17`、`T-11` |

## Decisions

| D-ID | Decision | Evidence | Material rejected alternative | Consequence/owner |
| --- | --- | --- | --- | --- |
| `D-01` | 首版只接受OpenRouter response-only assistant Image，其他provider/artifact kinds保持原行为 | `E-01`–`E-04` | 同时覆盖OpenAI/Gemini/MCP/tool/file/audio | `jaco-agent` `WP-201/202` |
| `D-02` | 按用户决定继续legacy Chat Completion；专用`/images`不进入本issue，RG-01失败后必须回到计划/产品决策 | `E-02`、`E-08`–`E-10` | 实施中自行切换当前官方Image API | root `RG-01` |
| `D-03` | OpenRouter models请求加`output_modalities=text,image`；missing/null/empty/unknown-only保留conservative text默认，至少含一个known modality时只按trimmed ASCII-CI `text`/`image`严格设置能力 | `E-07` | 默认`/models`漏掉image-only模型，或未知新值把模型误标为无输出 | `jaco-agent` `WP-201` |
| `D-04` | `provider_kind=openrouter && image_generation`时追加`modalities:["image","text"]`（text_output=false时仅image）并令effective streaming=false；snapshot capability本身不改 | `E-02`、`E-08`、`E-10` | UI mode switch、修改stored streaming capability、复用stream Unknown | `C-01`、`WP-201` |
| `D-05` | generated mode由`PersistingCompletionModel::completion`处理；保留局部provider step ID，先完成step/raw/usage，再把该显式ID传给artifact batch；hook只对该mode跳过Text/Reasoning/Image持久化 | `E-04`、`E-10` | 完成step后重新读取已清空的current ID，在UI/raw JSON/stream hook中提取图片，或把provider success与local failure合成step failure | `C-02`、`ST-01`、`WP-201/202` |
| `D-06` | attachment ID在文件准备前预分配；provider response中每个连续Text/Image run组成一个ordered Assistant Message，Reasoning保持独立entry；所有entries/attachments一次DB batch | `E-03`–`E-06` | 每张图片单独entry导致final text折叠，或DB自动把图片追加到末尾 | `C-03`、`DB-01`、`WP-101/202` |
| `D-07` | lineage唯一authority为entry.agent_run_id/provider_step_id → ContentPart::Image.attachment_id → attachment；不复制run/step到attachment | `E-03` | attachment新增重复foreign keys/source of truth | core/schema零diff；`C-03` |
| `D-08` | 文件与SQLite使用G-01补偿协议；不能宣称跨资源ACID；batch error后通过timeline读回attachment IDs判定立即删除/保留，未知结果交启动reconciliation | `E-05`、`E-11` | DB先commit后写file、所有DB error盲删final、或无crash cleanup | `G-01`、`DB-01`、`WP-101/202/301` |
| `D-09` | artifact error写入typed pending run failure，外层优先消费；DB可写时先保存全部Text/Reasoning再失败；取消不写Error | `E-02`、`E-04` | 把所有失败覆盖为prompt_error，或图片失败时丢弃provider text | `ERR-01`–`ERR-05`、`WP-201/202` |
| `D-10` | 采用固定安全限制与公网HTTPS下载策略；remote URL只做一次性source，不进入record/UI/log；图片raw locator在provider snapshot中脱敏 | `E-08`、`E-10`–`E-12` | 任意URL/redirect/proxy、无限下载/解码、持久化data URI/URL | `C-02`、`G-01`、`WP-202` |
| `D-11` | 复用现有core payload、GeneratedFile UI/access/save与runtime events；history忽略assistant images且不构造空assistant message | `E-03`、`E-06`、`E-10` | 新建artifact UI/domain，或把生成图片回传provider | `WP-201/301` |
| `D-12` | agent直接依赖已锁定image decoder并开启现有Tokio features；不升级版本、不改Rig、不增加HTTP/TLS stack | `E-10`–`E-11` | app回调decode、手写格式parser、升级Rig或添加第二网络runtime | `DEP-01`–`DEP-02`、`WP-202` |
| `D-13` | GIF/WebP 摄取只解码首帧像素；传输/存储边界使用实际落盘的单图编码字节与单响应累计编码字节，并在像素分配前校验逻辑画布尺寸/像素；不保证后续动画帧完整 | `E-13` | 继续解码全部帧像素，或仅看Content-Length/扩展名而不做magic、尺寸与首帧校验 | `C-02`、`ERR-01/02`、`WP-202` |

## C-01：OpenRouter capability、request 与 transport

Authority：`crates/jaco-agent::providers` 与 `runtime`。

```text
GET /models?output_modalities=text,image
  → architecture.input_modalities + output_modalities
  → ModelCapabilitiesSnapshot
      text_output = output_modalities contains text
      image_generation = output_modalities contains image
  → selected run snapshot
  → if openrouter && image_generation:
      additional_params.modalities = [image,text] or [image]
      use_streaming = false
    else:
      existing additional params + existing streaming decision
```

- existing`merge_additional_params`继续产生reasoning/provider-tools baseline；新增`merge_generated_output_params(existing, generated) -> Result<Option<Value>, AgentRuntimeError>`只把generated `modalities`插入该object。existing非object、generated非object或existing已含`modalities`都在provider request/step创建前返回Invariant；非generated路径原值不变，禁止静默覆盖。
- `image_config`没有用户输入，本issue不发送空对象或自造默认值。
- `output_modalities`按nullable array反序列化；missing、null、empty或trim后unknown-only都保持当前`text_output=true/image_generation=false`。只要至少出现一个known value，就忽略unknown并严格按known `text`/`image`集合设置能力。
- capability fetch fixture必须证明URL query、text/image/image-only、missing/null/empty/unknown-only与known+unknown mapping。

## C-02：Rig response ingestion 与安全边界

只接受同时满足以下条件的内容：

```rust
AssistantContent::Image(Image {
    data: DocumentSourceKind::Base64(_) | DocumentSourceKind::Url(_),
    additional_params: OpenRouter {
        response_only: true,
        source: "assistant.images",
    },
    ..
})
```

`Raw/FileId/String/Unknown`、无OpenRouter marker、或同一response同时包含Image+ToolCall进入`ERR-01`。Artifact ingestion不解析raw JSON来发现图片；raw只做D-10脱敏。

### 固定限制

| Limit | Value | Enforcement |
| --- | --- | --- |
| images per provider response | 10 | 在任何decode/download/write前计数 |
| encoded file bytes per image | 25 MiB | Base64 解码后字节数预检 + URL body streaming counter + staged-file metadata复核 |
| encoded file bytes per response | 100 MiB | 按每张实际落盘字节累计，超限清全部本轮stage/final |
| dimensions | width/height各≤16,384；总像素≤100,000,000 | image header + decoder limits，在像素分配前校验；随后只解码首帧 |
| decoder output allocation budget | 400 MiB | `image::io::Limits.max_alloc`；约束解码输出缓冲，不承诺包含WebP codec内部中间缓冲的总峰值RSS |
| URL connection / overall | 10s / 30s per hop-download | reqwest client/request timeout |
| redirects | at most 3 | 禁用自动redirect；每跳重新执行完整URL/DNS policy |

允许format只有JPEG、PNG、GIF、WebP，magic/decoder为authority；declared MIME或HTTP Content-Type存在时必须属于allowlist且与实际format一致。缺少MIME可以由magic补全。SVG/HEIC/HEIF与其他内容进入`ERR-01`。GIF/WebP 只要求首帧可解码；首帧之后的截断或损坏不在摄取阶段拒绝，交由展示解码器按需处理。

URL policy：

1. 仅`https`，端口必须为443；禁止userinfo、password、fragment、IP literal private/reserved。
2. 关闭system proxy、cookie store与自动redirect；不发送OpenRouter API key、provider headers或Referer。
3. DNS必须返回至少一个地址，全部通过`is_public_artifact_ip`；把本跳hostname固定到已验证地址，防DNS rebinding。
4. 每个redirect Location重新解析并重复1–3；相对Location按当前URL解析；超过3跳失败。
5. 只接受2xx；先检查Content-Length，再用response chunks累计；body超限立即drop。
6. error/UI/log不得包含URL、hostname、query、response body或local path。

Provider step raw response：有图片时，持久化前必须将`choices[*].message.images[*].image_url.url`替换为固定redaction marker；若无法证明所有normalized images已对应脱敏slot，则只保存固定`providerResponse` marker与integer image count，不能复制response id/model或任何原raw subtree。

## C-03 / DB-01：Prelinked batch 与 lineage

Root target types：

```rust
pub struct NewAttachment {
    pub id: AttachmentId,
    // existing fields unchanged
}

pub struct NewConversationEntryBatchItem {
    pub entry: NewConversationEntry,
    pub attachments: Vec<NewAttachment>,
}

pub struct AppendedConversationEntryBatch {
    pub entries: Vec<ConversationEntryRecord>,
    pub attachments: Vec<AttachmentRecord>,
}

fn append_conversation_entries_with_attachments(
    items: Vec<NewConversationEntryBatchItem>,
) -> Result<ConversationCommit<AppendedConversationEntryBatch>>;
```

同形async方法进入`AgentPersistence`并由Direct/Session adapters实现。既有composer/create/send helper继续使用“attachments追加到message末尾”的现有contract；新batch只服务已按provider response顺序构建的prelinked content。

Batch transaction invariants：

1. 非空batch；所有entry/attachment同一conversation。
2. 本批所有attachment ID非空、唯一且由调用方预分配。
3. authority run必须存在且属于同一conversation；authority provider step必须存在、属于该run且状态为Completed。该校验在第一条INSERT前完成。
4. 带attachment的item必须是Assistant Message；其content中的每个attachment part都必须由同一item提供。
5. 每个provided attachment恰好被引用一次，part kind与attachment kind匹配；本issue只产生Image。
6. 按batch item顺序，再按content首次引用顺序插入attachment与entry；seq/recency由现有append helper推进。
7. 任一验证/insert/entry失败回滚本批全部rows与conversation bookkeeping。
8. 返回值全部来自DB authoritative records；attachments按对应entry/content顺序，entries按seq顺序。

Lineage固定为：

```text
conversation_entries.agent_run_id       = current run
conversation_entries.provider_step_id   = completed OpenRouter step
conversation_entries.payload.content[*] = Image { attachment_id }
attachments.id                          = same preallocated ID
attachments.provider_id                 = selected provider
attachments.storage_kind                = generated_file
attachments.metadata.source             = GeneratedFile { final local path }
```

## G-01：文件与数据库补偿协议

Generated final filename固定为应用保留grammar`.jaco-generated-{attachment_id}.{ext}`；display name固定为`generated-image-{ordinal}.{ext}`。目录由`app/jaco`使用当前`DatabaseTarget.data_dir`和既有managed helper计算后注入agent，agent不能重新读取ambient data dir。无此前缀的`{uuid}.{ext}`、composer文件和其他旧文件均不属于provider-generated cleanup authority。

```text
validate source/network/limits
  → allocate stable attachment IDs
  → create/check immediate attachments root, managed conversation dir and .pending (reject symlink/non-directory)
  → write .pending/{id}.part with create_new
  → flush + sync_all; decode/sha/size/dim metadata complete
  → atomic rename each file to .jaco-generated-{id}.{ext}; sync parent
  → DB-01 batch
      ├─ success: disarm cleanup guard, publish C-04
      └─ error: read conversation timeline by IDs
           ├─ no rows: delete all finals, persist text/reasoning fallback
           ├─ all rows: preserve files, do not duplicate fallback
           ├─ partial: keep referenced/delete unreferenced, report invariant
           └─ read unavailable: preserve files for startup reconciliation
```

- 任一prepare/rename前错误删除全部`.part`与已rename final，DB尚无本批记录。
- 取消发生在DB commit前：删除本轮stage/final且不写Error；DB commit成功后观察到取消：保留committed output，run终态仍为Canceled。
- DB成功后的provider/run finalization错误不能反向删除已提交artifact；失败输出仍是会话上下文。
- startup reconciliation只扫描`attachments/<valid-conversation-id>/{.pending/, immediate files}`，拒绝symlink且不递归；final只删除符合`.jaco-generated-{uuid}.{png|jpg|jpeg|gif|webp}`保留grammar且无匹配DB `(id, conversation, exact path, GeneratedFile)`的文件。无此前缀的`{uuid}.{ext}`与composer/旧文件永不进入final删除谓词。root/引用文件缺失且DB已有record时保留row并发出safe degraded warning，不得静默当作clean或删除DB authority。
- 清扫删除失败记录warning并保留文件；DB读取失败令recovery失败。日志只含ID/count/category。

## C-04 / ST-01：Runtime lifecycle、publication 与 history

```text
provider request
  → provider step Running
  → normalized completion received
  → provider step Completed + usage + redacted raw
  → generated response persistence
       ├─ success → committed changes + final_entry_id → run continues/completes
       ├─ artifact failure → fallback text/reasoning → pending typed failure → run Failed + Error
       └─ cancel → cleanup → run Canceled
```

- Generated response projection把每个连续的非空Text/Image run按Rig choice顺序放入一个Assistant Message；Text/Image/Text仍属于同一run，Reasoning生成独立Reasoning entry并保持相对顺序；ToolCall由既有hook处理。
- response含Image+ToolCall时进入ERR-01；不会重排tool协议。
- projection/batch builder只接收wrapper局部保存并已完成的显式provider step ID；完成step后不得读取、恢复或复用已清空的`last_provider_step_id`。
- 一个DB commit发布一个`ConversationCommitted`，携带commit中的authoritative conversation summary；每个entry的AttachmentUpserted先于EntryAppended，所有值来自commit。`ConversationTimelineChanged`继续只服务provider-step等不推进conversation summary的变化。
- `final_entry_id`指向最后一个committed Assistant Message；图片-only Message也有效。
- artifact失败时，DB可用前提下把response中全部Text/Reasoning作为无图片batch持久化，再设置typed pending failure；同一DB故障使fallback也不可写属于物理限制，必须报告且仍保证无部分artifact rows/已知orphan。
- history中Assistant Message只在`content_text`非空时生成Rig assistant message；mixed message回放text，image-only返回None，永不把generated image或空assistant content发回provider。
- 单run顺序执行artifact步骤；不同conversation沿用现有active-run guard与目录/UUID隔离。所有network/file/blocking decode task都被当前run await，不detach。
- terminal分类统一采用`cancellation > pending generated-artifact failure > max-steps > prompt/runtime error`。pending payload first-wins、可重复读取，只有terminal run commit成功后才清除；所有inner/outer error分支和active-tool finalization都使用同一分类结果，Completed provider step永不被local failure降级。

## Error catalog 与隐私

| ERR-ID / code | Trigger | Retryable | Persisted/UI contract | Cleanup/recovery |
| --- | --- | --- | --- | --- |
| `ERR-01 generated_artifact_invalid` | source/marker/format/MIME/signature/dimensions/URL policy/混合tool shape非法 | false | 安全通用message，raw=None；保留Text/Reasoning + Error | 删除本轮stage/final；无attachment rows |
| `ERR-02 generated_artifact_limit_exceeded` | count、per-image、aggregate、pixel或decoder output allocation budget超限 | false | 不显示实际bytes、URL或path | 同ERR-01 |
| `ERR-03 generated_artifact_download_failed` | DNS、timeout、redirect、HTTP status、body read失败 | true | 不显示host/status body；可只显示“generated image download failed” | drop response，删stage/final，无rows |
| `ERR-04 generated_artifact_storage_failed` | managed dir/symlink/create/write/flush/sync/rename/cleanup准备失败 | true | 不显示local path；内部只记stage + ErrorKind | 尽力删除；失败交startup sweep |
| `ERR-05 generated_artifact_persistence_failed` | batch transaction或commit outcome probe失败 | true | provider step保持Completed；run Failed；不把DB/raw细节放UI | 按G-01读回判定；未知交startup sweep |

Cancellation继续使用既有`code=canceled`与Canceled终态，不属于artifact error。所有artifact error的`provider`最多是安全provider kind，`raw`必须为None。允许日志字段：conversation/run/step/attachment ID、ordinal、stage、error code、`io::ErrorKind`、count；禁止URL/host/query/path/base64/response body/provider secret/record Debug。

## Dependency inventory

| ID / dependency | Scope/kind | Current declaration/resolution | Target | Evidence and local uses | Runtime/platform constraints | Classification |
| --- | --- | --- | --- | --- | --- | --- |
| `DEP-01 image` | `jaco-agent` direct runtime decoder | agent无direct dependency；workspace已由`app/jaco`解析为registry `0.25.10`, default-features=false, gif/jpeg/png/webp | agent增加相同exact declaration/features；package resolution不变 | app manifest与locked source的`ImageReader/Limits`；C-02 header + 首帧decode | pure Rust formats，三平台已有构建；SVG/HEIC不启用 | `Compatible`; lock预期只给`jaco-agent.dependencies`增加`image` |
| `DEP-02 tokio` | `jaco-agent` direct runtime feature change | `1.53.1`, features process/sync/time；app已启用io-util/net | 同版本增加fs/io-util/net/rt，保留既有features | bounded async file I/O、DNS lookup、spawn_blocking | 不增加第二runtime；由现有gpui-tokio runtime执行 | `Compatible`; feature union通常无lock文本diff |

Unchanged material dependencies：`rig 0.42.0`、`reqwest 0.13.4`、`base64 0.23.0`、`sha2 0.11.0`、`url 2.5.8`与TLS source全部保持。`Cargo.lock`预期唯一文本变化是现有`jaco-agent` package dependency列表增加`"image"`；若出现新package或其他version/source/checksum/dependency edge变化，立即停止`WP-202`并更新本计划的dependency evidence。Repo-local skills、generated schema/assets、submodules、vendored source和native bootstrap均无同步项。

## Upstream reuse audit

| Subsystem | Decision | Exact action | Removal/residual check |
| --- | --- | --- | --- |
| Rig OpenRouter response normalizer | Adapt | 只消费marked `AssistantContent::Image(Base64/Url)`；增加Jaco storage，不复制provider response parser | 禁止新增`message.images` raw extraction；search仅允许raw redaction helper |
| Rig/OpenRouter request builder | Reuse directly | 通过existing `additional_params`传modalities | 不新建HTTP Chat adapter；专用`/images`无call site |
| Existing persistence hook/model wrapper | Adapt | generated mode在completion wrapper持久化并guard hook；normal mode零行为变化 | tests证明普通stream/text/tool路径不变 |
| Existing DB attachment helper | Retain + focused addition | composer helper保持；新增prelinked batch并共享low-level insert/append | 不复制SQL/schema或新transaction manager |
| #195 timeline/access | Reuse directly | 只发布现有Image + GeneratedFile shape | components/chat/detail、locales、assets expected no diff |
| OpenRouter dedicated Image API | Defer by user decision | 记录为RG-01失败后的候选后继计划 | 本issue无`/images`、ImageGenerationModel或SSE adapter |

## 兼容性、迁移与回滚

| Contract | Target result |
| --- | --- |
| core payload/serde | 零变更；旧/new app均可读取新GeneratedFile attachment与Image part |
| SQLite schema/version | 零变更；`SCHEMA_VERSION == 1`与唯一0001保持 |
| `NewAttachment` Rust API | workspace-internal source break；同一提交更新全部call sites/tests，无runtime migration |
| existing user attachments | producer预分配ID，DB/UI/storage semantics保持；旧composer helper继续append parts |
| ordinary providers/models | effective streaming/request/hook/history保持；artifact store未使用 |
| OpenRouter non-image model | models query仍返回text models；image_generation=false时无modalities且按既有streaming |
| generated image follow-up | stored/displayed locally；history只回放text，符合locked Rig OpenRouter response-artifact rule |
| rollback before data creation | 回退代码即可；无schema/data迁移 |
| rollback after generated data | 旧代码可加载core/DB shape，#195 UI仍显示；旧agent不再新增图片；managed files保留 |

## Release gate RG-01：真实 OpenRouter legacy Chat compatibility

当前计划可以实现和本地验证；以下门阻止`Done`、PR ready/merge claim：

1. 使用用户授权的真实OpenRouter credential请求`/models?output_modalities=text,image`，选择当时仍声明image output且可用于Chat的具体model；记录日期、model ID、output modalities与supported parameters，不记录key。
2. 通过Jaco真实non-stream Chat路径发送一个会产生图片的prompt；provider request snapshot必须含目标modalities且没有stream。
3. 响应必须由locked Rig得到至少一个marked AssistantContent::Image；记录实际Base64/URL source category，不记录locator/body。
4. 验证managed file、hash/size/dim/MIME、entry/run/step linkage、timeline、preview/save、restart reload与follow-up history。
5. 验证至少一个真实provider错误安全映射；不要求消费额度去遍历所有HTTP status。
6. 若当前OpenRouter不再返回legacy `message.images`、没有可用model或Rig无法normalize，`WP-001`停止在RG-01；不得切换`/images`或把fixture宣称为live success，需由用户重新选择后继route。

## Observable requirements

| R-ID | Requirement |
| --- | --- |
| `R-01` | OpenRouter models fetch保留text models并发现image output；missing/null/empty/unknown-only保持conservative behavior，known+unknown只按known值映射。 |
| `R-02` | 仅OpenRouter + image_generation触发modalities与effective non-stream；普通模型/provider请求和streaming不变。 |
| `R-03` | provider step先以Completed保存usage与脱敏raw；batch使用该显式step ID且DB验证Completed；local artifact failure只令run失败。 |
| `R-04` | 只摄取locked Rig marked Base64/URL assistant images；不从stream Unknown/raw发现artifact。 |
| `R-05` | 每个连续Text/Image run按原顺序进入一个Assistant Message；Reasoning、final_entry_id与runtime events遵守C-04。 |
| `R-06` | 每个文件/row/content part共享预分配stable ID，attachment包含GeneratedFile、provider、sha、size、MIME、dimensions。 |
| `R-07` | entry的run/step字段是lineage唯一authority；core/schema没有重复字段。 |
| `R-08` | DB batch满足DB-01，失败无部分rows/seq/recency；成功以一个`ConversationCommitted`发布authoritative summary与全部changes。 |
| `R-09` | G-01对prepare、cancel、DB error、commit uncertainty与crash给出确定清理/保留行为。 |
| `R-10` | C-02编码文件大小、MIME/magic、画布尺寸、首帧decode与公网HTTPS策略全部执行；URL/base64/secret/path不进入UI/log/raw snapshot。 |
| `R-11` | 现有#195 renderer直接显示Assistant生成图片并支持reload/preview/save；无UI/i18n/assets改动。 |
| `R-12` | artifact失败使用ERR identity，DB可用时保留全部Text/Reasoning + Error；取消保持Canceled且无artifact Error。 |
| `R-13` | image-only assistant history被省略，mixed只回放text；后续request没有空assistant message或生成图片。 |
| `R-14` | OpenRouter response包含Image+ToolCall时安全失败并保留Text/Reasoning，不重排或执行半个tool协议。 |
| `R-15` | 只执行DEP-01/02；Rig/reqwest/TLS/version/source保持，lock只出现`jaco-agent → image`预期metadata edge，其他manifests保持。 |
| `R-16` | focused、workspace、manual、real API和remote CI证据齐全后才能Done。 |
| `R-17` | 1000帧GIF/WebP在摄取校验中不解码后续帧像素；WebP仍可扫描容器/帧元数据；同一样本的首帧校验耗时相对原全帧基准有明确记录。 |

## Work packages

```text
WP-101 DB stable IDs + prelinked batch + generated index
       ↓
WP-201 OpenRouter capability/request + completion capture/error publication
       ↓
WP-202 bounded artifact ingestion + G-01 + history
       ↓
WP-301 Jaco target wiring + adapter + startup reconciliation
       ↓
WP-001 aggregate verification + manual reload + RG-01 + remote CI
```

| WP | Owner | Observable outcome | Dependencies | Owner plan |
| --- | --- | --- | --- | --- |
| `WP-101` | `crates/jaco-db` | callers can preallocate IDs and atomically commit ordered entries/attachments without schema change | `C-03`、`DB-01` | [DB plan](../../../crates/jaco-db/docs/dev/issue-196/README.md) |
| `WP-201` | `crates/jaco-agent` | image capability automatically selectslegacy non-stream Chat and capturesnormalized response with typed lifecycle | `WP-101`、`C-01/C-04`、`ERR-*` | [agent plan](../../../crates/jaco-agent/docs/dev/issue-196/README.md) |
| `WP-202` | `crates/jaco-agent` | Base64/URL images pass C-02/G-01 and persist/recover safely | `WP-101/201`、`DEP-01/02` | [agent plan](../../../crates/jaco-agent/docs/dev/issue-196/README.md) |
| `WP-301` | `app/jaco` | runtime uses exactdatabase target, production adapter executes batch, startup removes safe orphan set | `WP-101/202`、`G-01` | [Jaco plan](../../../app/jaco/docs/dev/issue-196/README.md) |

## 本地实施结果（2026-09-01）

- `WP-101`、`WP-201`、`WP-202`、`WP-301` 已实现；本轮 D-13 动画校验修订尚未提交或推送。
- 2026-09-01 命名隔离修复后的上一代码状态通过`cargo fmt --all -- --check`、`cargo build --locked`、`cargo test --locked`、`cargo clippy --locked --all-targets --all-features -- -D warnings`；首次沙盒内workspace test仅有`http-client`本地监听用例因`PermissionDenied/ServerError Bind`失败，原命令在允许本地监听的环境重跑后全部通过。
- 本轮 D-13 修订后的当前树执行了`cargo fmt --all`、`cargo test -p jaco-agent artifacts --locked`（`11/11`）、`cargo clippy -p jaco-agent --all-targets --all-features --locked -- -D warnings`与`git diff --check`；未重复运行workspace aggregate gates。
- 聚焦验证通过：jaco-db attachments `6/6`、agent `32/32`、catalog `9/9`；jaco-agent runtime `70/70`、generated `8/8`、artifacts `11/11`、providers `49/49`；Jaco conversation `89/89` 及 generated reconciliation/session/publication 聚焦测试。
- `cargo tree -p jaco-agent --locked -i image@0.25.10` 只显示现有 `image 0.25.10 -> jaco-agent` direct edge；`Cargo.lock` 文本 diff 只给现有 `jaco-agent.dependencies` 增加 `"image"`。
- 实际改动保持在 `crates/jaco-db`、`crates/jaco-agent`、`app/jaco`、四份 issue plan/index 与预期 manifest/lock 范围；schema、core、UI/locales/assets、packaging 和 workflow 无 diff。
- 未执行 `T-07` 手工 Jaco 图片 preview/save/restart、`RG-01/T-08` 真实 OpenRouter legacy Chat、`T-10` PR SHA 三平台 CI；这些门禁继续阻止 `Done`。
- `T-03` 已覆盖 Base64、格式/首帧解码、MIME、IP/DNS policy、limits、symlink、hash/path/metadata；动画后续帧不再做摄取完整性保证。生产 HTTPS downloader 的真实 redirect/status/timeout/body 联调尚未执行，留在发布验收门禁中。
- `T-11` 已用`image 0.25.10` release harness对同一样本比较旧全帧路径与目标首帧路径：23,039-byte GIF为`5016/5035/4967ms`对`4/4/4ms`；230,044-byte WebP为`8009/7978/7946ms`对`9/7/7ms`；3,877,039-byte全画布GIF为`9022/9063/9027ms`对`6/5/6ms`。两条路径都执行25 MiB实际编码文件大小、格式、画布、SHA校验；未测CPU、峰值RSS、网络与Tokio调度。
- 2026-09-01提交前P1审查发现无前缀`{uuid}.{ext}`无法证明为provider-generated；final grammar已收紧为`.jaco-generated-{attachment_id}.{ext}`，reconciliation只删除该保留前缀，`.pending/{id}.part`不变。该修复当时agent artifacts`10/10`、Jaco generated reconciliation`7/7`、startup recovery`2/2`、publication/reload`1/1`与`cargo fmt --check`通过；本轮增加动画WebP回归后当前artifacts为`11/11`，无前缀UUID文件保留已有精确回归。

### WP-001：聚合验收与完成证据

**Owner**

Workspace root。

**Prerequisites**

- `WP-101/201/202/301` complete。
- Owner plans没有unresolved deviations。
- RG-01 credential使用得到用户授权；没有授权时只阻止RG-01/Done，不阻止本地实现与tests。

**Procedure**

1. 核对diff只包含三个owner、四份plan/index与DEP-01/02；lock仅有`jaco-agent.dependencies += image`预期edge；`crates/jaco-core`、UI/locales/assets/schema/workflow无diff。
2. 执行T-01–T-06 focused gates与T-11动画摄取基准并保存exact command/result。
3. 执行T-07 manual managed/restart/history matrix。
4. 执行RG-01/T-08；失败时按RG-01 stop condition返回计划。
5. 执行T-09 workspace fmt/build/test/clippy；同一状态同类命令只跑一次。
6. 推送/PR后等待T-10 macOS/Linux/Windows CI，并将commit/PR/SHA/CI证据写回root/owners。

**Exit criteria**

- R-01–R-17全有证据；RG-01满足；所有owner状态同步为Done。
- 未引入dedicated `/images`、extra provider/artifact kind或schema/UI变化；lock只含DEP-01规定的direct-edge metadata。

## Validation matrix

| T-ID | Layer | Required evidence |
| --- | --- | --- |
| `T-01` | provider fixtures | models query；missing/null/empty/unknown-only/known+unknown output mapping；checked modalities merge/nonobject/duplicate invariant；image mode completion-only；ordinary stream/tool/reasoning regression |
| `T-02` | locked Rig/runtime fixtures | marked Base64/URL；Text/Image order；image-only；unmarked/unsupported/mixed ToolCall；raw redaction |
| `T-03` | artifact pure + downloader integration | 当前pure tests覆盖base64、URL/IP policy、encoded per-image/aggregate bytes、pixels、MIME/magic、pre-allocation canvas limits、GIF/WebP first-frame与accepted late-frame corruption、symlink、hash/metadata和exact`.jaco-generated-{attachment_id}.{ext}`path；真实redirect/timeout/status/body integration仍是发布前缺口 |
| `T-04` | DB owner | stable ID；prelinked order；run/conversation + Completed-step lineage；queued/running/wrong-run step rejection；duplicate/mismatch/wrong kind；mid-batch rollback；schema unchanged |
| `T-05` | agent integration | explicit completed step ID；`ConversationCommitted` summary/changes；cancellation/pending/max-steps/prompt/runtime priority；fallback/error；cancel before/after DB；final_entry/history |
| `T-06` | app integration | exact DatabaseTarget dir；Session batch/index adapters；startup pending andprefixed final orphan sweep；unprefixed UUID/composer/legacy files retained；missing root/reference degraded warning；referenced/unsafe files retained |
| `T-07` | manual Jaco | generated image visible, valid GIF/WebP animation plays, preview/save, restart reload, follow-up succeeds, ordinary streaming still live；late-corrupt animation允许停帧/播放失败但app必须保持响应 |
| `T-08` | real provider | RG-01 exact procedure and recorded non-secret evidence |
| `T-09` | local aggregate | `cargo fmt --check`; focused packages; exact expected lock metadata diff; then `cargo build`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` |
| `T-10` | remote release | latest PR SHA passes existing macOS/Linux/Windows CI |
| `T-11` | local performance | 已完成：同一2048×2048/1000帧GIF/WebP样本分别运行原全帧与目标首帧校验；release耗时与文件大小见“本地实施结果”，CPU/RSS未测 |

## Completion bookkeeping

实施过程中root记录：实际commit/PR、changed paths、owner WP状态、focused/aggregate/manual/live/CI结果、RG-01 model/date/source category、dependency/lock evidence、deviations与rollback。Owner文档只记录自己的file/API/test证据，不复制live credential或root release decision。

## Implementer reread

- [ ] 只实现OpenRouter legacy Chat assistant images；没有切换`/images`或扩展artifact kinds。
- [ ] 先完成WP-101，再按顺序完成WP-201/202/301。
- [ ] Normal streaming/hook/tool/provider路径有明确regression tests。
- [ ] Raw redaction、URL policy、limits与G-01没有被简化为best effort。
- [x] 动画校验只解码首帧像素；单图/单响应编码字节、pre-allocation canvas/pixel与decoder output allocation budget继续执行，测试明确记录后续帧损坏延迟到展示阶段暴露。
- [ ] DB/source break同步更新全部`NewAttachment`call sites，schema/core保持；lock仅有预期direct-edge metadata变化。
- [ ] Error/cancel/provider-step/run状态符合C-04与ERR catalog。
- [ ] RG-01失败会停止发布，不会改架构。

## Auditor reread

- [ ] S-01–S-19每行都有evidence、决定和owner。
- [ ] entry→content→attachment lineage可由reload验证，没有第二source of truth。
- [ ] DB batch与filesystem只宣称补偿一致性，没有跨资源ACID误述。
- [ ] SSRF、redirect、proxy、DNS rebinding、resource limits、symlink与raw/log leakage均有negative tests。
- [ ] commit error/cancel/crash各自的文件/row/context结果唯一且可恢复。
- [ ] #195 UI和follow-up history都用持久化事实验证。
- [ ] DEP-01/02之外没有manifest/resolution/lock变化；lock diff精确等于`jaco-agent.dependencies += image`。
- [ ] Done证据包含真实OpenRouter与latest PR SHA三平台CI。

## Implementation status

本计划状态为`Implemented locally`：本地实现、D-13动画首帧校验、T-03与T-11已完成；OpenRouter legacy Chat response兼容性、手工UI与远程CI门禁仍阻止`Done`。RG-01/T-07/T-10不会触发未经确认的替代API改造。
