# Feiwen：迁移到 gpui-form 新破坏性 API 的实施计划

## 状态、边界与生产者门禁

- 状态：`Done`（2026-08-09）；已消费 `C-900`–`C-904`，实际 UI 操作测试按范围未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 所有者：`app/feiwen`
- 本地编号：`E/D/F/L/ST/ERR/R/T-1300..1399`、`WP-1300..1309`。
- 实际 UI 操作测试：明确不执行；只做代码级、GPUI test 与 workspace 验证。

本文是新 Form public API 的 consumer 重迁移计划，不改写已完成的
`form-operation-store-migration.md`。本目录与根目录的 `README.md` 只登记状态和链接，不混入本计划正文；
产品代码在实施阶段才修改。

| 门禁 | Feiwen 依赖的最终能力 | 开始条件 |
| --- | --- | --- |
| `C-900` | `Form::new`/`with_validator`、total/dynamic path、递归 occurrence、resolver 与 `PathKey` | schema/path/topology fixture 成功 |
| `C-901` | mutation change facts、`FormEvent<QueryDraft>` 与 `PathImpact` | 值/结构/退休 impact fixture 成功 |
| `C-902` | 内建 Input/Select/Combobox/IntegerInput 的来源感知投影 | 自回传/无关投影/上游边界 fixture 成功 |
| `C-903` | 快照 validator、显式 validation trigger、Prepared/FormVersion | prepare/CAS/validation trigger fixture 成功 |
| `C-904` | 私有 `Transition` 与原子发布 | producer 聚焦门禁已通过；Feiwen 迁移后共同推进到 `consumer-complete` |

**兼容策略：** 允许 breaking，Feiwen 不保留 `try_new*`、`value`/`try_value`、`try_case`、旧 event 或
total/dynamic 通用 Result wrapper。不得为旧组件或现有 UI 行为编写 shim。

## 目标与非目标

### 目标

1. Fetch 成为纯 total-path 的机械 consumer：新构造器、descriptor `get`、内建 adapter 与
   `Prepared` 快照都使用最终 API。
2. Query 保持递归 typed tree，但删除把 total/dynamic path 统一成始终可失败的 `QueryPath` /
   `QueryItemsPath` 的抽象。
3. Query 的递归 validator 只读 `ValidationRequest` 的同一快照；非活动 enum case 是
   `Ok(None)`，已退休/错误 session 才是 `Err(ResolveError)`。
4. Query 行树仅由 `ModelChange` 的 collection/case 结构 impact、retirement 驱动 reconcile；无关字段
   修改、仅 validation 变化和 adapter 自写不重建原生控件。
5. 动态控件退休后移除旧 row，且旧 deferred callback 不会写入重建后的同位置 row。
6. 将 condition field 的异型选择器接入 custom binding/writer，将 sort direction 调整为可直接使用
   `FormSelect` 的 `SortDirection` 类型化值；不保留 raw `SelectState` 的单向写入。

### 非目标

- 不改 Query/Fetch private operation、task、snapshot、cancel/retry、result/log 行为。
- 不改 `gpui-store`、QueryCatalog、DuckDB resource/repair、database、catalog acquisition 或查询/抓取协议。
- 不改 Query/Fetch UI 结构、Fluent、资源、选项产品策略或运行准入。
- 不修复 `gpui-component#2652`；不在 Form adapter 或 Feiwen options 刷新中加入 hidden-item、fallback、
  direction flag 等补偿。

## 已核实的当前事实

