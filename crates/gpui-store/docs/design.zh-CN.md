# gpui-store 设计

[English](design.md) | [简体中文](design.zh-CN.md)

## 1. 定位

`gpui-store` 在 GPUI 应用中拥有类型化的 committed state。它统一 snapshot 读取、有效变化通知、观察、projection 和可选 backend reconcile，同时把领域命令与持久化策略留给应用。

它不是 form draft store、数据库 repository 或通用双向 binding framework。

## 2. Store 类型

### `LocalStore<S, B>`

Local store 由单个组件或 controller 直接持有。状态与生命周期属于该 owner、且不需要应用级访问时，应使用 local store。

### `SharedStore<S, B>`

Shared store 是承载应用级 committed state 或 catalog 的 GPUI entity。组件通过 GPUI subscription 观察它，并读取 owned snapshot。

两种 store 具有相同的状态语义，只在所有权和观察生命周期上不同，不代表不同的领域含义。

## 3. Snapshot 语义

读取 store 会得到 owned `S` snapshot（或其他明确 owned 的 projection）。调用方不跨 GPUI update 持有 store 内部引用，从而让状态边界可见，也避免长生命周期 borrow 泄漏到 store 外部。

Snapshot 表示一个时间点的值。验证、resolver 或 command creation 需要一致性时，消费方只捕获一次，并在整个操作中使用同一份值。

## 4. 修改与变化检测

Store 通过 replace、set、update 等显式命令修改。只有最终 committed state 按 store 的变化策略发生有效变化时，才通知 observer。

持久化或 backend refresh 失败时，保留最近一次有效 committed snapshot。外部 load/reconcile 成功后，一次性安装新 snapshot。

## 5. Selection 与 binding

`StoreSelection<T>` 是从 store state 派生的单向只读 projection。它可以缓存用于渲染的行、label 或 capability，但没有业务 setter，也不能成为 submit 的第二数据源。

`StoreBinding<T>` 的边界更窄：只有展示字段明确需要写回同一个 backing store 时才适用。它不能用来镜像组件局部编辑状态或 form snapshot。

派生出来的组件 options 属于展示配置。更新 options 不会更新 form 持有的当前选择。

## 6. Backend 边界

Backend 提供可复用的 load、save、subscribe 或 reconcile 机制。它不决定领域命令、验证、fallback selection 或 UI 行为。

应用 repository/service 已拥有持久化时，`MemoryBackend` 已足够。只有外部生命周期能在多个 store 用户之间真正复用时，才需要自定义 backend。

无论使用哪种 backend，内存 store 始终是应用的 committed source of truth。

## 7. Catalog

Provider、model、project 和 capability catalog 是典型的 `SharedStore` 值。Repository refresh 查询外部存储，只有成功后才原子替换 catalog。

消费方从一份 catalog snapshot 派生组件 options。组件当前选择已不再可用时，应用应明确报告或验证 mismatch，catalog 不会静默选择 fallback。

## 8. 与表单集成

Store 与 form 的所有权相互独立：

| 关注点 | 所有者 |
| --- | --- |
| Committed domain state/catalog | `gpui-store` store |
| 当前可编辑类型化值 | generated `gpui-form` store |
| 验证 baseline/report/submit runtime | generated `gpui-form` store |
| Focus、popup state 与 blur history | bound component instance |
| 持久化命令 | 应用 service/repository/store command |

应用通过显式 transition 协调它们：

```text
load committed state
  -> form.rebase(committed value)
  -> bound component 从 form 重新投影

user submit
  -> form.prepare_submit()
  -> 对同一份 form-owned model 做 validate/transform
  -> 执行应用命令
  -> reconcile committed store
  -> form.rebase(saved value)
```

这里没有隐式 form-to-store 或 store-to-form 同步。

## 9. GPUI 生命周期

Store observer 在 source callback 中读取并计算。跨 entity 变化通过显式 owner command 或 deferred task 发送；observer 不在 source entity 已经被 update 时递归 update 它。

Subscription 由拥有观察关系的 entity 持有。Owner drop 后，观察生命周期随之结束。

## 10. 公开不变量

- Store 表示 committed 应用状态，不表示组件临时编辑状态；
- 读取产生 owned、point-in-time snapshot；
- 外部操作失败时保留最近一次有效 snapshot；
- 变化通知只反映有效 committed change；
- selection 是只读 projection，不是第二数据源；
- catalog refresh 不静默改变组件选择；
- backend 机制不吸收领域或 form policy；
- store/form/component 同步始终是显式应用命令。

## 11. 非目标

`gpui-store` 不提供数据库 schema mapping、应用 repository、表单验证、组件焦点、错误展示、undo history 或任意状态所有者之间的自动冲突解决。
