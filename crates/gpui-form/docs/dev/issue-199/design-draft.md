# Issue #199：gpui-form 目标设计草稿

## 状态与职责

- 状态：`已确认`
- 所有者：`crates/gpui-form`、`crates/gpui-form-macros`、
  `crates/gpui-form-gpui-component`
- 总入口：[gpui-form 总指导、进度与状态](README.md)
- 兼容策略：本设计已在当前分支实施；进入 `main` 前不保留上一版事件、路径身份或 adapter API 的兼容层
- 文档职责：保存本轮 Form 大规模重构唯一有效的设计契约；实现流水、候选方案、历史问题与被取代
  的接口不在本文重复

本文的设计原则是：对外保留类型安全、显式 Form 所有权与领域方法；对内重做路径身份、topology、
变更路由、控件绑定与验证运行时。内部复杂度不得泄漏为普通用户必须处理的消息、control identity、
topology token 或普遍存在的 `Result`。

## 已确认的总体边界

1. 一个编辑会话仍由一个 `Entity<Form<M>>` 持有 current model、baseline、revision、validation、
   topology、异步验证任务与私有 transition runtime。
2. `FieldDef`、`ChildDef`、`ItemsDef`、`CaseDef` 及组合路径只保存 schema、访问器和定位信息，永远不保存
   `Entity<Form<M>>`、`WeakEntity<Form<M>>`、值、订阅或原生控件。
3. 每个字段只需要宏生成的一份静态 descriptor；使用字段时显式传入当前强 Form entity。
4. 保留 total path 与 dynamic path 的类型区别。普通静态字段操作不返回 `Result`；只有集合元素、enum
   case、`Option::Some` 等可能退休的路径使用 fallible API。
5. Form 是业务字段值的唯一权威。原生控件只拥有 IME、光标、选择区、popup、未完成文本等编辑器状态。
6. 集合元素、case 激活和 optional 激活的身份由 Form runtime 生成；业务 model、应用和 UI 不生成
   form-only ID。
7. 对外继续使用 `get`、`set`、collection mutation、`replace`、`reset`、`rebase`、`validate`、
   `prepare` 等领域方法，不公开 Form message 或 dispatch API。
8. `gpui_operation::Transition` 只用于 crate-private runtime，不使用 `refresh` 或 `repair` 预定义状态机。
9. focus、touched、错误可见性、提交任务、查询、catalog、持久化、数据库与业务 operation 仍由应用所有者
   管理。

## 一、公开 typed API

### Schema 与静态 descriptor

目标仍以 Rust 类型系统表达 schema。嵌套语义只在类型声明处标注，不在每次访问时使用宏或字符串路径：

```rust,ignore
#[derive(Clone, FormSchema)]
struct QueryDraft {
    keyword: String,
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}
```

derive 为字段生成静态 descriptor。普通调用显式传入 Form：

```rust,ignore
let form = cx.new(|_| {
    Form::new(QueryDraft::default())
        .with_validator(QueryValidator)
});

let keyword: String = QueryDraft::KEYWORD.get(&form, cx);
QueryDraft::KEYWORD.set(&form, "rust".into(), cx);
```

公开主构造函数应是 infallible。session/path identity 的计数耗尽属于内部不变量失败，不是用户可以恢复的
`FormBuildError`。

### TotalPath 与 DynamicPath

静态 descriptor 和只经过静态 child edge 的组合生成 `TotalPath<Root, T>`：

```rust,ignore
let path = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::TITLE);

let value: String = path.get(&form, cx);
let changed: bool = path.set(&form, next, cx);
```

- `get` 始终成功。
- `set` 返回 model 是否真正变化；等值 model 写入不推进 model revision。
- total path 的类型与身份在整个 Form session 中有效，包括 `replace`、`reset` 和 `rebase`。

经过 item、case 或 optional 激活边界后得到 `DynamicPath<Root, T>`：

```rust,ignore
let value: ConditionValue = path.try_get(&form, cx)?;
let changed: bool = path.try_set(&form, next, cx)?;
```

dynamic API 的错误只表达可操作的定位事实，例如 wrong session 或路径已退休；不把内部地址、token 或
generation 暴露给调用方。

### Case 与 Optional 解析

enum case 和 optional 的“当前未激活”不是 stale path。目标 resolver 明确区分：

```rust,ignore
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: ConditionValue = value.try_get(&form, cx)?;
}
```

