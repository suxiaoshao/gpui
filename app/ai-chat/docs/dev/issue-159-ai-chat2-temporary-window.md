# Issue #159 ai-chat2 Temporary Window 专项计划

本文档是 `app/ai-chat2` 临时对话窗口的实施计划。父级 UI 清单仍是
`app/ai-chat/docs/dev/issue-159-ai-chat2-ui.md`；本文档只固定 Temporary Conversation
Window 的真实页面、无项目对话列表、搜索、右侧 conversation detail / new conversation、键盘
focus 和数据流。

创建时间：2026-06-11。

当前状态：首版已实现。`app/ai-chat2/src/app/temporary_window.rs` 已替换菜单和 temporary hotkey 的
placeholder 打开路径，`features/temporary.rs` 已接顶部单行搜索、左侧 no-project conversation 列表、
右侧 new/detail、键盘 focus 和真实 `AgentRuntime` run。旧
`app/ai-chat2/src/app/placeholder_windows.rs` 已从模块树摘除，后续可整体删除旧占位文件。本轮不修改旧
`app/ai-chat` legacy temporary chat，也不引入纯内存 ephemeral chat。

实现时间：2026-06-11。

## 旧版外壳行为迁移状态

Temporary Window 首版之后已继续迁移旧 `app/ai-chat` 的临时工具外壳行为。本轮只迁移窗口
open/toggle/lifecycle 和 temporary hotkey 设置一致性；不包含 tray、shortcut execution、selected
text/screenshot input 或 save/promote flow。

- Temporary hotkey 语义已迁移：global temporary hotkey 恢复 toggle，窗口可见时隐藏，不可见时显示或创建；
  App menu 入口仍保持 open/reveal，不负责隐藏窗口。
- 窗口外壳已迁移：Temporary Window 使用 `WindowKind::PopUp`、透明或弱化 titlebar、不可 resize、按鼠标所在
  display 定位或移动；已有窗口 reveal 时也会重新定位。窗口失去 activation 时会隐藏并进入 600 秒延迟 remove；
  用户在其他应用或其他窗口继续工作时不会保留可见临时窗口。默认尺寸保留 `app/ai-chat2` 的 `960x620`，不复刻旧
  `app/ai-chat` 的 `800x600`，以适配当前左右分栏内容。
- IME 候选窗层级修复已完成：旧 `app/ai-chat` 和新 `app/ai-chat2` temporary window 都使用 GPUI
  `WindowKind::PopUp` 时，会被 macOS backend 映射到 `NSPopUpWindowLevel = 101`；这个层级明显高于
  Raycast/uTools 这类 launcher 搜索窗，并会干扰输入法候选窗显示。2026-06-12 本轮保留
  `WindowKind::PopUp` 的 popup lifecycle / nonactivating panel / all-spaces 行为，但通过
  `window-ext::WindowExt::set_window_level(WindowLevel::ModalPanel)` 把实际 window level 覆盖到
  `NSModalPanelWindowLevel = 8`。本机参考：Raycast 搜索窗实测 layer 8；uTools launcher 实测 layer 9
  （Electron `modal-panel + 1`）。当前选择 exact Raycast layer 8；如果后续 IME 仍异常，再单独评估
  `ModalPanel + 1`。
- Temporary hotkey 设置保存一致性已修复：Settings 保存时先 parse/register runtime，成功后才写 fresh DB
  app settings；注册失败不关闭 dialog、不写 DB，DB 保存失败时尝试把 runtime 回滚到 previous hotkey。

## 目标行为

- 打开 Temporary Conversation 后显示一个独立窗口：
  - 顶部是一行搜索输入框。
  - 下方左右分栏：左侧是历史无项目对话列表，右侧是新对话或已选对话详情。
- 左侧只列出 fresh DB 中 visible scratch project 下的 active conversations，也就是
  `ProjectKind::Scratch` 对应的 no-project conversations。
- 右侧状态由临时窗口自己的 route 决定：
  - `NewConversation`：显示无项目新对话输入区。
  - `Conversation(id)`：显示对应 conversation detail timeline 和底部输入区。
- 临时窗口中的选中项、route 和搜索不写入主窗口 `AiChat2WorkspaceStore.route`，避免切换临时窗口
  对话时联动改变主窗口右侧页面。
