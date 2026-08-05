# Jaco：迁移到 gpui-form vNext 的执行计划

## 状态与范围

- 状态：`Done`。Jaco active Form consumer 已迁移到 vNext，自动化与 workspace gate 通过；
  实际 UI 操作测试按本轮要求未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 子任务 ID：`JACO-199-02`
- 子任务索引：[Issue #199：Jaco 子任务跟踪](README.md)
- 前置历史：[JACO-199-01：显式 form owner API 迁移](form-migration.md)
- Form producer plan：
  [FORM-199-02](../../../../../crates/gpui-form/docs/dev/issue-199/form-vnext-refactor-plan.md)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- 所有者：`app/jaco`
- 本地 ID 范围：`E/D/F/L/ST/R/T-600..699`、`WP-600..699`

本文记录 Jaco 从上一轮已经实施的 `FormModel`、generated `*Form`、`FormField`、identified array
API 迁移到 greenfield `FormSchema` + `Entity<Form<M>>` + runtime `ItemPath`。上一轮文档和验证记录保持
原样；本轮使用新的子任务与文档，不把历史 README 或 `form-migration.md` 改写成当前计划。

## 目标

1. 把所有 Jaco form model 迁到 `FormSchema`，页面持有通用 `Entity<Form<M>>`。
2. 把 derive-time validation/transform policy 改为 session-injected validator 与
   `Prepared<M>::map`，继续保持现有保存/CAS rebase 语义。
3. 删除 Jaco 对 writable `project_value` 的三个依赖，改为只读 native projection + 对真实 parent path
   的显式 mutation。
4. 让 MCP 设置表单的动态 row identity 完全由 Form runtime 生成；业务 draft、保存 payload 和 action
   不再携带 form-only `FormItemId`。
5. 保持 Prompt、Provider、Shortcut、ChatInput、RunSettings、MCP 的业务规则、可见验证、保存、catalog
   gate、secret 与 OAuth 行为不变。
6. 不让 Jaco 消费 form 的 core-private message/Transition；Jaco 只使用 Form 的领域方法和 adapter。

## 非目标

- 不迁移 Jaco Conversation、agent run、MCP server runtime、MCP OAuth runtime 或其他 operation 状态机。
- 不改变 provider/model/prompt catalog、数据库、配置、secret 或持久化 payload。
- 不重做 UI 布局、图标、本地化 key、资源或 bundle。
- 不添加 Jaco-local form facade、旧 API extension trait、raw token registry 或 compatibility layer。
- 不修改上一轮 [form-migration.md](form-migration.md) 的历史结论、状态和测试记录。
- 不处理 Feiwen、HTTP Client 或 Novel Download。

这里对 MCP 的改动只限“设置编辑表单的动态行定位、控件 binding 与字段校验”；MCP 连接、进程、
OAuth、工具调用与其他 runtime 流程保持 No change。

## 适用范围

| 表面 | 动作 | 保持不变的边界 |
| --- | --- | --- |
| Prompt 设置 | 静态 Form vNext 迁移 | 保存 operation、可见 required 规则、CAS rebase |
| Provider 设置 | 静态/variant Form vNext 迁移 | catalog/resource gate、secret policy、save task |
| Shortcut + RunSettings | nested Form vNext 迁移 | hotkey、prompt/model choices、运行设置业务语义 |
| ChatInput | nested Form vNext 迁移 | ChatForm visual shell、发送准入、agent/runtime owner |
| MCP 设置表单 | Form-owned dynamic topology 迁移 | 配置 payload、OAuth 与 MCP runtime |
| Jaco validation | 适配新 validator/report | Fluent message、字段级展示和业务校验规则 |
| Jaco operation/store/database | No change | 不因 form breaking migration顺手改造 |

## 证据

| ID | 当前事实 | 主要位置 | 迁移影响 |
| --- | --- | --- | --- |
| `E-600` | ChatInput/RunSettings derive `FormModel`，nested field 使用 `#[form(group)]` | `components/chat/input/form_state.rs`、`components/chat/run_settings.rs` | 改 `FormSchema`/`#[form(child)]` 与 root-first path |
| `E-601` | Prompt/Shortcut derive 同时绑定 validation/transform | `features/settings/{prompts,shortcuts}/form_state.rs` | validator按 session注入，提交改 `Prepared::map` |
| `E-602` | Provider 有三个 form model、自定义 secret wrapper 与 variant form编排 | `features/settings/provider*.rs` | session/adapter迁移，secret与save owner不变 |
| `E-603` | token budget、Shortcut prompt、Provider API mode 使用 writable `project_value` | `run_settings.rs`、`shortcuts/dialog.rs`、`provider.rs` | 必须拆成只读projection与parent mutation |
| `E-604` | MCP 五类 row 在 business draft 保存 `FormItemId`，用 `AtomicU64` 分配 | `features/settings/mcp/form_state.rs` | 删除 ID 字段/allocator/array metadata |
| `E-605` | MCP controls、UI key、remove action、validation map 都消费 `FormItemId` | `mcp/{dialog,form_rows,validation}.rs` | 改 typed `ItemPath`/opaque `PathKey` callback |
| `E-606` | MCP `row_id` 不进入最终保存配置 | `McpServerFormInput::merge_into_config` | 可删除 form-only ID，不改变持久化格式 |
| `E-607` | Jaco 手写 Garde index->stable ID 映射与 field path过滤 | `mcp/validation.rs` | 由 Form snapshot mapper承担，Jaco只查询typed field issue |
| `E-608` | 当前测试都在相邻 inline unit/`#[gpui::test]`，没有平行 app harness | 各 form consumer文件 | 在原位置迁移fixture，避免第二套测试框架 |

## Consumer 契约

| ID | Producer | Jaco 消费方式 | Gate |
| --- | --- | --- | --- |
| `C-600` | Form `C-500` | `FormSchema`、`Form<M>`、total/dynamic path、validator、Prepared | 精确声明与 compile fixture通过 |
| `C-601` | Form `C-501` | MCP 设置表单的 dynamic item enumeration/mutation、stale path与typed issue | topology/validation tests通过 |
| `C-602` | Form `C-502` | gpui-component total/dynamic adapter、opaque key、typed row callback | adapter tests通过 |
| `C-603` | Jaco | 所有 active Jaco form consumer只使用新 surface | `WP-604` residual + Jaco gate通过 |

Jaco 不得通过本地 helper提前模拟尚未达到 producer-ready 的 contract；如果 `C-600`–`C-602` 变化，
先回 Form owner 修正 producer plan和 tests，再更新本文。

## 架构决定

### `D-600`：每个编辑场景直接持有 `Entity<Form<M>>`

- Prompt、Provider、Shortcut、MCP dialog、ChatInput controller 各自创建并强持有一个 form session。
- child model 不创建 child entity；RunSettings 等通过 root-first typed path定位。
- 同一业务 model 的不同编辑场景可注入不同 validator；model 不关联固定 validator/state 名。
- component/deferred callback 只在 adapter边界持有 weak form，Jaco controller不缓存 descriptor。

### `D-601`：保存只消费一个 Prepared snapshot

- 每个保存/发送入口在一次 form update 中调用 `prepare`，得到 revision + cloned typed model。
- 应用通过 `Prepared::map` 生成 Prompt/Provider/MCP/Shortcut/ChatInput 的领域 output，禁止先读 revision
  再单独读 model。
- save成功继续以 captured revision调用 `rebase_if_revision`；用户保存期间的新编辑不被覆盖。
- 删除 Jaco `SubmitTransform` glue，不把 I/O 或 operation错误塞进 Form prepare。

### `D-602`：writable projection 不做兼容替代

三个现有 consumer 分别改为：

- RunSettings token budget：显示值由 reasoning parent field只读计算；用户输入显式写回真实
  reasoning settings path。
- Shortcut prompt selection：picker从真实 prompt字段读取/写入；展示 label由当前 catalog只读投影。
- Provider API mode：从真实 Custom OpenAI draft字段计算当前 mode；切换 mode执行一个显式 parent
  mutation，原子建立对应字段组合。

不得创建“看起来像 field、内部回写 parent”的 Jaco-local projection descriptor。

### `D-603`：MCP row identity 归 Form runtime

- 从五类 row draft删除 `row_id: FormItemId`、`AtomicU64` 与 `#[form(array(id = ...))]`。
- add/insert/remove/move只使用 Form collection API返回的 `ItemPath`；UI key使用 opaque `PathKey`。
- row control map按 `PathKey` 管理 native state，但 key不进入 model/config/serde。
- remove按钮的 closure捕获当前 live typed `ItemPath` 并调用 dialog方法；删除
  `RemoveMcpRow { row_id: u64 }` 这条 raw-token action route。
- remove/reinsert相等 business row会得到新 key；旧 control、issue、callback不能复活。
- static `.then(...)` 不需要Form；enum/optional row payload只通过
  `.try_case(form.read(cx), CaseDef)` / `.try_some(form.read(cx))` 定位。Jaco不构造不接收Form的纯
  `.case/.some` helper，也不接触Form的private topology snapshot。

### `D-604`：MCP validation 不再手写 stable-ID 映射

- Jaco validator仍表达现有业务规则与 Fluent message，但通过 Form validation snapshot枚举 rows、定位
  enum/optional payload并把 issue放在typed path。
- Garde adapter mapping、index->runtime token、case segment与stale completion由 Form owner负责。
- UI只在对应row/field旁查询和展示错误；不建立Jaco report镜像或页面级重复错误。

### `D-605`：native controls 与 options owner不变

Input/Select/Combobox/IntegerInput entity、subscription、focus和不完整文本继续由 adapter/controller持有。
Provider model、Shortcut prompt和其他 catalog refresh仍由现有 owner投影 options；options变化不隐式修改
Form value、baseline或save state。

### `D-606`：Form private Transition 不进入 Jaco

Jaco form consumer不导入/构造 core private message/effect，也不为 form实现 `Transition`。Jaco现有合法的
resource/save operation继续保留，residual scan不能把它们误判为本迁移残留。

## 文件与所有权

| ID | 文件 | 动作 | No change 边界 |
| --- | --- | --- | --- |
| `F-600` | `components/chat/input/form_state.rs`、`input.rs` | `FormSchema`、通用 session、prepare/send path | agent run、Conversation、ChatForm shell |
| `F-601` | `components/chat/run_settings.rs` | root-first child path、删除 descriptor generic/cache、重写 token projection | picker/catalog/reasoning policy |
| `F-602` | `features/settings/prompts/{form_state,dialog}.rs` | session validator、Prepared map、CAS rebase | prompt保存业务与UI |
| `F-603` | `features/settings/shortcuts/{form_state,dialog}.rs` | child path、hotkey adapter、prompt真实field mutation | save operation、choice catalog |
| `F-604` | `features/settings/{provider,provider/forms,form_validation}.rs` 及 `provider/forms/*.rs` | 三个session/validator/adapter、API mode parent mutation、Prepared | secret/catalog/resource/save owner |
| `F-605` | `features/settings/mcp/form_state.rs` | 删除row ID/allocator/array metadata，改runtime collection path | config model与merge业务字段 |
| `F-606` | `features/settings/mcp/{dialog,form_rows,validation}.rs` | typed row callback、opaque key、field issue query、删除手写映射 | MCP OAuth/runtime/持久化 |
| `F-607` | 上述文件内 inline tests | 原位迁移与新增stale row fixture | 不新增平行 `app/jaco/tests` harness |

目前没有需要整体删除的 Jaco 文件。只删除旧 symbol、derive属性、projection helper、row ID/action path；
不得因为 form迁移移动与本任务无关的模块或重排 UI。

## 生命周期与数据流

### `L-600`：静态表单

```text
dialog/controller 创建 Entity<Form<Draft>>
  -> 注入场景 validator
  -> adapter 绑定 total/root-first path
  -> native event 显式更新同一 form
  -> prepare 得到 Prepared<Draft>
  -> map 为保存/发送 payload
  -> 现有 task/operation 执行 I/O
  -> 成功按 revision CAS rebase
```

### `L-601`：MCP 动态行

```text
Form collection append/insert
  -> 返回 ItemPath + opaque PathKey
  -> dialog为该 path创建native row controls
  -> row callback捕获live ItemPath
  -> remove/move先由Form验证freshness再原子提交
  -> subtree retire时binding/issue/queued callback失效
```

rebuild controls必须先基于同一 item snapshot完整构造新 container，再替换旧 container；失败保留旧
container，不能出现半套新控件。

### `ST-600`：本迁移不新增 Jaco 业务状态机

Form session只管理编辑状态。save task、resource operation、catalog state、ChatInput/agent runtime与 MCP
runtime继续由现有 owner持有；本计划不新增 Jaco message reducer来包裹普通 form调用。

## 错误契约

- total path 的同步读写不新增 access `Result`；强 form owner路径不处理“form released”。
- dynamic MCP path 的 `ResolveError` 回到 row rebuild/dialog边界；不能 `unwrap_or_default`、退回 index或
  whole-array write。
- `MutationError` 保留旧 model/components并记录诊断；stale callback作为预期生命周期 no-op，不显示
  用户错误。
- validation/prepare错误继续走现有字段提示/提交 gate；persistence/secret/resource错误仍归应用 owner。
- adapter policy错误在构造处显式处理；静态合法 policy可由相邻unit test证明后使用带具体 invariant
  文案的 `expect`。

## 风险

| ID | 风险 | 防护 |
| --- | --- | --- |
| `R-600` | Jaco在Form签名未固定时造本地shim | `C-600`–`C-602` release gate |
| `R-601` | writable projection重写改变业务值 | 三个consumer逐项输入/输出fixture |
| `R-602` | MCP raw token经action或map重新泄露 | typed closure + `PathKey` residual review |
| `R-603` | equal row重建命中旧control/error | remove/reinsert stale callback与inline issue测试 |
| `R-604` | 删除手写Garde mapping后错误错贴 | core mapping fixture + Jaco row field assertion |
| `R-605` | breaking迁移影响secret/persistence/OAuth | No change payload对照与现有tests |
| `R-606` | form私有Transition扩散进app | import/impl residual审阅，只排除合法Jaco operation |
| `R-607` | 一次大改遗漏test-only generated API | all-target/all-feature clippy + focused residual scan |

## 测试契约

| ID | 层级 | 场景 | 验收 |
| --- | --- | --- | --- |
| `T-600` | Prompt unit/GPUI | required、prepare/save、CAS rebase | 可见规则不变；同一snapshot；新编辑不被覆盖 |
| `T-601` | Provider GPUI | secret、URL/name、API mode、variant submit | secret释放安全；mode parent mutation正确；无report镜像 |
| `T-602` | Shortcut/RunSettings GPUI | hotkey、prompt choice、model/reasoning/token budget | 三个projection替换后双向行为不变 |
| `T-603` | ChatInput unit | composer、attachments、RunSettings、send snapshot | readiness与发送语义不变；不触碰agent run state |
| `T-604` | MCP unit | add/remove/reorder、五类row、prepare config | draft无form ID；配置payload保持一致 |
| `T-605` | MCP lifecycle | remove/reinsert相等值、旧callback、row error | 新PathKey；旧work no-op；错误只在正确row/field |
| `T-606` | MCP GPUI | transport切换、container原子rebuild、OAuth并存 | form重建失败不半安装；OAuth/runtime不回归 |
| `T-607` | residual | active Jaco source/tests | 旧derive/state/field/item ID/projection/array metadata零残留 |

## 工作包

### `WP-600`：刷新 inventory 并确认 producer gate

- 在 Form `C-500`–`C-502` producer-ready 后重新搜索所有 production/test consumer，固定最终 type/import
  matrix与三个projection写回规则；dynamic resolver直接采用已确认调用面，不重新设计。
- 用最小Jaco compile fixture确认 MCP typed row callback不需要 raw token/serde action。
- 验收：本文不含占位签名；所有consumer均归类；不修改业务代码。

### `WP-601`：迁移静态 settings forms

- 迁移 Prompt、Provider、Shortcut的derive/session/validator/prepare。
- 同时重写 Provider API mode与Shortcut prompt projection；保留现有catalog/save/secret owner。
- 验收：`T-600`–`T-602`静态部分；依赖 `WP-600`。

### `WP-602`：迁移 ChatInput 与 RunSettings

- 迁移 ChatInput root session、RunSettings child path和token budget parent mutation。
- 删除旧 descriptor generic/cache与generated state imports；不修改 Conversation/agent run状态。
- 验收：`T-602/T-603`；依赖 `WP-601`。

### `WP-603`：迁移 MCP 设置表单的 dynamic topology consumer

- 迁移 `F-605/F-606`，删除form-only row IDs/allocator/array metadata/action payload与手写Garde mapping。
- 使用 Form enumeration/mutation返回的 live paths建立controls、key、remove/move callback和inline errors。
- 保持MCP config/OAuth/runtime No change。
- 验收：`T-604`–`T-606`；依赖 `WP-600`、Form `C-501/C-502`。

### `WP-604`：删除旧 surface并验证

- 更新所有相邻inline tests，执行 `T-607` residual；历史文档不纳入零命中。
- 运行下方 Jaco 自动化门禁并回写真实结果；实际 UI 操作测试按本轮要求只记录为未执行。
- 验收：`C-603` consumer-complete；依赖 `WP-601`–`WP-603`。

## 验证

实现阶段按顺序执行一次：

```text
cargo fmt --all
cargo test -p jaco --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --all-features --locked
git diff --check
```

Form producer tests由 `FORM-199-02` 负责，不在Jaco重复执行。实际 UI 操作测试按用户要求未执行；MCP
runtime 保持 No change。

Residual scan至少覆盖：

```text
derive(gpui_form::FormModel)
form(state
form(group
form(array
FormField
PartialFormField
FormItemId
NEXT_FORM_ITEM_ID
project_value
FieldPath::join_item
RemoveMcpRow
SubmitTransform
```

合法的 GPUI `Form` 文本、历史文档、Jaco现有 `gpui_operation::Transition` 和业务数据库 ID必须逐项分类，
不能用过宽替换误删。

## 实施结果（2026-08-05）

| 工作包 | 结果 |
| --- | --- |
| `WP-600` | 已完成最终 producer inventory 与 typed path/adapter contract 对齐。 |
| `WP-601` | 已完成 Prompt、Provider、Shortcut 的 `FormSchema`、validator、prepare/rebase 与 projection 迁移；Provider rebase 后重绑 controls，避免 whole-model lifecycle 让旧 binding 继续工作。 |
| `WP-602` | 已完成 ChatInput、RunSettings 与 token budget parent mutation 迁移；Conversation/agent run 未改。 |
| `WP-603` | 已完成 MCP 设置行的 runtime-owned `ItemPath`/`PathKey`、binding、字段级错误与 move up/down；stale remove/move 对 retired/missing item 安静 no-op，container rebuild 失败不半安装；MCP runtime/OAuth 未改。 |
| `WP-604` | 已增加 remove/reinsert、stale callback、reorder、双输入精确错误、原子 rebuild 与 control rebind tests；旧 Form surface residual 已分类，自动化通过；实际 UI 操作测试按用户要求未执行。 |

验证结果：`cargo test -p jaco --bin jaco --all-features --locked` 通过（361 tests）；`gpui-operation`、
三个 Form crate、Jaco 与 Feiwen 的聚合 Clippy 严格门禁、workspace all-target/all-feature check 与
`git diff --check` 均通过。旧 derive、generated
form state、form-only MCP row ID、array metadata 与 writable projection 在 active Jaco Form consumer 中
已清除。`ChatFormState`、`FormModelPicker`、`ProviderFormField` 以及 Provider 领域层的
`prepare_submit`/`ProviderPreparedSubmit` 仅名称相似，不属于旧 gpui-form API。

## 完成与交接

Form `C-500`–`C-502` 与 Jaco `C-603` 已达到 consumer-complete，当前状态为 `Done`，并纳入本次
Issue #199 实施提交。实际 UI 操作测试被明确排除，PR 尚未请求。Jaco Conversation、agent run、
MCP连接/OAuth/tool runtime、数据库与 catalog owner 均保持 No change。