- 当前 enum 不是目标 case，或 `Option` 当前为 `None`：`Ok(None)`。
- 解析起点已经退休或属于其他 Form session：`Err(ResolveError)`。
- `Some`/case 重新激活后产生新 occurrence，旧 resolver 结果不会复活。
- resolver 返回的 typed path 决定后续值类型；向路径写入错误 Rust 类型在编译期被拒绝。

### 集合与递归 typed tree

调用方只能从当前 Form 枚举或 collection mutation 获得 `ItemPath`：

```rust,ignore
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);

for node in children.items(&form, cx) {
    let key = node.key();
    let kind = node.then(FilterNode::KIND).try_get(&form, cx)?;
}

let node = children.append(&form, FilterNode::default(), cx)?;
```

保留以下领域操作：

- `items` / `try_items`
- `append`
- `insert_before`
- `move_before`
- `remove`
- `replace_all`
- `ItemPath::move_to`

这些方法不接收业务 ID、数组下标或用户生成 token。mutation 返回新建或仍有效的 typed item path，供 UI
继续组合。跨父级移动退休 source occurrence，并在 destination 创建新 occurrence，因此返回 destination
的新 `ItemPath`。

### 普通 gpui-component 接入

普通应用用户只使用内置 adapter，不参与事件路由：

```rust,ignore
let keyword = FormInput::new(
    &form,
    QueryDraft::KEYWORD,
    window,
    cx,
);

let tags = FormCombobox::new(
    &form,
    QueryDraft::TAGS,
    options,
    window,
    cx,
);
```

`FormInput`、`FormIntegerInput`、`FormSelect`、`FormCombobox` 自行持有 native entity、native event
subscription 与 Form binding。页面不需要为了这些控件解析 `FormEvent`。

页面只有在自身 render、按钮状态或跨字段业务逻辑读取 Form 时，才需要观察 Form：

```rust,ignore
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

这不是控件双向绑定协议，内置控件不能依赖页面保存该 observer 才保持同步。

## 二、运行时身份与 topology

### Occurrence identity

Form session 为每次动态激活分配单调递增且永不复用的 `OccurrenceId`。私有 canonical address 由真实
schema edge 与 occurrence 组成：

```rust,ignore
enum AddressSegment {
    Field(FieldId),
    Item(OccurrenceId),
    Case(CaseId, OccurrenceId),
    Some(OccurrenceId),
}

struct CanonicalAddress(Arc<[AddressSegment]>);
```

身份规则固定为：

| 操作 | occurrence 结果 |
| --- | --- |
| 同父级 reorder | 保留原 item occurrence |
| remove 后重新 insert | 旧 occurrence 退休，新元素获得新 occurrence |
| case `A -> B -> A` | 两次 `A` 使用不同 occurrence |
| `Some -> None -> Some` | 两次 `Some` 使用不同 occurrence |
| cross-parent move | source 退休，destination 获得新 occurrence |
| whole-model replace/reset/rebase | total identity 保留；旧 dynamic occurrence 全部退休 |

`TotalPath` 保存纯 `PathPlan`；`DynamicPath` 额外保存 session-bound `LocationProof`：

```rust,ignore
struct PathPlan<Root, T> {
    access: TypedAccess<Root, T>,
    address: SchemaAddress,
}

struct LocationProof {
    session: SessionId,
    occurrences: Arc<[OccurrenceProof]>,
}
```

dynamic read/write 在访问 model 前验证整条 occurrence proof。不能继续使用“同一地址 + incarnation”
临时拼装，也不能让已退休路径因为地址再次出现而复活。

### PathKey

`PathKey` 是公开 opaque UI identity，但不再只是 canonical address 的 64-bit hash：

```rust,ignore
pub struct PathKey(Arc<PathIdentity>);