- `secondary-n` 在临时窗口内切到新对话并 focus 右侧输入框。
- 搜索框 focus 时：
  - `up` / `down` 切换左侧列表选中项，并立即更新右侧 detail。
  - `tab` 直接 focus 右侧当前 composer，不进入左侧列表。
  - `enter` 打开当前选中 conversation 的右侧 detail；如果选中项已经打开，只保持当前状态。
- 新对话发送后必须创建 fresh conversation、首条 user item 和 anonymous scratch project，刷新左侧列表，
  选中新 conversation，右侧切到 conversation detail，并启动真实 `AgentRuntime` run。

## 非目标

- 不实现 retry/resend UI。
- 不实现 approval approve/deny action。
- 不实现 attachments/multimodal input。
- 不实现 rich tool UI。
- 不实现 left-list pin/delete/context menu。
- 不实现 prompt selector 或 shortcut execution。
- 不做 legacy `app/ai-chat` 数据迁移。
- 不新增全文搜索索引或 FTS migration。
- 不把临时对话做成纯内存 ephemeral chat；首版仍持久化到 fresh DB 的 scratch/no-project conversations。

## 共享模块边界

Temporary Window 落地前必须先修正现有 Home-only ownership：`ChatForm`、composer editor、
provider/model picker、reasoning selector、conversation detail/timeline 和纯格式化函数不应继续由
`features::home` 独占。Temporary 不能通过 `features::home::{chat_form, conversation}` 横向复用 Home
内部模块，否则后续 Settings、Shortcut、Prompt 或其他窗口也会被迫依赖 Home。

新的依赖方向：

- `features/home` 只保留主窗口 shell、sidebar、Home 默认新对话页、Home route glue 和 Home-only actions。
- `features/temporary` 只保留 temporary window search/list/right-route/new-conversation glue。
- `components` 放可复用 UI entity/view：`ChatForm`、`ComposerEditor`、picker popover/list delegate、
  conversation detail/timeline/message rows。
- `foundation` 放无 UI 状态的基础函数：conversation timestamp、run time、item 文案/format helper 等纯函数。
- `state` 放 app-level 查询、snapshot、runtime/cache helper；feature 之间不能互相调用对方的 state glue。

实现约束：

- `features/temporary` 不允许 import `crate::features::home::chat_form` 或
  `crate::features::home::conversation`。
- `features/home` 和 `features/temporary` 都从 `crate::components::*`、
  `crate::foundation::*`、`crate::state::*` 取共享能力。
- 共享组件抽出后先保持 Home 行为等价，再在 Temporary 中消费；不要一边接 Temporary 一边复制 Home 组件。
- 现有 `pub(in crate::features::home)` 可见性要随移动收窄到新模块边界，例如
  `pub(in crate::components::chat_form)` 或 `pub(super)`；只有 Home/Temporary 都需要直接调用的 API 才提升到
  `pub(crate)`。

## 模块结构

禁止新增 `mod.rs`。新增模块使用同名入口文件和子目录文件。

```text
app/ai-chat2/src/app/
├── temporary_window.rs          # 真实临时窗口 open/reveal/window shell/root 入口
└── placeholder_windows.rs       # 删除 Temporary 分支后只保留仍需的 placeholder，或后续整体移除

app/ai-chat2/src/components.rs   # 增加 chat_form / conversation_detail / picker 模块声明
app/ai-chat2/src/components/
├── chat_form.rs                 # 从 features/home/chat_form.rs 移出
├── chat_form/
│   ├── composer_editor.rs       # 从 features/home/chat_form/composer_editor.rs 移出
│   ├── composer_editor/
│   │   ├── blink_cursor.rs
│   │   ├── buffer.rs
│   │   ├── element.rs
│   │   ├── history.rs
│   │   ├── snapshot.rs
│   │   └── token.rs
│   ├── effort_select.rs
│   ├── model_select.rs
│   └── thinking_effort.rs
├── conversation_detail.rs       # 从 features/home/conversation.rs 移出
├── conversation_detail/
│   ├── message.rs
│   └── timeline.rs
└── picker.rs                    # 从 chat_form/picker.rs 移出，供 ChatForm 和 Home project picker 共享

app/ai-chat2/src/foundation.rs   # 增加 conversation_format 模块声明
app/ai-chat2/src/foundation/
└── conversation_format.rs       # 从 features/home/conversation/format.rs 移出纯格式化函数

app/ai-chat2/src/features/
├── home.rs                      # 移除 chat_form / conversation 模块声明，改用 components
├── home/
│   ├── actions.rs
│   ├── new_conversation.rs      # Home-only default page，使用 components::chat_form / components::picker
│   ├── shell.rs
│   └── sidebar.rs
├── temporary.rs                 # TemporaryWindow view 入口
└── temporary/
    ├── list.rs                  # 左侧 no-project conversation list delegate/row
    └── new_conversation.rs      # 无项目 new conversation pane，使用 components::chat_form::ChatForm

app/ai-chat2/src/state/
└── temporary.rs                 # no-project conversation query/filter helper 和 snapshot 类型
```

