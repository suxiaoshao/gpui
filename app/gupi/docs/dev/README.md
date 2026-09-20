# Gupi development plans

**[总待处理文档：RPC/TUI 接入盘点、未完成项与能力边界](../../../../docs/dev/issue-217/follow-ups.md)**：跨阶段依赖阻塞、后续工作和验证边界的统一入口。

- [队列交互与输入框布局草稿](issue-222/queue-composer.md)：逐条返回草稿、编辑、删除；新会话项目选择器、累计用量及 Pi 接口限制与待确定项。
- [临时窗口、全局快捷键与托盘开发计划](issue-221/README.md)：多临时会话、图片/文件输入、模板快捷任务、手动清理及平台接入。
- [临时窗口 Jaco 对照与对齐方案](issue-221/temporary-window-comparison.md)：窗口配置、跨屏显隐、焦点与布局对照，以及窗口回收和会话保留的边界。
- [输入框组件与资源标签接入计划](issue-221/composer-resources.md)：InputGroup、原子内联标签的正式版本依赖，Skill/模板/附件行为、Jaco/Zed 编辑器对照与待确定项。
- [统一设置开发计划](issue-231/README.md)：分类导航、字段搜索、独立保存边界、快捷键与个人级插件、Skill、提示词管理；包含组件前置、数据契约和分步验证。
- [Pi RPC 能力缺口与社区调研](pi-rpc-gaps.md)：原生协议限制、可替代方案及上游公开进展。
- [会话目录读取优化](issue-229/README.md)：顺序字节读取、sonic-rs 按字段解析；保留现有加载、排序与搜索，含性能依据和实施步骤。
- [运行状态与过程展示](issue-229/runtime-display-plan.md)：工作计时、过程折叠、工具组摘要、技能图标统一与待确定边界；[Electron / Pi TUI 调研](issue-229/runtime-display-research.md)。

- [第一阶段：应用骨架、启动引导与恢复入口](issue-218/README.md)
- [第二阶段：Pi RPC 与进程生命周期](../../../../docs/dev/issue-219/README.md)：独立 crate、应用多实例管理、退出接入与集成验证计划。
- [第三阶段：会话页面功能与 UI/UX 草稿](issue-220/README.md)：功能要求与页面设计讨论；范围、职责、四个实现提交和必要验证见[开发计划](../../../../docs/dev/issue-220/README.md)。
- [会话历史](issue-220/history.md)：树/列表、三级内容、列表分支范围、过程折叠、视口与必要验证。
- [数据获取与加载状态](issue-220/data-loading.md)：目录扫描进度、自定义状态机及会话数据加载边界的设计与实施安排。
- [统一标题栏](issue-220/titlebar.md)：单行窗口顶部、侧栏对齐、会话菜单、当前 session 刷新与原生窗口行为。
- [主窗口快捷键与命令入口](issue-226/README.md)：动作路由、会话快速打开、统一命令面板的双入口与刷新重连；含完整 Pi 内置命令能力对照。