struct PathIdentity {
    session: SessionId,
    id: OpaquePathId,
    address: CanonicalAddress,
}
```

- `OpaquePathId` 在 Form session 内唯一且不复用；`Eq`、`Hash` 和 GPUI `ElementId` 使用
  `(SessionId, OpaquePathId)`，不存在地址 hash 碰撞。
- `CanonicalAddress` 仅供 crate 内进行前缀、祖先、后代和退休范围判断。
- topology 必须 intern canonical address，保证同一活动路径重复取得相同 `OpaquePathId`。
- `PathKey` 不提供 raw getter、serde、稳定 `Display` 或可由用户构造的入口。
- 自定义 `Debug` 只输出脱敏后的 session/path identity，不输出字段名、case、item token 或完整地址。
- wrong-session 的 impact 查询返回“不受影响”；typed read/write 仍返回明确的 wrong-session error。

### 只读 topology snapshot

初始 Form 构造和 staged topology edit 负责创建全部需要的 occurrence、path identity 与索引。读取、验证、
枚举 snapshot 和生成 `PathKey` 都不得再调用 `ensure_*` 或隐式分配身份。

一次 topology mutation 必须先完成全部解析、anchor 检查和 identity 分配，再改变 model。任何可恢复错误都
发生在提交前，并保证 model、topology、validation、revision 与事件全部不变。

## 三、变更事实与公开事件

### 私有 ChangeSet

内部不再使用一个笼统的 `AffectedPaths`。一次逻辑 mutation 生成三条独立路由：

```rust,ignore
struct ChangeSet {
    value: ValueImpact,
    structure: StructuralImpact,
    retired: RetiredImpact,
}

enum ValueImpact {
    None,
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

语义固定为：

- `SubtreeReplaced(P)`：`P`、它的祖先以及提交后仍活动的后代值发生变化。
- `AggregateChanged(P)`：集合 `P` 及其祖先值发生变化，但现有 item 的字段值没有变化。
- `StructuralImpact`：调用方需要重新枚举对应 collection/tree；它本身不要求现有控件重新设置值。
- `RetiredImpact`：命中的 dynamic path 永久失效。
- validation 使用独立 `ValidationImpact`，不得复用 value projection route。

mutation 映射如下：

| mutation | value | structure | retired |
| --- | --- | --- | --- |
| leaf `set` | `SubtreeReplaced(field)` | 无 | 无 |
| composite `set` | `SubtreeReplaced(path)` | 按实际拓扑变化 | 仅被重建的 dynamic 后代 |
| append/insert | `AggregateChanged(collection)` | collection | 无 |
| 同父级 reorder | `AggregateChanged(collection)` | collection | 无 |
| remove | `AggregateChanged(collection)` | collection | 被删除 item subtree |
| replace_all | `AggregateChanged(collection)` | collection | 全部旧 item subtree |
| cross-parent move | source 与 destination aggregate changed | 两个 collection | source item subtree |
| replace/reset/rebase | `All` | `All` | 全部旧 dynamic root（经 ancestor 压缩） |

因此 append、insert 和 reorder 不会向未改变的原有 item 控件调用 native setter。

scope 采用语义感知的归一化：完全相同的 scope 去重，retired subtree root 可以做 ancestor 压缩；
`AggregateChanged(collection)` 则必须保留每个实际发生结构变化的 collection。即使 destination collection
位于 source collection 的后代，cross-parent move 仍同时保留两者，因为 aggregate 只影响 collection 本身及
其祖先，不隐含任何后代 collection 也发生了变化。

### 公开 FormEvent

公开事件只表达应用可消费的业务事实，不公开控件来源和内部 ChangeSet：

```rust,ignore
pub enum FormEvent<M: FormSchema> {
    ModelChanged(ModelChange<M>),
    ValidationChanged {
        revision: FormRevision,
    },
}

pub struct ModelChange<M: FormSchema> {
    // private
}

pub enum ModelChangeKind {
    Edit,
    Replace,
    Reset,
    Rebase,
}

pub struct PathImpact {
    // private bit set
}

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

`ChangeTarget<M>` 是 sealed trait，由 core 为 `FieldDef`、total/dynamic/item path 与 `PathKey` 实现。
应用可以直接查询：

```rust,ignore
if change.impact(
    &QueryDraft::ROOT
        .then(QueryDraft::FILTERS)
        .then(FilterGroup::CHILDREN),
)
    .structure_changed()
{
    reconcile_rows();
}
```

collection mutation 同时具有 value aggregate 与 structure 事实，因此不提供会把两者错误互斥的
`TopologyChanged` kind。控件 origin、editor sequence、mailbox revision 和 lifecycle generation 永不进入
公开事件。

## 四、控件绑定 API

### 公开类型

删除公开 `ControlLease`。目标 API 使用三个角色：

```rust,ignore
pub struct ControlBinding {
    // non-clone lifecycle owner
}

#[derive(Clone)]
pub struct ControlWriter<Root: FormSchema, T: 'static> {
    // weak write capability
}

