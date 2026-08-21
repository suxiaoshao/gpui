# Issue #189：Settings 请求费用统计实施计划

## 状态与范围

- 状态：`Implemented`（代码、自动化与本地数据库手工迁移已完成；真实付费 provider 请求未自动发送）
- Plan ID：`issue-189-cost`
- Root plan：[Issue #189](README.md)
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- 最近更新：2026-08-21
- 实施顺序：`WP-103 -> WP-205 -> WP-403 -> WP-505 -> WP-006`

本计划只增加一种费用来源：使用 provider step 发出请求前冻结的 models.dev 精确模型目录价，对该 step 已有的 `ProviderUsageSnapshot` 做整数计算。它不读取 provider 原始响应中的金额，不改变现有 provider step、streaming、tool-call 或 usage-event 的完成时机。

## 目标

1. 用户执行现有 Fetch Models 时，为 eligible official provider 的 exact provider/model 保存 models.dev 目录价。
2. 每个 provider step 在网络请求发出前冻结自己的价格快照；后续目录刷新不改变该 step 或历史请求。
3. 成功请求沿用已经持久化的 Token 字段计算 USD 估算金额，并与 usage event 原子保存。
4. Settings Usage 以六项顶部摘要显示估算费用、请求数、输入、输出、缓存读取与缓存写入，并展示所选周期的稀疏每日费用趋势、provider 分布、model Top 10 与 provider/model 明细费用。
5. 实现完成后一次性手工迁移当前本地数据库；仓库不保留升级、兼容、修复或回填代码。

## 非目标

- Provider-reported amount、账单 API、余额、quota、预算、费用告警或账单对账。
- 解析、保存或传递 provider raw response、service tier、reasoning 费率证据或其他计费审计 JSON。
- 修改现有 streaming、tool-call、invalid-call、provider-step completion 或 usage-event 生命周期。
- 修复本计划之前就没有生成 usage event 的请求；费用覆盖范围与现有 persisted usage 覆盖范围一致。
- Manual/custom/local provider 定价编辑、bare model/family 回退、历史费用重算或后台价格刷新。
- Agent 消息、composer或Token活动热力图中的费用展示；不新增费用热力图，也不把稀疏日费用二次重采样为固定周/月 bucket。
- 追求 provider 发票级精度。界面必须说明这是按请求时目录价和已记录 Token 计算的估算值。

## 已确认决定

- 费用直接由已有 Token 乘以请求时模型价格得出。
- 只使用 models.dev；provider 返回的金额不进入本期设计。
- `NULL` 表示缺少可证明价格或可计价 usage；显式零价产生已计价的 `$0`。
- 每个 provider step 独立冻结价格，价格不放入 `RunSettingsSnapshot`、shortcut 或 conversation。
- fresh schema 仍为 version 1，不增加 `0002`、runtime migration、compatibility、repair 或 backfill。
- 本地真实数据库只在实现和自动化验证完成后手工迁移，临时 SQL 不进入仓库。

## 高影响摘要

- 新增 core fixed-point price/amount 类型和一个纯 `ProviderUsageSnapshot -> Option<UsdNanoAmount>` 计算器。
- `provider_models` 保存当前目录价；`provider_steps` 保存请求前冻结价；`usage_events` 只保存一个 nullable 金额列。
- agent 只接入 models.dev Fetch Models 和现有成功 usage completion；不增加 runtime state machine。
- Settings summary、稀疏日费用趋势、provider/model 图与 Table 消费同一 selected-range DB 快照；activity 仍只消费 Token。

## Surface applicability

| Surface | 适用性 | 本计划处理 |
| --- | --- | --- |
| `S-01` Contract ownership | Applicable | core 定义价格、金额与纯计算；DB 持久化/聚合；agent 获取价格并在既有 completion 调用计算；app 展示 |
| `S-02` GPUI lifecycle | Applicable | 复用现有 Settings Usage 与 Fetch Models lifecycle，不增加 Entity/Task |
| `S-03` Async operation | Applicable | models.dev 是现有 Fetch Models 的同一次显式操作；无 startup/background refresh |
| `S-04` Store/shared state | Not applicable | 不新增 Store、Global 或第二份价格缓存 |
| `S-05` Database | Applicable | fresh v1 增加 3 个 nullable 列、step-start snapshot、usage cost 与 analytics |
| `S-06` Date/time | Applicable | `fetched_at` 为 provenance；费用范围继续复用 Settings 既有 UTC/local-day 查询边界 |
| `S-07` Components/layout | Applicable | 复用 gpui-component `PieChart`、`Progress`、Table、GroupBox 与主题；不新增通用组件 |
| `S-08` Forms | Not applicable | 不增加 pricing editor 或设置项 |
| `S-09` Actions/keybindings | Not applicable | 无新 action/keybinding |
| `S-10` Icons/assets | Not applicable | 无新 icon 或 asset |
| `S-11` Localization | Applicable | en-US/zh-CN 增加估算费用、覆盖率和免责声明文案 |
| `S-12` Accessibility | Applicable | summary 与费用 cell 提供 exact amount/coverage 文本 |
| `S-13` Runtime resources | Not applicable | 无新 runtime asset |
| `S-14` Packaging | Not applicable | 无 bundle 资源或 entitlement 变化 |
| `S-15` Security/privacy | Applicable | models.dev 固定无凭据 URL；不发送 provider key，不记录响应 body |
| `S-16` Error behavior | Applicable | catalog 获取失败复用 Fetch Models 错误；无法估算只写 `NULL`，不让成功请求失败 |
| `S-17` Observability | Not applicable | 不新增unpriced reason或审计日志；既有Fetch Models/DB错误日志不得记录凭据、prompt或catalog body |
| `S-18` Compatibility/migration | Applicable | 仅改 fresh v1；实现后执行一次性本地手工迁移 |
| `S-19` Tests/CI | Applicable | core/DB/agent/app 聚焦测试、格式、strict clippy 与现有 CI |

## 当前代码依据

- `ProviderUsageSnapshot` 已持久化 `input_tokens`、`output_tokens`、`cached_input_tokens`、`cache_write_input_tokens`、`reasoning_tokens` 和 `total_tokens`；Settings analytics 的 request authority 已是 `usage_events`。
- provider step 已在网络请求前创建，并在成功完成时把 usage 传给 DB；价格快照和费用字段沿用该边界即可。
- provider model catalog 已由用户显式 Fetch Models 更新；models.dev 定价应附着同一批 exact model 结果，不增加独立刷新入口。
- Settings Usage 已有 selected summary、rolling activity 与 provider/model Table；费用在同一页面增加 selected sparse daily、provider 与 model 可视化。

## Cross-owner contracts

### `C-81`：Fixed-point price 与计算

`jaco-core` 新增私有字段、validated constructor 的类型：

```rust
#[serde(transparent)]
pub struct UsdNanoPerMillionTokens(u64);

#[serde(transparent)]
pub struct UsdNanoAmount(u64); // constructor 同时限制 <= i64::MAX

pub struct ProviderTokenPriceSnapshot {
    pub input: UsdNanoPerMillionTokens,
    pub output: UsdNanoPerMillionTokens,
    pub cache_read: Option<UsdNanoPerMillionTokens>,
    pub cache_write: Option<UsdNanoPerMillionTokens>,
}

pub struct ProviderTokenPriceTierSnapshot {
    pub input_token_threshold: u64,
    pub rates: ProviderTokenPriceSnapshot,
}

pub struct ProviderPricingRouteKey {
    pub provider_kind: String,
    pub canonical_base_url: String,
}

pub struct ProviderModelPricingSnapshot {
    pub models_dev_provider_id: String,
    pub models_dev_model_id: String,
    pub route: ProviderPricingRouteKey,
    pub fetched_at: OffsetDateTime,
    pub base: ProviderTokenPriceSnapshot,
    pub tiers: Vec<ProviderTokenPriceTierSnapshot>,
}
```

价格是“nano-USD / 1,000,000 tokens”，金额是 nano-USD。`ProviderPricingRouteKey`由同一个core helper从built-in official provider settings生成；agent在Fetch Models时保存，DB在step insert时从该step settings重新生成并比较。custom/local settings返回`None`。models.dev decimal 解析不得经过 `f64`；在 workspace 现有 `serde_json` 上启用 `arbitrary_precision`，不增加新 package。

纯函数：

```rust
pub fn estimate_request_cost(
    provider_kind: &str,
    usage: &ProviderUsageSnapshot,
    pricing: &ProviderModelPricingSnapshot,
) -> Option<UsdNanoAmount>;
```

算法固定为：

1. `total_tokens == 0` 视为现有 unreported sentinel，返回 `None`。
2. Anthropic 的 input 与 cache read/write 已分离：`uncached_input = input_tokens`。
3. OpenAI、Gemini、OpenRouter、DeepSeek、Mistral 的 input 包含 cache：使用 checked subtraction 得到 `input_tokens - cached_input_tokens - cache_write_input_tokens`；underflow 返回 `None`。
4. `cached_input_tokens` 使用 `cache_read.unwrap_or(input)`；`cache_write_input_tokens` 使用 `cache_write.unwrap_or(input)`。这是 Jaco 明示的目录价估算规则。
5. 全部 `output_tokens` 使用 output rate；`reasoning_tokens` 仅是 output 的诊断子字段，不单独相加，避免 double count。本期不解析 models.dev reasoning/mode 价格。
6. tier 选择使用该请求 `input_tokens`，命中 `input_tokens >= threshold` 的最大 threshold；无命中使用 base。
7. 以 `u128` 计算四项 `tokens * rate`，求和后统一 half-up 除以 `1_000_000`，再 checked 转为 `UsdNanoAmount`；任何 overflow 返回 `None`。

该函数不会修改或重新解释 persisted usage。它只把现有 provider inclusion 语义收敛为四个计价项。

### `C-82`：models.dev catalog acquisition

- 固定读取 `https://models.dev/api.json`，无 auth、query 或 provider credential；10 秒总超时、32 MiB 响应上限、禁止记录 body。
- 只为 built-in official endpoint 获取并匹配以下 exact provider/model：OpenAI、Anthropic、Gemini/Google、OpenRouter、DeepSeek、Mistral。Ollama、custom OpenAI-compatible、custom base URL、manual/unmatched model 保存 `pricing = None`。
- top-level provider key、payload provider ID、model map key、payload model ID 与 Fetch Models 返回的 exact model ID必须一致；不得按 display name、family 或 bare model 回退。
- 只解析 base `input/output/cache_read/cache_write` 和按 input threshold 的 tiers；忽略 experimental modes、账单附加项和 provider-reported cost。
- models.dev 获取、decode 或 matched pricing validation 失败时，整次 Fetch Models 复用现有失败状态，DB 不替换 catalog；这是有意的all-or-nothing边界，避免临时catalog故障把已有price覆盖成None。成功catalog中missing provider/model/cost仍是合法的 `pricing = None`。
- pricing snapshot 保存 typed official route key。provider 配置后来改变时，step-start 校验不匹配就冻结 `None`；切回 official 后由用户再次 Fetch Models 获取最新价。

### `C-83`：Step-start snapshot 与 usage completion

Fresh schema v1 只增加：

```sql
provider_models.pricing_json JSON NULL
provider_steps.pricing_snapshot_json JSON NULL
usage_events.cost_amount_nano_usd INTEGER NULL
    CHECK (cost_amount_nano_usd IS NULL OR cost_amount_nano_usd >= 0)
```

- `ProviderModelRecord` / `NewProviderModel` 增加 `pricing: Option<ProviderModelPricingSnapshot>`。
- `insert_provider_step` 不接受 caller-supplied price。DB 在插入 Running step 的同一事务中，按本地 provider ID 和 provider-facing model ID读取当前 catalog price。
- 只有 row provider/model、pricing models.dev model ID、pricing route key与从step settings生成的route key全部匹配时，DB 才写入 `pricing_snapshot_json`；其他情况写 `NULL`。DB 返回的 step record 携带该 frozen snapshot。
- 每个现有成功 usage completion path 使用同一 step record 的 frozen price调用 `estimate_request_cost`，并把 `Option<UsdNanoAmount>` 传给现有 `CompleteProviderStep`。
- DB 在既有 complete-step transaction 中同时写 step terminal state、usage event 和 `cost_amount_nano_usd`。failed/canceled step和没有 usage event 的旧生命周期保持原行为。
- 价格缺失、all-zero usage、checked subtraction失败或 arithmetic overflow只产生 `NULL`；成功 provider request不得因此失败，也不新增reason enum、审计列或日志协议。
- 不增加 `cost_source`、`cost_json`、raw evidence、reported/estimated enum 或新的 runtime completion boundary。

### `C-84`：Analytics 与 UI

`UsageAnalyticsAggregate` 与 provider/model bucket 增加：

```rust
pub priced_request_count: u64,
pub estimated_cost_nano_usd: u64,
```

- `priced_request_count = COUNT(cost_amount_nano_usd IS NOT NULL)`；显式零价也计入。
- `estimated_cost_nano_usd` 只 SUM non-NULL 金额并使用 checked conversion；0 priced request在 UI 显示 `—`，known zero显示 `$0`。
- selected summary 与 provider/model buckets 对 request count、priced count和金额做 cross-total；activity query、daily buckets与 heatmap保持 token-only。
- snapshot 增加 sparse `selected_cost_daily`：按 selected range 的 fixed local offset逐日聚合，仅包含cost非NULL的日期；unknown不补成0，显式零价仍保留bucket，finite与AllTime均按日期升序。
- Settings summary固定为六项：估算费用小计、请求数、输入、输出、缓存读取和缓存写入；费用项的secondary text显示`priced / total`覆盖率，不再把coverage、reported、reasoning或total作为独立顶部指标。
- 费用趋势对`selected_cost_daily`中的每个日期画一根柱，不增加固定bucket或缺失日补零。
- provider图按provider聚合全部已计价bucket，显示全量实心饼图与Top 5横向进度条；model图按已知费用降序显示Top 10横向进度条。全unknown隐藏，known-free明确显示`$0`语义。
- provider/model Table 增加一个“估算费用”列，cell显示金额与 `priced/requests`；保持 total-token 排序与横向滚动。
- money formatter直接从 nano-USD 整数生成精确 `$` 文本，最多9位小数并去除尾零，不使用 `f64` 或紧凑 `k/M`。
- 页面显示本地化免责声明：按请求时 models.dev 目录价与已记录 Token 估算，不代表 provider 最终账单。
- message toolbar、composer、activity heatmap、Provider Settings 与 model picker不显示费用。

## Error contract

| 场景 | 行为 | 用户可见结果 |
| --- | --- | --- |
| models.dev 网络/status/大小/decode失败 | intentional all-or-nothing：现有Fetch Models operation失败，旧catalog/price零mutation | 复用Fetch Models错误与Retry |
| exact model没有价格 | model保存 `pricing=None` | 后续请求费用unknown |
| step route与price endpoint key不匹配 | step snapshot写NULL | 请求正常；费用覆盖率分母增加、分子不增加 |
| usage all-zero、bucket underflow或金额overflow | cost写NULL；不增加reason持久化/日志协议 | 请求正常；费用显示部分覆盖 |
| DB complete transaction失败 | 沿用现有completion错误/回滚 | 不产生半条usage或cost |
| selected范围没有priced request | subtotal unknown | `—`，不是`$0` |
| 显式零价request | amount 0且priced count增加 | `$0`并计入覆盖率 |

## Compatibility 与本地手工迁移

- `SCHEMA_VERSION`保持1，fresh `0001`/schema直接包含三个新列；不新增`0002`或upgrade分支。
- 不给新 serde 字段编写旧数据补全层；旧本地库由`WP-006`在实现后手工处理。
- 旧 provider models、provider steps和usage events新增列均保持NULL；历史请求不按当前价格回算。
- 本地迁移必须在Jaco关闭后先创建可恢复备份，再执行三个 nullable `ALTER TABLE ... ADD COLUMN`。临时SQL位于repo外并在验收后删除。
- 迁移前后执行 `PRAGMA integrity_check`、`PRAGMA foreign_key_check`、`PRAGMA table_info` 与关键row count核对；失败时停止启动新binary并从干净备份恢复。

## Requirements

| ID | Requirement |
| --- | --- |
| `R-81` | 费用只由persisted usage Token和step-start frozen models.dev price计算 |
| `R-82` | 不读取provider-reported amount或raw response，不改runtime completion边界 |
| `R-83` | models.dev只做eligible official exact provider/model匹配，无family/bare-model fallback |
| `R-84` | decimal、rate和amount全程fixed-point/integer，不经过f64 |
| `R-85` | cache inclusion按`C-81` checked归一化，reasoning不重复加入output |
| `R-86` | missing price、unreported usage或overflow为NULL且不阻断成功request |
| `R-87` | provider step在网络请求前冻结price，catalog refresh不重算历史 |
| `R-88` | usage event与amount在同一complete-step transaction写入 |
| `R-89` | unknown与explicit free严格区分 |
| `R-90` | selected summary和provider/model bucket聚合金额与coverage并cross-total |
| `R-91` | activity、message、composer、Provider Settings和model picker无cost UI变化 |
| `R-92` | UI明确显示estimate disclaimer，不使用账单/实际花费措辞 |
| `R-93` | fresh schema仍version1且仓库无migration/compatibility/backfill代码 |
| `R-94` | 实现后安全手工迁移本地DB，临时SQL不入库 |

## Work packages

### `WP-103` — jaco-core fixed-point pricing calculator

1. 新增 `payloads/pricing.rs` 和exports。
2. 实现rate/amount、base/tier pricing snapshot、validated decimal parser与serde。
3. 实现`C-81`四项计价、tier选择、一次rounding和overflow返回None。
4. 不改`ProviderUsageSnapshot`、`RunSettingsSnapshot`、conversation或shortcut contract。

### `WP-205` — jaco-db snapshot、cost persistence与analytics

1. 直接修改fresh v1 SQL、Diesel schema和records，增加三个nullable列。
2. provider model catalog roundtrip pricing；step insert transaction按exact identity冻结price并返回typed snapshot。
3. 扩展`CompleteProviderStep`，在既有transaction中原子写usage和nullable amount。
4. selected summary/provider-model SQL增加amount与coverage；新增同transaction的selected sparse daily cost查询，并保留activity token-only查询。
5. 不增加migration文件、version、runtime schema detection或backfill。

### `WP-403` — jaco-agent models.dev与现有completion接线

1. 在现有Fetch Models操作中请求models.dev、exact merge并构造typed pricing；无新后台Task。
2. 只调整当前已经写入usage event的成功completion调用点：读取该step frozen price、调用core estimator、把amount传给DB。
3. 不新增raw response extractor、provider-reported amount、streaming/tool state或completion hook。
4. 验证official/custom、exact/unmatched和catalog更新后已运行step不变。

### `WP-505` — Jaco Settings费用展示

1. 让现有Fetch Models结果携带provider model pricing进入repository；app不建立第二缓存。
2. 扩展Usage snapshot adapter；将summary收敛为六项，并增加费用趋势、provider实心饼图/Top 5进度条、model Top 10进度条与Table费用列。
3. 增加nano-USD formatter、两locale、search和AX exact文本；所有图表沿用gpui-component主题。
4. 保持Usage Operation、heatmap、message、composer与Provider Settings交互不变。

### `WP-006` — 一次性本地数据库手工迁移

1. 自动化通过后关闭Jaco及所有DB连接，定位真实数据库并记录路径/大小/hash。
2. 使用SQLite online backup或确认WAL已checkpoint后的完整文件集创建repo外备份。
3. 核对迁移前schema正是当前已知pre-cost形状；未知、partial或already-migrated形状立即停止。
4. 在单一transaction中执行三个nullable `ALTER TABLE ... ADD COLUMN`，核对schema、row counts、integrity和foreign keys后再commit。
5. 启动新binary，Fetch Models，发送一个eligible exact matched请求；验证provider model price、step frozen snapshot、usage amount与Settings subtotal/coverage一致。
6. 验证custom/unmatched请求price与amount为NULL，旧rows仍NULL；删除repo外临时SQL，保留备份直到用户确认。

## Test matrix

| ID | Owner | Evidence |
| --- | --- | --- |
| `T-81` | core | decimal普通/科学计数、负数/超精度拒绝、nano rounding与i64 amount边界 |
| `T-82` | core | Anthropic separated input与其他provider inclusive input的四项公式、cache fallback、reasoning不double count |
| `T-83` | core | tier boundary/最大命中、underflow、all-zero和u128 overflow返回None |
| `T-84` | agent | official/custom eligibility、exact provider/model merge、missing price与无family fallback |
| `T-85` | agent | Fetch Models failure零mutation；price refresh后旧step snapshot不变、新step使用新price |
| `T-86` | agent | 现有non-streaming/streaming成功usage completion都调用同一estimator；既有tool/invalid/cancel事件顺序回归不变 |
| `T-87` | db | fresh schema version1且只有既有migration；三个nullable列与CHECK正确 |
| `T-88` | db | provider model price与provider step snapshot roundtrip、route/model mismatch冻结None |
| `T-89` | db | completion transaction原子写usage+amount；unknown/free区分与rollback |
| `T-90` | db | selected summary/provider-model subtotal、sparse local-day cost、priced coverage、partial/zero/overflow与cross-total；activity regression |
| `T-91` | app | nano-USD formatter、unknown/free/partial/full六项summary和estimate disclaimer |
| `T-92` | app | sparse日趋势、provider全量实心饼图/Top 5进度条、model Top 10进度条、Table费用列、token排序/scroll及heatmap/message/composer回归 |
| `T-93` | app | en-US/zh-CN key parity、search、summary/chart/cell AX exact text |
| `T-94` | root | 本地迁移备份/rollback演练与eligible/custom现场矩阵 |

## 最小验证

每个work package只跑直接相关验证；不因文档重写预跑实现阶段命令。

```sh
cargo fmt
cargo test -p jaco-core pricing
cargo test -p jaco-db pricing
cargo test -p jaco-db usage_analytics
cargo test -p jaco-agent pricing
cargo test -p jaco features::settings::usage::tests --no-fail-fast
cargo test -p jaco i18n --no-fail-fast
cargo clippy -p jaco-core -p jaco-db -p jaco-agent -p jaco --all-targets --all-features -- -D warnings
git diff --check
```

## 完成条件

- `WP-103`、`WP-205`、`WP-403`、`WP-505`完成且`T-81`–`T-93`通过。
- 每条新成功usage event只有一个nullable nano-USD amount；不存在provider-reported/raw evidence或第二套completion状态机。
- 目录价更新不会改变已插入provider step和历史usage event。
- Settings所选范围subtotal/coverage、稀疏日费用与provider/model图均来自DB exact整数聚合；unknown/free/partial语义、六项summary、两locale与AX通过。
- fresh schema仍version1，仓库无upgrade/compatibility/backfill/manual SQL。
- `WP-006`完成本地DB备份、迁移、现场验证和恢复路径核对。

## 实施证据（2026-08-21）

- `WP-103`、`WP-205`、`WP-403`、`WP-505` 已实现；费用只使用 frozen models.dev price 与既有 `ProviderUsageSnapshot`，没有新增 raw response、provider-reported amount 或 completion lifecycle。
- Settings顶部已收敛为六项；selected sparse日费用趋势、provider全量实心饼图/Top 5进度条、model Top 10进度条与保留的8列明细Table共用同一查询快照。
- `cargo test -p jaco-core pricing`：10 passed；`cargo test -p jaco-db usage_analytics --no-fail-fast`：15 passed；`cargo test -p jaco-agent`：138 passed；Jaco Usage：24 passed、Provider：30 passed、i18n：11 passed。
- `cargo fmt`、四个目标 package 的 strict clippy 与 `git diff --check` 通过；`cargo run -p xtask -- bundle jaco` 成功生成本地 macOS bundle。
- 真实数据库已通过 SQLite online backup 备份到 `jaco.sqlite3.pre-cost-20260821-153800.bak`，随后在单一 transaction 中增加三个 nullable 列；迁移前后 `integrity_check`、foreign keys、schema version、migration 记录和 `40/8/8` row counts 均一致，旧行新列保持 NULL。
- 新 bundle 已能启动并持有迁移后数据库；同 bundle id 的已安装旧版在验证时被关闭。为避免擅自消耗用户 API 额度，没有自动执行 Fetch Models 后的真实付费模型请求，因此 `T-94` 的 known-cost 现场正例仍待用户下一次正常请求验证；custom/unmatched 与金额聚合已有自动化覆盖。

## Handoff checklist

- [x] core calculator contract已实现并锁定四项公式。
- [x] provider catalog、step snapshot和usage amount三列已落地。
- [x] models.dev exact merge接入现有Fetch Models。
- [x] 现有usage completion接线完成且runtime事件顺序未变。
- [x] Settings六项summary、subtotal/coverage、稀疏日趋势、provider/model图、Table、Fluent与AX完成。
- [x] 聚焦自动化通过并记录命令/结果。
- [x] 本地DB一次性手工迁移、完整性核对与新bundle启动完成；真实付费请求留待用户下一次正常请求验证。
- [ ] implementation commits、PR与CI结果回填root plan。
