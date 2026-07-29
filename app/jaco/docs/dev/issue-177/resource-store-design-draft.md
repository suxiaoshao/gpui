# Issue #177：剩余问题草稿

关联 issue：[suxiaoshao/gpui#177](https://github.com/suxiaoshao/gpui/issues/177)

> 下面的结论已经同步到[实施计划](README.md)，当前没有遗留的待确认问题。本轮暂时保留这些
> 内容供用户对照审阅；用户确认实施计划后再清理。当前不对实施计划做完整性或最终检查。

## 1. Global、Project 与 Agent Skill 的统一事实源

### 已确认

- 当前支持的来源优先级固定为
  `ProjectLocal > Global > BuiltIn`。`Global` 对应 `~/.agents/skills`，
  `ProjectLocal` 对应当前 project 下的 `.agents/skills`。
- Skill 不固定到某次 catalog revision/hash。每次实际运行都重新扫描来源、按上述优先级解析
  当前同名 Skill，并重新读取当前文件内容；AgentRuntime 不消费 UI 保存的 catalog snapshot。
- Skill catalog 不是 app-global Resource，不安装 `GlobalSkillStore`，也不提供
  `SelectGlobalSkillEntries`。文件系统扫描结果只是各使用者的局部展示状态，不需要跨页面、
  跨窗口同步。
- `SkillsSettingsPage` 在自身 Entity 中保存局部 Skill catalog Operation，负责 Loading、错误、
  保留旧数据只读展示和手动刷新。只有页面内部确实出现多个独立 owner 需要订阅时，才考虑由
  页面持有局部 Store；不能因此提升为 Global。
- ChatInput 为当前 project root 保存自己的局部 Skill catalog Operation，用于 Skill 补全和详情。
  它与 Settings snapshot 可以不同；Agent 开始运行时的重新扫描才是最终输入。
- 单个 Skill 正文读取继续由发起读取的页面或组件保存局部 Operation，不进入共享 Store。
- 同名 Skill 不让用户选择来源。跨来源时直接使用
  `ProjectLocal > Global > BuiltIn` 的最高优先级项；同一来源内按稳定路径顺序保留第一个并记录
  warning，不能依赖文件系统遍历顺序。
- Plugin Skill 的来源层级等真正接入 Plugin 时再设计，不属于 issue #177。

## 2. 外部文件变化与自动刷新

外部文件监听不属于 issue #177，已经拆分为
[suxiaoshao/gpui#178](https://github.com/suxiaoshao/gpui/issues/178)。

本轮只处理应用内写入后的状态发布和用户显式 Refresh/Reload，不增加 watcher。Store 与
Operation 不负责监听文件；#178 后续单独设计监听来源、事件合并和刷新失败语义。

## 3. `http_proxy` 的真实运行时边界

`http_proxy` 的运行时接入不属于 issue #177，已经拆分为
[suxiaoshao/gpui#179](https://github.com/suxiaoshao/gpui/issues/179)。

本轮只保留现有 Config 字段，不扩展 provider、MCP、OAuth 或其他网络 client 的代理行为。

## 4. Database backup 的恢复、导入与检查工具

issue #177 的 Database UI 只提供两个操作：

1. Refresh：重新检查或打开当前数据库，不移动、覆盖或修改数据库文件。
2. Backup and create new database：保留当前有问题的数据库文件，再创建一份通过完整初始化与校验的
   新数据库，并向用户展示 backup 位置。

本轮不提供 restore、import、部分数据恢复或独立 backup 检查工具，也不提前设计这些能力。

## 5. 通用 gpui-component 的动态只读契约

Jaco 本轮通过 app-local `PickerListDelegate` 实现只读浏览：

- picker 仍可打开、搜索、滚动和移动高亮项；
- mouse、Enter、clear 和多选 toggle 都不能提交变化；
- 不修改当前 selection，也不发送表示值已变化的 Change/Confirm event；
- picker 已打开时切换为只读，立即阻止后续提交，但不强制关闭；
- 恢复可编辑后重新允许正常选择。

通用能力已经提交到上游
[longbridge/gpui-component#2600](https://github.com/longbridge/gpui-component/issues/2600)。
issue #177 不修改外部 `gpui-component`；上游完成前由 Jaco 局部实现，上游完成后再评估能否删减。

## 6. GPUI 启动前的故障 UI

tracing 日志目录/文件在 GPUI application 启动前创建。该步骤失败说明用户目录不可用或文件权限
异常，本轮不提供窗口、临时日志目录、内存日志或重试流程，保持启动失败并返回进程错误。