pub enum ControlProjection<T> {
    Value(T),
    Retired,
}
```

- `ControlBinding` 不带 `Root/T` 泛型，方便 `FormInput` 等 adapter 作为普通字段保存；它独占 Form
  projection subscription 和绑定生命周期。
- `ControlBinding` 不可 clone。drop 后绑定进入 `Dropped`，所有排队工作失效，并清理该控件的临时 issue。
- `ControlWriter<Root, T>` 可 clone，只能由 native event callback 捕获；它不延长 Form 或 adapter
  生命周期。
- `ControlProjection<T>` 是封闭且穷尽的 `Value`/`Retired` 协议，不使用 `Option<T>` 混淆退休与空值。

total 与 dynamic path 分别提供 infallible/fallible binding：

```rust,ignore
let (binding, writer) = path.bind_control_in(
    &form,
    &native_state,
    |state, projection, window, cx| {
        match projection {
            ControlProjection::Value(value) => {
                state.set_value_silently(value, window, cx);
            }
            ControlProjection::Retired => {
                state.set_disabled(true, window, cx);
            }
        }
    },
    window,
    cx,
);
```

`DynamicPath::try_bind_control_in` 在创建时先验证 location proof，并返回
`Result<(ControlBinding, ControlWriter<Root, T>), ResolveError>`。

绑定创建时由 core 完成 Form 订阅和路径过滤。自定义 adapter 只需要：

1. 读取初值并创建 native state。
2. 调用 `bind_control_in`/`try_bind_control_in`，提供 silent projector。
3. 订阅 native event，在回调中捕获 `ControlWriter`。
4. 保存 native entity、`ControlBinding` 与 native `Subscription`。

它不再手工订阅 `FormEvent`、判断 origin、解析 path、保存 lease 或实现本地方向布尔值。

### ControlWriter 命令

保留明确的编辑器到 Form 命令：

```rust,ignore
writer.defer_set(value, window, cx);
writer.defer_blur(window, cx);
writer.defer_set_issue(code, message, window, cx);
writer.defer_clear_issue(window, cx);
```

每条 deferred command 携带 crate-private `ControlId`、当前 lifecycle generation、dynamic occurrence proof
和 editor sequence。执行时再次验证这些事实；stale command 安全 no-op。

issue 规则固定为：

- 有效 `defer_set` 原子清除当前控件自己的临时 issue，即使 typed model value 与原值相等。
- 等值 writer 写入不发 `ModelChanged`；若 issue 被清除，只发 `ValidationChanged`。
- 外部 model value projection 在同一逻辑事务中清除该控件已经过时的临时 issue。
- validation-only、structure-only 与无关变化不清除 native issue，也不投影 value。
- binding 退休或 drop 后，其 control issue 不再参与有效 validation report。

### 来源感知语义

| 变化 | 发起控件 A | 同字段控件 B | 无关字段控件 C |
| --- | --- | --- | --- |
| A 提交新值 | 不回传 | 投影最新值 | 不投影 |
| 程序调用字段 `set` | 投影最新值 | 投影最新值 | 不投影 |
| B 提交新值 | 投影最新值 | 不回传 | 不投影 |
| validation-only | 不设置 value | 不设置 value | 不设置 value |
| structure-only | 不设置 value | 不设置 value | 不设置 value |
| dynamic path 退休 | `Retired` 一次 | 由各自路径决定 | 不投影 |
| replace/reset/rebase | total binding 投影；dynamic binding 退休 | 同左 | 按各自路径决定 |

来源抑制只禁止同一次 control write 原样回传给发起者，不会让 native component 成为第二份业务状态，也
不会阻止其他同 path 控件同步。

## 五、绑定生命周期与 projection mailbox

绑定 runtime 使用明确状态机：

```rust,ignore
enum BindingState {
    Active {
        lifecycle_generation: LifecycleGeneration,
        editor_sequence: EditorSequence,
        mailbox: ProjectionMailbox,
    },
    Retired {
        revision: FormRevision,
        delivered: bool,
    },
    Dropped,
}
```

状态语义：

- `Active`：binding 的 owner、Form 与路径均有效。
- `Retired`：topology 明确退休 dynamic path；向仍存活 adapter 交付一次 `ControlProjection::Retired`。
- `Dropped`：binding owner 或 Form 消失；静默取消，不伪造 topology retirement。
- whole-model replace/reset/rebase 更新 total binding 的 lifecycle generation 并投影新值；dynamic binding
  进入 `Retired`。

每个 binding 拥有一个 revision-aware mailbox：

1. 同一时间最多安排一个 drain。
2. 多个 external change 到达时只保留最新 revision；drain 最终从 Form 重新读取最新权威值。
3. 当前 binding 自己的 origin 不排入 value projection，并删除被这次 self commit 覆盖的旧 external
   projection。
4. `Retired` 覆盖所有尚未交付的 `Value`。
5. drain 执行前重新验证 owner、Form、session、lifecycle generation 与 occurrence proof。
6. native input 每发生一次就推进 editor sequence；比排队 projection 更新的 editor input 阻止旧 projection
   覆盖新编辑器状态。
7. Form event callback 只分类并更新 mailbox；native entity update 必须在释放 Form borrow 后 defer 执行，
   不允许 entity reentrancy。

lifecycle/occurrence 是 deferred writer 的 freshness barrier；不能把 Form revision 当作写入许可。revision
只用于 projection 排序与合并，否则两个合法的连续用户输入会错误互相拒绝。

## 六、验证与提交

### Validator snapshot

`Validator` 只接收一个自洽的 snapshot request，避免调用方把 model 与另一个 topology snapshot 组合：

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
    // snapshot-bound items/case/optional/value resolver
}
```

