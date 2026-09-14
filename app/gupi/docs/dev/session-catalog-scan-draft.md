# 会话目录扫描调研记录

状态：前期源码调研记录，2026-09-14 基于 `43991fd2` 核对。后续读取优化已实现，最新实现和验证以读取优化计划为准。

当前设计与实施步骤统一见[会话目录读取优化计划](issue-229/README.md)；本文仅保留目录扫描和 Codex 对照的源码依据。

## 已确认的方向

- 单文件顺序读取，复用字节缓冲，由 sonic-rs 按字段解析目录元数据。
- 保留现有全量目录扫描、后台有界并行、完成后发布及刷新/错误处理。
- 保留 user/assistant 外层 timestamp 最大值排序，不使用 mtime。
- 搜索保持现有内存过滤。本次不实施分批读取、独立文件搜索或新的 Operation。

## Gupi 优化前实现

源码入口：`src/foundation/session_catalog.rs`、`src/state/conversation.rs`、`src/state/conversation/catalog.rs`、`src/features/home/navigation.rs`。

1. 后台扫描默认 session 根目录的项目子目录、环境变量覆盖目录，以及全局和已知项目配置中的 `sessionDir`。自定义目录直接枚举 JSONL；不扫描整个文件系统。
2. 发现阶段读取每个文件的 header，从 `cwd` 继续发现项目配置目录，去重后固定文件集合。当前项目和已有内存会话也提供已知项目路径。
3. 读取阶段最多并发处理 8 个文件。为取得最新 `session_info.name`、首条用户消息摘要和全部 user/assistant 外层 timestamp 的最大值，逐行读取并解析完整 JSONL；内存目录仅保留元数据，不保留每个会话的完整正文。
4. 进度约每 100 毫秒通知一次，阶段切换与最终进度立即通知。扫描中只上报数量，全部读取成功后按活动时间倒序、路径作为相同时间的排序条件，一次性发布目录。
5. 首次扫描完成前不展示本轮已读出的历史条目；刷新时展示旧目录，成功后整体替换。扫描不在 UI 线程执行，也不以扫描完成作为新对话启动的前提。
6. 读取错误使整轮扫描失败，不发布部分结果；有旧目录则保留旧结果。运行中的重复手动刷新被拒绝，应用操作引发的刷新合并为一次后续扫描。
7. 侧栏项目展开后默认显示 5 条（当前选中项可额外保留），“显示更多”展开已经加载的其他条目，不请求下一批文件。折叠项目不减少后台读取量。
8. 当前没有独立目录索引数据库、文件监听或未变化文件的元数据缓存。启动、显式刷新及相关会话操作会触发目录扫描；快速打开搜索使用已取得的目录信息。

## Codex 对照：分页调研

侧对话已从本机 Codex Electron 26.908.40834（build 8881）的静态源码核对：前台目录按需分页，后台另有分页同步与本地目录数据库。未点击“更多”不代表后台不读取后续目录。目录元数据与会话完整正文属于不同加载层次。

上述内容仅为 Codex 的对照事实，不作为 Gupi 读取优化的实施要求。

## Codex 对照：时间来源与排序

本节区分已安装 Electron 包与本机 Codex 源码仓库。后者当前提交为 `aee8a55ab6`，本轮未拉取更新，也未证明其与安装包内 app-server 是完全相同的构建。

### 已安装 Electron 目录层

- 同时保存 `sourceCreatedAt`、`sourceUpdatedAt`、`sourceRecencyAt`。本地 Codex thread 的映射优先使用 RPC `recencyAt`，缺失或无效时回退 `updatedAt`。
- 目录的非创建时间排序使用 `source_recency_at DESC`，相同时按 `source_created_at DESC`、`thread_id` 排序；游标携带相同的排序字段。创建时间排序另有分支，手动顺序及置顶也不应概括为统一的最近时间排序。
- 传统 recent 路径默认排序键为 `recency_at`；不支持该字段的 app-server 会回退 `updated_at`。
- 后台目录同步通过 `thread/list` 按 `updated_at DESC` 分页，并使用 `updatedAt` 计算同步水位。用于发现数据变化的时间和用于侧栏展示的时间分开，不会把每次普通数据更新都等同于新的最近交互。
- 前端启动发送流程时也会把当前任务的 `updatedAt`、`recencyAt` 更新为当前时间；这是界面内存状态，不能单独据此推断后端所有写入规则。
- ChatGPT 会话进入目录的适配路径则把服务端 `update_time` 转换为更新时间与最近时间；不能将其与本地 Codex thread 的独立 `recencyAt` 规则混为一谈。

已提取的证据：[侧栏时间字段源码摘录](/private/var/folders/rq/c4thf05d5zz26j9g3nf759w00000gn/T/codex-sidebar-source-ld9z8im5/sidebar-time-evidence.md)。该路径是本机临时调研材料；以上结论已记录在本草稿中。

### 本机 Codex Rust 源码中的更新规则

- 实时追加记录包含 `TurnStarted` 时，将 `advance_recency_at` 设为当前 UTC 时间。这里表示新一轮开始，不能扩大解释为所有鼠标点击、打开会话或查看历史都会刷新最近时间。见 [thread_metadata_sync.rs](/Users/sushao/Documents/code/codex/codex-rs/thread-store/src/thread_metadata_sync.rs:159)。
- 普通输出及回合结束可以推进 `updated_at`，但不因此推进 `recency_at`。已有回归 `live_thread_output_advances_updated_at_but_not_recency_at` 明确检查回合开始后最近时间增长、后续输出完成时最近时间不变；本轮只读取测试源码，未执行该测试。见 [local/mod.rs](/Users/sushao/Documents/code/codex/codex-rs/thread-store/src/local/mod.rs:1049)。
- 名称更新和推进最近时间是独立分支；只有显式 `advance_recency_at` 才调用最近时间写入。见 [update_thread_metadata.rs](/Users/sushao/Documents/code/codex/codex-rs/thread-store/src/local/update_thread_metadata.rs:545)。
- 数据库持有 `recency_at_ms`、`updated_at_ms` 等元数据，最近时间更新保持单调；列表可以使用已维护的元数据分页，无须每次显示列表都完整解析全部 rollout。见 [threads.rs](/Users/sushao/Documents/code/codex/codex-rs/state/src/runtime/threads.rs:780)。
- 兼容路径确实会使用文件修改时间：从 rollout 文件提取元数据的代码会将文件 mtime 同时作为 `updated_at` 和 `recency_at`。因此不能声称 Codex 在所有路径都只依赖消息事件或完全不用 mtime。见 [rollout/metadata.rs](/Users/sushao/Documents/code/codex/codex-rs/rollout/src/metadata.rs:159)。

### 对 Gupi 的含义

当前生产代码按 user/assistant 外层时间最大值排序，与 Codex 上述实时路径的“新一轮开始时间”语义不同，也与 Pi 优先使用消息内部 timestamp 的规则有区别。本次已确认保留 Gupi 现有时间语义。

Codex 能按已知最近时间分页，依赖预先维护的目录时间字段。Gupi 本次采用优化的顺序元数据扫描，无需引入数据库或 mtime 排序。

## 验证边界

- Gupi 当前扫描行为来自源码核对。已用临时 Rust 程序测量完整目录发现、元数据读取与侧栏纯数据准备，结果与限制见读取优化计划；尚未测真实 GPUI 布局、绘制与首屏呈现。
- Codex 分页结论来自已转交的静态源码证据；未验证当前账号功能开关或运行时请求。
- 前期调研未修改生产代码；后续实现见读取优化计划。没有发起实际模型请求或执行 session 数据迁移。
