# Issue #177：gpui-operation 与目标 gpui-store 设计覆盖评估

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

本文判断当前目标版 `gpui-operation` 与 `gpui-store` 设计，是否足以支撑修复
[Jaco 全局状态与底层数据绕过现状审计](global-state-audit.md)记录的问题。

评估依据是两个 crate 当前的公开设计文档，而不是尚未迁移的实现：

- [`gpui-operation` 使用指南](../../../../../crates/gpui-operation/docs/guide.zh-CN.md)
- [目标版 `gpui-store` 使用指南](../../../../../crates/gpui-store/docs/guide.zh-CN.md)

本文不是实施计划，不重新审计 Jaco 代码，也不修改原问题记录。

## 1. 结论

**可以。当前两份设计足以作为 issue #177 大部分修复的通用基础。**

更准确地说：

- GS-01～GS-13 都能在不扩张两个 crate 核心职责的前提下形成修复路径；
- 其中 GS-01、GS-02、GS-06、GS-12 的通用语义已由当前契约直接覆盖；
- GS-03、GS-04、GS-05、GS-07～GS-11、GS-13 仍需要 Jaco 完成领域状态、
  command、依赖和数据边界设计；
- GS-14 是网络 runtime 是否消费 `http_proxy` 的应用接线问题，不属于这两个 crate
  的职责，而且是否纳入 issue #177 仍未确定。

因此：

> “这两个设计可以支撑修复大部分问题”成立；
>
> “只要实现两个 crate，Jaco 的问题就会自然消失”不成立。

当前没有证据要求：

- 给 `gpui-operation` 增加资源依赖图或自动调度；
- 让 `gpui-store` 重新负责 backend、持久化、reconciliation 或 transaction；
- 让 `gpui-operation` 依赖 `gpui-store`，或反过来；
- 把 Jaco 的多阶段领域命令抽象成通用 Store/Operation 能力。

## 2. 两个设计分别解决什么

### 2.1 `gpui-operation`

`gpui-operation` 负责表达一次可能失败工作的事实和合法转换：

- 首次加载尚未开始、正在加载、成功或失败；
- 刷新失败后保留最后有效 Data，同时暴露最新 Problem；
- Problem 是否只需再次读取，还是需要调用者选择 Repair；
- 运行中的 Task、取消以及取消后恢复准确的 previous state；
- 不把加载失败伪装成合法空 Data。

它不负责保存全局状态、通知消费者、选择 repository、安排依赖、持久化或多阶段事务。

### 2.2 目标版 `gpui-store`

目标版 `gpui-store` 负责：

- 保存一份权威的纯内存 `S`；
- 让所有消费者通过同一个 `Store<S>` 读取和修改；
- `update` 在确定发生变化时无条件发布，`update_if` 由调用者通过 `StoreChange` 决定是否发布；
- 通过 `select`、`observe` 和 `observe_select` 提供派生读取与通知；
- 通过 typed-global Store 为应用级资源提供稳定身份。

它不读取文件或数据库，不执行 Task，也不理解 Data、Problem、refresh 或 Repair。

### 2.3 Jaco Resource

两者组合后，Jaco 的应用级 Resource 可以表示为：

```text
repository / service
        │
        │ 构造异步工作
        ▼
Jaco command ──发送消息──> 应用定义的 Operation runtime enum
                                │
                                │ 作为唯一权威状态保存
                                ▼
                       Store<ResourceState>
                                │
                    read / select / observe
                                ▼
                      components / services
```

`Resource` 仍是 Jaco 的架构概念，不需要新增通用 trait。应用定义 runtime enum、command
和 completion route；Operation 提供安全转换，Store 提供共享所有权和发布。

## 3. 覆盖分级

本文使用三个等级：

| 等级 | 含义 |
| --- | --- |
| 直接覆盖 | 问题所缺少的通用状态或所有权语义已经由两个设计完整表达，不需要新的领域抽象；Jaco 仍需按契约迁移 owner、command 和旧 fallback |
| 需要 Jaco 设计 | 通用 primitive 已足够，但关键决定属于 Jaco 的领域 snapshot、command、依赖、路由或模块边界 |
| 不属于职责 | 两个 crate 不应解决该问题 |

## 4. 问题逐项评估

