# Gupi 未完成项与能力边界索引

归属：[#217](https://github.com/suxiaoshao/gpui/issues/217)。更新日期：2026-09-18。

本页集中查看各阶段留下的依赖阻塞、后续工作和未验证边界。详细设计、源码依据及验证仍归原文档；已有独立文件只链接，不在这里重写方案。表中的“待确定范围”不代表已经授权实现，“未验证”也不等于已发现缺陷。

## 等待上游或依赖更新

| 项目 | 当前边界与恢复条件 | 归属与详细记录 |
| --- | --- | --- |
| 设置字段搜索 | 已接入搜索入口和关键词；当前正式依赖 `gpui-kit/gpui-component = 0.6.0` 存在过滤后页面/分组索引错位，可能显示空白或错误页面。等待包含上游搜索修复的兼容正式版本，再升级并复测非首页搜索、当前页匹配/不匹配、无结果和清空搜索；不能仅因版本号提高就标记完成 | [#231 设置计划](../../../app/gupi/docs/dev/issue-231/README.md)、[本地修复补丁](../../../app/gupi/docs/dev/issue-231/gpui-component-settings.patch)、[上游修复 #3104](https://github.com/longbridge/gpui-kit/pull/3104) |
| 输入框组件与资源交互（整体暂缓） | 按用户 2026-09-18 决定，等待 **InputGroup 与原子内联标签两项能力均进入兼容的 gpui-kit 正式版本**，再升级依赖并接入。InputGroup 替换共用 Composer 的本地外壳；内联标签承接 Skill/模板的显示、点击和整体编辑。Skill 选择后填入正文、模板附带文件引用处理及可选 `@` 文件入口一并暂缓；不提前使用 Git 依赖、单独推进 Skill 行为或自建编辑器。恢复时核对发布 API，并确认原文中的待定行为和实施范围。现有输入、文件与图片附件功能继续保留 | [InputGroup #3042](https://github.com/longbridge/gpui-kit/pull/3042)、[内联标签 #3113](https://github.com/longbridge/gpui-kit/pull/3113)（对应 [#3110](https://github.com/longbridge/gpui-kit/issues/3110)）、[输入框接入计划](../../../app/gupi/docs/dev/issue-221/composer-resources.md) |
| question 组件与扩展 UI 完整体验 | 按用户决定暂停，等待上游 question 组件后再继续接入与调整。恢复时先核对发布版本和实际 API，再按原计划测试标准交互、取消/超时、连续请求和会话归属；组件能力不等于 Pi 增加了组合问卷协议 | [#222 扩展 UI 与体验环境](../../../app/gupi/docs/dev/issue-222/README.md) |
| Pi RPC 能力缺口 | 插件参数/子命令补全、自定义快捷键、任意 TUI UI/渲染、组合问卷、输入回读、显示控制等受协议限制；同文件树节点续聊和进程内 reload 也缺直接 RPC。逐项依据、现有降级、社区方案及调查时点统一见原文；后续恢复时重新核对上游，不自动采用私有桥接或社区 fork | [RPC 能力缺口与社区调研](../../../app/gupi/docs/dev/pi-rpc-gaps.md)；[树导航边界](../../../app/gupi/docs/dev/issue-220/history.md)、[刷新重连](../../../app/gupi/docs/dev/issue-226/reconnect.md) |

## 应用侧后续工作与待定范围

| 项目 | 当前已有内容与剩余范围 | 归属与详细记录 |
| --- | --- | --- |
| 完整队列交互 | 已有 steer、follow-up 提交和排队数量提示；完整队列查看/编辑、恢复到输入框以及停止与清队列的组合交互留待本阶段确定。Pi 有相关接口，不归为上游协议阻塞；不恢复已经删除的失败输入恢复队列 | [#222](https://github.com/suxiaoshao/gpui/issues/222)、[RPC 取消契约](../issue-219/README.md)、[快捷键范围调研](../../../app/gupi/docs/dev/issue-226/README.md) |
| Cmd/Ctrl+F 当前分支正文查找 | 尚未实现、尚未单独建 Issue。范围、高亮、折叠展开、结果定位与流式更新规则待独立讨论；不与现有会话目录搜索或跨会话全文索引混为一项 | [正文查找范围草案](../../../app/gupi/docs/dev/issue-226/README.md#独立于-226-的正文查找) |
| 额外 Pi 内置命令能力 | 统一会话信息页、模型范围管理、JSONL 导入/导出等仍需选择范围。快捷键设置已经可查看/修改/恢复，不能再列成缺失功能；`hotkeys` 的命令搜索别名尚未接入。分享、认证、信任管理等仅保留原有边界，不自动列为必做任务 | [完整内置命令对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)、[D-14 范围决定](../../../app/gupi/docs/dev/issue-226/decisions.md#待确定的产品范围) |
| 项目级设置 | #231 只做个人级设置。若后续接入项目级设置，应从 session 中独立对话框进入，具体范围尚未设计；不在全局设置中添加项目选择器 | [设置范围与生效边界](../../../app/gupi/docs/dev/issue-231/README.md#已确认方向) |

## 发行与验证边界

| 项目 | 已有证据与尚未覆盖部分 | 归属与详细记录 |
| --- | --- | --- |
| 临时窗口与全局快捷任务原生验收 | #221 当前已确认的应用侧实现已完成，输入框后续改造见上方依赖项。macOS 窗口启动、会话隔离、操作面板、停止/隐藏切换及快捷键设置已验证；真实系统热键、外部应用取词与自动粘贴成功路径、权限允许/拒绝、托盘、原生附件入口及跨屏体验仍未完整验证，Windows 尚无实机结果。保留这些验收边界，不再把已实现的面板、双栏布局与 600 秒回收列为缺失功能 | [#221 验证记录](../../../app/gupi/docs/dev/issue-221/README.md#验证记录) |
| 完整发行与性能验收 | macOS 打包、签名及重点原生功能已验证；Windows/Linux 实机体验、Windows Pi 启动脚本行为，以及长对话、冷启动、多实例占用的完整发行验收仍由发行阶段承接。既有 CI/单元测试不能替代这些结果 | [#223](https://github.com/suxiaoshao/gpui/issues/223)、[第一阶段平台边界](../issue-218/README.md)、[运行入口](../../../app/gupi/docs/dev/issue-218/README.md) |
| 开发期 Logo | 现有记录为临时使用 Pi 官方 logo，授权尚未确认。收到回复或准备发布时确定最终方案；不将开发期选择当作正式授权 | [D-04 Logo 决定](../issue-218/README.md#d-04开发期-logo)、[#223](https://github.com/suxiaoshao/gpui/issues/223) |
| 旧阶段原生验证缺口 | 原文仍记录 IME、真实插件交互、真实模型压缩等未覆盖场景，以及早期无 Dock 图标/不可点击的启动反馈。当前打包启动成功不证明早期反馈根因已定位；按具体复现或所属阶段处理，不把每条历史“未验证”都升级成新的阻塞任务 | [命令入口验证记录](../../../app/gupi/docs/dev/issue-226/validation.md#未验证与后续问题)、[扩展 UI 验收](../../../app/gupi/docs/dev/issue-222/README.md) |

2026-09-16 的设置补测：关闭 macOS 台前调度后，本轮不再出现 ScreenCaptureKit `-3811/-3812`；模板启停、编辑放弃、快捷键清除/取消/确认和全部恢复默认已通过控件状态与磁盘结果核对。截图仍有刷新滞后。这是测试工具的捕获边界，不列为 Gupi 待修复功能，也不推断所有 macOS 27 捕获问题都已解决。

## 已有结论，不重新列为未完成项

- 目录读取已完成顺序字节读取与 sonic-rs 按字段解析，用户选择保留原有搜索。分批加载、头尾并发、Rayon/Tokio fs 迁移、全文索引和数据库不是遗留实施任务。见[读取优化方案](../../../app/gupi/docs/dev/issue-229/README.md)；[前期调研](../../../app/gupi/docs/dev/session-catalog-scan-draft.md)仅作依据。
- 手动压缩、HTML 导出、复制会话、消息 fork、会话删除，以及个人级插件/Skill/提示词管理已有实现。不能因早期 RPC 调研或旧命令表写过“缺管理接口”而重复列为缺失。见[命令能力对照](../../../app/gupi/docs/dev/issue-226/builtin-commands.md)、[设置计划](../../../app/gupi/docs/dev/issue-231/README.md)。
- 运行计时、流式 Markdown、工具详情与卡片组织已按后续反馈实施；早期 loading/消息展示反馈已有专门工作承接，不继续保留为“尚待描述”的空白问题。见[运行展示](../../../app/gupi/docs/dev/issue-229/runtime-display-plan.md)、[工具详情](../../../app/gupi/docs/dev/issue-229/tool-details-research.md)、[卡片组织](../../../app/gupi/docs/dev/issue-229/tool-card-layout-research.md)。新问题需要具体复现。

## 维护方式

后续发现已有功能受阻、暂停或尚未覆盖时，在本页增加简短入口，并把详细说明写回所属文档。解决后同步原文并从待办表移除；不通过 Issue 是否关闭、PR 是否合并推断功能完成，不在此维护提交和 PR 流水账。