需要同步调整：

- `app/ai-chat2/src/app.rs`
  - `pub(crate) mod temporary_window;`
  - `menus::OpenTemporaryConversation` 调用 `temporary_window::open_temporary_window(cx)`。
- `app/ai-chat2/src/components.rs`
  - `pub(crate) mod chat_form;`
  - `pub(crate) mod conversation_detail;`
  - `pub(crate) mod picker;`
- `app/ai-chat2/src/foundation.rs`
  - `pub(crate) mod conversation_format;`
- `app/ai-chat2/src/features/home.rs`
  - 删除 `chat_form` / `conversation` 模块声明。
  - `home::init(cx)` 改为调用 `components::chat_form::init(cx)` 或由 `components::init(cx)` 统一注册
    composer key context。
  - `new_conversation.rs`、`shell.rs` 改为 import `components::{chat_form, conversation_detail, picker}`。
- `app/ai-chat2/src/features.rs`
  - `pub(crate) mod temporary;`
  - 如果临时窗口需要 app-level key bindings，则在 `features::init(cx)` 中调用 `temporary::init(cx)`。
- `app/ai-chat2/src/state.rs`
  - `pub(crate) mod temporary;`
- `crates/ai-chat-db/src/repository.rs`
  - 新增 no-project conversation 查询 API，供 `state::temporary` 调用。

## 数据结构

`state/temporary.rs` 负责把 repository records 转成窗口可消费的数据形状：

```rust
pub(crate) type TemporaryConversationNode = state::workspace::SidebarConversationNode;

pub(crate) struct TemporaryConversationSnapshot {
    pub(crate) conversations: Vec<TemporaryConversationNode>,
}
```

如果左侧需要额外展示摘要或 provider 状态，再扩展 `TemporaryConversationNode`，不要把
`ConversationRecord` 直接暴露给 UI。

`features/temporary.rs` 定义窗口本地 route/state：

```rust
enum TemporaryWindowRoute {
    NewConversation,
    Conversation(ConversationId),
}

struct TemporaryWindow {
    focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    list: Entity<ListState<TemporaryConversationListDelegate>>,
    query: String,
    route: TemporaryWindowRoute,
    conversations: Vec<TemporaryConversationNode>,
    selected_index: Option<usize>,
    new_conversation: Entity<TemporaryNewConversationPane>,
    conversation_pages: HashMap<ConversationId, Entity<ConversationDetailPage>>,
    runtime: Entity<ConversationRuntimeStore>,
    _subscriptions: Vec<Subscription>,
}
```

状态约束：

- `query` 是搜索框原始值的 trimmed copy；`InputState` 是文本输入 source of truth。
- `conversations` 是当前 query 下的排序结果。
- `selected_index` 必须始终是 `None` 或小于 `conversations.len()`。
- `route` 和 `selected_index` 可以短暂不一致：用户按 `secondary-n` 时 route 是 `NewConversation`，
  列表 selection 可保留，用于返回历史对话时恢复视觉上下文。
- `conversation_pages` 只缓存当前临时窗口创建过的 `ConversationDetailPage` entity，不写入全局 workspace route。

## 数据获取

新增 repository API：

```rust
impl FreshRepository {
    pub fn list_no_project_conversations(
        &self,
        query: &str,
    ) -> Result<Vec<ConversationRecord>>;
}
```

查询语义：

- 只返回 `conversations.status = Active`。
- `conversation.project_id` 必须属于 `projects.removed = false` 且 `projects.kind = 'scratch'` 的项目。
- 空 query 返回所有匹配 conversation，按 `conversations.updated_at DESC` 排序。
- 非空 query 使用和 sidebar search 一致的匹配语义：
  - conversation title。
  - `conversation_items.search_text`。
- 不匹配 normal project 的 display name 或 path，因为 temporary window 的列表范围已经固定为 no-project。
- 不返回 deleted conversation、removed project conversation 或 normal project conversation。