所有 item 枚举、case/optional 解析与 garde 位置映射都使用 request 内的同一 model/topology snapshot，不再
额外传入可能不一致的 `&M`，也不重读 live Form。

`ValidationTrigger::Dynamic` 改名为 `ValidationTrigger::External`，表示 catalog、依赖或其他外部事实变化；
它与 `DynamicPath` 没有概念关系。

Form 不在构造或每次普通 `set` 时无条件运行完整业务 validator。默认业务验证由 `Submit` 触发；只有
schema 明确声明的 `Mount`、`Change`、`Blur` trigger，或调用方显式请求 `External` 时才运行对应范围。

Form 保存实际 validation facts；页面决定何时显示、是否聚焦和如何布局。issue 必须附着到准确 typed
field，不提供错误汇总作为替代定位。公开 `ValidationSource` 只暴露语义来源类别；control identity、
bucket key 和 async generation 保持 crate-private。

validation 使用独立 `ValidationImpact`：

- model/topology mutation 只失效与变化范围相交的旧 bucket。
- validation-only 变化不推进 model revision，也不投影 native value。
- async validation 绑定 snapshot version、path occurrence 和 validation generation；任一事实变化即取消或
  丢弃旧结果。
- 由于 pending task 的 snapshot proof 使用全局 `FormVersion`，任何 model revision 都取消全部 pending task；
  已完成的 async issue 仍只按相交 scope 失效，不扩大成全表单清理。
- pending async validation 继续阻止 `prepare`。

### Prepared 与异步 CAS

裸 `FormRevision` 不能作为异步保存 CAS token，因为它不能阻止另一个 Form session 的 revision 被误用。
目标改为公开 opaque、session-bound version：

```rust,ignore
pub struct FormVersion {
    // private session + revision
}

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
```

应用保存成功后使用：

```rust,ignore
form.rebase_if_current(prepared.version(), canonical_model, cx);
```

`Prepared::map` 必须保留同一 `FormVersion`。version 不匹配时 `rebase_if_current` 返回 `false`，且 model、
baseline、validation、topology、revision 与事件全部不变。

## 七、私有 Transition 与原子 mutation

不把每个 typed field write 强制转换为公开或 boxed message。Form façade 负责类型化访问、拓扑预检查和
事务提交；私有 runtime 负责复杂状态演进。

适合实现 `gpui_operation::Transition` 的私有 runtime：

1. Form revision、lifecycle 与 publication effect reducer。
2. `BindingState::{Active, Retired, Dropped}` 与 projection mailbox。
3. 需要 generation、pending、settled 状态归约的 validation bucket runtime。

不使用 `gpui_operation::refresh` 或 `repair`，因为 Form 不是一次可重试的外部工作，也不存在它们定义的
Ready/Unavailable/Degraded 语义。

一次 typed mutation 固定按以下顺序执行：

1. 在 immutable model/topology snapshot 上解析 typed path、验证 dynamic proof、检查 anchor。
2. 预分配全部 occurrence/path identity，构造 `TopologyEdit`、model edit plan、`ChangeSet` 与
   `ValidationImpact`。
3. 在不存在剩余可恢复错误和用户回调的前提下，一次提交 model 与 topology。
4. 失效旧 validation bucket、更新 control issue，并安排配置过的验证 trigger。
5. 私有 Form runtime 执行一次 `CommitApplied` transition，产生 revision 与 publication effect。
6. binding runtime 按 origin、value impact、retired impact 和 lifecycle 分类为 self-suppressed、project、
   retire 或 unaffected。
