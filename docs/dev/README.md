# Workspace development plans

## Feature plans

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#200](https://github.com/suxiaoshao/gpui/issues/200) HTTP Client Response 音频迁移与 GStreamer 删除 | `In progress`；Rodio/CPAL/Symphonia 替换 GStreamer，保留 PDF、删除视频与全部 GStreamer 产品/打包链路 | [issue-200/README.md](issue-200/README.md) |
| [#199](https://github.com/suxiaoshao/gpui/issues/199) form owner、app store/form/operation 与 Transition 重构 | `Done`；显式 owner v2、Form vNext、Feiwen、Jaco（含 Conversation 私有 Transition）与 HTTP Client 基础单请求已交付；历史媒体计划已由 #200 的 Rodio 迁移取代，MCP runtime 已移交 #201 | [issue-199/README.md](issue-199/README.md) |
| [#190](https://github.com/suxiaoshao/gpui/issues/190) Jaco 持久化工具调用详情 | `Implemented`；生产实现与本地自动化已验证，完整 Local/MCP 人工场景和远端三平台 CI 待验证 | [issue-190/README.md](issue-190/README.md) |
| [#189](https://github.com/suxiaoshao/gpui/issues/189) Jaco 消息请求用量、输入框上下文占用、时间范围统计与活动热力图 | `In progress`；activity heatmap及既有工作包已实施，最终人工与CI门待做 | [issue-189/README.md](issue-189/README.md) |
| [#178](https://github.com/suxiaoshao/gpui/issues/178) Jaco 外部文件变更监控 | `Implemented on branch / 已在分支实施，等待原生/人工/CI验证`；固定 data-dir 数据库目标，并以共享 watcher 自动刷新 config 与 global/project Skill | [issue-178/README.md](issue-178/README.md) |
| [#175](https://github.com/suxiaoshao/gpui/issues/175) previous typed form delivery | Issue/PR 已完成；form API 计划被 #199 取代 | [issue-175/README.md](issue-175/README.md) |

## Framework migrations

这里仅登记跨 workspace 的迁移批次。每次框架迁移使用独立的目标版本或 Git hash 标识，
不会用一个无版本文件覆盖历史计划；具体 app/crate 的实现内容放在各自的 `docs/dev`。

| 日期 | 迁移批次 | Source | Target | 状态 | 总计划 |
| --- | --- | --- | --- | --- | --- |
| 2026-07-21 | `gpui-1a246efd-component-5b45bcb` | GPUI `0.2.2@1d217ee`；gpui-component `0.5.2@c36b0c6` | GPUI `0.2.2@1a246efd`；gpui-component `0.5.2@5b45bcb` | **当前迁移**；计划待审阅；TextView 主题生命周期存在上游阻断，修复后必须新建后继 target 文档 | [README.md](migrations/gpui-1a246efd-component-5b45bcb/README.md) |

## 目录约定

- `docs/dev/issue-<number>/README.md`：跨 owner feature/refactor 的总指导、状态、顺序与命名专题文档索引；多轮任务的执行细节写入独立文件。
- `app/<name>/docs/dev/issue-<number>/README.md`、
  `crates/<name>/docs/dev/issue-<number>/README.md`：同一 issue 的 owner状态与专题文档索引；历史单轮README保持原样归档。
- `docs/dev/migrations/<target-id>/README.md`：只保存跨 workspace 的顺序、发布门和子计划索引。
- `docs/dev/migrations/<target-id>/workspace.md`：root manifest/toolchain、dependency graph 与最终 CI 门。
- `docs/dev/migrations/<target-id>/dependency-evidence.md`：共享依赖与上游证据。
- `docs/dev/migrations/<target-id>/skill-sync.md`：不属于任何 Cargo package 的 repo-local skill 同步。
- `app/<name>/docs/dev/migrations/<target-id>.md`：应用自己的迁移计划。
- `crates/<name>/docs/dev/migrations/<target-id>.md`：crate 自己的迁移计划。

Git dependency 的 `<target-id>` 固定为
`gpui-<gpui-target-sha前8位>-component-<gpui-component-target-sha前8位>`；完整 crate version 与
source/target SHA 写入文档状态区。若未来改用正式 release，则 ID 使用明确的 `v<version>`，不能只写
`latest`、`upgrade` 或其他会被复用的名字。

表格按创建日期倒序排列，并且只能有一项标记为“当前迁移”。新迁移必须新增 `<target-id>`，
不能修改旧批次来表示新的目标版本。
