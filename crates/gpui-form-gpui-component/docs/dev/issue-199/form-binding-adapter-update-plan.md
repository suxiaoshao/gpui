# gpui-form-gpui-component：绑定与 adapter 目标更新计划

## 状态、边界与依赖

- 状态：`Done`（2026-08-09）；自动化 GPUI harness 已通过，实际 UI 操作测试按范围未执行。
- 计划 ID：`issue-199`
- 架构来源：[gpui-form 目标设计草稿](../../../../gpui-form/docs/dev/issue-199/design-draft.md)
- 对外契约：[组件使用指南](../../guide.md)
- 所有者：`crates/gpui-form-gpui-component`
- 本计划实现 Input、IntegerInput、Select、Combobox 对 `C-902` binding 的消费，并提供自定义 stateful adapter 的范式。
- 不负责 core Form/topology/mailbox/Transition、schema derive、应用 catalog/page observer/错误可见性、Jaco/Feiwen 迁移或 gpui-component 上游修复。
- 本计划保持为独立专题文档；Issue README 只增加状态和链接，历史计划、代码与 manifest 不在计划创建阶段改写。
- 兼容策略：破坏性变更；删除 `ControlLease`、cloneable generic `ControlBinding`、adapter-owned `FormEvent` subscription 和本地方向 flag；不提供 wrapper/alias。

本计划消费 core 所有者定义的 `C-900`–`C-904`，不另建共享 C-ID：

| Core 契约 | adapter 的消费方式 |
| --- | --- |
| `C-900` | `Entity<Form<M>>`、total/dynamic path、runtime-owned occurrence、`ResolveError` 与 Form session 生命周期。 |
| `C-901` | mutation/change/typed impact；由 core binding 消费，adapter 不再订阅或过滤 event。 |
| `C-902` | non-`Clone` `ControlBinding`、cloneable `ControlWriter<M,T>`、`ControlProjection::{Value,Retired}` 与来源感知 lifecycle。 |
| `C-903` | validation facts、blur/external trigger、prepare 和 control issue 的 report 语义。 |
| `C-904` | private Transition/atomic rollout；adapter 不命名内部状态，完成后参与残留/聚合验收门槛。 |

## 已核对事实

| ID | 分类 | 事实与证据 | 后果 |
| --- | --- | --- | --- |
| `E-1100` | 当前事实 | `src/{input,select,combobox}.rs` 的 wrapper 都保存 `subscriptions + ControlLease + native Entity`，并各自 `subscribe_in(form, ..., FormEvent)` 后 defer 回读。 | 移除四份手工 Form subscription，binding 成为唯一 projection owner。 |
| `E-1101` | 当前事实 | 当前 `ControlBinding<Root,T>` 可 clone，`lease()` 另给 wrapper 弱存活标记；integer 的 form subscription 还捕获 binding 以清除 issue。 | wrapper 改为持有唯一 non-generic binding，native callback 只捕获 writer。 |
| `E-1102` | 当前事实 | `FormInput` 监听 `InputEvent::Change/Blur`；`FormSelect` 监听 `SelectEvent::Confirm`；`FormCombobox` 监听 `ComboboxEvent::Change`。 | 保留这些 native intent 边界和使用当前 delegate 的投影，不保留手工 event routing。 |
| `E-1103` | 当前事实 | `IntegerInputState` 已将精确 integer、editor text、policy、checked step 和 parse error 保持在 native state；`integer_event_subscription` 用 control issue 阻断 `prepare`。 | 保留精确整数和 issue code/message；issue 原子清理交给 writer/core contract。 |
| `E-1104` | 当前事实 | `tests/adapters.rs` 已有 total/dynamic、drop、queued callback、whole-model replacement、integer issue、typed select/combobox 覆盖，但仍依赖 `Form::try_new`、`try_some`、lease 和旧 event routing。 | 将 fixture 重写为最终 API，并新增 source suppression、peer projection、Retired 和 custom adapter 测试。 |
| `E-1105` | 当前事实 | manifest 只依赖 `gpui`、`gpui-component`、`gpui-form`；没有 story、adapter feature 或生成步骤。 | 不增加 `gpui-operation` 依赖、feature、lockfile 或 story。 |
| `E-1106` | 用户决定 | Form 是业务值权威；native entity 只拥有 IME、focus、selection、popup 和未完成 editor text。 | adapter 不缓存 Form value、delegate/value-index map、Form entity 或 page business state。 |
| `E-1107` | 用户决定 | 控件 A 的写入不回传 A，但同 path 的 B 收到最新值；无关/validation-only/structure-only 变化不设置 native value。 | source suppression、impact filter、mailbox 和 freshness 一律委托 `C-901/C-902`。 |
| `E-1108` | 用户决定 | options/catalog 属于应用；刷新不能 fallback、写 Form、隐式校验或持久化。 | adapter 不增加 option setter/reconcile API。 |
| `E-1109` | 外部边界 | `gpui-component#2652` 负责 `set_selected_values` 在 filter 下从完整 source 找回已提交 values。 | 不在 Form adapter 复制 source、注入 hidden option 或实现补偿。 |