| ID | 覆盖 | 当前设计提供的修复路径 | Jaco 仍需完成 |
| --- | --- | --- | --- |
| GS-01 | 直接覆盖 | 首次读取成功得到空集合时是 `Ready(empty)`；读取失败进入 `Unavailable(Problem)`，不能再由状态模型伪装成空 Data。Store 发布完整 Resource 状态。 | 删除 Provider、Project、Shortcut 的 `unwrap_or_default` 初始化语义，始终保存真实 completion。 |
| GS-02 | 直接覆盖 | `Ready(Data)` 刷新失败后进入 `Degraded(Data, Problem)`；旧 Data、最新错误和再次 `Refresh` 的入口同时存在。Store 让错误和数据变化可观察。 | 为 Provider、Project 暴露统一 refresh command，UI 根据产品策略决定是否继续使用 degraded Data。 |
| GS-03 | 需要 Jaco 设计 | Store 明确不把 persistence command 当作内存 mutation；DB commit 成功后可以单独 refresh Resource，refresh 失败进入 `Degraded`，不会改写已经发生的 commit 事实。 | 分开返回“持久化已提交”和“内存 snapshot 尚未 reconciliation”，避免把后者重新包装成 mutation `Err`。 |
| GS-04 | 需要 Jaco 设计 | Operation 可以分别表达可失败阶段，Store 可以保存 Jaco 定义的复合结果，但两个 crate 都不提供跨 DB、system hotkey 和 catalog 的事务。 | 为 Shortcut command 明确建模 persistence、hotkey sync、catalog reconciliation 三份结果，以及允许的补救操作。 |
| GS-05 | 需要 Jaco 设计 | Config 使用 repair-capable Operation 后，TOML 解析失败可以保持 `Unavailable(Problem)`，由调用者选择 Retry、恢复备份或重置；库不会生成默认 Config Data。 | 把 config `Ready` 作为选择 data dir 和打开数据库的显式前置条件；决定失败时进入 recovery UI 还是终止启动。 |
| GS-06 | 直接覆盖 | 启动时先同步安装包含 `Idle` 的 typed-global `Store<ResourceState>`，再启动加载。失败后 Global 仍然存在，只是状态变为 `Unavailable`。 | 调整 Hotkey 安装顺序，禁止“加载成功才安装 Global”和缺失时静默 no-op。 |
| GS-07 | 需要 Jaco 设计 | Store 能成为唯一 committed snapshot owner，但无法阻止调用者继续获取 raw repository。 | 收紧 `FreshRepository` 的模块可见性；只让 Resource/command 层执行 catalog query 和 mutation，保留确实属于按需内容加载或事务的访问。 |
| GS-08 | 需要 Jaco 设计 | Provider Resource 在 Store 中保存消费者真正需要的完整 snapshot；settings、conversation 和 agent 可以统一从同一个权威当前 source 读取。 | 确定 Provider Data 的完整边界和版本语义；创建 conversation/run 时传递或钉住所选 committed provider/model snapshot，停止重新查询并创建隐式缓存。 |
| GS-09 | 需要 Jaco 设计 | Prompt、Shortcut 分别拥有唯一 Store snapshot 后，dialog、hotkey 和 settings 可以共享同一份 Data 与通知通道。 | 把 validation、hotkey resolve 和 shortcut settings snapshot 改为消费 Resource；仅在 repository command 内保留必要的约束查询。 |
| GS-10 | 需要 Jaco 设计 | UI 可以通过 `select` 渲染 Project Resource；需要更新 Workspace 自身状态时，由 controller 使用 `observe_select` 发出明确 Workspace command，而不是把通知当作重新查询 DB 的 invalidation signal。 | 先确定 Project committed snapshot 是否包含 scratch project；区分可由 snapshot 回答的问题与必须访问搜索 backend 的正文查询。 |
| GS-11 | 需要 Jaco 设计 | Skill Resource 可以用 `Degraded(last-valid entries, Problem)` 保存扫描失败，并通过 Store 让 UI 与 agent 消费同一份合成 snapshot。 | 定义 global 与 project skill 的合并边界、版本/hash 和 refresh 触发方式；agent request 必须引用同一次 discovery 结果。Operation 不负责资源依赖。 |
| GS-12 | 直接覆盖 | typed-global Store 有明确安装点；Operation 用 `Unavailable` 表达初始化失败，因此不需要把“Global 缺失”解释为重新读取底层来源。 | 删除 provider、shortcut、layout helper 中的 DB/文件 fallback；把未安装的 required Global 当作程序错误。 |
| GS-13 | 需要 Jaco 设计 | Operation 统一提供 `Fetching`、`Unavailable`、`Degraded`、Problem、Refresh 和 Retry；Store 统一发布状态并让消费者观察。 | 文件 watcher、数据库事件或其他外部变化仍由应用 service 产生，并统一触发同一个 refresh command；两个 crate 都不提供 external subscription。 |
| GS-14 | 不属于职责 | Store 的 observation 可以传播配置变化，但不会让任一网络客户端实际使用 proxy；Operation 与此无关。 | 单独决定是否纳入 issue #177，并在 Jaco 网络 runtime 中完成真实接线。 |

