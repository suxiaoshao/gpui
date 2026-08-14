# gpui-form 运行时破坏性重构实施计划

## 状态与职责

- 状态：`Done`（2026-08-09）。`C-900`–`C-904` 已达到 `consumer-complete`；实际 UI 操作测试按范围未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 权威设计：[Form 当前设计草稿](design-draft.md)
- 主所有者：`crates/gpui-form`；本文同时拥有 `C-900`–`C-904` 的跨 owner producer contract。
- 协同 owner 计划：[gpui-form-macros](../../../../gpui-form-macros/docs/dev/issue-199/form-schema-generation-update-plan.md)、
  [gpui-form-gpui-component](../../../../gpui-form-gpui-component/docs/dev/issue-199/form-binding-adapter-update-plan.md)、
  [Jaco](../../../../../app/jaco/docs/dev/issue-199/form-breaking-api-remigration-plan.md)、
  [Feiwen](../../../../../app/feiwen/docs/dev/issue-199/form-breaking-api-remigration-plan.md)。
- 兼容策略：允许 breaking change，不保留旧 API、旧事件、旧 binding 或旧 identity 的兼容层。
- 本地稳定编号：`E/D/F/L/ST/ERR/R/T-900..999`、`WP-900..909`。
- 共享生产者契约：`C-900`–`C-904`。本文是五份执行计划消费这些契约时的唯一 producer contract
  权威入口；消费者计划只能引用，不得另行解释。
- 验证边界：执行自动化测试、编译、Clippy、rustdoc/compile fixture 与残留扫描；本轮明确不执行实际
  UI 操作测试或 Computer Use。

本文把权威设计转换为可逐项执行和验收的实现契约。若实现阶段发现本文与
[设计草稿](design-draft.md)冲突，以设计草稿为准，并先更新本文再继续实现；不得通过兼容 wrapper、
隐式 fallback 或弱化类型约束绕开冲突。

## 目标

1. 用 session-local、无碰撞且不可伪造的 `PathKey` 表示公开 UI identity，用真实
   `CanonicalAddress` 表示 crate-private 地址关系。
2. 由 Form runtime 为 item、active case 与 active `Some` 分配单调且不复用的 occurrence identity，
   并精确处理保留、退休和跨父节点迁移。
3. 将一次成功 mutation 归约为一个 `ChangeSet`、一次 revision、一个 typed `FormEvent` 和一次
   `cx.notify()`；失败必须在可见提交前原子拒绝。
4. 用 `ModelChange::impact/affects` 提供精确、sealed、typed 的公开影响查询，不公开内部
   `CanonicalAddress`、控件来源或 topology 细节。
5. 用不可 clone 的 `ControlBinding`、可 clone 的弱能力 `ControlWriter` 与单槽 projection mailbox
   统一所有控件的来源抑制、合并、退休和销毁语义。
6. 让一次 validation run 只读取一个不可变 model/topology snapshot；同步和异步结果都绑定明确的
   session、revision、path occurrence 与 validation generation。
7. 用 `FormVersion` 取代裸 revision CAS；`Prepared::map` 保留版本，
   `rebase_if_current` 只接受同一 session 的当前版本。
8. 只在 crate-private runtime 中使用 `gpui_operation::Transition` 归约 Form、binding 和 validation
   状态；不把消息或 operation 类型暴露给宏、adapter 或应用。

## 非目标

- 不保留或废弃期兼容旧 `FormEvent`、`ControlLease`、cloneable
  `ControlBinding<Root, T>`、地址 hash `PathKey`、`rebase_if_revision`、`try_new`/
  `try_new_with_validator` 或 adapter 私有订阅 helper。
- 不把 UI entity、focus、IME、selection、popup、未完成输入文本或 native component 内部状态放进
  Form。
- 不把 query/fetch/save/loading/retry、catalog/options、数据库或其他业务 operation 放进 Form。
- 不改变 GPUI 组件的视觉样式、布局或交互文案。
- 不迁移 HTTP Client、Novel Download，也不处理 MCP runtime。
- 不执行实际 UI 操作测试；自动化 adapter 测试必须覆盖本轮可验证的交互协议。

## 适用性矩阵

| 表面 | 适用性 | 本轮结论 |
| --- | --- | --- |
| Form schema 与派生 | 适用 | 增补 runtime 构建/遍历所需的隐藏契约，公开 typed descriptor 保持权威设计形状 |
| Path 与 topology | 适用 | 重写 identity、address、occurrence、snapshot 与动态 resolver |
| 变更与公开事件 | 适用 | 引入私有 `ChangeSet` 和公开 typed `ModelChange`/`PathImpact` |
| Control 绑定 | 适用 | 改为 lifecycle owner、writer capability 和 mailbox |
| 验证与提交 | 适用 | snapshot request、freshness、`FormVersion`、`Prepared` |
| `gpui-operation` | 适用但私有 | 只实现 crate-private `Transition`，不使用 `refresh`/`repair` |
| Garde 适配器 | 适用 | 必须消费同一次 validation snapshot，不能重读 live form |
| gpui-component 适配器 | 适用 | 四个 adapter 统一接入 binding 协议，删除各自 event 订阅/回写判断 |
| 持久化、数据库、网络、权限 | 不适用 | Form 只管理内存编辑 session，不新增相关协议 |
| UI 视觉与可访问性 | 无变更 | 不修改布局、视觉或焦点协议；不做实际 UI 操作测试 |
| 依赖与 feature | 无新增 | 继续使用现有 `gpui-operation`、`garde-adapter` 和 workspace 依赖 |
| 打包、资源、本地化 | 不适用 | 不修改资源、bundle 或用户可见文案 |

## 实施前源码证据

以下证据只描述实施前源码事实，不表示目标设计已经实现。