## 所有者决策

| ID | 决策 | 排除的方案 | 后果 |
| --- | --- | --- | --- |
| `D-1100` | 每个 stateful wrapper 持有 `Vec<Subscription>`、一个 non-`Clone` `ControlBinding` 和 native `Entity`；保留 `Deref<Target = Entity<State>>`。 | wrapper 不持有 binding、用 lease，或让 native subscription 保存 binding clone。 | drop 顺序为 subscriptions → binding → native entity；binding drop 是唯一 runtime cancellation/issue cleanup 点。 |
| `D-1101` | constructor 先读 initial value/create native state，再调用 `bind_control_in/try_bind_control_in`；projector 接收穷尽 `Value/Retired`。 | wrapper 自订阅 `FormEvent`、回读 path、自己识别 origin/impact。 | 绑定在 core 内统一 source suppression、合并投影和 lifecycle。 |
| `D-1102` | native event subscription 只捕获 cloneable `ControlWriter<Root,T>`，调用 `defer_set/defer_blur/defer_set_issue/defer_clear_issue`。 | direct `Form::update`、public Form message、writer 反向读取 Form 或本地方向 boolean。 | native→Form 永远 defer，writer 不延长 Form/adapter 生命周期。 |
| `D-1103` | 内置 dynamic projector 收到 `Retired` 时不再调用 native value setter；renderer 按 dynamic `PathKey` 移除 wrapper。自定义 state 可用 `Retired` 标记 disabled/unavailable。 | 将退休 location 重新定向到相同 schema address 的新值，或把 `Retired` 编码成 `Option<T>`。 | value type 不丢失，旧 writer 永久失效；core 保证只交付一次。 |
| `D-1104` | Input/Select/Combobox 使用其现有 silent programmatic setter；integer 使用 `IntegerInputState::set_value`。setter 不得发 native Change。 | 把 Form projection 当作 native user event，再依靠 guard 消环。 | source suppress 是正确性契约，silent setter 是回路终点。 |
| `D-1105` | integer 的有效 `defer_set` 由 writer 原子清除自身 issue，即使值相等；parse error 只 `defer_set_issue`；Retired/drop 不保留 issue。 | adapter 在 Form subscription 中任意清除 issue，或让 validation-only 清空 editor state。 | 精确 control issue 不会阻断已移除/退休的 field。 |
| `D-1106` | Select/Combobox projection 总使用 native state 当前 delegate；option refresh 由应用完成 setter 后立即按 Form `get/try_get` 重投影。 | adapter 缓存 delegate/option map、选择 fallback、写回 Form 或修上游选择问题。 | catalog policy 与 `gpui-component#2652` 保持在正确 owner。 |

## 文件地图

```text
crates/gpui-form-gpui-component/
├── src/lib.rs                       # F-1100 修改：最终导出；不 re-export lease/旧 binding helper
├── src/input.rs                     # F-1101 修改：FormInput 的 binding/writer projector 与 native intent
├── src/select.rs                    # F-1102 修改：FormSelect 的当前 delegate Value/Retired projector
├── src/combobox.rs                  # F-1103 修改：FormCombobox 的当前 delegate Value/Retired projector
├── src/integer_input.rs             # F-1104 修改：精确 integer 的 binding/writer/Retired/issue 生命周期
├── src/error.rs                     # F-1105 修改：保留 Policy 与动态 Resolve build error 的最终拼写
├── src/integer_input/{error,parse,policy}.rs
│                                   # F-1106 复核后不修改：精确 parsing/policy 是 native-owner logic
├── tests/adapters.rs                # F-1107 修改：最终 API integration/custom adapter fixture
├── README*.md, docs/guide*.md       # F-1108 实现后仅校验/调整最终拼写；设计内容已由本轮文档确定
└── Cargo.toml                       # F-1109 不修改：不加 operation/feature/lockfile/story
```