| ID | 当前事实 | 证据 | 迁移后果 |
| --- | --- | --- | --- |
| `E-1300` | Fetch 使用 `Form::try_new_with_validator`、`form.value()`、旧 `FormEvent` 重绘订阅 | `features/fetch.rs:77-142,169-176` | 平面表单机械迁移 |
| `E-1301` | Fetch URL 显式标注 `on_change/on_blur/on_submit`；Query draft 无非-submit trigger | `features/fetch/form.rs:22-79`、`features/query/form.rs:19-104` | Fetch 保留明确触发；Query 默认仅 submit |
| `E-1302` | Query controller 用 `QueryPath` / `QueryItemsPath` 抹平 total/dynamic，所有读取/写入返回 `Result` | `features/query/advanced/controller.rs:37-136` | 必须结构性删除 wrapper |
| `E-1303` | 递归 UI 使用 `try_value`、`try_case(form.read(cx), ...)`，并在 mutation 后手工 reconcile | `controller.rs:340-790,805-1102` | 改为最终 resolver/类型化拆分/impact 订阅 |
| `E-1304` | Query validator 同时传入 model 与 request，递归使用 request items/value/try_case | `features/query/form.rs:178-430` | 改为单快照 request API |
| `E-1305` | Query reset/load draft 后总是 `replace_controls`；collection mutation 后手工 reconcile | `controller.rs:305-317,565-713` | 用结构 impact 精确 reconcile |
| `E-1306` | options 刷新直接重新设置 Combobox selected values | `controller.rs:1160-1215` | 保留为上游 #2652 边界，不把它误当 Form API 修复 |
| `E-1307` | condition field 与 sort direction 使用 raw `SelectState` + 原生订阅直接写 Form，没有 Form→原生 binding 投影 | `controller.rs:805-840,1040-1089` | condition field 改 custom binding；sort direction 改内建 `FormSelect` |

## 设计与所有权契约

### `ST-1300`：Fetch 表单

- **权威：** `FetchView.form: Entity<Form<FetchDraft>>` 唯一持有 draft/baseline/validation/version。
- **原生：** `FormInput` 与 `FormIntegerInput` 自身持有 binding 和原生 entity；FetchView 只保留它们以维持
  原生生命周期。
- **运行：** `Store<FetchRun>`、唯一 task、不可变 `FetchRequest` 快照不迁移。
- **发布：** 页面需要按钮/错误状态时使用普通 `cx.observe(&form, ...)`，不因控件同步订阅 FormEvent。

### `ST-1301`：Query 递归表单与行投影

- **权威：** `AdvancedQueryController.form: Entity<Form<QueryDraft>>` 是唯一可编辑 draft；`QueryRun` 保存一次
  prepared snapshot，Catalog 只持有 options。
- **运行时身份：** `ItemPath`/`PathKey` 只来自 Form enumeration/mutation；`PathKey` 是行 map/element ID，
  不进入 QueryDraft/QuerySpec/Store。
- **投影：** 内建动态 adapters 自己接收 `Value/Retired`；controller 只负责结构 reconcile 和
  options 刷新。Catalog refresh 不修改 Form 值，也只在规则需要时显式 external validation。

### `D-1300`：删除 total/dynamic 通用包装器

删除 `QueryPath<T>`、`QueryItemsPath<Item>` 及其 `.value/.set/.items`。目标调用面：

```rust,ignore
let relation = FilterGroupDraft::RELATION.get(&form, cx);       // total 路径
let item = children.items(&form, cx);                           // total 集合
let value = dynamic_value.try_get(&form, cx)?;                  // 已跨越 occurrence 边界
let changed = dynamic_value.try_set(&form, next, cx)?;
```

根筛选组与递归筛选组改成两个明确所有者：`RootFilterGroup` 保存 total fields/items，
`DynamicFilterGroup` 保存 dynamic fields/items；condition、嵌套 group 与 sort row 都只保存具体 dynamic path。
渲染层可以在控件树 enum 上匹配 root/nested variant，但不得再提供统一返回 `Result` 的 value/items façade。
可能退休的动态 row 在 resolver/read 失败时由 reconcile 移除；静态字段不产生虚假的 `ResolveError` 分支。

### `D-1301`：resolver 与快照 validator

```rust,ignore
let condition = node
    .then(FilterNodeDraft::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterConditionDraft::FIELD).try_get(&form, cx)?;
}

impl Validator<QueryDraft> for QueryDraftValidator {
    fn validate(
        &self,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<'_, QueryDraft>,
    ) {
        let model = request.model();
        // 所有 items/case/optional/value 解析均使用该 request 快照。
    }
}
```