`state::temporary` 再封装 app-level API：

```rust
pub(crate) fn load_no_project_conversations(
    query: &str,
    cx: &App,
) -> ai_chat_db::Result<TemporaryConversationSnapshot>;
```

转换规则：

- `ConversationRecord.id` -> `TemporaryConversationNode.id`
- `ConversationRecord.project_id` -> `TemporaryConversationNode.project_id`
- `ConversationRecord.title` -> `SharedString`
- `ConversationRecord.updated_at.unix_timestamp_nanos()` -> `updated_at`
- `ConversationRecord.pinned` 保留给未来排序/视觉，但首版不提供 pin action。

新对话发送继续复用现有 `state::conversations::create_conversation`：

```rust
CreateConversationRequest {
    project_id: None,
    content_parts: submit.composer.content_parts.clone(),
    title_seed: submit.composer.text.clone(),
    skill_requests: submit.composer.skill_requests.clone(),
    provider_model: submit.provider_model,
    reasoning_selection: submit.reasoning_selection,
}
```

`project_id: None` 是必须项，它会走 `projects::create_anonymous_scratch_project(cx)`，创建
`ProjectKind::Scratch` project，并让该 conversation 后续出现在 no-project 列表。

## 数据流

### 打开窗口

```text
menu item / global hotkey
  -> OpenTemporaryConversation
  -> app::temporary_window::open_temporary_window(cx)
  -> find existing TemporaryWindow Root or cx.open_window(...)
  -> TemporaryWindow::new(window, cx)
  -> state::temporary::load_no_project_conversations("", cx)
  -> build ListState delegate
  -> route = first conversation if any, otherwise NewConversation
  -> defer focus search_input
```

说明：

- menu action 已存在，替换其 handler 即可。
- global hotkey runtime 当前只记录 diagnostics；首版真实行为应在 hotkey press handler 能 dispatch action 后复用
  `OpenTemporaryConversation`，不要另写一条打开路径。
- 打开已有 temporary window 时只 show/activate，并 focus 搜索框；不要重建窗口和丢失当前 route。

### 搜索

```text
InputEvent::Change
  -> query = search_input.value().trim()
  -> state::temporary::load_no_project_conversations(query, cx)
  -> delegate.set_items(...)
  -> selected_index = first item if any
  -> route = Conversation(first.id) if any, otherwise keep current route or NewConversation
  -> cx.notify()
```

错误处理：

- 查询失败时保留上一批列表，记录 `last_error` 并在左侧列表区域显示 localized error row。
- 搜索失败不应清空右侧当前 conversation，避免用户在输入时丢失上下文。

### 左侧选择

```text
search focused + up/down
  -> move_selected(delta)
  -> selected_index wraps around when list is non-empty
  -> route = Conversation(conversations[selected_index].id)
  -> ensure ConversationDetailPage entity
  -> cx.notify()
```

鼠标点击行也走同一条 `select_conversation(index)` 路径。

### Tab 到右侧输入框

```text
search focused + tab
  -> match route
     NewConversation => TemporaryNewConversationPane::focus_primary(window, cx)
     Conversation(id) => ConversationDetailPage::focus_primary(window, cx)
  -> stop propagation
```

`components::conversation_detail::ConversationDetailPage` 需要暴露 focus 方法：

```rust
pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.chat_form.update(cx, |chat_form, cx| chat_form.focus_composer(window, cx));
}
```

`NewConversationPage` 已有同名能力，但 temporary window 不应复用它，因为它会读取/写入 default project。

### `secondary-n`

```text
TemporaryWindow key context + secondary-n
  -> route = NewConversation
  -> new_conversation.focus_primary(window, cx)
  -> cx.notify()
```

不清空搜索文本，不清空左侧 selection。

### 新对话发送

```text
TemporaryNewConversationPane components::chat_form::ChatFormEvent::SendRequested
  -> CreateConversationRequest { project_id: None, ... }
  -> state::conversations::create_conversation(...)
  -> chat_form.clear_after_submit(cx)
  -> parent TemporaryWindow reloads no-project list with current query
  -> parent selects/opens created conversation id even if current query does not match
  -> ConversationRuntimeStore::start_run(created.run_request, window, cx)
```

如果当前 query 不匹配新 conversation title/content，仍临时把新 conversation 插入左侧结果顶部或清空 query 后刷新。
推荐默认清空 query，因为用户刚创建的 conversation 应该可见且成为当前上下文。

