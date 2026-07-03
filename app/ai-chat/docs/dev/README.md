# ai-chat 开发文档索引

本目录保存 `app/ai-chat` legacy 迁移期和 `app/ai-chat2` 重构期的开发协调文档。文档按 GitHub issue 分组，避免所有专项计划堆在同一层目录。

## 入口

| 范围 | 文档 | 用途 |
| --- | --- | --- |
| #137 LLM 抽象集成 | `issue-137/README.md` | 跨 issue 状态、分支策略、架构决策和阶段记录。 |
| #155 fresh database 设计 | `issue-155/README.md` | `ai-chat2` crate/database 设计入口。 |
| #159 ai-chat2 UI/runtime | `issue-159/README.md` | UI、runtime、settings、tools、MCP 等专项计划索引和全量状态板。 |

## 组织规则

- 新的 #159 专项文档放到 `issue-159/` 下，文件名只保留主题名，例如 `prompt-settings.md`。
- 超过千行的专项文档优先放进子目录，并提供短 `README.md` 入口；完整细节放在 `plan.md` 或更具体的主题文件里。
- 跨 issue 只写短引用，详细设计留在对应 issue 目录中，避免 #137 协调文档继续膨胀。
- 旧路径不要再新增文档；如果需要引用历史路径，改为当前目录结构下的新路径。