| ID | 当前事实 | 位置 | 对实施的约束 |
| --- | --- | --- | --- |
| `E-900` | `PathKey` 当前保存 session、address hash、incarnation，`ElementId` 也由该 hash 格式化 | `crates/gpui-form/src/topology/address.rs` | 必须改为 runtime interned `Arc<PathIdentity>`；禁止用 hash 代替真实身份或地址 |
| `E-901` | `ensure_incarnation`、`ensure_items` 可在读取路径时分配 identity，topology 由 `RefCell` 隐式修改 | `crates/gpui-form/src/topology/index.rs`、`crates/gpui-form/src/path.rs` | 只读 resolve/snapshot/key 查询必须零分配；所有分配进入 staged edit |
| `E-902` | current event 是非泛型 `Committed/ModelReplaced/ValidationChanged`，只携带单个 `PathKey` | `crates/gpui-form/src/form.rs`、`crates/gpui-form/src/form/transition.rs` | 无法表达 aggregate、structure 与 retirement，需整体替换 typed event |
| `E-903` | binding 当前可 clone，另有 `ControlLease`，并由 adapter 各自订阅 `FormEvent` | `crates/gpui-form/src/control.rs`、四个 adapter 源文件 | 必须把订阅、过滤、来源抑制与 mailbox 收回 binding runtime |
| `E-904` | validation request 借用 live model/topology，path key 构造仍可触发 ensure 逻辑 | `crates/gpui-form/src/validation.rs`、`crates/gpui-form/src/garde.rs` | 每次 validation 必须先冻结 snapshot，所有 resolver/issue 使用同一份快照 |
| `E-905` | `Prepared` 只保存裸 `FormRevision`，Form 暴露 `rebase_if_revision` | `crates/gpui-form/src/submit.rs`、`crates/gpui-form/src/form.rs` | 裸 revision 不能拒绝跨 session 冲突，需改为 opaque `FormVersion` |
| `E-906` | Form 已私有使用一层 `Transition`，但只归约简单 revision/event；binding 没有 reducer | `crates/gpui-form/src/form/transition.rs`、`crates/gpui-form/src/control.rs` | 保留私有方向并重写完整状态/effect，不把 transition 泄漏到 public API |
| `E-907` | 宏已有独立 definition、driver、validation 展开模块与 trybuild fixture | `crates/gpui-form-macros/src/derive/expand/*`、`tests/ui.rs` | runtime 隐藏契约应由 derive 生成，并用 pass/fail fixture 固定 |
| `E-908` | adapter 已各自持有 native entity 与 subscription，四份代码重复 total/dynamic 同步 | `crates/gpui-form-gpui-component/src/{input,select,combobox,integer_input}.rs` | 保留 native ownership，删除重复 Form 订阅并统一 binding façade |

## 共享生产者契约

| ID | 契约 | 达成条件 |
| --- | --- | --- |
| `C-900` | typed schema/path/topology identity | 构造器、total/dynamic resolver、runtime occurrence、`PathKey`/`CanonicalAddress` 语义全部通过 core 与 compile fixture |
| `C-901` | mutation/change/event | 每种 public mutation 都有唯一 `ChangeSet` 映射；公开 typed event 的 `impact/affects` 与一次发布语义通过测试 |
| `C-902` | control binding/adapter | `ControlBinding`/`ControlWriter`/mailbox/source suppression 契约在四个 adapter 上一致并通过自动化测试 |
| `C-903` | validation/prepare/version | snapshot-bound validation、异步 freshness、`Prepared<FormVersion>` 与 CAS 契约通过测试 |
| `C-904` | private transition/atomic rollout | `producer-ready`：core Transition 保持私有、macro/adapter 不依赖内部协议、失败零可见副作用且 focused gate 通过；`consumer-complete`：Jaco/Feiwen 迁移后旧 surface 清零并通过 aggregate gate |

`C-900`–`C-904` 不允许部分发布，依赖固定为两个单向阶段：

1. 生产者阶段：core `WP-900`–`WP-905`、macro `WP-1000`–`WP-1003` 与 adapter
   `WP-1100`–`WP-1103` 协同完成，先让三个 Form crate 一起达到 `producer-ready`。此阶段不依赖 Jaco
   或 Feiwen 的迁移结果。
2. 消费者阶段：Jaco 与 Feiwen 只在 `producer-ready` 后按各自计划开始迁移；两个 app 完成旧 surface
   清零与汇总门禁后，才把 `C-904` 推进到 `consumer-complete`。

因此 `consumer-complete` 不是任何生产者工作包的前置条件，消费者也不会反向阻塞
`producer-ready`，依赖图不存在环。

## 架构决定

### `D-900`：权威设计与 breaking 边界

- [设计草稿](design-draft.md)定义公开 API 和运行时语义，本文只规定实现路径与验收。
- 所有旧 surface 直接删除，不添加 deprecated alias、双事件发送、双 binding 模式或 fallback resolver。
- 三个 Form crate 和两个当前消费者采用原子 rollout；工作包内部的临时编辑状态可以短暂不编译，但
  不得把 `todo!`/panic stub 或不可编译状态作为提交、交接或完成门禁，最终必须满足 `C-904`。

### `D-901`：身份与地址分离

- `CanonicalAddress` 是 crate-private 的真实结构地址，必须由稳定字段/case 标识与 runtime occurrence
  segment 组成；所有祖先、后代、相交、retire scope 和 structure scope 都直接比较 segment。
- `PathKey` 是公开 opaque handle，内部为 `Arc<PathIdentity>`；`PathIdentity` 保存 session-local
  `OpaquePathId` 与对应 `CanonicalAddress`。
- `Eq`、`Hash` 和 `ElementId` 只使用 `(SessionId, OpaquePathId)`；不能再使用 address hash。
- total path identity 在一次 session 内稳定；dynamic path identity 只在 occurrence 存活期间稳定。
- `PathKey` 不公开 raw getter、serde、稳定 `Display` 或构造器；`Debug` 只显示脱敏 session/id，不显示
  业务字段名、case 名或地址内容。

### `D-902`：topology occurrence 与只读快照

- item、active case、active `Some` 都分配独立、单调、不复用的 `OccurrenceId`。
- same-parent reorder 保留 item occurrence；remove/reinsert、case 重建、`Some` 重建、cross-parent move、
  whole-model replace/reset/rebase 退休旧 occurrence。
- cross-parent move 在目标父节点创建新 occurrence，并返回新的 destination `ItemPath`；旧 path 必须
  永久退休。
- Form 构造时完整 materialize 当前 model 所需的 total identity 与活动 topology；mutation preflight 在
  `TopologyEdit` 中预分配全部新 occurrence/path identity。
- `TopologySnapshot` 是一份不可变、revision-bound 视图；`get`、`try_get`、`items`、resolver、validation
  和 `PathKey` 查询只能 lookup，不能 `ensure_*` 或修改 index。
- identity 计数器耗尽是内部不可恢复 invariant failure；公开构造器保持 infallible，不恢复到 hash、index
  或业务 ID。

### `D-903`：ChangeSet 与类型化事件

- `ChangeSet` 只在 core 内存在，分别记录 value、structure、retired 三类事实；validation 影响由 validator
  scope 单独计算，不能混入公开 model impact。
- `FormEvent<M>` 只公开 `ModelChanged(ModelChange<M>)` 与
  `ValidationChanged { revision }`。
- `ModelChangeKind` 固定为 `Edit/Replace/Reset/Rebase`。
- `ChangeTarget<M>` 是 sealed trait，仅由 schema defs、total/dynamic/item paths 与 `PathKey` 实现；应用
  不能自定义地址解析。