Query 的递归 validation 不会重读 live `Form`、按索引重建 path，也不会将非活动 case 的 `Ok(None)` 变为字段错误。
`request.items`/case/optional resolver 返回的 validation path 受 request lifetime 约束，只在本次 validator
调用中组合并交给 `out.at(...)`；不得保存到 controller 或转换成 live mutation path。
`ResolveError` 只中止该过期遍历分支，不能把 issue 附着到替换后的节点。

### `D-1302`：impact 驱动的 reconcile

`AdvancedQueryController` 为递归结构所有权保留一个订阅：

```rust,ignore
match event {
    FormEvent::ModelChanged(change) => {
        let filters = change.impact(&QueryDraft::ROOT.then(QueryDraft::FILTERS));
        let sorts = change.impact(&QueryDraft::ROOT.then(QueryDraft::SORTS));
        if filters.structure_changed() || filters.retired() ||
           sorts.structure_changed() || sorts.retired() {
            this.reconcile_controls(window, cx);
        }
    }
    FormEvent::ValidationChanged { .. } => cx.notify(),
}
```

- 仅字段值 impact：内建 adapter 投影它；controller 只在需要时重绘错误/依赖值的布局。
- case 替换：受影响的 condition row 做结构 reconcile，旧 editor binding 退休，活动新 case 获得新的控件。
- 整模型 replace/reset：total 根保持活动；所有动态行退休，并从新快照重建。
- 同父级 reorder：按未变化的 `PathKey` 复用行，只调整显示顺序，不重建其原生组件。

### `D-1303`：validation 可见性与 options 边界

除非字段显式选择其他 trigger，Query validator metadata 仍保持仅 submit。因此 `prepare` 是不完整 Query 字段
变得可见的正常位置；Fetch URL 有意保留显式 change/blur validation。Catalog option 更新保留已选值，按现状显示
禁用/不可用 option/hint，且不调用 Form set/rebase。

### `D-1304`：Query 自定义与可归一化选择控件

- condition field 的原生值是 `FieldKind`，Form 值是包含 payload 的 `ConditionField`，不能伪装成
  内建同型 adapter。`ConditionRow` 持有 non-clone `ControlBinding`；projector 把
  `ConditionField::field()` 静默投影到 `FieldSelectState`，原生 Confirm 通过
  `ControlWriter<QueryDraft, ConditionField>` 提交 `ConditionField::for_field(field)`。
- 该提交产生 case 结构 impact，只 reconcile `ConditionEditor`；同一个 condition row 与 field select
  binding 保持，来源控件不收到自身值回投。
- sort direction 没有异型业务语义：把 delegate item 值改为 `SortDirection`，直接使用 dynamic
  `FormSelect`。`SortDirectionChoice` 若不再被其他 consumer 使用则删除，不保留转换层。
- group relation、negated 等渲染时按钮/checkbox 继续从 Form 读取并调用 typed `set`；它们不是持久
  原生 editor state，不需要人为套 binding。

## 文件级改动地图

```text
app/feiwen/
├── src/features/fetch.rs                              # F-1300 [修改] 构造器、total 读取、Form observer、Prepared 提取
├── src/features/fetch/form.rs                         # F-1301 [修改] 快照 Validator 签名；保留显式 URL triggers
├── src/features/query.rs                              # F-1302 [仅当 Prepared/FormVersion 类型或 Query observer 调用点变化时修改]
├── src/features/query/form.rs                         # F-1303 [修改] 递归快照 validator、cases/items、测试
├── src/features/query/advanced/controller.rs          # F-1304 [修改] 类型化拆分、resolver、impact 订阅、行协调、内建 adapters
├── src/features/query/advanced/render.rs              # F-1305 [修改] 渲染具体 total/dynamic path 及感知退休的行
├── src/features/query/advanced/sort.rs                # F-1306 [仅在不透明 PathKey/行投影 API 需要调整时修改]
└── src/features/query/advanced/options.rs             # F-1307 [仅改类型] 将 sort direction 值归一为 SortDirection；catalog 语义不变
```

本计划不包含 form model schema、operation/store/database/catalog owner、manifest、生成产物、locale 或 asset 的修改。
现有 Query/Fetch 行内测试在其源模块中更新；不新建测试 harness。

## 错误与生命周期契约

### `ERR-1300`：动态退休