7. 发布至多一个 `FormEvent` 并调用至多一次 `cx.notify()`；projection 仅进入 mailbox，不能在该 update
   scope 内重入 native entity。

成功的 model mutation 只推进一次 revision。等值普通 set 是完整 model no-op；等值 control writer set
仍可因清除自身 issue 而产生一次 validation-only publication。

## 八、gpui-component 边界

来源感知 binding 与 [`gpui-component#2652`](https://github.com/longbridge/gpui-component/issues/2652)
解决不同问题：

- Form binding 阻止 Combobox 自己提交 `[A, B, C]` 后立即再次收到相同的
  `set_selected_values([A, B, C])`。
- reset、rebase、replace、catalog refresh 或另一个控件修改仍需要真正执行外部 value projection。
- 因此 `set_selected_values` 在 active filter 下是否从完整 source 解析选中值，仍必须由
  gpui-component 自身保证。

Form 不增加隐藏 option 补偿，不复制组件 source，不用 adapter 本地方向布尔值掩盖上游选择行为。

## 九、明确非目标

- 不让 Form 拥有焦点、touched、错误可见性、页面布局或提交按钮策略。
- 不让 Form 拥有 query/fetch/save task、catalog、Store、数据库或持久化。
- 不公开 `ControlId`、origin、canonical address、session、occurrence、generation、topology snapshot、
  Form message 或 transition dispatch。
- 不把 application operation 改造成 Form message，也不让 Form 直接依赖应用状态机。
- 不在本轮为旧 `FormEvent`、`ControlLease`、`ControlBinding<Root, T>`、地址 hash `PathKey`、
  `rebase_if_revision` 或 adapter 私有订阅 helper 提供兼容 wrapper。
- 不由 Form 修复 gpui-component 自身的选择集合解析问题。

## 十、实施验收

后续实施至少固定以下自动化场景：

1. 字段 descriptor 与 composed path 不保存 strong/weak Form entity；每次操作显式传入 Form。
2. total path 普通读写无 `Result`；dynamic path 在 wrong session、remove/reinsert、case 重建和 optional
   重建后准确返回退休错误。
3. case/optional 当前未激活返回 `Ok(None)`，与 stale path 错误区分。
4. Form 生成所有 collection/case/optional occurrence；同父 reorder 保留 identity，remove/reinsert、
   cross-parent move 与重新激活获得新 identity。
5. `PathKey` 在 session 内稳定且不可碰撞；真实地址关系能准确计算 ancestor、descendant、structure 与
   retired impact，公开 API 不泄漏地址。
6. append、insert 和 reorder 只产生 collection aggregate/structure impact，不调用现有 item 控件的
   native setter。
7. 控件 A 提交后自身不收到 projection；同 path 控件 B 收到一次；无关 path 控件 C 不收到。
8. programmatic `set` 向所有相关 binding 投影；validation-only 和 structure-only 变化不设置任何 native
   value。
9. 多个 external change 合并后只投影最新 Form value；比 queued projection 更新的 native edit 不被旧值
   覆盖。
10. self-origin commit 能淘汰更早排队的 external projection，不使用共享布尔 direction guard。
11. dynamic retirement 覆盖 queued value，只交付一次 `Retired`；owner/Form 消失静默进入 `Dropped`。
12. replace/reset/rebase 后 total binding 保持活动并投影新值；全部旧 dynamic binding 与 deferred writer
    永久失效。
13. 等值 writer set 清除自己的 control issue，不发 model event；如 validation facts 改变，只发一次
    validation event。
14. validator 的 model、topology、items、case 与 optional resolver 全部来自同一个 snapshot；异步结果
    不能越过 version、occurrence 或 generation。
15. `Prepared::map` 保留 session-bound `FormVersion`；其他 Form session 或旧 version 不能通过
    `rebase_if_current`。
16. cross-parent move 对 source/destination 只提交一次 revision、一个 model event 和一次 notify，并退休
    source occurrence。
17. 四个 gpui-component adapter 与一个独立自定义控件 fixture 使用同一 binding/writer/projection
    协议，不直接解析 `FormEvent`。
18. public API 与 rustdoc 不暴露 control identity、canonical address、occurrence、generation、topology
    token、私有 message 或 dispatch。

本草稿当前没有需要在实现阶段临时选择的架构分支。进入执行计划前，应把这里的目标类型、文件拆分、
breaking consumer 迁移与测试映射提取为独立实施文档；本文继续作为设计来源，不承担进度记录。