- `PathImpact` 是私有 bitset 的公开值类型，至少提供 `value_changed/structure_changed/retired/is_affected`；
  不公开内部 address 或控件 origin。
- 一次 model transaction 至多发一个 `ModelChanged`；同步 validation 在同一 transaction 内变化时，
  不再额外发送第二个 `ValidationChanged`。只有 model 未变化而 validation 可见状态变化时才发送
  `ValidationChanged`。

### `D-904`：绑定生命周期与来源抑制

- `ControlBinding` 非泛型、不可 clone，是 subscription、registration、mailbox 与清理的唯一 owner；drop
  后进入 `Dropped` 并使所有排队工作失效。
- `ControlWriter<Root, T>` 可 clone，只保存 weak form capability、`ControlId`、lifecycle generation、
  occurrence proof 与单调 editor sequence；只能提交 native event，不拥有 subscription。
- `ControlProjection<T>` 只含 `Value(T)` 与 `Retired`；不能用 `Option<T>` 混淆空值和退休。
- native event 先递增 editor sequence，再 defer writer command；Form 收到有效 command 后比较新值。
  值相等时仍清理该控件自己的 issue，但不增加 model revision、不发送 model event。
- 写入 origin 与 binding 的 `ControlId/lifecycle generation/occurrence proof/editor sequence` 一致时，
  该 binding 不回投同值；其他 binding 仍按影响范围收到最新 authoritative projection。
- 每个 binding 最多只有一个待执行 drain。新外部值覆盖旧 `Value`，`Retired` 覆盖任何 `Value`，旧
  editor sequence 的 projection 不得覆盖更新的 native edit。
- Form event callback 只能分类并填 mailbox；native entity update 必须 defer 到 Form borrow 结束后。
- dynamic binding 退休时最多交付一次 `Retired`；total binding 在 replace/reset/rebase 后保持存活并投影
  新值；Form owner 消失或 binding drop 后不再回调。

### `D-905`：验证快照与新鲜度

- 每次 validation run 先冻结一份 model clone 与 topology snapshot，生成一个
  `ValidationRequest<'_, M>`；validator 只能通过 `request.model()` 和 snapshot-bound path API 读取。
- `request.items`、case/optional resolver、`request.get/try_get` 与 `ValidationSink` 必须使用同一 snapshot；
  不能在 validator 中读取最新 live Form 或隐式分配 identity。
- 默认只在 Submit 验证；Mount/Change/Blur 只按 schema trigger 开启，手动触发统一命名为 External。
- mutation 的 `ChangeSet` 单独推导 validation scopes；只重跑受影响 validator scope，并精确清除已
  退休 occurrence 的 issue。
- async validation completion 必须同时匹配 `FormVersion`、path occurrence 与 validation generation；
  任一不匹配就静默丢弃，不能污染新 model、重建节点或新一轮验证。
- pending task 的 snapshot proof 使用全局 `FormVersion`，所以任何 model revision 都在该次 model publish 前
  取消全部 pending task；已经完成的 async issue 仍只按与 mutation 相交的 validation scope 失效。

### `D-906`：Prepared 与会话绑定 CAS

- `FormVersion` 是 opaque、可复制/比较的 `(SessionId, FormRevision)` 事实，不提供用户构造器。
- `Prepared<T>` 保存 `FormVersion` 与 value；公开 `version/value/map/into_parts`。
- `Prepared::map` 必须原样保留 version。
- `rebase_if_current(version, value, cx)` 仅在 session 和 revision 都匹配时提交 rebase；不匹配返回
  `false`，model、baseline、topology、validation、task、revision、event 与 notify 全部不变。
- 删除 `Prepared::revision` 与 `rebase_if_revision`，不保留 alias。

### `D-907`：私有 Transition 与变更顺序

- `gpui_operation::Transition` 分别用于 Form revision/lifecycle/effect、binding state/mailbox，以及确有
  多状态竞态的 validation task runtime；全部类型保持 crate-private。
- 不使用 `gpui_operation::refresh` 或 `repair` 预定义状态机。
- 每次 mutation 固定执行以下顺序：
  1. 从当前 immutable model/topology snapshot 解析并完成 session/freshness/precondition 检查；
  2. 在临时 model edit plan、`TopologyEdit`、`ChangeSet` 和 validation work 中预分配全部资源；
  3. 任一步失败即丢弃 staged data，Form 可见状态保持逐字段相等；
  4. 原子安装 model 与 topology，清理 retired issue/task，运行同一 transaction 的同步验证；
  5. 向 Form reducer提交一条成功消息，增加一次 revision 并得到一个 publish effect；
  6. 依据 `ChangeSet` 分类 active binding 并只填 mailbox；
  7. 发布一个 typed event并调用一次 `cx.notify()`；
  8. 在 Form borrow 结束后执行各 binding 的单一 deferred drain。
- reducer 不读取 GPUI context、不直接更新 native entity；effect 是外部副作用的唯一出口。

### `D-908`：适配器统一协议

- `FormInput`、`FormSelect`、`FormCombobox`、`FormIntegerInput` 继续拥有 native component entity、
  native subscription 与一个 `ControlBinding`。
- 构造时通过 total `bind_control_in` 或 dynamic `try_bind_control_in` 一次性取得
  `(ControlBinding, ControlWriter)`；adapter 不再直接订阅 `FormEvent`。
- binding projection 是 Form 到 native 的唯一同步入口；native event 捕获 writer 是 native 到 Form 的
  唯一入口。
- IntegerInput 的未完成文本、parse/policy 错误继续属于 native adapter；只有成功解析值和 control issue
  进入 Form。四个 adapter 不实现本地 direction guard。

## 目标类型与接口契约

### `L-900`：身份与地址

```rust,ignore
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SessionId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct OpaquePathId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct OccurrenceId(u64);

#[derive(Clone, PartialEq, Eq, Hash)]
struct CanonicalAddress(Arc<[AddressSegment]>);

enum AddressSegment {
    Field(FieldId),
    Item(OccurrenceId),
    Case(CaseId, OccurrenceId),
    Some(OccurrenceId),
}

struct PathIdentity {
    session: SessionId,
    id: OpaquePathId,
    address: CanonicalAddress,
}

#[derive(Clone)]
pub struct PathKey(Arc<PathIdentity>);
```

`PathKey` 手写 `PartialEq/Eq/Hash/Debug/From<PathKey> for ElementId`；identity intern table 以
`CanonicalAddress` lookup 活动 `Arc<PathIdentity>`，但相等与 hash 只使用 session/id。retire 从 active
lookup 移除 address，已发出的 `Arc` 仍可安全比较但不能重新解析为当前 path。