## UI 组件和布局

### 窗口根

- 使用 `Root::new(view, window, cx)`，和主窗口/placeholder 保持一致。
- 不渲染内部 `TitleBar::new()` titlebar row；临时窗口内容区域从顶部搜索框开始，非 macOS
  component menu bar 不在该窗口内部单独占一行。OS/window title 仍保留为系统窗口标识。
- 使用 `Root::render_sheet_layer`、`Root::render_dialog_layer`、`Root::render_notification_layer`，保证
  provider picker、notification 和后续 dialog 能在临时窗口内正常显示。

### 顶部搜索

- 组件：`gpui_component::input::{Input, InputState}`。
- icon：`IconName::Search` 作为 prefix。
- 样式：
  - 单行，高度固定在顶部 toolbar 内。
  - `cleanable(true)`。
  - `appearance(false)` 可用于嵌入自定义 toolbar 容器。
  - 不能使用 `ComposerEditor` 或多行输入。

### 左侧列表

- 组件：`gpui_component::list::{List, ListState, ListDelegate}`。
- 空态 icon：`IconName::SquarePen`。
- row 内容：
  - title 单行 truncate。
  - 不显示 conversation icon；列表范围固定是 conversation，不需要重复视觉提示。
  - 不显示 updated time；左栏保持更轻的标题列表。
  - 当前选中行使用 theme accent/selected 背景。
- 不显示 pin/delete hover action；这些是后续范围。

### 右侧详情

- `TemporaryWindowRoute::Conversation(id)` 使用
  `components::conversation_detail::ConversationDetailPage`，由 Home 和 Temporary 共同消费。
- `TemporaryWindowRoute::NewConversation` 使用 `TemporaryNewConversationPane`：
  - 内部持有 `Entity<components::chat_form::ChatForm>`。
  - 不渲染项目选择器。
  - skill catalog 使用 `None`，因为 no-project conversation 没有真实用户项目根目录。
  - 发送成功后通过 event/callback 通知 `TemporaryWindow` 切到新 conversation。

### 分栏

推荐使用 `gpui_component::resizable::{h_resizable, resizable_panel}`，与 Home shell 对齐：

- 左栏默认宽度：`280px`。
- 左栏范围：`220px..420px`。
- 右栏 `min_w_0()`、`flex_1()`。
- 如果临时窗口尺寸较小，左栏不折叠；先保持 desktop fixed split，后续再定义 responsive 行为。

## Icons

- 搜索输入 prefix：`IconName::Search`。
- 新对话按钮/空态：`IconName::SquarePen`。
- 加载失败/错误空态：`IconName::CircleAlert`，如果当前 app-local `IconName` 没有该图标，则先补 app-local
  Lucide enum/asset 声明；不要手写 SVG。
- 不新增 provider brand icon。

## 键盘和 focus

临时窗口使用独立 key context：

```rust
pub(crate) const KEY_CONTEXT: &str = "AiChat2TemporaryWindow";
```

Action 定义：

```rust
actions!(ai_chat2_temporary, [OpenTemporaryNewConversation, FocusTemporarySearch]);
```

Key bindings：

- `secondary-n` -> `OpenTemporaryNewConversation`
- 可选 `secondary-f` -> `FocusTemporarySearch`，如果实现则只在临时窗口 context 内 focus 顶部搜索框，不打开全局 search dialog。

搜索输入 focused 时处理：

- `MoveUp` / `MoveDown`：来自 `gpui_component::input` action，移动列表 selection，并 `cx.stop_propagation()`。
- `Tab`：使用 key binding 映射到 app-local action `ToggleTemporaryInputFocus`；搜索 input focused 时 focus
  右侧当前 composer，右侧 composer focused 时回到搜索 input。
- `Enter`：confirm 当前 selection；右侧已同步时仍 stop propagation，避免触发 composer send。

Focus 规则：

- 创建窗口后 defer focus 到 `search_input`。
- reveal 已有窗口后 focus 到 `search_input`。
- 窗口 activation 恢复时 focus 到 `search_input`；窗口失去 activation 时调用 temporary lifecycle 的 delayed
  hide/remove，不在 `features/temporary` 内直接持有关闭状态。
- `secondary-n` 后 focus 到右侧 new conversation `components::chat_form::ChatForm` composer。
- `tab` 在搜索 input 和右侧当前 route composer 之间切换 focus。
- 选择历史 conversation 后不自动把 focus 从搜索框拿走；用户可以继续用上下键浏览。

