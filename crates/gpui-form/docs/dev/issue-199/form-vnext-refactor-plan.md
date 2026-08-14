# gpui-form vNext：递归 schema、runtime topology 与 session form 重构计划

## 状态与范围

- 状态：`Done`。三个 Form crate 的 vNext producer、Jaco/Feiwen consumer、旧 surface 删除与
  workspace aggregate gate 已完成；实际 UI 操作测试按本轮要求未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 子任务 ID：`FORM-199-02`
- 总入口：[gpui-form 总指导、进度与状态](README.md)
- 设计依据：[当前设计草稿](design-draft.md)
- 根入口：[Issue #199 多轮任务索引](../../../../../docs/dev/issue-199/README.md)
- 所有者：`crates/gpui-form`、`crates/gpui-form-macros`、
  `crates/gpui-form-gpui-component`
- 本地 ID 范围：`E/D/F/L/ST/R/T-500..599`、`WP-500..599`
- 本轮文档语言：中文；三个 crate 的 README/guide 属于对外文档，实施时继续维护中英文版本。

本文是 `FORM-199-02` 的独立执行计划。目录 `README.md` 只维护 Issue #199 在 Form 所有者下的多轮
状态和链接；上一轮已经完成的 explicit form ownership、Field 描述符和 core-private Transition
仍由[历史专题](field-descriptors-and-internal-transitions.md)记录，不在本文改写。

本文记录一次不保留 API 兼容性的 greenfield 重构。公开 README/guide 已按实际实现更新；本文保留实施
依据、契约、工作包与验证结果。

## 目标

1. 以 `FormSchema` 描述业务模型，以通用 `Entity<Form<M>>` 持有一次编辑 session，删除 generated
   per-model form state。
2. 把静态 schema definition、typed located path、runtime canonical address 三层彻底拆开。
3. 支持有限但任意深度的 struct child、`Option<T>`、单 payload enum case 与显式 `Vec<Item>` items。
4. 由 Form runtime 为动态 item occurrence 生成 session-local identity；业务 model 不保存 form-only
   ID，调用方也不能从 index、业务值或 raw token 构造路径。
5. 保持最终 leaf 类型由 Rust 类型系统决定；普通静态路径不引入无意义 `Result`，只有动态定位和
   topology mutation 显式失败。
6. 让每次成功 mutation 成为一个 root transaction：一次 revision、一次完整 event、一次 notify；失败
   在 commit 前原子拒绝。
7. 让 validation、binding、async completion 和 UI key 使用同一 address/incarnation/freshness
   事实源，删除后重建的节点不能继承旧错误或旧 callback。
8. 保持公开 API 为 `set`、`replace`、`reset`、`append`、`remove`、`prepare` 等领域方法；
   `gpui_operation::Transition` 只在 core 内部归约消息，不成为宏、adapter 或应用协议。

## 非目标

- 不保留 `FormModel`、generated `*Form`、`FormField`、`PartialFormField`、`FormItemId`、
  `#[form(array(...))]`、child-first `within` 或 writable `project_value` 的兼容层。
- 不把 query/fetch/save/loading/retry、catalog/options、持久化或业务 operation 放进 Form。
- 不让 Form 拥有 native component entity、focus、IME、selection、popup 或不完整的编辑器字符串。
- 不在本计划迁移 Jaco 或 Feiwen；两者分别由自己的独立执行文档负责。
- 不迁移 HTTP Client、Novel Download，也不处理 Jaco Conversation 或 MCP runtime 的 operation 重构。
- 不为性能测试设置易抖动的墙钟阈值；只固定结构、分配与 clone 语义。

## 适用范围

| 表面 | 动作 | 结论 |
| --- | --- | --- |
| `gpui-form` runtime | 重写 | 通用 `Form<M>`、五类 path、topology、validation、submit、binding |
| `gpui-form-macros` | 重写 derive | 唯一公开 derive 为 `FormSchema`；生成 definition 与惰性 driver |
| `gpui-form-gpui-component` | breaking 迁移 | 继续适配 Input/Select/Combobox/IntegerInput，但消费新 path/session |
| Garde | 保留为可选 adapter | 不再是 core path authority；必须经同一 snapshot 的 topology 映射 |
| `gpui-operation` | 保持内部依赖 | 只组织 core-private mutation/validation message/effect |
| 三个 crate 的对外文档 | 已按实际实现更新 | 双语 README/guide 描述当前可用签名与集成方式 |
| 下游 app | 独立 owner 文档实施 | Jaco/Feiwen 已消费同一 producer contract；HTTP Client/Novel Download 暂缓 |

## 证据

### 实施前事实（保留作为重构依据）

| ID | 当前事实 | 主要位置 | 对计划的影响 |
| --- | --- | --- | --- |
| `E-500` | `FormModel` 只接受 named-field struct，并生成专用 state | `gpui-form-macros/src/derive/*` | derive 与 generated surface 必须整体替换 |
| `E-501` | `FormField`/`PartialFormField` 同时承担 definition、located path 与 lens | `gpui-form/src/field.rs` | 不能继续在现有类型上补 nested API |
| `E-502` | identified array 依赖 model 内 ID、`FormItemId` 与 index 映射 | `gpui-form/src/schema/array.rs`、macro expand | identity 与 duplicate/identity-change contract 全部删除 |
| `E-503` | current mutation clone root candidate 后再提交 | `gpui-form/src/form.rs` | vNext 改为预检查后原地 root transaction |
| `E-504` | public `FieldPath` 暴露 item ID，Garde adapter 解析 positional path | `schema/path.rs`、`validation/*` | public path key 必须 opaque；Garde 只能通过 snapshot resolver 映射 |
| `E-505` | enum fragment 尚不支持，递归 child/items 受 recursive trait bound 阻断 | macro compile-fail fixtures | 新 driver 必须惰性遍历并增加递归 compile-pass fixture |
| `E-506` | adapter 已正确拥有 native state/subscription，整数解析 policy 已独立 | adapter `input/select/combobox/integer_input` | 保留 native ownership 与数值 policy，只替换 binding/path 边界 |
| `E-507` | `SubmitTransform` 绑定在 model policy 上 | `gpui-form/src/submit/*` | 改为 session `prepare` 后由调用方一次性 `Prepared::map` |

### 实施前预览中的两个硬矛盾

#### `E-508`：`.case` / `.some` 创建时拿不到 incarnation

当前预览把 `.case(...)` 和 `.some()` 设计成纯 path 组合，但同时要求 enum payload 经
`A -> B -> A`、optional 经 `Some -> None -> Some` 后旧 `DynamicPath` 本身失效。纯组合阶段没有
form/session，无法捕获当前 incarnation；如果 resolve 时总是读取最新 incarnation，旧 path 会重新可达。

用户已确认在 `D-505` 采用带 resolver 的定位边界：

```rust,ignore
let group = node
    .then(FilterNode::KIND)
    .try_case(form_entity.read(cx), FilterNodeKind::GROUP)?;

let payload = optional_path.try_some(form_entity.read(cx))?;
```

resolver 的 public 参数是 `&Form<Root>`，不接收 `Entity<Form<Root>>` 或 `cx`。返回的
`DynamicPath` 记录 session、address 与当次 incarnation，但不保存 entity。验证阶段使用
snapshot-bound 等价 resolver。不得用 `OnceCell` 或 per-session 全局 map 给可复用静态 path 偷渡状态。

#### `E-509`：`Access::get(&Root)` 无法解析 runtime token

业务 model 不再保存 ID 后，item token 到当前 `Vec` index 的映射只存在于 `TopologyIndex`。因此内部
typed access 至少必须同时接收 root 与 topology snapshot；只接收 `&Root` 会迫使实现退回 index 或
业务 ID，直接破坏 runtime-owned identity。

## 共享生产者契约

| ID | 契约 | 达成条件 |
| --- | --- | --- |
| `C-500` | schema/session/path public contract | derive、构造器、五类 path、case/optional resolver、error 与 `Prepared` 精确声明固定并通过 doc/compile fixture |
| `C-501` | topology/validation runtime contract | runtime token、epoch/incarnation、mutation 原子性、validation snapshot resolver 与 Garde 映射通过 core tests |
| `C-502` | component adapter contract | total/dynamic 构造、opaque `PathKey` UI key、row callback、teardown 与 control issue 通过 adapter tests |
| `C-503` | breaking removal contract | 三 crate active source/test/docs 不再暴露旧 API；Jaco/Feiwen 可只消费新 surface |

`C-500`–`C-502` 全部达到 `producer-ready` 后，Jaco/Feiwen 可以开始 consumer migration；
`C-503` 只能在两个 app 已迁移且 workspace aggregate check 通过后达到 `consumer-complete`。

## 架构决定

### `D-500`：model schema、session 与 located path 分层

- `FormSchema` 只提供静态 edge、typed lens 与惰性 traversal driver，不保存 session identity。
- `Form<M>` 是 current、baseline、revision、validator、issues、async tasks 与 topology 的唯一 owner。
- definition 不持有值或entity；path不持有entity。value/mutation façade按各自契约显式接收strong
  `Entity<Form<M>>`，dynamic resolver只借用当前 `&Form<M>`。
- weak form 只允许出现在 binding、deferred callback、subscription 或 async completion。

### `D-501`：初版 schema grammar

- `#[form(child)]` 显式进入 struct、enum 或 `Option<T>`；未标注的业务 struct 可作为 leaf。
- `#[form(items)]` 初版只支持 `Vec<Item>` 且 `Item: FormSchema`。
- enum 初版只支持 unit variant 和单 payload tuple variant；struct variant、多 payload tuple、generic
  schema 在 derive 阶段产生稳定诊断。
- 不生成 state 名，不接受 `state/group/array/identity/variant/validation/transform` 旧属性。

### `D-502`：definition 与 path family

目标 public family 固定为：

```rust,ignore
FieldDef<Owner, T>
ChildDef<Owner, Child>
ItemsDef<Owner, Item>
CaseDef<Enum, Payload>

TotalPath<Root, T>
DynamicPath<Root, T>
TotalItemsPath<Root, Item>
DynamicItemsPath<Root, Item>
ItemPath<Root, Item>
```

- `FieldDef`/`ChildDef` 通过 sealed conversion 进入 total path；`ItemsDef` 进入 total items path。
- `.then(...)` 只做类型安全 edge 组合并传播 total/dynamic。
- `ItemPath` 只能由 Form enumeration/mutation 返回；不能构造、serde 或读取 raw token/index。
- `ItemPath::key()` 返回 public opaque `PathKey: Clone + Eq + Hash`，供 GPUI key/map 使用；
  `PathKey` 不实现 raw getter、Display 稳定格式或跨 session 持久化。

### `D-503`：runtime topology invariant

- 初始 model 遍历与每次 insert 为 occurrence 分配 token；same-parent reorder 保留 token。
- remove 后插入相等业务值、`replace_all`、case/optional payload 重建、whole-model replace/reset/rebase
  retire 旧 handle；cross-parent move 返回 destination 下的新 `ItemPath`。
- source、destination、anchor、session、collection、freshness 与 cycle 在 commit 前检查；失败时 model、
  revision、issue、task、event、notify 全部不变。
- public 不暴露全局 registry、按 ID lookup 或让旧 path 自动跟随 cross-parent move。

### `D-504`：collection mutation surface

实施前按以下语义固定精确签名：

```rust,ignore
items / try_items
append
insert_before
move_before
remove              // 返回被移除的业务值
replace_all          // 返回全部新 ItemPath
ItemPath::move_to    // cross-parent，返回 destination 下的新 ItemPath
```

所有 topology mutation 返回 `Result<_, MutationError>`。items path 不提供普通 leaf `set(Vec<T>)`；
bulk replacement 只能显式调用 `replace_all`。

### `D-505`：case / optional 必须在当前 session 中定位

采用 `E-508` 的用户确认方案，public resolver 固定为：

```rust,ignore
impl<Root: FormSchema, Enum: FormSchema> TotalPath<Root, Enum> {
    pub fn try_case<Payload: FormSchema>(
        self,
        form: &Form<Root>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<DynamicPath<Root, Payload>, ResolveError>;
}

impl<Root: FormSchema, Enum: FormSchema> DynamicPath<Root, Enum> {
    pub fn try_case<Payload: FormSchema>(
        self,
        form: &Form<Root>,
        case: CaseDef<Enum, Payload>,
    ) -> Result<DynamicPath<Root, Payload>, ResolveError>;
}

impl<Root: FormSchema, T: FormSchema> TotalPath<Root, Option<T>> {
    pub fn try_some(
        self,
        form: &Form<Root>,
    ) -> Result<DynamicPath<Root, T>, ResolveError>;
}

impl<Root: FormSchema, T: FormSchema> DynamicPath<Root, Option<T>> {
    pub fn try_some(
        self,
        form: &Form<Root>,
    ) -> Result<DynamicPath<Root, T>, ResolveError>;
}
```

- `ItemPath` 进入enum/optional target时暴露与 `DynamicPath` 相同的inherent façade；root definition通过
  sealed total-path conversion提供同样调用面，不要求调用方导入额外trait。
- resolver不接收 `Entity<Form<Root>>` 或 `cx`；调用方使用 `form_entity.read(cx)` 取得本次同步借用。
- `try_case`/`try_some` 检查当前可达性并捕获incarnation，返回session-bound、entity-free
  `DynamicPath`。
- static case/optional definition 可以参与 schema 描述，但不能直接伪装成已经定位的 runtime path。
- validation request 提供同一 snapshot 上的等价 resolver，validator 不从最新 live form 重新定位。
- 删除纯 `.case(...)` / `.some()` public surface；不能添加不接收当前Form的等价捷径。

### `D-506`：typed access 必须消费 topology snapshot

public API不暴露或接收topology。core内部固定为：

```rust,ignore
pub(crate) struct TopologySnapshot<'a> {
    index: &'a TopologyIndex,
    epoch: TopologyEpoch,
}

pub(crate) trait Access<Root, T>: 'static {
    fn get<'a>(
        &self,
        root: &'a Root,
        topology: &TopologySnapshot<'_>,
    ) -> Result<&'a T, ResolveError>;

    fn get_mut<'a>(
        &self,
        root: &'a mut Root,
        topology: &TopologySnapshot<'_>,
    ) -> Result<&'a mut T, ResolveError>;
}
```

一次resolve、validation或mutation transaction只创建一份snapshot；preflight、typed access、issue定位
与commit不允许切换topology。`TopologySnapshot`、`TopologyIndex`、`CanonicalAddress`、session ID、
item token、epoch与incarnation全部保持crate-private。

### `D-507`：错误边界

精确 variant 在 `WP-500` 固定，但责任边界不再变化：

- `FormBuildError`：初始 schema/topology 构建失败；
- `ResolveError`：wrong session、retired handle、missing optional、inactive case；
- `TopologyError`：wrong collection、invalid anchor、move into descendant、内部 identity exhaustion；
- `MutationError`：只包装 resolve/topology 原因；
- `PrepareError`：blocking validation report 或 pending async validation。

上述错误均为非泛型 public 类型，并只公开 opaque `PathKey`；semantic validation issue 不伪装成
topology error，topology error 也不能写进 Form report 后继续 mutation。

### `D-508`：validation 使用 snapshot resolver

- native `Validator<M>` 通过 `ValidationRequest` 与 `ValidationSink` 在同一 model/topology snapshot 上
  产生 issue。
- `ValidationRequest` 提供 item enumeration、case/optional resolution；返回值同时携带 typed value 与
  snapshot-bound path，validator 不按 index 构造路径。
- issue key 包含 address、incarnation、source、trigger 与 generation；subtree retire 清理其全部 bucket。
- Garde 0.23 的 `rows[1].field`、nested array、单 tuple payload可选 `[0]` 形式通过 fixture 验证，再结合
  active case 补 canonical `Case` segment；无法映射时产生 blocking form-level adapter issue。
- async completion 同时检查 revision dependency、epoch、address、incarnation、generation 与 input
  snapshot；任一不匹配即 no-op。

### `D-509`：validator replacement

`replace_validator` 清除旧 validator-owned sync issue、取消其 async work，保留 required/control issue；
它不修改 model/baseline/revision，也不自动执行 dynamic validation。owner 替换 context 后显式调用
validation，因此 catalog/options 更新不会偷偷修改表单值。

### `D-510`：prepare 与 CAS rebase

- `prepare` 在同一 snapshot 执行 submit validation，拒绝 blocking/control issue 与 pending async work。
- `Prepared<M>` 同时保存 revision 与 model snapshot；`map` 消费自身并返回 `Prepared<U>`，不丢失 CAS
  revision；`into_parts` 才显式拆出二者。
- 保存成功只允许 `rebase_if_revision`；CAS 失败保留保存期间的新编辑。
- 删除 model-associated `SubmitTransform`；同一 model 可以在不同场景映射为不同 request。

### `D-511`：core-private Transition 与发布规则

core mutation/validation 可继续使用 private message/effect + `Transition`，但一条公开 mutation 的所有
effects 必须在同一 entity update 中收束。成功且确有变化时 revision `+1`、一个 `Committed` event、
一次 notify；equal leaf set 和语义 no-op reorder 不发布。消息、effect 与 Transition impl 不 re-export。

### `D-512`：component adapter 边界

- total constructor 只可能返回 component policy/config error；dynamic constructor显式返回
  `ResolveError` 与 component error 的组合。
- wrapper 只拥有 native entity 与 subscriptions，不缓存 static definition，不强持有 form。
- row/remove 等 callback 捕获 live typed `ItemPath` 或 opaque `PathKey` lookup lease；不得把 raw token 塞进
  可序列化 action。
- native entity、focus、IME、selection、popup 与 incomplete input 保持 adapter-owned。

## 文件与所有权

### `crates/gpui-form`

| ID | 文件 | 动作 |
| --- | --- | --- |
| `F-500` | `src/lib.rs`、`src/typed.rs` | 重建 public exports/facade；只导出新 schema/session/path/error/validation/submit surface |
| `F-501` | `src/form.rs`、`src/form/transition.rs` | 通用 `Form<M>`、root transaction、event、revision、private Transition |
| `F-502` | `src/error.rs` | 新增四组 public error 与 opaque path 信息 |
| `F-503` | `src/path.rs`、`src/path/access.rs` | 新增五类 path、sealed composition、topology-aware typed access |
| `F-504` | `src/schema.rs`、`src/schema/{definition,driver}.rs` | definition、generated driver bridge 与惰性 traversal |
| `F-505` | `src/topology.rs`、`src/topology/{address,index}.rs` | token arena、canonical address、epoch/incarnation、mutation invariant |
| `F-506` | `src/validation.rs`、`src/validation/{report,trigger}.rs` | snapshot resolver、issue buckets、Garde/async/control freshness |
| `F-507` | `src/control.rs` | binding weak boundary、lease 与 deferred callback teardown |
| `F-508` | `src/submit.rs` | `Prepared<M>`、prepare 与 CAS rebase contract |
| `F-509` | `src/field.rs`、`src/schema/{array,path}.rs`、`src/submit/transform.rs` | 下游迁移完成后删除旧实现，不保留 shim |

新增 Rust module 使用同名 `.rs` 入口，不新增 `mod.rs`。

### `crates/gpui-form-macros`

| ID | 文件 | 动作 |
| --- | --- | --- |
| `F-510` | `src/lib.rs`、`src/derive.rs` | 唯一公开 derive 改为 `FormSchema` |
| `F-511` | `src/derive/{attributes,model,expand}.rs` | 新 grammar、有限 semantic model 与稳定诊断 |
| `F-512` | `src/derive/expand/{definition,driver,validation}.rs` | 生成 definition、惰性 driver 与 validation bridge |
| `F-513` | `tests/ui.rs`、`tests/ui/**` | 删除旧 grammar fixture；新增 unsupported shape/removed attribute/recursive pass |

### `crates/gpui-form-gpui-component`

| ID | 文件 | 动作 |
| --- | --- | --- |
| `F-514` | `src/{input,select,combobox,integer_input,error,lib}.rs` | 迁移到 `Entity<Form<M>>` 与 total/dynamic path；保持 native owner |
| `F-515` | `src/integer_input/{error,parse,policy}.rs` | 保留 typed parsing/policy，调整 error 组合和 binding 接口 |
| `F-516` | `tests/adapters.rs` | total/dynamic mount、key、teardown、queued callback、control issue fixture |

### 文档

| ID | 文件 | 动作 |
| --- | --- | --- |
| `F-517` | 三 crate 的 `README.md` / `README.zh-CN.md` | 仅在精确 API 固定后同步为真实可编译快速入门 |
| `F-518` | 三 crate 的 `docs/guide*.md` | 同步完整用法、nested/validation/submit/custom adapter；中英文语义一致 |
| `F-519` | 三 crate `docs/dev/issue-199/*` 与索引 | 回写实际 contract、WP 状态、验证和 consumer gate；README 仍只作索引 |

## 生命周期与状态约束

### `L-500`：session 构建与 whole-model replacement

构造时一次遍历 model 建 topology。`replace/reset/rebase` 在一次 transaction 中 retire 全部旧 dynamic
handle、取消相关 async/control work、推进 topology epoch，再为新 model 重建 topology；旧 deferred
callback 即使 schema 位置重新出现也不能命中。

### `L-501`：item mutation

mutation 先在 immutable snapshot 上验证 source/destination/anchor/cycle，再同时改 model 与 topology。
中途任何错误不得留下一半 model 或一半 token sequence。same-parent reorder 保留 handle；cross-parent
move 使 source handle失效并返回新 handle。

### `L-502`：binding

mount 读取当前 path并建立 native state；callback 延迟到 native update 结束后，升级 weak form并检查
session/epoch/address/incarnation/generation。subtree retire 主动撤销 lease；wrapper drop 后 queued work
静默取消，不显示“form released”。

### `ST-500`：唯一权威状态

`Form<M>` 是 model、baseline、revision、validation 与 topology 的唯一权威状态；adapter、validator、
application Store 不复制这些字段。core-private Transition 不能在 transaction 外留下第二份 phase。

## 风险

| ID | 风险 | 防护 |
| --- | --- | --- |
| `R-500` | pure `.case` 让旧 path 在 case 重建后复活 | `D-505` producer gate + `A->B->A` fixture |
| `R-501` | typed access 偷偷退回 Vec index | `D-506` + reorder 后旧 path 仍解析原 occurrence 的测试 |
| `R-502` | schema driver 与 typed definition 形成两份结构事实 | 生成一份 metadata，driver/definition共享；schema snapshot 对照测试 |
| `R-503` | mutation 先改 model 后发现 topology error | 全部 preflight；失败前后完整 state/effect count 对照 |
| `R-504` | Garde positional path 错贴到重排后的 row | 同 snapshot index->token 映射；unmappable 变 blocking internal issue |
| `R-505` | opaque identity 无法支持 GPUI key 或 row callback | `PathKey` + typed callback contract，不暴露 raw token |
| `R-506` | recursive model导致 eager trait expansion或栈/分配异常 | lazy driver、128 层与 10,000 节点结构测试 |
| `R-507` | `Prepared::map` 丢失 revision | 返回 `Prepared<U>`，只有 `into_parts` 显式拆分 |
| `R-508` | breaking 删除让 app 处于半迁移状态 | producer-ready 后按 Jaco/Feiwen owner计划迁移，最后统一 residual gate |

## 测试契约

| ID | 层级 | 场景 | 验收 |
| --- | --- | --- | --- |
| `T-500` | core unit | flat total、optional、case、recursive items/case/items 读写 | leaf Rust 类型保持；total/dynamic 传播正确；无 string/index public path |
| `T-501` | core unit | 128 层、10,000 nodes topology 构建/遍历 | 正确完成；不设墙钟阈值 |
| `T-502` | core unit | leaf set + clone-counting root | transaction 内零 root clone；equal set 不发布 |
| `T-503` | core unit | reorder/remove-reinsert/replace_all/A-B-A/replace/reset/rebase | token/incarnation 与旧 handle失效符合 `D-503` |
| `T-504` | core unit | cross-parent cycle、wrong session/collection、stale anchor | commit 前失败，全部权威状态和 effect count 不变 |
| `T-505` | core unit | 每类成功 mutation | revision +1、一个完整 event、一次 notify |
| `T-506` | validation | bucket replacement、snapshot traversal、async stale completion | sibling issue保留；旧 completion no-op；只对应字段收到 issue |
| `T-507` | Garde adapter | `rows[1].field`、nested array、tuple `[0]`、inactive/unmappable | 正确 canonical path；无法映射时阻断且不误贴 |
| `T-508` | component GPUI | total/dynamic mount、reorder、teardown、queued callback、lease drop | native state ownership正确；旧 callback不命中新节点 |
| `T-509` | submit | blocking/pending、map once、CAS rebase失败 | 同一 snapshot；revision不丢；新编辑不被覆盖 |
| `T-510` | trybuild | 全部新 grammar 和删除项 | 支持项 compile-pass；不支持项稳定、可读诊断 |
| `T-511` | residual | 三 crate active source/test/docs | `FormModel`/old state/field/item ID/array identity/transform surface 零残留 |

## 工作包

### `WP-500`：固定剩余 public/adapter contract

- 直接实现已确认的 `D-505` resolver与 `D-506` topology-aware access，不再重新选择调用面。
- 固定 `PathKey`、四组error、`Prepared::map`、validator snapshot resolver与adapter row callback的
  剩余精确Rust声明。
- 用最小 compile-only fixture 证明 root-first composition、recursive item、row callback 和 adapter
  constructor 可以表达；此阶段不实现完整 runtime。
- 把确认后的调用面回写设计草稿和三 crate 中英文 public docs。
- 验收：`E-508/E-509` 有可执行 contract；`C-500` 达到 `producer-ready`。

### `WP-501`：实现 FormSchema macro 与 schema driver

- 实现 `F-510`–`F-513`，生成 definition 与惰性 driver；先让 recursive schema compile-pass。
- 删除旧 grammar 的 parser/expansion，但暂不删除 core 旧类型，避免 producer 未完成时破坏 workspace。
- 验收：`T-500` schema compile subset、`T-510`；依赖 `WP-500`。

### `WP-502`：实现 topology、path 与原地 transaction

- 实现 `F-502`–`F-505` 与 `D-502`–`D-506`。
- 先覆盖构造/enumeration/resolve，再覆盖同 parent mutation，最后实现 cross-parent move。
- 验收：`T-500`–`T-505`；失败原子性必须在进入 validation 前通过。
- 依赖：`WP-501`。

### `WP-503`：迁移 validation 与 control lifecycle

- 实现 `F-506/F-507`、`D-508/D-509`，建立 snapshot resolver、issue freshness、Garde mapper、async
  completion 和 control lease。
- 验收：`T-506`–`T-508` core 部分；`C-501` 达到 `producer-ready`。
- 依赖：`WP-502`。

### `WP-504`：实现 prepare、event 与 whole-model lifecycle

- 实现 `F-501/F-508`、`D-510/D-511`；统一 replace/reset/rebase/topology epoch 与 CAS。
- 验收：`T-509`、whole-model stale handle、event/notify count。
- 依赖：`WP-503`。

### `WP-505`：迁移 gpui-component adapters

- 实现 `F-514`–`F-516`；固定 total/dynamic constructor、opaque key 与 typed row callback。
- 不把 app-specific catalog/options 或 Form messages放入 adapter。
- 验收：`T-508` 完整矩阵；`C-502` 达到 `producer-ready`。
- 依赖：`WP-504`。

### `WP-506`：迁移下游并删除旧 surface

- 按独立 Jaco/Feiwen owner plans迁移 consumer；不得在三个 crate 增加 compatibility shim。
- consumer compile 后执行 `F-509` 和 macro旧 fixture删除，再运行 `T-511`。
- 验收：`C-503` 达到 `consumer-complete`。
- 依赖：`WP-505`、Jaco/Feiwen 对应 consumer WP。

### `WP-507`：文档与最终验证

- 以实际签名更新三 crate 双语 README/guide；中文开发文档登记最终 contract、WP 和测试结果。
- 执行下方最小充分门禁一次；失败回到唯一 owner修复，不用兼容层遮盖。
- 依赖：`WP-506`。

## 验证

阶段性 producer gate：

```text
cargo fmt --all
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
```

两个 app consumer迁移完成后的 aggregate gate：

```text
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
git diff --check
```

实际 UI 操作测试按用户要求不执行；自动化 GPUI 测试属于代码测试，已纳入实施门禁。

Residual scan 至少覆盖 active source、tests 与 public docs：

```text
FormModel
FormState
FormField
PartialFormField
FormItemId
ToFormItemId
form(state
form(group
form(array
project_value
within(
SubmitTransform
```

历史专题和 Git 历史不纳入零命中要求。

## 实施结果（2026-08-05）

| 范围 | 结果 |
| --- | --- |
| `WP-500`–`WP-504` | 已完成：固定并实现 `FormSchema`、`Form<M>`、五类 typed path、session topology、分 trigger/source/scope validation bucket、async freshness、`Prepared`/CAS 与产生真实 `FormEvent` effect 的 core-private Transition；collection mutation 使用 staged topology，失败不污染 live identity，cross-parent move 同一 revision 同时覆盖 source/destination scope。 |
| `WP-505` | 已完成：Input、Select、Combobox、IntegerInput adapter 使用 typed path、`PathKey` 与 `ControlLease`；whole-model/topology lifecycle 会立即撤销 stale lease，queued callback 通过 topology epoch 失效，adapter 只在 model event 上做 native projection。 |
| `WP-506` | 已完成：Jaco 与 Feiwen 已迁移，三个 crate 的旧实现、旧 derive grammar 与旧 tests 已删除，不提供 compatibility shim。 |
| `WP-507` | 已完成自动化与双语对外文档；实际 UI 操作测试按用户要求未执行。 |

实际验证：

- `cargo fmt --all`：通过。
- `cargo test -p gpui-form-macros --locked`：通过，trybuild 覆盖 vNext 支持/拒绝 grammar。
- `cargo test -p gpui-form --all-features --locked`：通过；覆盖 validation bucket、stale async、collection
  原子性、cross-parent effect、queued lifecycle callback 与 lease/subtree retirement。
- `cargo test -p gpui-form-gpui-component --all-features --locked`：通过；覆盖 typed adapter 双向投影、
  control issue 与 lifecycle 失效。
- `gpui-operation`、三个 Form crate、Jaco 与 Feiwen 的聚合 Clippy 严格门禁：通过。
- `cargo check --workspace --all-targets --all-features --locked` 与 `git diff --check`：通过。

Residual 分类：active Form producer/consumer 不再使用旧 derive、generated form state、旧 field/item ID、
array metadata 或 core submit transform。`ControlBinding` 内部保留一个 `WeakEntity<Form<M>>`，这是排队回调
的唯一生命周期边界；Jaco 的 `ChatFormState`、`FormModelPicker`、`ProviderFormField` 与应用自有
`prepare_submit`/`ProviderPreparedSubmit` 是业务/UI 名称，不是已删除的 gpui-form surface；compile-fail
fixture 与历史开发文档中的旧名称按设计保留。

## 完成与交接

本轮 contract 完成条件：

- [x] `D-505` / `D-506` 精确签名已经用户确认并同步到双语对外预览；
- [x] `FormBuildError`、`ResolveError`、`TopologyError`、`MutationError`、`PrepareError` 精确声明固定；
- [x] validator snapshot resolver、`PathKey` 与 adapter row callback contract固定；
- [x] 设计草稿与三 crate 双语预览已同步，不再展示会复活旧 path 的纯 `.case/.some`；
- [x] Jaco/Feiwen owner plans引用相同 producer contract，且未增加应用 compatibility shim。

当前代码与自动化已经达到 `Done`，并纳入本次 Issue #199 实施提交。实际 UI 操作测试被明确排除，
因此本文不把它伪造为完成证据；PR 尚未请求。