来自动态行的 `ResolveError` 仅在已退休/错误 session 的位置属于正常结果。Reconcile 丢弃旧 row/control；
排队的原生回调为静默 no-op。不得回退到按位置的集合查找、旧 `PathKey`、新建的同地址行，或整个 `QueryDraft`
替换。

### `ERR-1301`：Query validation 与编译拒绝

`PrepareError` 使已经清空的结果表保持为空，只展示字段局部错误，且不启动 Query operation。现有
`QueryDraft::to_spec` 拒绝仍属于应用层：除非它是类型化字段规则，否则不变为 Form validation。不新增错误汇总
或重复的详情复制操作。

### `ERR-1302`：options 投影/上游组件边界

如果 catalog 缺少某个选择，仍在 Form 中保留它并展示现有禁用/hint 投影。`set_selected_values` 的活动过滤 bug
由 `gpui-component#2652` 负责；reset/rebase/catalog 刷新后的外部值投影仍是合法行为，不得被 Feiwen 抑制。

## 工作包

### WP-1300：确认最终 producer API 并迁移 Fetch

**前置条件：** `C-900`、`C-902`、`C-903`。

1. 以无失败构造替换 Fetch `try_new_with_validator`，并以 total descriptor 读取替换 `form.value()` 按钮 gate。
2. 保留现有内建控件构造器；只删除过时的 form event listener，改用普通观察。
3. 将 `FetchValidator` 改为单 request 签名，并保留显式 URL validation trigger metadata。
4. 保持 Fresh/Resume/Retry 快照、database gate 和 operation 状态不变。

**测试：** `T-1300`–`T-1302`。
**完成条件：** Fetch 可编译，且不存在旧 form 构造/读取/event 接口；其运行语义不变。

### WP-1301：分离 Query total/dynamic path 存储并迁移 resolver

**前置条件：** `C-900`、`C-902`。

1. 删除 `QueryPath`/`QueryItemsPath`；分离 root group 存储与嵌套动态 group 存储，并让全部 item/case 后代都保存具体动态 path。
2. 将每个 `try_value` 改为 `try_get`，每个 case 访问改为 `case(...).resolve`，并显式分支 `Option`。
3. 保持 `PathKey` 作为行身份，`ItemPath` 作为存活 mutation 能力；不引入业务 ID。
4. 仅在确实动态的 path 重建 adapter 构造；将 condition field 迁移到 `D-1304` custom binding，并将 sort direction
   迁移到内建 `FormSelect`。

**测试：** `T-1303`–`T-1305`。
**完成条件：** 由类型检查器而不是 `QueryPath` 决定每个调用方是否必须处理 `ResolveError`。

### WP-1302：迁移递归 validator 与 validation 可见性

**前置条件：** `WP-1301`、`C-900`、`C-903`。

1. 重写 Query validation helper，使其接收一个 request 快照并通过 `request.model()` 取得 model。
2. 针对该快照枚举嵌套 children 并解析 cases；将现有 Fluent 消息附着到同一类型化字段。
3. 验证未标注的 Query validation 仅 submit；不得添加 change/mount trigger 恢复旧的急切展示。

**测试：** `T-1306`、`T-1307`。
**完成条件：** validation 不能读取更新的 live topology，也不能将 issue 附着到已退休/重建的 Query node。

### WP-1303：以 `PathImpact` 驱动 Query 行协调

**前置条件：** `WP-1301`、`C-901`、`C-902`。

1. 以由 `AdvancedQueryController` 所有的 impact 订阅替换旧的通用 FormEvent notify 订阅和 mutation 本地 reconcile 调用。
2. 对相同 `PathKey` 保留行对象；只重建新增/退休的 case/item 行；更新 sort 顺序时不重建行。
3. 通过仅替换 editor 来 reconcile condition case 变化；保持行级 custom field binding 活动。
4. 让 `load_draft/reset` 依赖整模型 impact：total controls 接收值投影，动态行只 reconcile 一次。
5. 将 option 刷新保留为仅原生 option 投影；不得为了匹配 catalog 数据而修改 Form。

**测试：** `T-1308`–`T-1311`。
**完成条件：** 无关字段编辑和 `ValidationChanged` 永不重建原生 Query 控件；过期动态回调不能指向新的 occurrence 行。

