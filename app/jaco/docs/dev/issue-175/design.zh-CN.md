# Jaco 聊天输入与快捷键架构设计

[English](design.md) | [简体中文](design.zh-CN.md)

## 1. 目标

本设计让普通聊天、新对话、临时会话和快捷键编辑拥有一致的输入体验，同时不强迫它们共享自己不需要的业务状态。

快捷键创建的会话在运行时也必须表现为临时会话，而不是出现在普通应用窗口中。

## 2. 展示架构

### `ChatForm`

`ChatForm` 是纯展示组件。它拥有布局、样式和局部视觉组合，但不拥有：

- `gpui-form` session 或应用输入模型；
- provider/model/project catalog；
- 持久化或数据库访问；
- 快捷键、Conversation 或 AgentRun 业务逻辑。

调用方为需要展示的控件传入组件 state 与事件处理器。这样可以保持统一视觉体验，同时让每个页面继续拥有正确的数据。

### `ControlSlot`

每个可选控件统一表示为三种状态：

- `Hidden`：不参与布局和交互；
- `Disabled`：保持统一外观，但拒绝用户交互；
- `Enabled`：展示并允许交互。

这个抽象同时管理可见性、disabled 行为、是否可聚焦和布局。各页面不再为同一控件维护多组互相组合的 bool。

## 3. 页面组合

| 页面 | Project | Composer/attachments | Run settings | Primary action |
| --- | --- | --- | --- | --- |
| 新对话 | Enabled | Enabled | Enabled | Enabled |
| 已有对话 | Hidden | Enabled | Enabled | Enabled |
| 临时新对话 | Hidden | Enabled | Enabled | Enabled |
| 快捷键编辑 | Hidden | Disabled | Enabled | Disabled |

快捷键编辑器复用聊天输入的视觉 shell 和运行设置控件，但只编辑属于快捷键的配置。Disabled 控件可以展示上下文，但键盘、粘贴、IME 和 action handler 都不能修改它。

## 4. 状态所有权

| 状态 | 所有者 |
| --- | --- |
| Composer 文本 | generated parent form store |
| Composer 交互 | composer bound control |
| 附件 | generated parent form store |
| 附件流程 | chat-input controller |
| model、reasoning effort、token budget、approval/tool access | generated parent form store |
| 运行设置交互与 options | owning bound control 与 run-settings controller |
| Project 选择 | new-conversation form store 与 bound control |
| options/catalog/capability | 应用 `gpui-store` store/controller |
| 当前类型化值、baseline、验证与 submit runtime | 每个编辑页面的一个 generated parent form store |
| 焦点与错误可见性 | 当前可见页面/dialog 和具体组件实例 |
| Conversation、AgentRun 与持久化 | 应用 service/store |

这里没有平行的 component-owned 业务值或 String draft。owning bound control 与 generated store 双向同步类型化值，本地只保存交互与配置状态。

## 5. 运行设置

generated parent form 拥有模型执行所需的类型化值：

- model selection；
- reasoning effort；
- 精确整数 token budget；
- approval/tool-access mode。

`RunSettingsController` 拥有 bound controls，并把它们连接到 parent form 的嵌套字段；它不会创建第二个 nested form store。

model options 与 capability 始终是配置。刷新它们不能静默选择另一个模型，也不能改写当前选择。已选择模型不可用或不兼容时，验证应返回明确错误。

## 6. 验证、焦点与提交

组件事件发生后，bound control 把类型化值写入 `FormField`，generated validation 再读取已经更新的 parent model。

提交时，`prepare_submit` 只 clone 一次 parent form value，并用同一份值完成：

- 验证；
- model/capability/attachment 兼容性检查；
- 转换为应用命令；
- 持久化或创建 run。

提交路径不会重新读取数据库或 catalog 来选择 fallback model。选择缺失、被禁用或已失效时应直接报错。

form store 返回错误路径，但不直接聚焦 UI。当前页面/dialog 把路径映射到可见 bound control。即使同一数据由多个组件实例展示，这个边界仍然正确。

字段渲染从 generated schema 读取静态 `required` 元数据，从 form 读取数据错误与 pending 状态。Jaco 负责翻译 error message key，bound control 再结合自身交互状态决定是否展示。字段级异步验证可以通过 `is_validating_at(path)` 驱动 input spinner，但 spinner 和 focus 状态不会因此进入 form。

## 7. Catalog 与持久化边界

provider、model、project 和 capability catalog 是由 `gpui-store` store/controller 拥有的已提交应用状态。页面读取这些 store 的类型化快照。

普通 UI 路径不直接查询数据库获得 catalog。表单验证可以接收调用方捕获的 catalog snapshot 作为上下文，但不能修改 catalog 或替换当前选择。

持久化成功后，应用 service/store 返回结果，编辑页面才用实际保存的类型化值 rebase generated form store。

## 8. 临时窗口运行时

全局临时会话快捷键，以及配置为创建会话的自定义快捷键，都进入同一个 popup `TemporaryWindow` 运行时。

该运行时拥有 popup 可见性、临时会话列表、当前 route 和焦点恢复。hotkey 代码只请求动作，不 downcast 窗口内部，也不 fallback 到普通主窗口。

会话列表的键盘与鼠标交互都通过临时窗口 controller 请求选择。delegate callback 不同步 update 自己的 `ListState`；route/focus 变化在当前 delegate update 结束后由 owner 应用。

## 9. GPUI 生命周期不变量

- callback 不同步 update 当前正在被 update 的同一 entity；
- picker/list delegate 只发出意图，owner 负责跨 entity 状态变化；
- deferred work 捕获 weak entity，并在执行时重新检查存活；
- popup 的打开和关闭不会永久抢走 search/composer 应获得的焦点；
- 页面级 subscription 由页面/controller 持有，组件内部 subscription 由组件持有。

这些规则属于架构本身，因为违反它们会触发 `already being updated` 或 `RefCell already borrowed`。

## 10. 非目标

本设计不重构 prompt 系统、provider adapter、agent 执行协议、MCP 逐工具权限、数据库 schema 或视觉主题，也不在 Jaco 和 `gpui-form` 之间增加新的通用 binding framework。

## 11. 最终不变量

- 所有聊天类页面通过 `ChatForm` 与 `ControlSlot` 共享同一视觉语言；
- 每个页面只拥有自己可以编辑的值；
- 当前值、配置、验证 runtime 和持久化各有独立所有者；
- 一个编辑页面只有一个顶层 form session；
- catalog 刷新不静默改变用户选择；
- 通过验证的值就是实际提交的值；
- 快捷键创建的会话始终使用 popup 临时窗口生命周期；
- GPUI event handler 不对当前 active entity 执行嵌套 update。