`gpui-form/src/control.rs` 是 `C-902` producer；`form/path/topology/validation` 是
`C-900`–`C-903` producer。adapter crate 只消费公开 domain API，不导入 private transition/message/mailbox
module，也不依赖 `gpui-operation`。

## 目标契约

### L-1100：wrapper 布局与构造器

四个 wrapper 保留 native entity 的 `Deref`，但生命周期所有者改为单一 binding：

```rust,ignore
pub struct FormInput {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<InputState>,
}
pub struct FormSelect<D> {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<SelectState<D>>,
}
pub struct FormCombobox<D> {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<ComboboxState<D>>,
}
pub struct FormIntegerInput<N> {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<IntegerInputState<N>>,
}
```

每类保留 total `new` 和 dynamic `try_new`；参数顺序固定为
`(&Entity<Form<Root>>, path, build, &mut Window, &mut Context<Owner>)`。total 只可能返回本地 integer
policy error；dynamic 将 `ResolveError` 与 policy error 保留为 `FormIntegerInputBuildError`。
Input/Select/Combobox total constructor 无 path `Result`。

### L-1101：binding/writer 连接

所有 stateful constructor 使用相同顺序：

```rust,ignore
let initial = path.get(form, cx); // dynamic 路径使用 try_get(...)?
let state = cx.new(|state_cx| build(window, state_cx));
silent_project_initial_value(&state, initial, window, cx);

let (binding, writer) = path.bind_control_in(
    form,
    &state,
    |state, projection, window, cx| match projection {
        ControlProjection::Value(value) => silent_project(state, value, window, cx),
        ControlProjection::Retired => handle_retired(state, window, cx),
    },
    window,
    cx,
);
```

dynamic path 使用 `try_bind_control_in` 并返回
`Result<(ControlBinding, ControlWriter<Root,T>), ResolveError>`。projector 是 core 注册的唯一
Form→native 投影；不得安装 `subscribe_total/subscribe_dynamic` 或保留 `WeakEntity<Form<_>>`。

`Value` 时 setter 必须不发 native event；`Retired` 时 built-in wrapper 不再 setter 或写 Form，等待 keyed
renderer 移除它。custom stateful component 可在其 projector 内显式设置 disabled/unavailable。Form/owner 消失时
silent drop，不伪造 `Retired`。

### L-1102：native intent 与 issue 命令

| 适配器 | 原生事件 | writer 命令 | projector 的 Value | Retired |
| --- | --- | --- | --- | --- |
| Input | `InputEvent::Change` / `Blur` | `defer_set(String)` / `defer_blur` | `InputState::set_value` | 不 setter，等待 renderer 移除 |
| Select | `SelectEvent::Confirm(Option<Value>)` | `defer_set` | `set_selected_value` 或 `set_selected_index(None)` | 不 setter |
| Combobox | `ComboboxEvent::Change(Vec<Value>)` | `defer_set` | current delegate 的 `set_selected_values` | 不 setter |
| Integer | `IntegerInputEvent::Change(Result<N,_>)` / `Blur` | 合法值：`defer_set`；非法值：`defer_set_issue`；失焦：`defer_blur` | `IntegerInputState::set_value` | 不 setter，issue 已由 core 撤销 |

有效 `defer_set`（包括相等 value）由 `C-902`/`C-903` 原子清除此 control issue；integer adapter 不再先
`defer_clear_issue`。validation-only、structure-only、unrelated impact 均不写 native value、不清 integer
editor text。writer 命令只能来自 native subscription，且不得持有 strong Form/adapter。

### L-1103：自定义 stateful adapter 的最小范式

```rust,ignore
pub struct FormSlugInput {
    subscriptions: Vec<Subscription>,
    _binding: ControlBinding,
    state: Entity<SlugInputState>,
}

// 读取初值 -> bind_control_in(projector) -> native subscription 捕获 writer。
// 无 FormEvent subscription、无 ControlLease、无 binding clone、无 control ID/方向 flag。
```

projector 必须穷尽匹配 `ControlProjection::Value/Retired`。drop wrapper 时先 drop native subscriptions，
再 drop binding；已经排队的 writer command 由 `C-902` 验证 owner/lifecycle/location 后 no-op。

