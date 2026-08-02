# Issue #199：统一 form 所有权，并为 store / operation 迁移建立前置契约

## 状态与范围

- 状态：`Form 阶段已实施`（自动化门禁通过；Jaco 定向 UI smoke 已执行；Issue #199 后续 app/store/operation 阶段仍待规划）
- Tracking issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- Plan ID：`issue-199`
- Root hub：`docs/dev/issue-199/README.md`
- 当前分支：`codex/199-adopt-gpui-store-form-operation`
- 当前规划范围：`gpui-form`、`gpui-form-macros`、
  `gpui-form-gpui-component` 与 Jaco 调用方的 breaking form API 重构
- 后续范围：其余 app 接入 `gpui-store` / `gpui-form` / `gpui-operation`，以及复杂业务逻辑改为
  `gpui-operation::Transition`；这些工作必须在 form 契约落地后另补同一 Issue 下的 owner plan
- 实施引用：当前分支 `codex/199-adopt-gpui-store-form-operation` 工作树（待提交/PR）

### 高影响变更摘要

当前 `FormField<Form, T>` 在构造时绑定一个 `WeakEntity<Form>`。因此即使同步调用方明确持有
`Entity<Form>`，读取、写入、验证和订阅仍要处理 form 已释放这一结构性错误；derive 也必须为每个
字段生成接收 entity 的 accessor。目标设计将字段改为可复用的 schema-level descriptor：静态字段由
generated state 暴露为 associated `const`，descriptor 不保存 entity、值、订阅或每个 form 的状态；
所有同步操作显式接收 `&Entity<Form>`。只有 identified item 或 computed projection 使用
`PartialFormField` 和 `try_*`，只有 control binding、deferred callback、subscription 与 async task
允许持有弱引用。

这是一次 workspace 内原子 breaking 迁移，不保留旧 `FormStore` derive、`*_field(&form)`、
`*_in(...)`、descriptor-owned weak form 或兼容 facade。

### 目标

1. 让 generated `FormState` 成为每次编辑会话唯一的 form runtime entity；字段 descriptor 只描述
   类型化访问路径和静态 schema。
2. 让每个静态字段只生成一个 allocation-free associated `const` 定义，例如
   `ProviderForm::NAME: FormField<ProviderForm, String>`；调用方可以直接重复使用，无需按调用实例化。
3. 让 total path 的同步 API 在显式 strong entity 下成为无结构性错误的普通操作；让真正可能消失的
   path 通过 `PartialFormField` 和 `try_*` 暴露不确定性。
4. 保留现有 typed model、revision-safe rebase、精确 validation scope、stable-ID array、owning
   controls 与纯 submit 的正确边界，同时删除由隐式 form ownership 产生的错误和泛型复杂度。
5. 一次性迁移三个 form crate 与 Jaco 的 Prompt、Provider、MCP、Shortcut、ChatInput、
   RunSettings；active source 中不得残留双轨 API。
6. 为 Issue #199 后续 app/store/operation 工作提供稳定 form 契约，但不在本阶段实现那些工作流。

### 非目标

- 不在本阶段迁移 `feiwen`、`http-client`、`novel-download` 的应用状态或业务流程。
- 不在本阶段选择或改写任何具体 `gpui-operation::Transition` 业务状态机。
- 不把 form runtime 存入 `gpui-store`；表单编辑会话继续由页面或 controller 持有
  `Entity<GeneratedFormState>`。
- 不改变 Jaco 数据库 schema、Diesel migration、持久化格式、provider/MCP 协议、assets、icons
  或本地化 key。
- 不让 form 或 adapter 持有 options/catalog、focus、popup、IME、save task、retry、notification
  或应用 operation 状态。
- 不承诺所有运行时组合出来的 descriptor 都实现 `Copy`；只要求 generated static descriptor 的
  定义零分配且可重复使用，组合 descriptor 保持轻量并按需要 move/clone。