## 5. 仍由 Jaco 决定的四类问题

### 5.1 Domain snapshot 边界

需要为 Provider、Project、Prompt、Shortcut、Skill 和 Workspace 分别确定：

- 哪些值属于完整 committed Data；
- 哪些值需要版本或 hash，供一次业务操作钉住；
- 哪些查询可以由 snapshot 回答；
- 哪些查询是按需内容、全文搜索或事务约束，应该继续访问 repository。

Store 只能保证一份 `S`，不能替 Jaco 决定正确的 `S` 是什么。

### 5.2 分阶段 command 结果

持久化是否已提交是不能被后续 refresh 结果覆盖的一级事实。特别是 Shortcut command，
至少要区分：

1. DB persistence；
2. system hotkey sync；
3. catalog reconciliation。

Operation 可以表达每个可失败工作，Store 可以发布最终内存状态，但阶段划分、部分成功语义和
补救动作必须由 Jaco 定义。

### 5.3 依赖与 completion route

`config → database → catalogs`、`project → workspace`、`catalogs → hotkey` 等关系由 Jaco
coordinator 显式门控和触发：

- 依赖尚未 `Ready` 时，不构造下游 load task；
- 依赖变化后，由 observer 发出明确 command；
- 应用级 Resource 可以通过 typed-global Store 找回同一个 owner 并投递 completion；
- 非 global Store 使用其外部 owner、Entity 或 Jaco 自己的 locator 路由 completion。

Operation 不保存依赖，Store 不建立依赖图，这一点不需要为 issue #177 改变。

### 5.4 持久化与 external sync

- repository/service 负责文件、数据库、keychain 和系统 API；
- watcher、数据库事件或系统回调只负责触发 Resource command；
- Operation 表达一次工作的生命周期；
- Store 只保存并发布最新内存事实。

恢复旧 Store backend 抽象不会自动解决 Jaco 的错误语义，反而会重新混合 ownership、I/O 和
状态机职责。

## 6. 当前设计尚未证明的内容

两个 crate 的文档描述的是目标 API，当前实现尚未迁移。因此本结论是设计充分性判断，不是
实现完成或集成已经验证。

后续原型至少需要证明：

- 应用定义的 Operation runtime enum 可以作为 `Store<S>` 的唯一字段安全转换；
- running state 中的 Task 不会形成强 owner cycle；
- completion、cancel 与 Store notification 各发布一次且没有临时无效状态；
- `Degraded` 的 Data 和 Problem 可以分别被 selector 消费；
- typed-global Store 能满足应用级 Resource 的 completion route；
- persistence success 与随后 refresh failure 可以在 Jaco command 中分别报告。

这些属于实现验证风险，目前没有显示出新的通用设计缺口。

## 7. 最终架构判断

issue #177 应继续采用当前职责拆分：

- 用 `gpui-operation` 统一初始化、刷新、修复和取消状态；
- 用目标版 `gpui-store` 保存并发布唯一的共享内存 Resource；
- 用 Jaco Resource/command/coordinator 定义领域 snapshot、持久化、多阶段结果和依赖；
- 不让 component 或 feature 直接绕过 Resource 重建同一份 catalog；
- 不把 failure、missing Global 或 stale snapshot 伪装成默认业务数据。

在这个前提下，当前 Operation 与 Store 设计可以支撑修复已确认的大部分全局数据问题，无需先给
两个 crate 增加新的通用抽象。
