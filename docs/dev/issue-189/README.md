# Issue #189：Jaco 消息请求用量、输入框上下文占用与使用统计

## 状态与范围

- 状态：`In progress`（`agent-message-request-usage-plan.md` 保留最终验证项；composer 的 `WP-102`、`WP-202`、`WP-302`、`WP-402`、`WP-502` 已 `Implemented`；workspace-wide gates、现场provider refresh/新请求、完整人工矩阵与三平台 CI 待做；Settings 计划仍为 `Draft`）
- 关联 issue：[#189](https://github.com/suxiaoshao/gpui/issues/189)
- 父 issue：[#159](https://github.com/suxiaoshao/gpui/issues/159)
- Plan ID：`issue-189`
- 根计划：`docs/dev/issue-189/README.md`
- 根索引：[Workspace development plans](../README.md)
- 分支：`codex/189-jaco-show-context-usage`
- 最近更新：2026-08-20
- 实施引用：composer 工作包已实现；implementation commit/PR `Pending`

## 三个独立执行文档

| 顺序 | 执行文档 | 状态 | 独立职责 |
| --- | --- | --- | --- |
| 1 | [Agent 消息单次请求用量](agent-message-request-usage-plan.md) | `In progress` | 最终 agent 消息与 provider step 的单次 usage 关联、Copy/时间工具栏入口、始终可hover的图标与group-hover total摘要、原生HoverCard、实时与重载一致性 |
| 2 | [输入框上下文占用](composer-context-occupancy-plan.md) | `Implemented` | 当前模型 context window、最新成功请求占用、模型切换与未知状态、footer Gauge + 百分比 HoverCard |
| 3 | [设置页时间范围使用统计](settings-usage-analytics-plan.md) | `Draft` | 本地日历范围、数据库聚合、趋势、provider/model 分组与设置页状态 |

三个文档共享 persisted `usage_events`，但不共享展示语义：消息显示单次 request usage，输入框显示 context occupancy，设置页显示范围聚合。任何执行文档都不得用另一个文档的投影替代自己的数据契约。

## 已确认的用户决定

- 一个 issue 内完成三个产品面，并使用三个独立执行文档。
- 先完成 `agent-message-request-usage-plan.md`。
- Agent 消息用量入口放在复制按钮与完成时间所在的 action row。
- Agent消息HoverCard参考Alma：输入、输出、缓存读取、缓存命中率、缓存写入、推理与总Token。
- 统计图标始终可hover，且只有图标本身是详情pointer热区；reported total token摘要和完成时间采用相同字体、字号、颜色与message group-hover reveal，摘要使用`k`/`M`等紧凑表示且不触发详情；详情保留完整精确整数；使用`HoverCard`组件默认打开/关闭延迟。
- Partial、unreported、unavailable 状态不在 action row 把未知显示为 0，只保留统计图标；具体状态和可用字段在详情中显示。
- 消息用量是纯pointer HoverCard；不提供点击固定、Escape、键盘打开或focus return，也不在app层维护hover状态/Task。
- Agent 消息不显示 context window 或占用百分比；上下文占用只属于 composer。
- #189 不采集或展示 TTFT、Token/秒、延迟或吞吐指标。
- Composer 常驻摘要采用 `Gauge + 百分比`；未知显示 `Gauge + —`，完整精确值和原因放入详情。
- Composer 整组摘要使用原生 pointer-only `HoverCard` 默认延迟；不提供点击固定、Escape、键盘打开或 focus return。
- Composer numerator 使用当前 conversation 最新成功请求唯一 usage event 的 `total_tokens`；running、failed、canceled 不替换上一个成功请求。
- 最新成功请求如果是 partial、unreported 或 missing usage，则显示 unknown 且不向前回找；当前 provider/model 与 latest fact 不匹配时同样不回找历史。

## 共享非目标

- Provider pricing、成本、账单、预算或 quota。
- 将未知 context window 替换成通用、family-wide 或启发式默认值；官方文档对精确型号公布的正整数上限可作为带 provenance 的 capability profile。
- 对未发送草稿做 tokenizer 或下一次请求估算。
- 自动 compact、截断、阻止发送或 context-limit enforcement。
- #194 的 manual model CRUD、编辑器与 override layering。
- 导出、远程 telemetry、TTFT、TPS 或其他性能统计。

## 兼容与迁移策略

- 三个工作面以当前 `usage_events` 为持久化 authority；旧记录缺失 usage 时保留 unknown/unavailable，不回填猜测值。
- Agent 消息计划不修改 SQLite schema、migration 或已持久化 `ProviderUsageSnapshot` JSON。
- Composer 计划若扩展 capability/run snapshot，必须以 serde default 保持旧 JSON 可读。
- Settings 聚合必须独立定义本地日历边界与覆盖率，不能复用消息或 composer 投影。

## 计划映射

前两个执行文档复用同一组 owner 计划入口；各 owner README 分开登记 `WP-?01` 的已实施证据与 `WP-?02` 的 composer Ready contract：

| Owner | 文档 | 职责 |
| --- | --- | --- |
| `crates/jaco-core` | [owner plan](../../../crates/jaco-core/docs/dev/issue-189/README.md) | usage contract；context capability、latest request fact、Conversation change/effect |
| `crates/jaco-db` | [owner plan](../../../crates/jaco-db/docs/dev/issue-189/README.md) | message projection；composer selector/assembler 与 reload/finalization 事务边界 |
| `crates/jaco-conversation` | [owner plan](../../../crates/jaco-conversation/docs/dev/issue-189/README.md) | message collection 与 composer singular fact hydration |
| `crates/jaco-agent` | [owner plan](../../../crates/jaco-agent/docs/dev/issue-189/README.md) | message live publication；capability discovery 与 composer live publication |
| `app/jaco` | [owner plan](../../../app/jaco/docs/dev/issue-189/README.md) | message action row；composer projection、footer HoverCard、图标、Fluent与UI验证 |

Settings owner/WP 映射在第三执行文档完成时补充；不得直接复用 composer singular fact 做范围聚合。

## 跨文档顺序

1. 实施并验收 Agent 消息单次请求用量。
2. 在不改变历史消息语义的前提下完成 composer context occupancy。
3. 以全部 eligible usage events 为 universe 完成 Settings 聚合。

## 聚合完成条件

- 三个执行文档均达到 `Done`，且各自的指标、unknown 语义和测试互不替代。
- 记录实际 commits、PR、变更文件、自动化、人工场景、跨平台 CI 和未验证边界。
- 同步 root/owner 索引与最终稳定 owner README；若产生长期跨 issue 约束，再单独评估 ADR。

## 完成证据

| 证据 | 当前结果 |
| --- | --- |
| Implementation commits / PR | `Pending` |
| Agent 消息执行文档 | `In progress`，代码、本地自动化已完成，最终HoverCard交互已由用户检查确认；真实provider请求和三平台CI待执行 |
| Composer 执行文档 | `Implemented`；`WP-102`、`WP-202`、`WP-302`、`WP-402`、`WP-502` 的聚焦验证已通过；旧模型缓存保持unknown直至用户refresh；workspace-wide gates、现场provider refresh/新请求、最终bundle、完整人工矩阵与三平台 CI 待执行 |
| Settings 执行文档 | `Draft` |
| 自动化、人工与 CI | Composer focused tests、`cargo fmt`、selected-package combined strict clippy 与 `cargo check -p jaco` 通过；此前UI构建的隔离配置fresh no-model `Gauge —`、AX label、默认HoverCard/details/layout已验证；移除读取时兼容补全后的最终bundle、workspace-wide build/test/clippy、现场provider refresh/新请求、完整人工矩阵与CI未执行 |