### `L-901`：topology 暂存与快照

```rust,ignore
struct TopologyData {
    next_occurrence: u64,
    next_path_id: u64,
    items: HashMap<CanonicalAddress, Vec<ItemToken>>,
    active_cases: HashMap<CanonicalAddress, (CaseId, OccurrenceId)>,
    active_optionals: HashMap<CanonicalAddress, OccurrenceId>,
    identities: HashMap<CanonicalAddress, Arc<PathIdentity>>,
}

struct TopologyIndex {
    session: SessionId,
    data: RefCell<Arc<TopologyData>>,
}

struct TopologyEdit {
    session: SessionId,
    data: TopologyData,
}

struct TopologySnapshot {
    session: SessionId,
    data: Arc<TopologyData>,
}
```

- `TopologyIndex::stage()` clone 出本次 edit；只有 `TopologyEdit` 暴露私有 allocate/insert/move/retire。
- `TopologySnapshot` 只暴露 lookup/resolve/intersects/identity 查询。
- `FormSchema::__visit` 是 derive 生成的隐藏 traversal authority；构造、whole-model install 和 snapshot
  校验都使用同一 driver，禁止另写一套反射规则。
- total identity 在 Form 构造时 materialize；动态 identity 在 occurrence 创建时 materialize。

### `L-902`：ChangeSet 与影响枚举

```rust,ignore
struct ChangeSet {
    value: ValueImpact,
    structure: StructuralImpact,
    retired: RetiredImpact,
}

enum ValueImpact {
    Scopes(Vec<ValueScope>),
    All,
}

enum ValueScope {
    SubtreeReplaced(CanonicalAddress),
    AggregateChanged(CanonicalAddress),
}

enum StructuralImpact {
    None,
    Roots(Vec<CanonicalAddress>),
    All,
}

enum RetiredImpact {
    None,
    Roots(Vec<CanonicalAddress>),
}
```

所有 scope 在 stage 结束时做语义感知的归一化：完全相同的 scope 去重，retired subtree root 做 ancestor
压缩。`AggregateChanged(collection)` 必须保留每个实际变更的 collection；即使 destination 位于 source
collection 的后代，cross-parent move 也不能用 source ancestor 抹掉 destination，因为 aggregate 只影响
collection 本身及其 ancestor，不隐含后代 collection 变化，也不把未变 sibling item 标记为 value changed。
`SubtreeReplaced(root)` 影响 root 及后代。structure 与 retired 分别计算，不从 value scope反推。

### `L-903`：公开 typed event

```rust,ignore
pub enum FormEvent<M: FormSchema> {
    ModelChanged(ModelChange<M>),
    ValidationChanged { revision: FormRevision },
}

pub enum ModelChangeKind {
    Edit,
    Replace,
    Reset,
    Rebase,
}

pub struct ModelChange<M: FormSchema> { /* 私有字段 */ }
pub struct PathImpact { /* 私有位集合 */ }

impl<M: FormSchema> ModelChange<M> {
    pub fn revision(&self) -> FormRevision;
    pub fn kind(&self) -> ModelChangeKind;
    pub fn impact(&self, target: &impl ChangeTarget<M>) -> PathImpact;
    pub fn affects(&self, target: &impl ChangeTarget<M>) -> bool;
}

impl PathImpact {
    pub fn value_changed(&self) -> bool;
    pub fn structure_changed(&self) -> bool;
    pub fn retired(&self) -> bool;
    pub fn is_affected(&self) -> bool;
}
```

`affects` 必须等价于 `impact(target).is_affected()`。对 wrong-session 或已退休的 `PathKey`，
`impact` 只依据事件中冻结的 session/change facts 计算：wrong-session 返回空 impact；本次 retirement 命中返回
`retired=true`；事件创建后不得回读最新 Form/topology。

### `L-904`：绑定 API

```rust,ignore
pub struct ControlBinding { /* 不可 Clone 的生命周期所有者 */ }

#[derive(Clone)]
pub struct ControlWriter<Root: FormSchema, T: 'static> { /* 弱写入能力 */ }

pub enum ControlProjection<T> {
    Value(T),
    Retired,
}

impl<Root, T> TotalPath<Root, T> {
    pub fn bind_control_in<Owner>(
        &self,
        form: &Entity<Form<Root>>,
        owner: &Entity<Owner>,
        project: impl Fn(&mut Owner, ControlProjection<T>, &mut Window, &mut Context<Owner>)
            + 'static,
        window: &mut Window,
        cx: &mut App,
    ) -> (ControlBinding, ControlWriter<Root, T>);
}
```

dynamic path 提供相同形状的 `try_bind_control_in`，返回
`Result<(ControlBinding, ControlWriter<Root, T>), ResolveError>`。`ControlWriter` 的公开写入 façade 固定为
deferred set、control issue set/clear 与 blur；External validation 仍由应用通过 Form/path 领域方法显式触发。
writer 不能读取 Form、订阅 FormEvent 或
直接投影 native entity。

### `L-905`：验证快照

```rust,ignore
pub trait Validator<M: FormSchema>: 'static {
    fn validate(
        &self,
        request: ValidationRequest<'_, M>,
        out: &mut ValidationSink<'_, M>,
    );
}

impl<'a, M: FormSchema> ValidationRequest<'a, M> {
    pub fn model(&self) -> &'a M;
    pub fn trigger(&self) -> ValidationTrigger;
    pub fn includes(&self, target: &impl ValidationPath<M>) -> bool;
    // `items` / `try_items`、`get` / `try_get`、case / optional resolver
    // 所有结果均为受 request 生命周期约束的验证路径或引用。
}
```

validation traversal 使用独立的 `ValidationItemPath<'a, M, I>`、
`ValidationDynamicPath<'a, M, T>` 与 dynamic-items counterpart；它们支持与 live path 相同的 typed
`then` 组合并实现 `ValidationPath<M>`，但不提供 mutation、binding 或转换回 live `ItemPath` 的入口。
`items`/`try_items` 返回这些 request-lifetime-bound handle，value resolver 返回 snapshot model 内的借用。
case/optional resolver 保持 live API 的 `Result<Option<_>, ResolveError>` 区分，返回的同样是
request-bound validation path。这样 validator 可以 `out.at(path)`，但不能把路径逃逸后拿去读取较新的 Form。

### `L-906`：版本与 Prepared

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FormVersion { /* session 与 revision */ }

pub struct Prepared<T> {
    version: FormVersion,
    value: T,
}

impl<T> Prepared<T> {
    pub fn version(&self) -> FormVersion;
    pub fn value(&self) -> &T;
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Prepared<U>;
    pub fn into_parts(self) -> (FormVersion, T);
}