### 用户决定

1. 可以进行大范围破坏性更新，以清晰的所有权和易用 API 为目标，不以最小 diff 为目标。
2. `FormField` 内部不得保存 `Entity<Form>` 或 `WeakEntity<Form>`；明确持有 form 的调用方必须显式
   传入。
3. 每个静态字段只有一个 generated schema-level descriptor 定义，不需要每次调用 accessor
   实例化字段。
4. 公开文档中的 `FormModel` / `FormState`、associated const、total/partial、显式 form、最小
   owning control 与 `PreparedSubmit` 设计是本计划的目标契约。
5. 旧开发文档按其实际交付 PR 归档。旧 form 重构由
   [PR #176](https://github.com/suxiaoshao/gpui/pull/176) 合入，而该 PR 的 closing issue 是
   [#175](https://github.com/suxiaoshao/gpui/issues/175)；因此归档到各 owner 的 `issue-175`
   目录，即使旧文档正文曾写“无独立 issue”。

### 兼容与迁移策略

- 三个 crate 和 Jaco 在同一迁移序列中切换；不发布或保留可被下游消费的半迁移状态。
- 直接删除 `#[derive(FormStore)]`、`#[form(store = ...)]`、generated field enum、
  `*_field(&form)`、`*_in(...)`、`identified_item(..., id_fn)`、`set_user_value` 及旧泛型
  `FormEvent<Form::Field>`。
- 新公开命名为 `#[derive(FormModel)]`、`#[form(state = ...)]`、`FormState`、associated const、
  `within`、`item`、`set`、total API 与 partial `try_*` API。
- 不提供 deprecated alias、feature flag、compatibility trait、双重 codegen 或机械转发 wrapper。
- public README/guide 可以先作为 Issue #199 设计预览存在；实现完成前必须保留“not implemented”
  提示，完成后再移除。

### Plan Map

| Owner | 状态 | 计划 | Local IDs | Assigned WPs | 职责 |
| --- | --- | --- | --- | --- | --- |
| workspace | Form 阶段已实施 | 本文 | `E-01..10`、`S-01..12`、`C-01..04`、`ERR-01..04` | `WP-000`、`WP-900` | 共享规格、归档、跨 crate 顺序与完成门 |
| `gpui-form` | Done | [core owner plan](../../../crates/gpui-form/docs/dev/issue-199/README.md) | `E/D/F/L/ST/R/T-100..199` | `WP-100..104` | descriptor、runtime、validation、submit、event |
| `gpui-form-macros` | Done | [derive owner plan](../../../crates/gpui-form-macros/docs/dev/issue-199/README.md) | `E/D/F/L/ST/R/T-200..299` | `WP-200..205` | canonical grammar、associated const 与 schema/access codegen |
| `gpui-form-gpui-component` | Done | [adapter owner plan](../../../crates/gpui-form-gpui-component/docs/dev/issue-199/README.md) | `E/D/F/L/ST/R/T-300..399` | `WP-300..304` | `ControlBinding` 与最小 owning controls |
| `jaco` | Implemented；UI smoke 部分受 Computer Use 命中限制 | [Jaco owner plan](../../../app/jaco/docs/dev/issue-199/README.md) | `E/D/F/L/ST/R/T-400..499` | `WP-400..406` | 所有当前 form 消费方原子迁移 |

历史交付入口见 [Issue #175 form 归档](../issue-175/README.md)。

## Applicability

| 系统表面 | 适用性 | 本阶段决定 |
| --- | --- | --- |
| Core form runtime | Change | 移除 descriptor-owned form；引入 total/partial descriptor |
| Derive macro | Change | `FormStore` -> `FormModel`，生成 state 与 associated const |
| GPUI component adapter | Change | 构造器显式接收 form；弱引用收敛到 `ControlBinding` |
| Jaco form consumers | Change | 所有 derive、字段、绑定、验证、submit 调用原子迁移 |
| `gpui-store` | No change | 本阶段不修改其 API 或接入范围 |
| `gpui-operation` | No change | 本阶段不修改 trait、family 或 Jaco operation |
| Database / persistence format | No change | 不改 schema、migration、序列化或 secret 格式 |
| Dependencies / `Cargo.lock` | No planned change | 不升级版本、不新增 crate；若实现证明必须变化，先回写计划 |
| UI layout / i18n / assets | No change | 只改变 ownership 与调用 API，不改变产品表现 |
| Platform / bundling | No change | 无 native/system dependency 或 bundle resource 变化 |
| repo-local `gpui-form` skill | Change at release | 当前 skill 正确描述已实现的 v1，只作为 current evidence；v2 源码落地前不得提前改成未实现 API，`WP-900` 再与最终契约原子同步 |

### Shared specification applicability

| S-ID | 适用性 | Implementing owners / WPs |
| --- | --- | --- |
| `S-01` descriptor ownership | Applicable | core `WP-100`；macro `WP-201`；adapter `WP-300`；Jaco `WP-401..405` |
| `S-02` static descriptor | Applicable | core `WP-100`；macro `WP-201`；Jaco `WP-401..404` |
| `S-03` total/partial | Applicable | core `WP-100..101`；macro `WP-202`；adapter `WP-300`；Jaco `WP-402..404` |
| `S-04` path/schema/composition | Applicable | core `WP-100..102`；macro `WP-202`；Jaco `WP-403..404` |
| `S-05` mutation/revision/event | Applicable | core `WP-101`；macro `WP-203`；adapter `WP-300..303`；Jaco `WP-405` |
| `S-06` validation | Applicable | core `WP-102`；macro `WP-203`；Jaco `WP-401..403` |
| `S-07` ControlBinding | Applicable | core `WP-102`；adapter `WP-300..302`；Jaco `WP-402..405` |
| `S-08` prepared submit | Applicable | core `WP-103`；macro `WP-203`；Jaco `WP-401` |
| `S-09` derive/naming | Applicable | core `WP-100/104`；macro `WP-200..201`；Jaco `WP-401` |
| `S-10` breaking migration | Applicable | all owner final WPs；root `WP-900` |
| `S-11` minimal controls | Applicable | adapter `WP-300..303`；Jaco `WP-402..404` |
| `S-12` application ownership | Boundary/no new runtime | Jaco `WP-400..405` 消费；三个 library owner以 No application state 验收 |

## Evidence

### 当前流程

```text
derive FormStore
  -> generated *_field(&Entity<FormStore>)
  -> FormField stores WeakEntity<FormStore>
  -> value/set/validate/subscribe upgrade internally
  -> every synchronous call returns Result<FormReleased | path error>

component constructor receives only FormField
  -> field creates attachment/subscription from its hidden weak form
  -> wrapper lifetime indirectly represents form relationship

Jaco page owns Entity<FormStore>
  -> repeatedly constructs *_field(&form)
  -> clones/stores descriptors in controllers and closures
  -> handles errors even where a strong form is already present
```

### 目标流程

```text
derive FormModel
  -> generated FormState + one associated const per static field
  -> FormField contains schema/path/access only

page owns Entity<FormState>
  -> ProviderForm::NAME.value(&form, cx)
  -> ProviderForm::NAME.set(&form, value, cx)
  -> FormInput::new(&form, ProviderForm::NAME, ...)

only a deferred boundary
  -> ControlBinding captures WeakEntity<FormState>
  -> upgrades when deferred work runs
  -> silently cancels if owner is gone
```

### Evidence Registry

| ID | 当前证据 | 结论 |
| --- | --- | --- |
| E-01 | `crates/gpui-form/src/field.rs` 的当前 `FormField` 与访问方法 | entity lifetime 与 path availability 被压进同一个 `Result` 边界 |
| E-02 | `crates/gpui-form/src/control.rs` | control attachment 是真正需要 weak/deferred lifetime 的边界 |
| E-03 | `crates/gpui-form/src/form.rs`、`validation.rs`、`submit.rs` | model、revision、validation、task 与 submit runtime 已由 form state 统一持有，应继续保留 |
| E-04 | `crates/gpui-form-macros/src/derive/{attributes,expand}.rs` | 当前 derive grammar 与 accessor/codegen 仍围绕 `FormStore` 和 per-call field construction |
| E-05 | `crates/gpui-form-gpui-component/src/{input,select,combobox,integer_input}.rs` | adapter 已是 owning handle，但构造和同步依赖 field 内部的 form handle |
| E-06 | `app/jaco/src/features/settings/{prompts,provider,mcp,shortcuts}` | Jaco settings 是 total、nested、identified item、custom control 与 validation 的完整消费矩阵 |
| E-07 | `app/jaco/src/components/chat/{input,run_settings}.rs` | shared nested RunSettings 与 computed projection 会验证 composition/partial/lifetime 契约 |
| E-08 | 三个 form crate 的 README 与 `docs/guide*.md` | 当前 Issue #199 v2 preview 已描述目标调用体验，可作为公开契约审阅面，不代表源码已实现 |
| E-09 | commit `6351898874b727ae8155903645a2dbfcc1f0da54`、PR #176、Issue #175 | 旧四份 form 计划随 #175 的实现 PR 交付，按 delivery provenance 归档到 `issue-175` |
| E-10 | `.agents/skills/gpui-form/SKILL.md` | 当前 skill 描述已实现的 v1 `FormStore`/`ControlAttachment` 契约；规划和实施 #199 时它是 current-state evidence，不是 v2 target，最终由 `WP-900` 同步 |

## Decisions

### Shared Specification Registry

#### S-01：descriptor ownership

`FormField<Form, T>` 与 `PartialFormField<Form, T>` 不保存 strong/weak form entity、当前值、
subscription、control lease 或 per-form allocation。同步调用始终显式传 `&Entity<Form>`。

#### S-02：静态 descriptor 定义

derive 为每个静态声明字段生成一个 `SCREAMING_SNAKE_CASE` associated `const`。该 const 只引用
const-friendly schema/path 与 function-item access fast path，定义和直接使用不分配内存。组合、item
和 computed projection 可以产生轻量运行时 descriptor；本规格不要求它们全部 `Copy`。

#### S-03：total / partial availability

- `FormField<Form, T>` 表示在 strong form 下必然可达的 total path；`value/set/errors/validate/
  is_validating/bind_control/subscribe_in` 等不返回结构性 `Result`。
- `PartialFormField<Form, T>` 表示 identified/computed path；只暴露对应 `try_*`，包括
  `try_bind_control` 与 `try_subscribe_in`。
- `within` 保持 parent availability；`item` 与 `project_value` 产生 partial，partial 的后代仍 partial。
- 同步错误中没有 `FormReleased`。

#### S-04：path、schema 与组合

静态 path/schema 必须支持 const-friendly 存储；located/composed path 支持动态 segment。group child
通过 `ChildForm::FIELD.within(parent)` 组合；array 通过 `RootForm::ITEMS.item(id)` 定位。
descriptor 只定位已有 schema，不在组合时创造新 schema。

#### S-05：mutation、revision 与 event

所有成功写入统一进入 core transaction：读取 candidate、检查 availability/identity、比较值、提交
model、推进一次 revision、失效相交 validation、运行 change validation、发出一个事件并 notify
一次。相等 `set` 是完整 no-op。`FormEvent` 非泛型并携带 path/revision 或 lifecycle scope；
`ValidationChanged` 不触发 value reprojection。

#### S-06：validation ownership

generated state 继续持有 validation context/report/async tasks。static `ValidationAdapter` 接收 model、
trigger、scope、typed context 与 `App`；scoped run 只替换选中 bucket并保留 sibling issue。identified
array path 在验证前按稳定 ID 映射；unknown/duplicate/identity violation 不得静默丢失。
Garde 的唯一 helper 为 `messages = ProviderType`：provider 静态地把规则映射成语义化
`ValidationMessage`，Jaco 继续在渲染时按当前 locale 翻译；不重新引入已移除的 `i18n = ...`
validation-time localization。

#### S-07：ControlBinding weak boundary

`ControlBinding` 只能由显式 strong form + descriptor 在 mount 时创建。它可以保存
`WeakEntity<Form>` 与 control lease，供 `defer_set/defer_blur/defer_set_issue/
defer_clear_issue` 使用；upgrade 失败静默取消。descriptor 本身不得继承这一 ownership。

#### S-08：submit snapshot

`prepare_submit` 对同一 model snapshot 完成 submit validation、pending/control gate 与一次静态、
纯、不可失败的 transform，返回 `PreparedSubmit { revision, output }`。持久化 task 与错误属于应用；
成功后只通过 `rebase_if_revision` 合并。

#### S-09：derive 与公开命名

唯一 canonical grammar 为 `#[derive(FormModel)]` 与 `#[form(state = StateName, ...)]`；generated
state 实现 `FormState`。不生成公开 field enum 或 `FormFieldId`；不接受旧命名 alias。

#### S-10：breaking migration

三个 crate 与 Jaco 调用点完成后，在 active source、tests、examples 和 public docs 中对旧 API 做
residual audit。旧 API 只允许出现在 `docs/dev/issue-175` 历史归档里。

#### S-11：最小 owning controls

每个 stateful bound wrapper 的持久字段严格为 `subscriptions: Vec<Subscription>`，随后是 native
`Entity<State>`。wrapper 不保存 form、descriptor、binding、值、options、delegate、focus flags
或 validation snapshot；所需 binding/descriptor 只由 subscription closure 捕获。

#### S-12：应用 owner 边界

页面/controller 持有 strong form entity、catalog/options、persistence operation 与 UI state。
订阅、deferred callback、async completion 可捕获 weak owner/form；普通同步 helper 不接受或返回
隐藏 entity ownership 的 descriptor。

### Integration Contract Registry

| ID | Producer -> Consumer | 契约 | 验收 |
| --- | --- | --- | --- |
| C-01 | `gpui-form` -> `gpui-form-macros` | core 提供 const constructor、static access、availability composition、state/runtime traits；macro 只生成 schema/access metadata | macro expansion 只调用公开/hidden-stable core contract，core integration tests 可编译运行 |
| C-02 | `gpui-form` -> adapter | total/partial descriptor、explicit entity、non-generic event 与 `ControlBinding` | total constructor无 access error；partial constructor只返回真实 path error |
| C-03 | core/macro -> Jaco | generated state/const、validation、submit 和 lifecycle API | Jaco 不自建兼容 facade、不直接构造 descriptor |
| C-04 | adapter -> Jaco | total `new`、partial `try_new`、minimal wrapper 与 native entity deref | `producer-ready`：`WP-300..302` 后 v2 API 足以迁移；`consumer-complete`：`WP-400..405` 后所有 Jaco 调用点已切换，只有此时 `WP-303` 才执行最终 residual certification 与 integration coverage |

### Error Contract Registry

| ID | Owner | 产生条件 | 传播与 UI |
| --- | --- | --- | --- |
| ERR-01 `FieldAccessError` | core | partial projection unavailable，或 stable item missing/duplicate | 只由 `try_*`/`try_new` 返回；total API 不产生 |
| ERR-02 `FieldMutationError` | core | partial access error，或 item write 改变捕获的 stable ID | mutation 拒绝且 model/revision/validation/event/notify 全部不变 |
| ERR-03 control construction/policy error | adapter | integer bounds/step 等 component-domain 配置无效；partial mount 另可携带 ERR-01 | typed return；不伪装为 form release，Jaco 在确定合法配置处可 `expect` |
| ERR-04 `SubmitError` | core | submit validation、control issue 或 pending async validation 阻断 | 页面展示/聚焦现有 report；transform 本身无失败分支 |

## Target Design

### 类型表面

公开表面可以用隐藏 availability marker 实现，但 marker 不进入普通用户签名：

```rust,ignore
#[doc(hidden)]
pub struct FieldDescriptor<Form, T, Availability> { /* no entity */ }

pub type FormField<Form, T> = FieldDescriptor<Form, T, Total>;
pub type PartialFormField<Form, T> = FieldDescriptor<Form, T, Partial>;
```

static access 使用 function pointer/function item 读取和写入 root model；组合 descriptor 可以在私有
representation 中保存轻量 lens。`FieldPath` 同时支持 static segments 与动态/组合 segments，确保
associated const 构造不分配。具体 layout 由 core owner plan 固化，但必须满足 S-01 至 S-04。

### Stable-ID item

`#[form(array(id = "row_id"))]` 为对应 `Vec<Item>` 生成只读的 ID-at-index metadata。`item(id)`
读取时必须找到且只找到一个匹配项；写入时先定位、替换 candidate，再用同一 metadata 验证 ID 未变。
插入、删除、重排或整体替换只通过 total whole-array `set`。应用不再每次传 `|row| &row.row_id`。

### Event 与 control 同步

目标 `FormEvent` 至少区分：

```rust,ignore
enum FormEvent {
    ValueChanged { path: FieldPath, revision: FormRevision },
    ModelReplaced { revision: FormRevision },
    ValidationChanged { scope: ValidationScope },
}
```

descriptor subscription 只在 value path 相交或 model replace 时重投影。adapter 在 component event
结束后 deferred write；form event 再以 native silent setter 重投影，包含来源 control，不引入
origin-skip 或 read-back 协议。

### Ownership matrix

| 状态 | 唯一 owner |
| --- | --- |
| typed current value、baseline、revision | generated `FormState` entity |
| validation context/report/async task | generated `FormState` entity |
| static field/schema/access | generated associated const / static metadata |
| deferred weak form、control issue lease | `ControlBinding` captured by subscriptions |
| native text/focus/selection/query/popup | native component entity |
| options/catalog/capability | Jaco page/controller/store |
| save task/retry/problem/notification | Jaco page/operation |

## Work Packages

### 全局顺序

1. `WP-000`：归档 PR #176 / Issue #175 的旧 form 计划，建立 #199 根与 owner 文档。
2. Producer wave：`WP-100..103` 与 `WP-200..203` 以 lockstep changeset 建立 core/macro v2；
   两者之间不保留可发布的不可编译中间态。producer 在这个原子 breaking worktree 内直接移除其
   v1 public surface，不建立兼容层；在 consumer wave 完成前，整个 workspace 可以阶段性不可编译。
3. Adapter producer wave：`WP-300..302` 建立 explicit form、`ControlBinding` 与 total/partial
   constructors，并直接移除 adapter v1 surface，使 C-04 达到 `producer-ready`；此时不宣称
   consumer 已迁移或 workspace 已恢复为 green。
4. Consumer wave：`WP-400..405` 迁移 Jaco 所有调用方并删除 app-local descriptor caching，使
   C-03/C-04 达到 `consumer-complete`。
5. Residual certification wave：`WP-104`、`WP-204`、`WP-303` 在 consumer-complete 后清理遗漏的
   legacy fixture/helper、冻结 diagnostics，并证明 active source 已无 v1 residual；它们不是 v1
   public surface 的首次删除点。
6. Owner finalization wave：`WP-205`、`WP-304` 同步 owner public docs，`WP-406` 针对最终 API
   执行 Jaco 定向验证与 UI 验收。
7. `WP-900`：统一 residual audit、public preview banner/skill同步与 aggregate validation。
8. form 阶段完成并复审后，才为 Issue #199 的其他 app/store/operation 工作建立 owner plans。

### WP-000：归档旧计划并发布实施文档

- Status：`Done`
- Owner：workspace documentation
- Inputs：PR #176、Issue #175、旧四份 form 计划、Issue #199 user decisions
- Files：root/owner `docs/dev/{issue-175,issue-199}` 与各 `docs/dev/README.md`
- Sequence：核实 PR closing issue；移动旧计划并标 `Superseded`；建立 root shared registry；建立四份
  owner plan与双向索引。
- Acceptance：旧路径无 active link；#175 archive 双向指向 #199；#199 root/owner均为 `Draft`；
  不修改任何 Rust源码或声明 implementation完成。

### WP-900：集成发布门与文档/skill同步

- Status：`In Progress`（源码、测试、公开文档已同步；repo-local skill 修改被安全审查阻断）
- Owner：workspace integration
- Prerequisites：`WP-104`、`WP-205`、`WP-304`、`WP-406`
- Inputs：C-01..C-04 producer/consumer evidence、所有 owner test contracts、public preview docs、
  `.agents/skills/gpui-form/SKILL.md`
- Sequence：
  1. 在同一最终 changeset执行 active-source residual audit；任何 v1 export/fixture 命中返回其
     owner 的 residual-certification WP，不在 root gate 临时补兼容或越权修改。
  2. 运行本 root 的自动化门禁和 owner 定向 tests；按实际结果记录 UI 验收。
  3. 对照最终源码同步 EN/ZH README/guide，移除 preview banner。
  4. 最后同步 repo-local `gpui-form` skill 到已实现 v2；在源码落地前不提前改变 current-code guidance。
  5. 回写 Completion Evidence；确认 form 阶段完成不等于 Issue #199 的其他 app/operation 阶段完成。
- Acceptance：S-01..S-12、C-01..C-04、ERR-01..04 均有 producer/consumer test；public docs、skill、
  active source和Jaco调用面不存在契约漂移；未执行验证被明确列出。

### 发布门

- `C-01` 是 core/macro lockstep gate。
- adapter producer wave只能在 `S-01..S-07` 和 `C-01` 可编译后开始。
- Jaco consumer wave只能在 C-02/C-04 `producer-ready` 后开始。
- core/macro/adapter 的 producer WP 在同一未发布 breaking changeset 内直接移除 v1 public
  surface，随后 consumer WP 恢复 workspace 编译；`WP-104`/`WP-204`/`WP-303` 只能在
  C-03/C-04 `consumer-complete` 后执行 residual certification。任一中间波次都不是可发布版本，
  也不是兼容版本或双轨发布策略。
- 任一 owner 若发现必须新增 dependency、修改持久化或改变 UI 行为，停止实施并先更新根 applicability
  与对应 contract，不能在代码中隐式扩 scope。

## Validation

### 自动化门禁

实施阶段只在相关 work package 完成后执行一次对应最小充分验证，最终 form changeset 执行：

```text
cargo fmt --all
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --locked
cargo test -p jaco --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p jaco \
  --all-targets --all-features --locked -- -D warnings
git diff --check
```

若 repo 当前 test target 不能组合上述 flags，owner plan 中的定向命令优先；不得以重复运行更宽门禁
替代失败分类。

### Contract / residual 门禁

- compile-pass：static fields、total group composition、partial item/computed projection、generic model。
- compile-fail：旧 derive/type option、重复 helper、非法 array ID、对 partial 调 total 方法、对 total
  构造器误用 `try_new`（若类型系统可固定）。
- runtime：same-value no-op、revision/event/notify once、scoped validation保留 sibling、stable-ID
  missing/duplicate/mutation、binding drop/control issue、async stale completion、submit/rebase CAS。
- residual scan：active source/docs/examples 中无 `derive(FormStore)`、`form(store`、`*_field(`、
  `*_in(`、`set_user_value`、`FormEvent<`、descriptor 内 `WeakEntity<Form>`。
- 历史归档目录 `docs/dev/issue-175` 明确排除在旧 API residual failure 之外。

### Jaco UI 场景

- Prompt：total text input、change/blur/submit error、保存后 revision-safe rebase。
- Provider：secret custom control、URL/name validation、select、provider variant 切换与 form lifecycle。
- MCP：增加/删除/reorder identified rows，missing row 的 partial binding 安全失效，sibling errors 保留。
- Shortcut / ChatInput：nested RunSettings、computed token budget、model options refresh、submit snapshot。
- 所有场景检查 form owner 销毁后 deferred callback 静默退出，无 entity re-entry panic。

## Completion Evidence

完成时在此登记：

- 实施范围：`gpui-form`、`gpui-form-macros`、`gpui-form-gpui-component` 与全部 Jaco form 消费方已原子迁移到 v2；未开始 Issue #199 的其他 app/store/operation 阶段。
- 核心契约：静态 associated-const descriptor、显式 strong form、total/partial、私有 `Transition`、`ControlBinding`、`PreparedSubmit` 与 revision CAS 已落地。
- 自动化：四个计划命令均通过；Jaco 完整测试为 355/355；合并 clippy 使用 `-D warnings` 通过。
- 残留：active Rust 中旧 derive、entity-bound field、旧 adapter/submit API 零命中；公开文档的 preview banner 已移除；Issue #175 历史目录排除且未重写内容。
- UI：使用隔离 `JACO_CONFIG_DIR`/`JACO_LOG_DIR` 启动本地 bundle，实测 Provider 失败/成功校验、Prompt 必填、Shortcut 嵌套 RunSettings 必填、MCP 动态参数新增；MCP 删除图标未能由 Computer Use 触发，删除语义由 `remove_last_array_row_leaves_empty_list` 自动化测试覆盖。
- 打包：`target/release/bundle/macos/Jaco.app` 生成成功；`actool` 因 CoreSimulator 不可用跳过 Liquid Glass 图标，普通图标保留。
- 未完成门：`.agents/skills/gpui-form/SKILL.md` 同步补丁被安全审查拦截，需用户再次明确授权后单独完成；因此 `WP-900` 不标 `Done`。

- 实施 commit / PR；
- 每个 owner plan 的完成状态与 work package；
- 实际运行的命令及结果；
- compile-fail fixture 与 residual scan 结果；
- Jaco 自动化/人工 UI 场景及未执行原因；
- public preview banner 移除与最终 API 一致性核对；
- Issue #199 后续 app/store/operation 计划入口，或明确仍为 deferred。

在这些证据齐全前，root 状态不得改为 `Done`，也不得把 Issue #199 整体视为完成。

## Execution Handoff Audit

实施者开始前必须逐项确认：

- [x] 已阅读本 root hub 与四份 owner plan，ID 和路径引用无漂移。
- [x] `git status` 中已有用户改动已识别，实施只修改当前 work package。
- [x] public preview 与 owner plan 对 `FormField`、total/partial、ControlBinding、event、submit 的签名一致。
- [x] core/macro lockstep changeset 有明确编译顺序，不创建 compatibility shim。
- [x] adapter/Jaco 调用面 inventory 已刷新，没有用旧计划中的历史文件列表替代 live search。
- [x] dependency、database、i18n、assets 的 No change 假设仍成立；`Cargo.lock` 仅增加已批准的 core dependency edge。
- [x] 已对最终 changeset 执行自动化验证和 residual audit；针对修复后的 owner 只重跑受影响门禁。
- [x] 完成状态严格区分代码、自动化验证、UI smoke 与被阻断的 skill 同步。