## i18n

新增 Fluent keys：

```text
temporary-window-title
temporary-search-placeholder
temporary-empty-conversations
temporary-no-results
temporary-load-failed
```

英文建议：

```text
temporary-window-title = Temporary Conversation
temporary-search-placeholder = Search conversations
temporary-empty-conversations = No temporary conversations
temporary-no-results = No matching conversations
temporary-load-failed = Load temporary conversations failed
```

中文建议：

```text
temporary-window-title = 临时对话
temporary-search-placeholder = 搜索临时对话
temporary-empty-conversations = 暂无临时对话
temporary-no-results = 没有匹配的临时对话
temporary-load-failed = 加载临时对话失败
```

`temporary-new-conversation-title` 和 `placeholder-temporary-body` 不应再出现在真实窗口中；可保留 key 以兼容
旧代码删除前的编译过程，但真实 UI 不能继续显示页面 title 或“运行时暂不接入”。

## 实现顺序

1. 共享组件抽取：
   - 把 `features/home/chat_form.rs` 和 `features/home/chat_form/*` 移到
     `components/chat_form.rs` 与 `components/chat_form/*`。
   - 把通用 `PickerListDelegate` / `PickerSection` / `picker_popover` 从
     `features/home/chat_form/picker.rs` 移到 `components/picker.rs`，供 ChatForm model picker 和 Home
     project picker 共同使用。
   - 把 `features/home/conversation.rs` 和
     `features/home/conversation/{message,timeline}.rs` 移到
     `components/conversation_detail.rs` 与 `components/conversation_detail/*`，并将类型改为
     `ConversationDetailPage`。
   - 把 `features/home/conversation/format.rs` 的纯函数移到 `foundation/conversation_format.rs`。
   - 更新 `features/home/{new_conversation,shell}.rs` import，确保 Home 行为等价。
2. 给 `components::conversation_detail::ConversationDetailPage` 暴露 `focus_primary`。
3. 新增 `state::temporary` 和 `FreshRepository::list_no_project_conversations`，补 DB tests。
4. 新增 `features::temporary::new_conversation`，使用 `components::chat_form::ChatForm` 并强制
   `project_id: None`。
5. 新增 `features::temporary::list`，实现 `ListDelegate`、row、selection 和 empty/error states。
6. 新增 `features::temporary::TemporaryWindow` 根 view，接搜索、列表、右侧 route、keyboard/focus。
7. 新增 `app::temporary_window`，替换 menu action 的 placeholder 打开逻辑。
8. 更新 i18n 和文档状态。

## 验证计划

代码实现后运行：

- `cargo fmt`
- `cargo test -p ai-chat-db no_project`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 conversation`
- `cargo test -p ai-chat2 temporary`
- `cargo check -p ai-chat2`
- `git diff --check`

2026-06-11 首版实现已运行：

- `cargo fmt`
- `cargo test -p ai-chat-db no_project`
- `cargo test -p ai-chat2 temporary`
- `cargo test -p ai-chat2 chat_form`
- `cargo test -p ai-chat2 conversation`
- `cargo check -p ai-chat2`
- `git diff --check`

待完成：

- 手动 GPUI UI 验证。

新增测试覆盖：

- no-project 查询返回 scratch project conversations。
- no-project 查询不返回 normal project conversations。
- no-project 查询不返回 deleted conversations。
- no-project 查询不返回 removed scratch project conversations。
- no-project 搜索匹配 title。
- no-project 搜索匹配 `conversation_items.search_text`。
- `secondary-n` key 常量使用 GPUI `secondary-n`。
- 空列表时 up/down 不 panic，selection 保持 `None`。
- 新对话 submit 构造 `CreateConversationRequest.project_id = None`。

手动验证：

- 菜单或临时热键打开临时窗口后，顶部搜索框获得 focus。
- 搜索框保持单行，输入换行键不会插入多行。
- 搜索框 focus 时，`up` / `down` 切换左侧列表，并同步右侧 detail。
- 搜索框 focus 时，`tab` focus 右侧 composer。
- 任意焦点下，`secondary-n` 切到右侧新对话并 focus composer。
- 发送新临时对话后，左侧出现该 conversation，右侧显示 timeline，并启动 agent run。
- 已有 conversation 继续发送时，复用 `ConversationDetailPage` 的 stop generation 行为。