### WP-1304：残留删除与验证

**前置条件：** `WP-1300`–`WP-1303`。

1. 只在活动源码/测试代码中更新全部受影响的行内测试和残留扫描。
2. 保留历史计划，且不执行实际 desktop UI 操作。

**测试：** `T-1312`、`T-1313`。

## 自动化测试与验收

| ID | 层级 | 场景 | 断言 |
| --- | --- | --- | --- |
| `T-1300` | Fetch GPUI/单元测试 | 构造、total descriptor 读取、合法/非法 URL/页码范围 | 保留显式 trigger 行为与启动 gate |
| `T-1301` | Fetch adapter | 输入自身写入、外部 reset/replace | 无自回传；投影 canonical total 值 |
| `T-1302` | Fetch 运行 | Fresh prepared 快照后编辑 form | 不可变运行 request 保持不变 |
| `T-1303` | Query 编译/单元测试 | 类型化嵌套 item/case 组合 | 错误类型写入无法编译；静态字段无失败 |
| `T-1304` | Query GPUI | relation/case 替换 | 非活动 case 产生 `Ok(None)`；旧 editor 退休 |
| `T-1305` | Query 集合 | 嵌套 append/remove/reorder | item `PathKey` 经同父级 reorder 保留，重新插入时不保留 |
| `T-1306` | Query validator | 递归 group/condition/sort 错误 | 每个现有 Fluent 错误附着到精确的当前字段 |
| `T-1307` | validation trigger | 新 Query draft 在提交前后 | 无默认 mount/change validation；提交展示字段错误 |
| `T-1308` | impact/reconcile | 无关叶子 set 与仅 validation event | controller 不重建行/不调用原生 setter |
| `T-1309` | impact/reconcile | append/remove/case 变化/reset/rebase | 受影响行只 reconcile 一次；total 根保持 binding |
| `T-1310` | 生命周期 | remove/reinsert 后排队的 control 写入 | 旧 writer 为 no-op；不投影到新行 |
| `T-1311` | custom/select binding | condition field 自身/程序化写入；sort direction reset/外部写入 | condition selector 无自回传且 editor 只重建一次；direction 由 FormSelect 同步 |
| `T-1312` | options 边界 | 活动过滤下保留已选 tags/authors | Form 自回传由 adapter 修复；#2652 仍单独归类 |
| `T-1313` | 残留扫描 | 活动 Feiwen 源码/测试 | 不存在 `try_new*`、`try_value`、`try_case`、旧 FormEvent 或 QueryPath wrapper |

执行阶段命令（实际 UI 操作测试明确不执行）：

```text
cargo fmt --all
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p feiwen --bin feiwen --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p feiwen --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --all-features --locked
git diff --check
```

残留扫描的活动代码/测试目标：`Form::try_new`、`try_new_with_validator`、`FormEvent::Committed`、
`FormEvent::ModelReplaced`、`.try_value(`、`.try_case(`、`QueryPath`、`QueryItemsPath`、`ControlLease`、
`rebase_if_revision`。历史开发文档、普通 gpui-component `value()`、Query/Fetch 的现有 operation/store/database
类型和上游 issue 链接必须逐项排除，不能用宽泛替换误删。

## 实施证据

- 实现位置：当前工作区，尚未提交；`WP-1300`–`WP-1304` 已完成。
- Fetch 已迁到 total path/快照 validator；Query 删除 `QueryPath`/`QueryItemsPath`，以具体 total/dynamic
  typed path、request-bound resolver 和 `PathImpact` 驱动递归行协调。
- root/dynamic group 分离；condition field 使用 custom binding，sort direction 使用 `FormSelect`；reconcile
  先完整构造候选结构再原子替换，失败保留旧控制树。
- `cargo test -p feiwen --bin feiwen --all-features --locked` 通过：93 项；
  `cargo clippy -p feiwen --all-targets --all-features --locked -- -D warnings` 通过。
- 活动 Feiwen 源码/测试旧 Form surface 与 path wrapper 精确扫描零命中；保留原有 Operation、Store、DB 与 Catalog。
- 未执行实际 UI 操作测试。
