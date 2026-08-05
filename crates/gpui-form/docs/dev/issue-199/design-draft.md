# Issue #199：gpui-form 设计草稿

## 状态与范围

- 状态：`Draft`
- 所有者：`crates/gpui-form`、`crates/gpui-form-gpui-component`
- 总入口：[gpui-form 总指导、进度与状态](README.md)
- 已完成实现：[Form vNext 重构计划](form-vnext-refactor-plan.md#实施结果2026-08-05)
- 文档职责：只保存尚未进入代码的来源感知控件投影设计

已经由源码、测试和已完成执行计划固定的 schema、typed path、runtime identity、topology、validation、
prepare/rebase 与应用迁移设计不再在本草稿重复。被取代的讨论由 Git 保存，不在当前设计中保留。

## 用户决定

- 这套新设计是在现有 Form vNext 进入 `main` 前确认，因此不保留现有事件或 adapter API 兼容层。
- 按完整目标重新设计，不采用 adapter 本地布尔方向保护，也不把 raw control identity 暴露给应用。
- Form 仍是字段值的唯一权威；控件提交的值不得立即原样投影回同一个控件。

## 尚未解决的问题

当前 `ControlBinding::defer_set` 最终进入与程序调用相同的 `set` 路径，提交时丢失发起控件身份。
adapter 又会监听所有 model event，并把 Form 当前值静默设置回 native component。因此一次 native
change 会形成以下无意义回路：

```text
native control change
  -> ControlBinding::defer_set
  -> Form commit
  -> adapter observes FormEvent
  -> native setter receives the same value
```

同时，现有 adapter 只排除 validation event，并不统一判断提交是否影响自身路径。一个字段提交可能
使所有 adapter 都读取并重投影各自的值。

## 目标设计

### 一、事件公开业务事实，内部携带不可伪造的路由信息

目标公开事件改为可扩展 payload：

```rust,ignore
#[non_exhaustive]
pub enum FormEvent {
    ModelChanged(ModelChange),
    ValidationChanged {
        revision: FormRevision,
    },
}

pub struct ModelChange {
    revision: FormRevision,
    kind: ModelChangeKind,
    route: ChangeRoute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelChangeKind {
    ValueCommitted,
    TopologyChanged,
    ModelReplaced,
}

impl ModelChange {
    pub fn revision(&self) -> FormRevision;
    pub fn kind(&self) -> ModelChangeKind;
    pub fn affects(&self, path: &PathKey) -> bool;
}
```

`ModelChange` 的字段保持私有。普通事件消费者只能读取 revision、变化种类以及某个 opaque
`PathKey` 是否受影响，不能读取或构造控件来源和 canonical topology 信息。

内部路由契约：

```rust,ignore
struct ChangeRoute {
    origin: MutationOrigin,
    affected: AffectedPaths,
    topology_epoch: TopologyEpoch,
}

enum MutationOrigin {
    Programmatic,
    Control(ControlId),
}
```

- `ControlId`、`MutationOrigin`、`AffectedPaths`、canonical address 和 topology epoch 全部为
  crate-private。
- `AffectedPaths` 表达路径相交语义，不只保存一个展示用 path。cross-parent move 同时覆盖 source 与
  destination；subtree removal 同时覆盖被移除子树和 collection；whole-model replacement 覆盖全部路径。
- `ModelChange::affects` 对 wrong-session path 返回 `false`，且不泄漏内部 token 或 address。
- validation-only 变化不携带 value projection route，也不会触发 native value setter。

### 二、写入入口在进入私有 Transition 前固定来源

公开业务调用保持领域 API：

```rust,ignore
path.set(&form, value, cx); // Programmatic
form.reset(cx);             // Programmatic
form.replace(model, cx);    // Programmatic
```

控件 adapter 只通过 binding 提交：

```rust,ignore
binding.defer_set(value, window, cx); // Control(binding.control_id)
```

`defer_set` 不再转调会丢失来源的普通 `set`，而是调用 core-private control commit。私有 Form message
必须在一次逻辑 mutation 中同时携带：

```rust,ignore
Commit {
    kind: ModelChangeKind,
    origin: MutationOrigin,
    affected: AffectedPaths,
}
```

私有 `Transition` 负责 revision 与 effect；Form façade 负责应用 model/topology/validation 事务并发布
effect。相等写入仍是完整 no-op；一次成功逻辑 mutation 仍只推进一次 revision、发出至多一个 model
event，并 notify 至多一次。

### 三、`ControlBinding` 统一决定是否向控件投影

adapter 和自定义控件不再直接订阅 `FormEvent` 后自行判断。`ControlBinding` 提供唯一受支持的 value
projection 入口：

```rust,ignore
#[non_exhaustive]
pub enum ControlProjection<T> {
    Value(T),
    Retired,
}

impl<Root, T> ControlBinding<Root, T>
where
    Root: FormSchema,
    T: Clone + PartialEq + 'static,
{
    pub fn subscribe_projection_in<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
        callback: impl FnMut(
                &mut Owner,
                ControlProjection<T>,
                &mut Window,
                &mut Context<Owner>,
            ) + 'static,
    ) -> Subscription
    where
        Owner: 'static;
}
```

`subscribe_projection_in` 在 core 内部统一执行以下规则：

1. `ValidationChanged` 不产生 value projection。
2. model change 的 `origin` 等于当前 binding 的 `ControlId` 时不投影。
3. `affected` 与当前 binding path 不相交时不投影。
4. 其他控件修改同一字段时，读取 Form 的当前权威值并产生一次 `Value(T)`。
5. 程序调用、reset、rebase、replace 和相关 topology mutation 影响该 path 时产生一次 `Value(T)`。
6. 动态 path、incarnation、lease 或 topology epoch 已失效时不读取旧位置；仍存活的订阅产生
   `Retired` 后不再投影。
7. 投影回调延迟到当前 Form update/event 发布边界之外，并在执行前重新检查 binding freshness。

内置 `FormInput`、`FormIntegerInput`、`FormSelect` 和 `FormCombobox` 必须全部使用该入口。自定义组件
也使用相同入口，不再复制 `subscribe_total`/`subscribe_dynamic`、event filtering 或 path resolution。

### 四、多控件与多路径语义

| 变化 | 发起控件 A | 同字段控件 B | 无关字段控件 C |
| --- | --- | --- | --- |
| A 提交新值 | 不回传 | 投影新值 | 不投影 |
| 程序调用字段 `set` | 投影新值 | 投影新值 | 不投影 |
| reset/rebase/replace | 按影响范围投影 | 按影响范围投影 | 按影响范围投影 |
| validation-only 变化 | 不设置 value | 不设置 value | 不设置 value |
| A 所在 dynamic path 被 retire | `Retired` | 由各自 path 决定 | 不投影 |

Form 在所有情况下仍持有权威值。来源抑制只禁止把同一次提交原样回传给发起控件，不会阻止另一个
绑定同一路径的控件同步，也不会让 native component 成为第二份业务状态。

### 五、API 边界

- 普通 Form 用户继续使用 typed path、`set`、collection mutation、`replace`、`reset`、`rebase`、
  `validate` 和 `prepare`，不传 origin。
- 观察整个 Form 的应用可以继续监听 `FormEvent`；该事件用于重绘、审计或跨字段业务观察，不是自定义
  控件实现双向同步的底层协议。
- 自定义控件通过 `bind_control`/`try_bind_control`、`defer_*` 与
  `subscribe_projection_in` 接入。
- 不公开 raw `ControlId`、`MutationOrigin`、Form message、dispatch、canonical address 或 topology token。
- 不保留旧 `FormEvent::{Committed, ModelReplaced}` pattern、adapter 私有订阅 helper 或兼容 wrapper。

## 与 gpui-component Combobox 问题的边界

来源感知投影与 [`gpui-component#2652`](https://github.com/longbridge/gpui-component/issues/2652)
解决不同问题：

- 本设计阻止 Combobox 自己提交 `[A, B, C]` 后立即再次收到相同的
  `set_selected_values([A, B, C])`。
- reset、rebase、replace、catalog refresh 或另一个控件修改仍需要真正执行外部 value projection；
  因此 `set_selected_values` 在 active filter 下保留完整已选集合的问题仍须由 gpui-component 修复。

两者不能互相替代，也不在 Form 内增加隐藏选项补偿或 adapter 本地方向布尔值。

## 实施验收

后续实施必须至少固定以下自动化场景：

1. 控件 A 提交后自身不收到 projection；同 path 控件 B 收到一次；无关 path 控件 C 不收到。
2. programmatic `set`、reset、rebase 和 replace 向所有受影响 binding 投影一次。
3. validation-only event 不调用任何 native value setter。
4. A、B 快速连续 deferred write 的 origin 不串线，不能用共享布尔状态误判。
5. dynamic item remove/reinsert、case `A -> B -> A`、optional 重建和 whole-model epoch 变化不会让旧
   binding 复活。
6. cross-parent move 的 source/destination binding 均按一次逻辑 mutation 得到正确投影。
7. stale subscription callback 不读取新 incarnation，并只产生一次 `Retired` 或安全 no-op。
8. 一次成功 mutation 仍只有一次 revision、一个 model event 和一次 notify；相等写入没有事件。
9. public API 与 rustdoc 不暴露 `ControlId`、origin 或 topology 内部表示。
10. 四个 gpui-component adapter 和一个独立自定义控件 fixture 使用同一 projection 协议。

不进行兼容 fixture；旧事件 pattern 和旧 adapter subscription helper 应在同一轮直接删除。