### ERR-1100：adapter 可见失败与恢复

| 失败 | 产生者 | 传播与恢复 |
| --- | --- | --- |
| dynamic mount 已退休/错 session | `try_get/try_bind_control_in` | `try_new` 返回 `ResolveError`；renderer 不创建或移除对应 keyed adapter。 |
| dynamic 运行时退休 | `C-902` | projector 仅收一次 `Retired`；旧 native event/writer 不再改变 Form。 |
| integer policy 非法 | `IntegerInputState::validate_policy` | total `new` 返回 `IntegerInputPolicyError`；不写 Form。 |
| dynamic integer policy/resolve | dynamic constructor | `FormIntegerInputBuildError::{Resolve,Policy}` 保留原因。 |
| incomplete/syntax/overflow/range | `IntegerInputState` parser | writer 设置带现有 code/message 的 control issue；用户修正为合法值后由 `C-902`/`C-903` 原子清除。 |
| native state/Form/adapter 消失 | GPUI drop/lifecycle | 沉默 no-op；不向页面伪造 runtime error。 |

### ST-1100：权威、投影与生命周期

- **Form 值：**`Entity<Form<M>>`（`C-900`）是唯一业务值/validation authority。
- **Binding：**wrapper 的 `ControlBinding` 独占 Form subscription、source suppression、impact filter 与 projection lifecycle。
- **Native state：**adapter 的 `Entity<State>` 持有 editor text、focus、selection、popup 与 delegate；不缓存 Form authority。
- **Writer：**native subscription 中可 clone 的弱 capability；不延长 Form/binding/wrapper。
- **Options：**应用/Store 持有；adapter 仅使用 state 的当前 delegate。
- **页面 observer：**只重绘页面 error/button/布局，不参与 adapter 同步。

## 工作包

### WP-1100：替换公共绑定持有与手工订阅

**文件：**`F-1100`–`F-1104`

**前置：**`D-1100`–`D-1104`、core `WP-905` 已提供最终 binding façade；不要求本 owner 参与交付的 `C-902` 已 producer-ready

1. 将四个 wrapper 改为 `L-1100` 三字段布局，删除 `ControlLease`、custom `Drop`、`subscribe_total`、
   `subscribe_dynamic` 与所有 `FormEvent` import。
2. total/dynamic constructors 按 `L-1101` 创建 state、安装 projector、保存 binding、只订阅 native event。
3. native callback 捕获 writer；保持 Input/Select/Combobox 的现有 event edge，禁止 direct Form update。
4. 内置 `Retired` 分支不写 native value/issue，依赖 renderer keyed reconciliation 销毁动态 wrapper。

**完成条件：**adapter 活跃源码没有 `ControlLease`、`binding.clone()`、`FormEvent` subscription、weak Form、
path 回读或方向 guard。

### WP-1101：重接精确整数与 control issue

**文件：**`F-1104`–`F-1106`

**前置：**`WP-1100`、core `WP-903`/`WP-905` 的 control issue contract

1. 保留 parse/policy/checked arithmetic/native editor ownership，移除 integer form subscription 中的 value 回读和 issue 清理。
2. 合法 event 只调用 `writer.defer_set(value, ...)`；非法 event 用 `defer_set_issue`；失焦用 `defer_blur`。
3. 保持 `FormIntegerInputBuildError` 的 Resolve/Policy 分类，按 final core error name 修正转换和 exports。
4. 验证 retired/drop/外部 Value 的 issue 清理由 core 一次完成，adapter 不在 validation-only 投影中清状态。

**完成条件：**`u64 > 2^53` 不经 `f64`/ad-hoc String business value；相等的合法 write 清 issue 而无 model echo。

### WP-1102：重写 integration 与 custom adapter fixture

**文件：**`F-1107`

**前置：**`WP-1100`、`WP-1101`、core `WP-901`–`WP-905`

1. 将 harness 改为 `Form::new`、`get/try_get`、最终 optional/case resolver 和最终 `FormVersion` API。
2. 新增 test-local stateful probe adapter，按 `L-1103` 保存 binding/writer，记录 silent projection 与 Retired。
3. 用同一 test fixture 覆盖四个 built-in adapter 的最终来源感知契约；不为 page observer 添加同步职责。
4. 保留 integer parser unit tests，删除仅为 lease/旧 event path 存在的断言。