impl<M: FormSchema> Form<M> {
    pub fn rebase_if_current(
        &mut self,
        version: FormVersion,
        value: M,
        cx: &mut Context<Self>,
    ) -> bool;
}
```

### `L-907`：Form 构造与路径 façade

```rust,ignore
impl<M: FormSchema> Form<M> {
    pub fn new(value: M) -> Self;
    pub fn with_validator<V: Validator<M>>(self, validator: V) -> Self;
}

// total path：全 session 可达
pub fn get(&self, form: &Entity<Form<Root>>, cx: &App) -> T;
pub fn set(&self, form: &Entity<Form<Root>>, value: T, cx: &mut App) -> bool;

// dynamic path：可退休
pub fn try_get(&self, form: &Entity<Form<Root>>, cx: &App) -> Result<T, ResolveError>;
pub fn try_set(
    &self,
    form: &Entity<Form<Root>>,
    value: T,
    cx: &mut App,
) -> Result<bool, ResolveError>;
```

case/optional builder 的 live resolver 固定返回 `Result<Option<DynamicPath<...>>, ResolveError>`：
inactive case/`None` 是 `Ok(None)`，wrong session/stale parent 等 topology 错误是 `Err`。total items 的
`items(&Form)` 为 infallible；dynamic items 的 `try_items(&Form)` 显式返回 `ResolveError`。

## 状态与所有权

| ID | 状态 | 唯一 owner | 转移入口 | 失效条件 |
| --- | --- | --- | --- | --- |
| `ST-900` | current model、baseline、revision | `Form<M>` | staged mutation 成功后由 Form reducer提交 | Form entity drop |
| `ST-901` | session、occurrence、address identity、active topology | `TopologyIndex` | `TopologyEdit` 原子安装 | occurrence retire 或 Form drop |
| `ST-902` | 一次 mutation 的 model/topology/change/validation staged facts | 当前同步调用栈 | preflight/commit pipeline | commit 或任一失败丢弃 |
| `ST-903` | binding subscription、mailbox、generation | `ControlBinding` | binding reducer | binding drop、dynamic retire、Form drop |
| `ST-904` | native 写能力与 editor sequence | `ControlWriter` capability | native event deferred command | generation/occurrence 不匹配或 Form drop |
| `ST-905` | validation model/topology snapshot | 单次 `ValidationRequest` | validation run 创建 | run 结束 |
| `ST-906` | async validation result freshness | Form 的 validation task registry | completion reducer | version/occurrence/generation 任一过期 |
| `ST-907` | submit/rebase CAS 事实 | `FormVersion`/`Prepared<T>` 值 | `prepare` 创建，`map` 传递 | Form session/revision 前进后不再 current |

## 错误、失败与回滚语义

| ID | 条件 | 公开结果 | 可见状态要求 |
| --- | --- | --- | --- |
| `ERR-900` | dynamic path wrong session、parent stale 或 occurrence retired | `ResolveError` | model/topology/revision/issue/task/event/notify 全部不变 |
| `ERR-901` | case inactive或 optional 为 `None` | resolver 返回 `Ok(None)` | 不是错误，不创建 occurrence/path identity |
| `ERR-902` | collection anchor/parent/session/freshness/cycle 不合法 | `MutationError`/`TopologyError` | staged edit 丢弃，零部分提交、零 identity 泄漏到 active index |
| `ERR-903` | total `set` 或 dynamic `try_set` 新值等于当前值 | `false`/`Ok(false)` | model、revision、event 与 validation 均不变；若入口是 writer 的等值提交，则额外原子清理该 writer 对应控件的 issue，必要时只发 validation event |
| `ERR-904` | `FormVersion` 跨 session 或 revision 过期 | `rebase_if_current` 返回 `false` | baseline/model/topology/validation/task/revision/event/notify 全部不变 |
| `ERR-905` | async validation completion 过期 | 静默丢弃 | 不生成 issue、event 或 notify |
| `ERR-906` | binding 对应 dynamic occurrence 退休 | 一次 `ControlProjection::Retired` | 后续投影/写入失效；drop 后不再回调 |
| `ERR-907` | binding owner/Form owner 已 drop | 静默停止 | 不 panic、不恢复 owner、不创建全局 registry |
| `ERR-908` | session/path/occurrence identity 计数器耗尽 | 内部 invariant panic | 不提供 fallible public constructor，不复用 identity、不降级到 hash/index |
| `ERR-909` | validation 失败或仍 pending | 既有 `PrepareError::Validation/ValidationPending` | 不创建 `Prepared`，不改变 model revision |

staged mutation 不维护“反向修改”回滚路径：提交前不碰可见状态，失败直接丢弃 staged model/topology/
change/validation work。提交点之后的代码只能执行不可失败的安装、reducer 和发布步骤；任何可失败分配或
路径解析都必须前移到提交点之前。

## 变更操作到 ChangeSet 的固定映射

| 公开操作 | `ModelChangeKind` | 值影响 | 结构影响 | 退休影响 |
| --- | --- | --- | --- | --- |
| total/dynamic 叶值 `set` | `Edit` | `SubtreeReplaced(path)` | `None` | `None` |
| child/optional/case payload 整体替换 | `Edit` | `SubtreeReplaced(path)` | 活动 topology 改变时为对应 root | 被重建动态后代 roots |
| 集合 `append`/`insert` | `Edit` | `AggregateChanged(collection)` | 集合根 | `None` |
| 同父级重排 | `Edit` | `AggregateChanged(collection)` | 集合根 | `None` |
| 集合 `remove` | `Edit` | `AggregateChanged(collection)` | 集合根 | 已移除元素根 |
| `replace_all` | `Edit` | `AggregateChanged(collection)` | collection root | 旧 item roots |
| cross-parent move | `Edit` | 两个 collection aggregate | source 与 destination roots | source item root |
| 整体模型 `replace` | `Replace` | `All` | `All` | 全部旧 dynamic roots（经 ancestor 压缩） |
| 表单 `reset` | `Reset` | `All` | `All` | 全部旧 dynamic roots（经 ancestor 压缩） |
| `rebase`/成功 CAS rebase | `Rebase` | `All` | `All` | 全部旧 dynamic roots（经 ancestor 压缩） |

实现不得根据最终值“猜测”structure 或 retirement；这些事实必须由同一个 `TopologyEdit` 产生。若同值
set 没有 topology 变化，按 `ERR-903` 处理，不创建上述 model ChangeSet。

## 文件级改动地图

| ID | 文件 | 动作 | 精确职责 |
| --- | --- | --- | --- |
| `F-900` | `crates/gpui-form/src/topology/address.rs` | 重写 | `FieldId/CaseId/OccurrenceId/OpaquePathId`、真实 `CanonicalAddress`、`PathIdentity`、opaque `PathKey` 与脱敏 trait 实现 |
| `F-901` | `crates/gpui-form/src/topology/index.rs` | 重写 | infallible session 创建、active identity intern、occurrence registry、`TopologyEdit`、只读 snapshot、retire 与 lookup |
| `F-902` | `crates/gpui-form/src/topology.rs` | 修改 | 仅 crate-private 导出 topology types；不得公开 raw identity/address |
| `F-903` | `crates/gpui-form/src/path.rs` | 重写 | total/dynamic façade、case/optional resolver、items/mutation、sealed `ChangeTarget` 接入、binding constructors |
| `F-904` | `crates/gpui-form/src/path/access.rs` | 修改 | 所有 typed access 显式消费同一 `TopologySnapshot`，删除读取时 ensure |
| `F-905` | `crates/gpui-form/src/change.rs` | 新增 | `ChangeSet`/scope 语义归一化、public typed event、`ModelChange`/`PathImpact` 与 sealed target matching |
| `F-906` | `crates/gpui-form/src/form.rs` | 重写 | infallible 构造、staged transaction、validation 协调、binding 分类、event/notify publish、version CAS |
| `F-907` | `crates/gpui-form/src/form/transition.rs` | 重写 | crate-private Form message/state/effect reducer；一次成功消息对应一次 revision/publish effect |
| `F-908` | `crates/gpui-form/src/control.rs` | 重写 | public binding/writer/projection façade，registration、weak capability、drop cleanup、deferred command |
| `F-909` | `crates/gpui-form/src/control/transition.rs` | 新增 | `BindingState`、mailbox、generation/editor sequence/source suppression reducer |
| `F-910` | `crates/gpui-form/src/validation.rs` | 重写 | frozen request、snapshot-bound resolver/sink、scope validation、async freshness |
| `F-911` | `crates/gpui-form/src/validation/trigger.rs` | 修改 | `External` 命名与 trigger 选择；删除旧手动 trigger 名称 |
| `F-912` | `crates/gpui-form/src/validation/report.rs` | 修改 | issue 与 active occurrence/version 对齐，退休清理与公开报告稳定化 |
| `F-913` | `crates/gpui-form/src/garde.rs` | 修改 | 只从当前 `ValidationRequest` snapshot 映射 Garde path，不重读 live topology |
| `F-914` | `crates/gpui-form/src/submit.rs` | 重写 | opaque `FormVersion`、`Prepared<T>` 与 version-preserving `map` |
| `F-919` | `crates/gpui-form/src/validation/transition.rs` | 新增 | crate-private async validation task registry 的 reserve/attach/cancel/complete reducer |
| `F-915` | `crates/gpui-form/src/schema/driver.rs` | 修改 | derive 可实现的隐藏 topology/validation traversal authority |
| `F-916` | `crates/gpui-form/src/schema/definition.rs` | 修改 | stable field/case identifiers、trigger metadata 与 sealed target metadata |
| `F-917` | `crates/gpui-form/src/error.rs` | 修改 | 删除 `FormBuildError`，收敛 resolve/mutation/topology/prepare errors 到本计划失败语义 |
| `F-918` | `crates/gpui-form/src/lib.rs`、`src/typed.rs` | 修改 | 只导出目标 public surface；删除旧 alias/lease/revision CAS exports |
| `F-940` | `crates/gpui-form/tests/vnext.rs` | 重组 | 保留 schema/path/mutation 基线并改为新构造器、事件和 version API |
| `F-941` | `crates/gpui-form/tests/path_identity.rs` | 新增 | session/opaque identity、真实地址关系、零读取分配、occurrence 保留/退休 |
| `F-942` | `crates/gpui-form/tests/change_impact.rs` | 新增 | mutation 映射、scope 压缩、`affects/impact` 与 frozen event facts |
| `F-943` | `crates/gpui-form/tests/binding.rs` | 新增 | lifecycle、source suppression、mailbox、deferred drain 与清理 |
| `F-944` | `crates/gpui-form/tests/validation_snapshot.rs` | 新增 | snapshot 一致性、trigger、retire 清理、async freshness、Garde feature |
| `F-945` | `crates/gpui-form/tests/prepared_version.rs` | 新增 | `Prepared::map`、跨 session/旧 revision CAS 与零副作用失败 |
| `F-946` | `crates/gpui-form/README*.md`、`docs/guide*.md` | 核对并按实现同步 | 只描述最终 core public API；macro/adapter 文档由各自 owner 计划负责 |
| `F-947` | `.agents/skills/gpui-form/**` | 核对并按实现同步 | 保持 target API、owner/lifecycle、consumer route 与实际源码一致 |

macro 源码、fixture 与文档只由其 `F-1000..` owner 地图管理；adapter 源码、测试与文档只由其
`F-1100..` owner 地图管理，本文不为同一文件分配第二个 F-ID。

除 `F-905`、`F-909` 和 `F-941`–`F-945` 外，不新增平铺替代现有模块边界；不创建 `mod.rs`。若实现
需要增加本文未列出的源文件，先在本计划中分配新的 `F-9xx` 并写明职责，不能边实现边隐式扩展。

## 需求与测试追踪

| ID | 可验收需求 | 自动化证据 |
| --- | --- | --- |
| `R-900` | `PathKey` 在 session 内无碰撞、不可构造、真实地址不泄漏，ElementId 使用 opaque id | `T-900` |
| `R-901` | total identity 稳定；item/case/Some occurrence 按 `D-902` 保留或退休 | `T-901` |
| `R-902` | read/snapshot/key/validation 查询不分配 identity，不改变 topology | `T-902` |
| `R-903` | 每种 mutation 产生固定 ChangeSet；scope relation 与 retirement 精确 | `T-903` |
| `R-904` | 成功 mutation 一次 revision/event/notify；失败零可见副作用 | `T-904` |
| `R-905` | `impact/affects` 对 defs、total/dynamic/item path、PathKey 结果一致且不回读 live Form | `T-905` |
| `R-906` | self-origin 不回投；other binding 收到 authoritative value；同值写只清本控件 issue | `T-906` |
| `R-907` | mailbox 单槽合并、Retired 优先、editor sequence 阻止旧投影覆盖新编辑 | `T-907` |
| `R-908` | dynamic retire 一次、total lifecycle 后继续、drop/Form drop 后静默 | `T-908` |
| `R-909` | validation 全程使用一个 snapshot，错误只绑定该 snapshot 的 occurrence | `T-909` |
| `R-910` | trigger 默认为 Submit，External/Change/Blur/Mount 按 schema 契约执行 | `T-910` |
| `R-911` | async completion 只有 version+occurrence+generation 全部 current 才生效 | `T-911` |
| `R-912` | `Prepared::map` 保留 version，旧/跨 session version CAS 返回 false 且零副作用 | `T-912` |
| `R-913` | Transition 全部 private，public rustdoc 不出现 operation message/effect/state | `T-913` |
| `R-914` | 四个 adapter 统一协议，native event 不被 Form 立即回投给来源组件 | `T-914` |
| `R-915` | 三个 crate 和 Jaco/Feiwen 的当前生效源码不再引用旧公开接口 | `T-915` |

### 测试定义

| ID | 层级 | 内容 |
| --- | --- | --- |
| `T-900` | core unit/integration | 同地址重复 key 等价、不同地址大量 key 不冲突、跨 session 不等、Debug/ElementId 不含 canonical 字段/case 文本 |
| `T-901` | core integration | same-parent reorder 保留；remove/reinsert、case A-B-A、Some-None-Some、cross-parent、whole-model install 退休 |
| `T-902` | core instrumentation | snapshot 前后比较 next counters 与 topology data，覆盖 get/try_get/items/resolver/key/validator reads |
| `T-903` | table-driven core | 覆盖 Mutation 到 ChangeSet 固定映射表，断言 retired ancestor 压缩、nested cross-parent aggregate roots 均保留、aggregate sibling 不误报 |
| `T-904` | GPUI test context | 成功调用计数 revision/event/notify；wrong-session/stale anchor/cycle/identity preflight failure 比较完整可见状态 |
| `T-905` | core event test | 同一 frozen `ModelChange` 在 Form 后续继续变化后仍返回原 impact；wrong-session target 空 impact |
| `T-906` | binding test | 一个来源与第二观察者绑定同一路径，断言来源无重复 projection、观察者收到一次、同值 issue 清理 |
| `T-907` | binding reducer test | 多外部值合并最新、Retired 覆盖 Value、native edit 后旧 drain 不覆盖、同时只排一个 drain |
| `T-908` | binding lifecycle test | dynamic retire exactly once、total replace/reset/rebase projection、drop 清 issue、owner 消失无 callback |
| `T-909` | validation test | validator 内 item/case/optional/value 均解析同一 snapshot；并发 model 变化不混入本次 report |
| `T-910` | validation trigger test | 未提交默认不验证；schema opt-in 的 Mount/Change/Blur 与显式 External 精确运行 |
| `T-911` | async validation test | 分别制造 stale version、retired occurrence、old generation，completion 都静默；全匹配只提交一次 |
| `T-912` | prepared test | map/into_parts version 保留；same session current 成功；old revision/other session false 且全状态不变 |
| `T-913` | rustdoc/compile/residual | public API compile fixture 与 `rg` 扫描不出现 public Transition/message/effect 或旧 surface |
| `T-914` | adapter GPUI automated test | Input/Select/Combobox/IntegerInput 的 native→Form→other control、mailbox、retire、drop；不进行实际 UI 操作 |
| `T-915` | aggregate compile | 三 crate测试、Jaco/Feiwen测试与 workspace check 全部只消费 `C-900`–`C-904` |

## 工作包与依赖顺序

### `WP-900`：冻结公开契约与编译骨架

依赖：无。

改动：`F-915`–`F-918` 的 public/hidden trait 骨架；macro 生成与文档门禁由其
`WP-1000`–`WP-1003` 消费。

1. 按 `L-903`–`L-907` 改 public declarations 与 exports，直接删除旧 surface。
2. 在 core schema driver 中固定 stable field/case ID 与 topology traversal 隐藏入口，并向 macro owner
   交付精确生成签名。
3. 先更新 compile fixture，使目标 API 可编译、旧 API 必须编译失败；不得用 `todo!`、panic stub 或
   兼容 alias 冒充可交接实现。

完成门禁：`C-900` public signatures 可由 fixture 精确引用；旧构造器/event/lease/revision CAS 无法导入。

### `WP-901`：实现身份、occurrence 与快照

依赖：`WP-900` 与 macro `WP-1000`–`WP-1001`；这是有向依赖，不等待 `C-900` producer-ready。

改动：`F-900`–`F-904`、`F-941`。

1. 实现 `L-900`/`L-901`，构造时 materialize 当前 topology。
2. 将所有 occurrence 分配移动到 `TopologyEdit`，删除 read path 中的 `ensure_*`。
3. 实现保留/退休/cross-parent 规则与 snapshot lookup。
4. 完成 `T-900`–`T-902`。

完成门禁：`R-900`–`R-902` 全部通过；任何 read-only API 前后 identity counters 不变。

### `WP-902`：实现 ChangeSet、Form 事务与类型化事件

依赖：`WP-901`。

改动：`F-905`–`F-907`、`F-917`、`F-940`、`F-942`。

1. 实现 `L-902`/`L-903`、scope 语义归一化和 sealed target matching。
2. 所有 mutation 先产出 model edit plan、TopologyEdit 与 ChangeSet，再进入唯一 commit pipeline。
3. 重写 Form reducer，固定一次 revision/event/notify 与失败零副作用。
4. 完成 `T-903`–`T-905`。

完成门禁：`C-901` producer-ready；Mutation 映射表每一行均有 table-driven case。

### `WP-903`：实现验证快照与异步新鲜度

依赖：`WP-902`。

改动：`F-910`–`F-913`、`F-944`。

1. validation run 先冻结 model/topology/version，request/sink/Garde 共享该 snapshot。
2. 由 ChangeSet 推导 validation scope 与 retired issue 清理。
3. async task registry 增加 version/occurrence/generation proof，completion 走私有 reducer。
4. 重命名 External trigger 并完成 `T-909`–`T-911`。

完成门禁：验证过程中没有 live Form lookup；stale completion 全部静默且不 notify。

### `WP-904`：实现 FormVersion 与 Prepared CAS

依赖：`WP-902`；可与 `WP-903` 并行，但 aggregate 前必须合并 freshness 事实源。

改动：`F-906`、`F-914`、`F-945`。

1. 实现 session-bound `FormVersion` 和 version-preserving `Prepared`。
2. 用 `rebase_if_current` 替换裸 revision CAS。
3. 完成 `T-912`，包含逐状态零副作用比较。

完成门禁：`C-903` 的 prepare/version 部分通过；跨 session revision 数字相同也必须失败。

### `WP-905`：实现绑定 reducer、mailbox 与来源协议

依赖：`WP-901`、`WP-902`；使用 `WP-903` 的 control issue/trigger 入口。

改动：`F-908`、`F-909`、`F-943`。

1. 建立 `BindingState::{Active, Retired, Dropped}`、generation 与单槽 mailbox。
2. 实现 `ControlBinding`/`ControlWriter` 职责分离、deferred command 和 deferred projection drain。
3. 用 ChangeSet 分类投影；实现 self-origin、editor sequence、retire/drop/Form-drop 语义。
4. 完成 `T-906`–`T-908`。

完成门禁：Form borrow 内不更新 native entity；每个 binding 同时最多一项 drain task。

### `WP-906`：迁移 gpui-component 适配器

依赖：`WP-905`。

改动：由 adapter owner 的 `F-1100`–`F-1109`、`WP-1100`–`WP-1103` 负责；core 只修正其暴露的
`C-902` producer 缺陷。

1. 四个 adapter 删除直接 FormEvent 订阅和本地 total/dynamic helper。
2. 构造器一次取得 binding/writer；projection 更新 native，native subscription 只调用 writer。
3. 保留 IntegerInput draft/parse policy 的 native ownership。
4. 完成 `T-914`。

完成门禁：`C-902` producer-ready；四个 adapter 不再直接匹配 `FormEvent`。

### `WP-907`：消费者原子迁移与旧 surface 清零

依赖：`WP-900`–`WP-906`。

改动：Jaco/Feiwen 由各自执行计划负责；本工作包只定义 producer gate 与残留验收。

1. 向消费者交付已达 `producer-ready` 的 `C-900`–`C-904`，禁止在 app 内增加兼容 adapter。
2. 两个消费者迁移完成后扫描 workspace 当前生效源码、测试与对外文档中的旧接口。
3. 扫描 producer public rustdoc，确保 Transition/message/effect 均未泄漏。

完成门禁：`T-913`、`T-915` 通过；`C-904` 从 `producer-ready` 推进到 `consumer-complete`。

### `WP-908`：文档、skill 与实现证据同步

依赖：`WP-907`。

改动：`F-946`、`F-947` 与本文的实施证据区；macro/adapter owner 同步各自文档。实施时才修改，
不在计划创建阶段预写完成状态。

1. 对照实际 public API 核对 core 中英文 README/guide，并验收 macro/adapter owner 的对应文档门禁。
2. 对照最终源码核对 gpui-form skill，删除旧调用面和不成立的内部描述。
3. 在本文记录实际提交、实际执行命令与结果；没有执行的检查明确标记。

完成门禁：文档示例只使用最终 API；本文状态只能在所有自动化门禁实际通过后改为 `Done`。

### `WP-909`：汇总自动化验收

依赖：`WP-908`。

按“自动化验证”逐条执行并保存输出摘要。不得用实际 UI 操作测试代替任何失败的自动化门禁，也不得因
本轮不做 UI 测试而跳过 adapter GPUI test context。

## 自动化验证

### 目标测试

```bash
cargo fmt --all -- --check
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component \
  --all-targets --all-features --locked -- -D warnings
```

### 消费者与汇总门禁

```bash
cargo test -p jaco --all-features --locked
cargo test -p feiwen --all-features --locked
cargo check --workspace --all-targets --all-features --locked
git diff --check
```

### 破坏性变更与私有边界残留扫描

```bash
rg -n "ControlLease|ControlBinding<|rebase_if_revision|FormBuildError|try_new_with_validator|Form::try_new\(" \
  crates/gpui-form crates/gpui-form-macros crates/gpui-form-gpui-component app/jaco app/feiwen
rg -n "FormEvent::(Committed|ModelReplaced)|ValidationTrigger::Manual" \
  crates/gpui-form crates/gpui-form-macros crates/gpui-form-gpui-component app/jaco app/feiwen
rg -n "^pub " crates/gpui-form/src/form/transition.rs crates/gpui-form/src/control/transition.rs \
  crates/gpui-form/src/validation/transition.rs
```

前两条扫描预期在当前生效源码、测试与对外文档中零命中；若历史执行文档保留旧名称，必须用限定路径排除并在实施
证据中列明，不得为得到零结果改写历史文档。第三条必须零命中。

### 明确不执行

- 不启动 Jaco、Feiwen 或组件 story。
- 不使用 Computer Use、屏幕点击、人工选择/输入或视觉回归。
- 不以“手动看起来正常”作为任何 `R-9xx` 的验收证据。

## 最终验收标准

1. `C-900`：公开 total/dynamic API 符合设计草稿；runtime occurrence 完全由 Form 生成；只读路径不
   分配 identity。
2. `C-901`：每个 mutation 的 value/structure/retired impact 精确，成功只产生一次 revision/event/
   notify，失败零可见副作用。
3. `C-902`：四个 adapter 使用同一 binding mailbox/source protocol，来源组件不会收到自己的即时
   authoritative 回投，其他观察者仍同步。
4. `C-903`：validation request 使用单一 snapshot，stale async result 不生效；Prepared CAS 同时校验
   session 与 revision。
5. `C-904`：Transition 完全 crate-private；旧 API 零 active 引用；三个 producer crate、Jaco、Feiwen
   和 workspace aggregate gate 全部通过。
6. `T-900`–`T-915` 均有自动化证据；没有执行实际 UI 操作测试，且该缺口不被误报为已验证。
7. 本文实施证据区填入真实命令与结果后才可标记 `Done`；任何 pending/failed gate 都保持未完成。

## 实施证据

当前状态：`Done`（2026-08-09）。

- 实现位置：当前工作区，尚未提交；本轮未要求创建提交。
- 已完成工作包：`WP-900`–`WP-909`；`C-900`–`C-904` 已完成 producer 与 Jaco/Feiwen consumer 验收。
- `cargo test -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-features --locked`
  通过：core unit/integration/trybuild 全部通过，adapter 为 2 个 unit + 15 个 integration，macro 的 12 个
  compile-fail fixture 全部通过。
- `cargo test -p jaco --bin jaco --all-features --locked` 通过：362 项；
  `cargo test -p feiwen --bin feiwen --all-features --locked` 通过：93 项。
- 三个 producer crate 与 Jaco/Feiwen 的 `cargo clippy --all-targets --all-features --locked -- -D warnings`
  均通过；`cargo check --workspace --all-targets --all-features --locked` 通过。
- `cargo fmt --all -- --check`、`git diff --check`、旧 surface 精确扫描和三份 private transition 的公开项扫描通过。
- 未执行：启动 Jaco/Feiwen、Computer Use、人工点击输入、视觉/焦点/popup 等实际 UI 操作测试。
- 已知例外：Cargo 仅报告既有依赖 `block 0.1.6`、`proc-macro-error2 2.0.1` 的 future-incompat note；
  本轮代码、测试和 Clippy 无失败。