**完成条件：**测试无需访问 control ID、canonical address、mailbox 或 private message，也不通过 sleep/实际 UI 操作验证时序。

### WP-1103：选择组件边界与文档一致性

**文件：**`F-1108`–`F-1109`

**前置：**`WP-1102`、`E-1108`、`E-1109`

1. 复核 README/guide 的示例与最终 constructor/error 拼写一致，保留 page observer、options refresh 与 custom adapter 的职责边界。
2. 验证 Select/Combobox 只使用当前 delegate；catalog refresh 后由应用显式重投影，不产生 Form write/fallback。
3. 不添加 `gpui-component#2652` workaround；若上游 API/测试修复，按上游 issue 单独消费，不能在本 crate 复制 source。

**完成条件：**文档和活跃 adapter 源码不暗示 options 属于 Form，或以 self-echo suppression 代替上游 selection correctness。

## 测试矩阵与命令

| R-ID | T-ID | 文件/层级 | 场景与断言 |
| --- | --- | --- | --- |
| `R-1100` | `T-1100` | `tests/adapters.rs` | Input A 写入只更新 Form，不对 A setter；同 path B 收一次 Value；无关 C 不投影。 |
| `R-1101` | `T-1101` | `tests/adapters.rs` | 程序 `set/replace/reset/rebase` 向 total binding 投影；validation-only/structure-only 不 setter。 |
| `R-1102` | `T-1102` | `tests/adapters.rs` | append/reorder 不重设现有 item control；remove/optional/case 重建只令旧 dynamic adapter 收一次 Retired。 |
| `R-1103` | `T-1103` | `tests/adapters.rs` | 连续 external change 合并为最新值；新 native edit 不被旧 projection 覆盖；drop/Form disappearance 后 writer no-op。 |
| `R-1104` | `T-1104` | `tests/adapters.rs` + integer unit tests | invalid text 阻断 prepare；相等的合法 edit 清自身 issue；retired/drop 清 issue；超大 `u64` 仍正确。 |
| `R-1105` | `T-1105` | `tests/adapters.rs` | Select Confirm/Combobox Change 写 typed values；options refresh 无 fallback/Form write；adapter 不补偿 `set_selected_values` filter 行为。 |
| `R-1106` | `T-1106` | test-local custom adapter | non-Clone binding 被 owner 持有、writer 仅 native callback 捕获、Value/Retired projector 穷尽匹配，零 FormEvent subscription。 |

```sh
cargo fmt --all
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p gpui-form --all-features --locked
cargo clippy -p gpui-form -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
git diff --check
```

不进行实际 UI 操作测试。GPUI integration test 使用现有 test-support window/harness；它验证 entity/event
契约，不替代人工视觉、焦点或 popup 测试。

## 验收

1. 四个 built-in adapter 和 custom fixture 都只通过 `ControlBinding/ControlWriter/ControlProjection` 接入 Form。
2. wrapper 不含 lease、cloneable binding、weak Form、手工 `FormEvent` subscription、origin/path filter 或方向 guard。
3. self echo、peer projection、unrelated/validation/structure suppression、latest-value coalescing、Retired/drop 与 whole-model lifecycle 均由自动化测试覆盖。
4. integer 的 typed value、editor text、policy 和 control issue 边界保持准确；无 app-side `f64` 转换。
5. options 继续属于应用；本 crate 不隐藏/重建 option source，不修 `gpui-component#2652`。
6. 聚焦测试、core 消费方门禁、格式化、Clippy 与 `git diff --check` 通过；不进行实际 UI 操作测试。

## 实施证据

- 实现位置：当前工作区，尚未提交；`WP-1100`–`WP-1103` 已完成。
- 四个 adapter 已统一使用 non-clone `ControlBinding`、cloneable `ControlWriter` 和
  `ControlProjection::{Value, Retired}`，活动源码无 lease、手工 Form event routing 或方向 guard。
- `cargo test -p gpui-form-gpui-component --all-features --locked` 通过：2 个 unit + 15 个 integration；
  覆盖 self echo、peer projection、whole-model lifecycle、dynamic retirement、mailbox freshness、
  collection reorder、integer issue 与 options refresh 边界。
- producer 聚合测试、Clippy、workspace check、格式、diff-check 与旧 surface 扫描均通过。
- 未执行启动 story、人工输入/选择、视觉、焦点或 popup 等实际 UI 操作测试。
