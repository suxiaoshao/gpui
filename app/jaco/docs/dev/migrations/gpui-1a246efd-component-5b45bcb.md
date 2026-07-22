# Jaco GPUI `1d217ee39d381ac101b7cf49d3d22451ac1093fe` -> `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` / gpui-component `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` -> `5b45bcb26b9343d91a123a4d5ed8a654360512e5` 迁移实施计划

> GPUI：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
> `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`
> gpui-component：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
> `5b45bcb26b9343d91a123a4d5ed8a654360512e5`

## 1. 状态与范围

- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- 文档身份由两组 source/target SHA 唯一确定；后续升级必须新建迁移文档，不覆盖本文件。
- 基线提交：`6351898 refactor: redesign typed form state and bindings`。
- [已完成] 当前 target 可实施的窗口所有权、timer/container query、22 份 JSON 主题、条件式 tab
  overlay、ThemeToken 背景、官方 ListItem/Scrollable/Input 接入和流式 Markdown 非主题证据已实现，
  `cargo test -p jaco` 的 320 个测试及 workspace 自动门通过。
- [未执行] 按本轮约定，不执行 Jaco 实际 UI/Computer Use 验收，也不打包。
- [上游阻断] gpui-component `5b45bcb` 的 rendered Markdown `CodeBlock` 会保存 parse-time
  highlight theme，并使用与主题无关的 styles cache。主题生成与静态映射可以继续实施，但主题切换
  验收必须等待 `UPSTREAM-TEXT-15` 落地，并在包含修复的新 SHA 上新建后继迁移批次。
- [兼容策略] 允许删除被目标上游 API 覆盖的 Jaco 视觉 wrapper；不保留旧 API 兼容层。持久化主题 ID、数据库格式、对话数据和产品行为必须保持兼容。

### 目标

1. 满足目标 GPUI 的自绘标题栏所有权契约，并把 GPUI-owned timer、响应式布局迁移到官方 API。
2. 以目标 gpui-component SHA 的完整 `themes/` 目录为基线同步全部 22 份 JSON 主题，而不是
   只补 `aurora.json`；随后逐个 theme variant 判断上游是否已区分 active/inactive tab：已区分则
   完全采用上游，仍未区分才重放 Jaco 既有 tab overlay，并让 Aurora 渐变在解析、预览和应用中完整保真。
3. 将 Jaco 自定义 UI 的背景绘制迁移到 `Theme.tokens`；文字、图标、边框、caret 和低层颜色计算继续使用 `Hsla`。
4. 使用官方 `ListItem` 承载简单列表行视觉，同时保留 Jaco picker、delegate、selection owner 和 `window.defer` 生命周期边界。
5. 按目标 Scrollable、Input 与焦点行为回归所有受影响表面，只删除可证明没有职责的 wrapper。
6. 显式验证 `TextViewState` 流式 Markdown 与代码块高亮链路，确保 tree-sitter feature、
   语言识别、`app-theme` 提供的共享代码 palette 和 plain fallback 被正确消费；既有消息的
   代码高亮必须在不修改 source/revision 的情况下随当前主题更新。

### 非目标

- 不修改 Jaco 数据库 schema、Diesel migration、repository query 或事务边界。
- 不修改 `gpui-store` state schema、provider/MCP persistence、secret 格式或快捷键数据格式。
- 不修改网络协议、认证、代理、缓存、超时、重试或离线策略。
- 不以标准 `Combobox` 替换 Jaco 的模型、思考程度、权限和项目 picker；目标组件仍没有满足现有互斥打开、分组搜索与 footer 的完整受控契约。
- 不以新版 `InputState` 替换结构化 `ComposerEditor`；其 token、IME、skill、附件、undo 和 completion 语义继续由 Jaco 拥有。
- 不删除 picker、临时对话列表或 skill completion 的 `window.defer`；上游 focus/popover 修复不等于同一 `ListState` 的重入安全。
- 不在 Jaco 中生成、缓存、覆盖或同步 editor/Markdown code palette；颜色映射问题回到
  `crates/app-theme` 修复，Jaco 不维护局部 `HighlightThemeStyle` patch。
- 不由 Jaco 监听主题后遍历 `TextViewState`、调用同值 `set_text`、强制 reparse 或维护
  generation counter；TextView 当前主题读取与 styles cache 失效属于 gpui-component。
- 不采用本次升级新增但 Jaco 没有产品需求的 mobile/touch、`run_embedded`、native popup、exclusive zone、`request_attention`、`reduce_motion` 或 system-wake API。
- 不在本计划中迁移其他 app、共享 crate 或 repo-local skill。

### 实施顺序与发布门

```text
JACO-WINDOW-10
  -> JACO-GPUI-20
  -> JACO-THEME-30
  -> JACO-TOKEN-40
  -> JACO-COMPONENT-50
  -> JACO-MARKDOWN-55
  -> JACO-VERIFY-60
```

`JACO-MARKDOWN-55` 的主题切换部分另有外部前置门：
`UPSTREAM-TEXT-15 -> 包含修复的新 gpui-component SHA -> 新 hash-specific 迁移批次`。
当前文件保留 `5b45bcb` 的迁移证据，不原地改写 target identity。

以下任一条件未满足都阻止完成本迁移：

- Jaco 任一调用 `start_window_move` 的自绘标题栏窗口未设置 `app_owns_titlebar_drag: true`；
- Aurora 渐变在 JSON 解析、设置预览或应用背景中退化为代表色；
- picker、temporary list 或 skill completion 的 confirm/cancel 再次发生 `ListState` 重入 panic；
- 临时窗口搜索焦点或 Up/Down/Enter/Escape 导航回归；
- Taffy/root-fill/Scrollable 变化破坏主窗口、设置窗口、对话框、列表或 composer 布局；
- 流式 Markdown 出现过期帧、丢内容或 fenced code block 语言/颜色/plain fallback 回归；
- rendered Markdown 未消费已安装的共享 syntax palette，或 Jaco 出现第二套代码配色；
- 既有 rendered Markdown code block 只更新背景、不更新 syntax，或必须由 Jaco 修改文本才能刷新主题；
- Jaco 自动化测试、clippy 或 macOS/Linux/Windows CI 失败。

## 2. 证据快照

### 2.1 固定上游契约

| 上游变化 | 固定证据 | Jaco 处理 |
| --- | --- | --- |
| 自绘标题栏所有权 | 目标 GPUI 新增 `WindowOptions::app_owns_titlebar_drag` | 所有主动调用 `start_window_move` 的 Jaco window 显式设为 true |
| GPUI timer | 目标 GPUI/skill 使用 `BackgroundExecutor::timer` | UI task 中三处 `smol::Timer::after` 迁移，task owner 不变 |
| Taffy/root fill | Taffy `0.10.1 -> 0.12.1`，auto-sized root 自动填满 viewport | 不机械删除 `.size_full()`；用关键表面回归决定是否保留显式约束 |
| 实际容器测量 | 目标 GPUI 新增 `container_query` | 设置主题网格删除 viewport/chrome 常数推算 |
| 主题背景 | gpui-component 引入 `ThemeToken`、`ThemeTokens` 与 gradient background | 背景使用 renderable token；前景和颜色计算继续用 `Hsla` |
| JSON themes | 目标 gpui-component 新增 Aurora，共 22 份主题 | 固定 SHA 同步，再重放 Jaco allowlisted tab overlay |
| List row | 目标 gpui-component `ListItem` 覆盖 selectable row 的通用视觉 | picker row 使用官方 `ListItem`；temporary/completion/provider 保留既有无边框 selected 视觉，不改变 delegate 与 callback 时序 |
| Scrollable | 目标版本保留 source element 并修复 auto-height/gap | 删除两处确认的 wrapper-only 层，其余逐项回归 |
| Input | 目标版本修复长文本、soft wrap、selection、content type 等 | 采用明确的 Password/Url 类型；普通输入和 Composer 做回归，不整体替换 |
| TextView streaming | `e416af7f` 合并快速 append 并丢弃过期 parse result | Jaco 的 agent 流式消息直接走 `TextViewState::push_str`，必须验证最终内容和中间更新 |
| syntax highlighter | `372446c0` 将 tree-sitter core feature 化；`3de68cd1`/`78095154` 调整 plain/Markdown source highlighting | Jaco 保留 basic language feature，区分 Markdown source editor 与 rendered `CodeBlock`；颜色生成归 `app-theme` |
| shared theme schema/lifecycle | Input editor 与 `TextView::CodeBlock` 使用同一份 `ActiveTheme.highlight_theme.syntax` schema；但 `5b45bcb` 的 CodeBlock 在 parse 时保存 theme，styles cache 不随 active theme 失效 | 映射关系由 `app-theme` invariant tests 验证；运行时刷新由 `UPSTREAM-TEXT-15` 修复，Jaco 不二次赋色或伪造文本更新 |

### 2.2 当前 Jaco 实现

| 表面 | 当前实现 | 迁移结论 |
| --- | --- | --- |
| main/about/settings window | 自绘标题栏会调用 `start_window_move` | 三处 WindowOptions 设置 owned drag；temporary/screenshot 不设置 |
| temporary/layout/screenshot delay | GPUI task 中直接调用 `smol::Timer::after` | 改用创建该 task 的 GPUI background executor |
| 主题网格 | viewport 宽度减 `SETTINGS_THEME_GRID_WIDTH_CHROME` | `container_query` 使用 grid 自身宽度，纯列数函数保留 |
| JSON themes | 21 份；上游目标 22 份；缺 `aurora.json` | exact snapshot + tab overlay + exact inventory test |
| 主题预览 | `preview_theme` 返回完整 `Theme`，渲染只消费 `colors` | 背景改用 `preview.tokens`，前景/边框继续用 `preview.colors` |
| 自定义背景 | Jaco 31 个文件仍以顶层 `ThemeColor/Hsla` 绘制背景 | 迁移到对应 `Theme.tokens.*`，禁止借 Deref 静默丢 gradient |
| 四个列表行 | 本地实现 selectable/selected/separator 视觉 | picker 使用官方 `ListItem`；其余三处保留场景既有无边框 selected/hover 视觉，业务 children 和 delegate 保留 |
| picker/list callback | confirm/cancel 使用 `window.defer` 更新 owner | 原样保留；禁止改回同 entity 上的 `defer_in` |
| Scrollable | 多个 source 外再包仅承载 size/gap 的容器 | 只删除 settings outer body 和 MCP server list 两处确认冗余层 |
| 会话 Markdown | `conversation_detail.rs` 为每条消息持有 `TextViewState::markdown`，append-only 更新调用 `push_str` | 直接受 TextView rapid-update 改动影响；代码块继续由官方 `CodeBlock` 高亮 |
| 代码块语言 | Jaco default feature 明确启用 basic tree-sitter languages | 已启用语言保持语法色；无语言和未知语言必须稳定退化为 plain text |
| 代码主题 | generated Material theme 由 `crates/app-theme` 产生，Jaco 只通过 GPUI context 消费当前 `ActiveTheme` | editor 与 Markdown 共享 plain/muted/syntax；当前 CodeBlock 的 parse-time snapshot 是上游缺口，Jaco 不持有 palette、订阅或同步状态 |

### 2.3 JSON 主题差异

目标上游主题清单固定为：

```text
adventure alduin asciinema aurora ayu catppuccin everforest fahrenheit
flexoki gruvbox harper hybrid jellybeans kibble macos-classic matrix
mellifluous molokai solarized spaceduck tokyonight twilight
```

Jaco 可保留的 overlay 来源为提交 `173f0e4 Fill ai-chat preset tab theme tokens`，覆盖 14 个文件：

```text
alduin asciinema ayu catppuccin fahrenheit harper kibble macos-classic
matrix mellifluous molokai solarized spaceduck tokyonight
```

允许覆盖的键只有：

```text
tab_bar.background
tab.background
tab.foreground
tab.active.background
tab.active.foreground
```

overlay 不是无条件重放。以每个 `themes[]` variant 为单位检查目标上游 JSON：只要上游显式提供的
`tab.active.background` / `tab.background` 或 `tab.active.foreground` / `tab.foreground` 任意一组值
不同，就视为上游已经提供 active/inactive 区分，该 variant 的五个 tab 键全部采用目标上游值；
只有上游仍未形成这种区分、且该 variant 在既有 14 文件 overlay 中有对应记录时，才重放原来的
五键覆盖。没有历史 overlay 的主题不得新造本地 tab 配色。

`ayu.json` 的 theme-set 顶层 `name` 同步为 `Ayu`；持久化选择仍使用内部
`themes[].name = "Ayu Light"`，因此不增加配置迁移。

## 3. 已冻结决策与 API contract

### 3.1 Window、timer 与 layout

```rust,ignore
WindowOptions {
    titlebar: Some(...),
    app_owns_titlebar_drag: true,
    ..Default::default()
}
```

- 只对 Jaco main/about/settings 三个带自绘 draggable titlebar 的窗口设置 true。
- temporary window、screenshot overlay 及使用 native titlebar 的窗口保持默认 false。
- `is_movable` 保持 true，不以禁用窗口移动绕过上游契约。

GPUI-owned delay 统一为：

```rust,ignore
let timer = cx.background_executor().timer(duration);
cx.spawn(async move |owner, cx| {
    timer.await;
    // existing owner/global update
});
```

- executor/timer 在同步 context 可用时取得；task 的保存、detach、drop/cancel 语义保持不变。
- 不把 timer 迁移扩展为 unrelated async 重构。

主题网格统一使用一次性 owned render snapshot：

```rust,ignore
struct ThemeGridRenderData {
    title: SharedString,
    text: ThemeGridText,
    mode: ThemeMode,
    selected_id: String,
    choices: Vec<ThemeChoice>,
    deletable_material_theme_ids: Vec<String>,
}

fn render_theme_grid(data: ThemeGridRenderData) -> impl IntoElement {
    container_query(move |size, window, cx| {
        render_theme_grid_content(
            data,
            theme_grid_columns(size.width.as_f32()),
            window,
            cx,
        )
    })
}
```

- closure 消费 owned render data，不捕获 `&self` 或 `Context<Self>`。
- 删除 `SETTINGS_THEME_GRID_WIDTH_CHROME` 与 `theme_grid_available_width`。
- 宽度非法或过小时由现有纯函数稳定返回一列。

### 3.2 ThemeToken

```rust,ignore
// 可渲染背景：保留纯色或渐变。
.bg(cx.theme().tokens.button_primary)
.bg(cx.theme().tokens.list_hover.background.opacity(0.45))

// 前景、边框、caret、颜色计算：仍为 Hsla。
.text_color(cx.theme().button_primary_foreground)
.border_color(cx.theme().border)
let selection = cx.theme().selection;
```

- opacity 必须调用 `token.background.opacity(factor)`；禁止
  `theme.tokens.<name>.opacity(...)`，因为 `ThemeToken` 对 `Hsla` 的 Deref 会静默丢失 gradient。
- screenshot overlay 的固定 black scrim/white selection、用户内容颜色、代表 swatch 和 syntax/chart seed 不机械迁移。
- ComposerEditor 的低层 selection quad 使用 `theme.selection` 的代表色，因为 `PaintQuad` 不接受 gradient；token chip 使用 selection/muted 语义。

预览必须同时消费完整主题的两层数据：

```rust,ignore
let preview = app_theme::preview_theme(&choice.config);
let colors = preview.colors;
let tokens = preview.tokens;
```

- tile、button、sidebar 等背景使用 `tokens.*.background`。
- border、label 与只表示颜色的 swatch 使用 `colors.*`。
- Aurora 至少一个代表 token 的 `background.as_solid()` 必须为 `None`。

### 3.3 ListItem、picker 与 defer

`PickerListItem<T>` 的通用视觉改用 `gpui_component::list::ListItem`。
`TemporaryConversationListItem`、`SkillCompletionItem` 与 `ProviderListEntry` 保留原有自定义行，
因为官方 `ListItem` 默认 `active_highlight = true` 会增加 `list_active_border`，改变这些场景已经确认的
无边框 selected 视觉。

契约：

- picker delegate 的薄业务投影使用 `ListItem`；其余三处保留各自既有 hover/selected/separator 样式。
- 业务 content、固定高度、separator、selected 与 suffix 通过公开 builder/style 组合。
- grouped search、受控 popover、互斥 open、footer、selection owner 保持由 Jaco 拥有。
- `ListDelegate::confirm/cancel` 在当前 `ListState` update 释放后，才允许更新 owner 或同步同一个 list；现有 `window.defer` 不得移除或替换为同 entity 的 `defer_in`。
- 官方 selected 分支当前使用代表 `Hsla`；Jaco 不维护额外 gradient workaround。

### 3.4 Scrollable 与 Input

- Scrollable 的 source element 直接拥有 gap、padding 和 size；wrapper 只有承担真实 clipping、overlay 或独立布局职责时才保留。
- 已冻结只删除两处 wrapper-only case：
  - `app/jaco/src/features/settings/layout.rs` outer body；
  - `app/jaco/src/features/settings/mcp.rs::render_server_list`。
- provider secret 使用 `InputContentType::Password` 并继续 `.mask_toggle()`。
- provider Base URL 和 MCP URL 使用 `InputContentType::Url`。
- 普通 text、cookie、environment value 不猜测 content type。
- ComposerEditor 不接入这些 native Input API；只做 IME、selection、completion、focus 与 scroll 回归。

### 3.5 Markdown、代码块与 editor token 边界

- generated Material theme 的唯一颜色链路固定为
  `crates/app-theme -> MaterialCodePalette -> HighlightThemeStyle.syntax -> ActiveTheme`。
  Input editor 与 rendered Markdown `CodeBlock` 共用这份 schema 和 palette 来源，Jaco 不创建中间 palette。
  但 gpui-component `5b45bcb` 的 Input 在 render 时读取当前 theme，而 CodeBlock 保存 parse-time
  theme；`UPSTREAM-TEXT-15` 必须把后者改成 render-time current theme，并让 cache 按 theme identity 失效。
  Jaco 当前没有需要另行验收的代码编辑器 surface，因此本计划只验证 rendered Markdown consumer；
  editor 输出由 `THEME-10` 的语义与对比度测试覆盖。
- 会话渲染链路固定为
  `conversation_detail.rs -> TextViewState::markdown -> CodeBlock -> SyntaxHighlighter(lang)`；
  `message.rs` 与 `tool_blocks.rs` 只组合 `TextView`，不维护第二套 Markdown/highlighter。
- `78095154` 改进的是 Markdown **source editor** 的 fenced-language injection；渲染后的
  `TextView` 已先把 fence 解析为 `CodeBlock`，再直接按 `lang` 高亮，不能把两条路径混为一谈。
- `crates/ui/src/input/lsp/semantic_tokens.rs` 在本次 gpui-component 精确区间没有差异，
  且 Jaco `ComposerEditor` 没有接入该 LSP semantic-token provider；它不是本次会话 Markdown 的直接风险源。
- `e416af7f` 直接改变 Jaco 使用的流式路径：连续 `push_str` 可合并解析，旧 revision
  的完成结果必须被丢弃，最终 document 必须等于完整消息源。
- Jaco 的 `tree-sitter-languages-basic` 继续通过各 language feature 自动启用新的 core
  `tree-sitter` feature；不得因为 core 变为 optional 而只保留依赖、丢失实际高亮能力。
- 已注册语言（至少 Rust、JavaScript）必须使用当前 `HighlightTheme`；无语言和未知语言
  必须渲染全部代码文本并稳定退化为 plain text，不允许空白、黑字或 panic。
- 共享的是代码内容色：plain/muted/syntax。Input editor 的 background/active-line/gutter
  属于 `MaterialEditorChrome`，rendered Markdown `CodeBlock` 的背景属于
  `Theme.tokens.muted`；两个场景的背景有意允许不同。
- `CodeBlock` 背景随上游使用 `Theme.tokens.muted`；Jaco 不覆盖该背景、不复制 upstream
  syntax capture，也不对 `HighlightThemeStyle` 二次赋色。主题切换验证必须在包含
  `UPSTREAM-TEXT-15` 的后继 target 上进行，并覆盖不修改旧消息 source/revision 的场景。
- 目标 gpui-component 当前对普通代码文本和行号使用全局 `foreground` /
  `muted_foreground` fallback；`app-theme` 负责使其与 highlight plain/muted 一致。若未来需要
  独立 editor foreground，应在 gpui-component 上游补齐 consumer，不在 Jaco 添加 wrapper。

## 4. 目标文件结构

```text
app/jaco/
├── assets/themes/gpui-component/*.json       # 22 份 upstream snapshot + 明确 tab overlay
├── docs/dev/
│   ├── README.md                              # Jaco 开发文档索引
│   ├── theme-sources.md                       # upstream SHA、overlay 来源与同步方法
│   └── migrations/
│       └── gpui-1a246efd-component-5b45bcb.md # 本实施计划
└── src/
    ├── app.rs                                 # main WindowOptions
    ├── app/about.rs                           # about WindowOptions + token backgrounds
    ├── app/temporary_window.rs                # background-executor timer
    ├── foundation/assets.rs                   # exact inventory/parse/gradient tests
    ├── state/layout.rs                        # debounce timer，持久化内容不变
    ├── features/settings.rs                   # settings WindowOptions
    ├── features/settings/appearance.rs        # container query + gradient preview
    ├── components/conversation_detail.rs      # streaming TextViewState owner
    ├── components/conversation_detail/message.rs     # rendered Markdown consumer；不做主题映射
    ├── components/conversation_detail/tool_blocks.rs # tool detail Markdown consumer；不做主题映射
    ├── components/picker.rs                   # 受控 picker + ListItem row
    ├── features/temporary/list.rs              # deferred selection + 场景自定义无边框 row
    └── components/chat_input/composer_editor/  # 保留结构化 editor，迁移语义背景
```

JSON theme 同步流固定为：

```text
gpui-component@5b45bcb26b9343d91a123a4d5ed8a654360512e5/themes/*.json
  -> exact 22-file snapshot
  -> reapply commit 173f0e4 allowlisted 5 tab keys in 14 files
  -> Jaco embedded assets
  -> ThemeSet parse/register
  -> Theme.tokens gradient smoke
```

`theme-sources.md` 必须记录仓库、完整 SHA、22 文件清单、overlay commit、14 文件、5 键 allowlist 与三方比较流程。后续不能把当前目录误认为无改动上游快照。

## 5. 工作包

### JACO-WINDOW-10：迁移自绘标题栏所有权

**Prerequisites**

- `Cargo.lock` 已锁定本计划标题中的两组 target SHA。

**Evidence**

- 目标 GPUI 要求主动调用 `Window::start_window_move` 的 app 设置 `app_owns_titlebar_drag`。
- Jaco main/about/settings 使用自绘 `TitleBar`；temporary 与 screenshot overlay 没有该 draggable shell。

**Files**

- 修改 `app/jaco/src/app.rs`。
- 修改 `app/jaco/src/app/about.rs`。
- 修改 `app/jaco/src/features/settings.rs`。

**API contract**

- 三个窗口的 `WindowOptions::app_owns_titlebar_drag` 必须显式为 true；temporary/screenshot 与 native-titlebar 窗口保持 false。
- 现有 `TitlebarOptions`、placement、window kind 和 reveal 行为不变。

**Implementation flow**

1. 在三个 `WindowOptions` literal 加入 `app_owns_titlebar_drag: true`。
2. 保留 title、transparent、traffic-light placement、background、window kind 与 reveal 顺序。
3. 全仓搜索 Jaco 的 `start_window_move`，确认每个调用点所属 window 都被覆盖。

**Errors and lifecycle**

- 无 async/persistence 变化。
- 不通过 `is_movable: false` 或额外事件拦截掩盖拖动问题。

**Tests**

| Requirement | Test location | Proposed test/smoke | Assertions |
| --- | --- | --- | --- |
| options 不回归 | 现有 app/about/settings options tests | 保留并扩充现有测试 | title、transparent、placement 不变，owned drag 为 true |
| 窗口行为 | Computer Use/manual | `jaco_owned_titlebar_drag_smoke` | 三窗口点击无异常延迟，拖动/双击正确 |

**Validation**

```bash
rg -n "start_window_move|app_owns_titlebar_drag" app/jaco/src
cargo test --locked -p jaco
```

**Done condition**

- 三个自绘标题栏窗口显式为 true，其他窗口未误设，macOS 定向 smoke 通过。

### JACO-GPUI-20：迁移 timer 与响应式布局

**Prerequisites**

- `JACO-WINDOW-10` 完成。

**Evidence**

- temporary close、layout debounce、screenshot capture 是 GPUI-owned task，当前使用 `smol::Timer::after`。
- 设置主题网格的真实可用宽度属于局部容器属性，不应从 viewport 减 chrome 常数推算。

**Files**

- 修改 `app/jaco/src/app/temporary_window.rs`。
- 修改 `app/jaco/src/state/layout.rs`。
- 修改 `app/jaco/src/features/screenshot/overlay.rs`。
- 修改 `app/jaco/src/features/settings/appearance.rs`。

**API contract**

- 三处 delay 改用创建 task 的 `BackgroundExecutor::timer`；duration、owner、drop/detach 与 update target 不变。
- 主题网格列数只由 `container_query` 返回的实际宽度和纯函数 `theme_grid_columns` 决定。
- query closure 只捕获 owned render snapshot，不捕获 `&self` 或正在 update 的 entity context。

**Implementation flow**

1. 在同步 GPUI context 中取得 background-executor timer，再创建现有 task。
2. 保持三个 delay 的 duration、task owner、存储/detach 与错误处理不变。
3. 将主题 choices、selected metadata 与文案组装成 owned `ThemeGridRenderData`。
4. 在 `container_query` 内按实际宽度调用现有 `theme_grid_columns` 并渲染 grid。
5. 删除 viewport/chrome helper 和常数；不批量删除其他 `.size_full()`。

**Errors and lifecycle**

- owner/task drop 仍取消等待；关闭窗口后不得执行 stale update。
- layout debounce 仍写入同一 state/path，不改变去抖时长与失败日志。
- screenshot capture 仍沿用现有捕获错误和 overlay close 流程。

**Tests**

| Requirement | Test location | Proposed test/smoke | Assertions |
| --- | --- | --- | --- |
| columns 算法 | `features/settings/appearance.rs` | existing `theme_grid_columns_*` | narrow/wide 上下界不变 |
| 实际容器宽度 | GPUI test/manual | `theme_grid_uses_container_width` | resize/sidebar/font/language 下不溢出、不过早换列 |
| timer lifecycle | 对应现有 focused tests | preserve existing behavior | owner drop 不执行 stale update，时序不变 |

**Validation**

```bash
cargo test --locked -p jaco theme_grid
rg -n "smol::Timer::after" app/jaco/src
```

第二条 residual 预期为空。

**Done condition**

- 三处 timer 与 theme grid 完成官方 API 迁移，timer 生命周期、布局持久化和截图行为均未改变。

### JACO-THEME-30：完整同步 22 份 JSON themes、Aurora 与渐变预览

**Prerequisites**

- `JACO-GPUI-20` 完成。
- `THEME-10` 已由 `crates/app-theme` 提供与目标 gpui-component schema 一致的完整
  `ThemeConfig { colors, highlight }`，包括共享 code palette 与 editor chrome；若共享工作包未完成，
  本工作包暂停在资产同步前。

**Evidence**

- 目标上游有 22 份主题。当前分支虽已补入 `aurora.json`，但“只新增缺失文件”不等于完成
  source snapshot 迁移：其余 21 份也必须从同一目标 SHA 重新同步，避免本地继续混用旧版本内容。
- 14 份本地主题只有五个 tab 键具有明确的 Jaco overlay 来源；这些值只是上游未区分 tab 时的
  fallback，不得覆盖目标上游已经改进的 active/inactive tab 配色。其他差异没有保留依据。
- 当前 preview 丢弃 `Theme.tokens`，会把 gradient 能力退化为代表色。

**Files**

- 修改 `app/jaco/assets/themes/gpui-component/*.json`。
- 新增 `app/jaco/assets/themes/gpui-component/aurora.json`。
- 新增 `app/jaco/docs/dev/theme-sources.md`。
- 修改 `app/jaco/docs/dev/README.md` 增加来源文档入口。
- 修改 `app/jaco/src/foundation/assets.rs`。
- 修改 `app/jaco/src/features/settings/appearance.rs`。

**API contract**

- `bundled_theme_sets()` 返回全部 22 份可解析的 embedded theme strings。
- 22 份文件必须全部来自 gpui-component
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5` 的 `themes/`；本地目录是带 allowlisted overlay
  的 vendored snapshot，不允许逐文件挑选更新。
- tab overlay 决策按 theme variant 执行：上游 active/inactive background 或 foreground
  已有显式差异时五个 tab 键均保留上游值；否则才从迁移前 Jaco snapshot 重放该 variant 的
  既有五键 overlay。不能混合“部分上游、部分本地”生成第三套配色。
- embedded path 测试检查精确文件名集合，不能以 `len >= 20` 代替。
- `preview_theme` 的背景路径保留 `Background`，不得提前调用代表色转换。
- 持久化 theme ID 继续是 `preset:<themes[].name>`；Ayu 不迁移用户配置。
- 本工作包只同步 JSON preset 自带的主题数据；不把 generated Material code palette 迁入 Jaco。
- generated Material 的 M3 button colors、state layers 和共享 code palette 由 `THEME-10`
  唯一生成；Jaco 只通过 `ThemeRegistry` / `ActiveTheme` 接入。Jaco 不覆写组件 border、radius、
  padding、size、shadow/elevation 或 focus ring。

**Implementation flow**

1. 清点目标 SHA 的完整 `themes/` 目录，用其 22 份 JSON 覆盖本地同名 snapshot；不能只复制
   新增的 Aurora，也不能保留旧 SHA 的其余 21 份内容。
2. 对既有 14 文件逐个 variant 比较目标上游的 active/inactive tab 值；上游已区分时不覆盖，
   未区分时才从迁移前 snapshot 重放该 variant 的五个 allowlisted tab 键。
3. 写 `theme-sources.md`，记录来源、判断规则、实际保留 overlay 的 variant 与下次三方同步方法。
4. 资产测试逐个解析 `ThemeSet` 并注册；断言包含 `Ayu Light`、`Aurora Light`。目标 SHA 的
   Aurora theme set 只有 `Aurora Light`，不得虚构 `Aurora Dark`。
5. 应用 Aurora 配置，断言至少一个代表 token 的 `background.as_solid().is_none()`。
6. 设置 tile/button/sidebar 等预览背景改用 tokens；颜色 swatch 和文本继续用 colors。

**Errors and lifecycle**

- 任一 JSON 无效或 inventory 漂移由测试阻止合入，不在启动时静默跳过。
- 主题仍编译进 runtime assets；不增加网络 fetch 或缓存。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| exact inventory | `foundation/assets.rs` | `assets_embed_exact_upstream_theme_inventory` | 精确 22 文件名 |
| parse/register | 同上 | `bundled_theme_sets_parse_and_register` | 所有文件可解析、预期主题名存在 |
| conditional tab overlay | 同上/资产比较测试 | `bundled_themes_prefer_upstream_tab_distinction` | 已区分的 variant 五键等于上游；未区分且有历史覆盖的 variant 五键等于既有 overlay；其余键等于上游 |
| gradient | 同上 | `aurora_theme_preserves_gradient_tokens` | 代表 token background 非 solid |
| preview | appearance/manual | `aurora_preview_preserves_gradient_backgrounds` | 预览与实际应用不退化 |

**Validation**

```bash
cargo test --locked -p jaco assets_embed_exact_upstream_theme_inventory
cargo test --locked -p jaco bundled_theme_sets_parse_and_register
cargo test --locked -p jaco aurora_theme_preserves_gradient_tokens
```

**Done condition**

- 全部 22 份主题与目标 SHA 同步；上游已区分 tab 的 variant 使用上游颜色，仍未区分的 variant
  才保留既有 overlay；来源记录、资产比较和 gradient tests 完整；Ayu 选择兼容；Aurora 预览与应用保真。

### JACO-TOKEN-40：迁移自定义背景到 ThemeTokens

**Prerequisites**

- `JACO-THEME-30` 完成，Aurora 可作为 gradient 回归夹具。

**Evidence**

- 下列 Jaco 文件仍以顶层 `ThemeColor/Hsla` 绘制元素背景。
- `ThemeToken` 的 Deref 会让错误 API 继续编译，因此必须以 source residual 和 Aurora 视觉测试共同约束。

**Files**

- `app/jaco/src/app/about.rs`
- `app/jaco/src/app/title_bar_menu.rs`
- `app/jaco/src/components/chat_form.rs`
- `app/jaco/src/components/chat_input/composer_editor.rs`
- `app/jaco/src/components/chat_input/composer_editor/completion.rs`
- `app/jaco/src/components/chat_input/composer_editor/element.rs`
- `app/jaco/src/components/chat_input/composer_editor/skill_detail.rs`
- `app/jaco/src/components/conversation_detail.rs`
- `app/jaco/src/components/conversation_detail/attachments.rs`
- `app/jaco/src/components/conversation_detail/message.rs`
- `app/jaco/src/components/conversation_detail/tool_blocks.rs`
- `app/jaco/src/components/hotkey_input.rs`
- `app/jaco/src/components/image_preview.rs`
- `app/jaco/src/components/picker.rs`
- `app/jaco/src/features/home/shell.rs`
- `app/jaco/src/features/home/sidebar/row.rs`
- `app/jaco/src/features/home/sidebar/search.rs`
- `app/jaco/src/features/settings/layout.rs`
- `app/jaco/src/features/settings/mcp/detail.rs`
- `app/jaco/src/features/settings/mcp/dialog.rs`
- `app/jaco/src/features/settings/mcp/row.rs`
- `app/jaco/src/features/settings/projects.rs`
- `app/jaco/src/features/settings/prompts/dialog.rs`
- `app/jaco/src/features/settings/prompts/rows.rs`
- `app/jaco/src/features/settings/provider/components.rs`
- `app/jaco/src/features/settings/provider/list_delegates.rs`
- `app/jaco/src/features/settings/shortcuts/rows.rs`
- `app/jaco/src/features/settings/skills.rs`
- `app/jaco/src/features/settings/skills/rows.rs`
- `app/jaco/src/features/temporary.rs`
- `app/jaco/src/features/temporary/list.rs`

**API contract**

- 元素背景必须保留 `Background` 到最终 `.bg(...)`；不能通过 `ThemeToken` 的 `Deref<Hsla>` 或 `as_solid()` 提前降级。
- `text_color`、`border_color`、caret、icon 与低层 paint 继续接收 `Hsla`。
- opacity 只能写在 `token.background` 上。
- Composer/editor 文件只消费全局 `ActiveTheme`；不得在 Jaco 生成或修补 editor/syntax palette。

**Implementation flow**

1. 对每个文件先区分 background、foreground、border 与 color-math。
2. 元素背景映射到对应 `tokens`；透明度写为 `token.background.opacity(...)`。
3. raw `theme.blue` selection/token chip 改为 selection/primary/muted 语义。
4. 逐条审阅 residual；目标是 Jaco 元素背景不再走顶层 theme `Hsla`。低层 API 只接受 `Hsla` 时保留并在测试/代码语义中明确原因。
5. 用 Material light/dark 与 Aurora 检查 main、settings、about、temporary、conversation/composer
   和 dialogs；这里只验证 `ActiveTheme` 消费，不在应用层修补 colors/highlight 映射。

**Errors and lifecycle**

- No async/error/lifecycle change。
- 不用 `as_solid()` fallback 掩盖不接受 gradient 的高层容器；应把背景一路保留为 `Background`。

**Tests**

| Requirement | Test location | Proposed test/gate | Assertions |
| --- | --- | --- | --- |
| gradient API | `foundation/assets.rs` | reuse Aurora token test | opacity 后仍非 solid |
| composer selection | composer focused tests | `composer_uses_theme_selection_color`（可纯提取时） | 不依赖 raw blue |
| residual | source gate | two `rg` commands | 无顶层 theme 背景；无 token 直接 opacity |

**Validation**

```bash
rg -n '\.bg\(cx\.theme\(\)\.' app/jaco/src -g '*.rs' | rg -v '\.tokens\.'
rg -n 'tokens\.[a-zA-Z0-9_]+\.opacity\(' app/jaco/src -g '*.rs'
cargo test --locked -p jaco
```

第一个 scan 只允许 `chat_form.rs` 中直接使用 `input_background()` 及其与页面背景的既有 blend；
这是 gpui-component 提供的计算后输入外观。第二个 scan 预期为空。

**Done condition**

- Jaco 自定义元素背景遵循 ThemeToken 契约，Aurora 主要 surface 不丢 gradient，前景与低层 paint API 没有错误接收 Background。

### JACO-COMPONENT-50：迁移列表行并回归 Scrollable/Input

**Prerequisites**

- `JACO-TOKEN-40` 完成，以目标主题视觉验收组件替换。

**Evidence**

- 官方 `ListItem` 覆盖 picker row；其余三处保持既有无边框 selected 视觉。
- 目标 Scrollable 让 source element 成为实际 content；两处本地 outer wrapper 没有额外职责。
- Jaco 的受控 picker、ComposerEditor 与 deferred callback 仍没有可直接删除的官方替代。

**Files**

- 修改 `app/jaco/src/components/picker.rs`。
- 修改 `app/jaco/src/features/temporary/list.rs`。
- 修改 `app/jaco/src/components/chat_input/composer_editor/completion.rs`。
- 修改 `app/jaco/src/features/settings/provider/list_delegates.rs`。
- 修改 `app/jaco/src/features/settings/layout.rs`。
- 修改 `app/jaco/src/features/settings/mcp.rs`。
- 修改 `app/jaco/src/features/settings/provider.rs`（Password/Url content type）。
- 修改 `app/jaco/src/features/settings/mcp/dialog.rs`（Url content type）。

**API contract**

- picker row 的视觉由官方 `ListItem` 提供；temporary/completion/provider 保持既有视觉；
  delegate 数据、业务 children、selection owner 和 confirm/cancel 时序不变。
- `ListState` callback 继续以 `window.defer` 跨出当前 update；禁止同 entity `defer_in`。
- 只删除两处已冻结的 wrapper-only Scrollable 层；其他表面必须依据回归证据决定，不得批量重写。
- content type 只描述 native input 语义，不承担 validation、masking 或 value normalization。

Scrollable 回归清单如下；除两处确认删除外，不预设代码改动：

- `app/jaco/src/app/title_bar_menu.rs`
- `app/jaco/src/components/chat_form.rs`
- `app/jaco/src/components/chat_input/composer_editor.rs`
- `app/jaco/src/components/chat_input/composer_editor/skill_detail.rs`
- `app/jaco/src/components/conversation_detail.rs`
- `app/jaco/src/components/conversation_detail/attachments.rs`
- `app/jaco/src/features/settings/layout.rs`
- `app/jaco/src/features/settings/mcp.rs`
- `app/jaco/src/features/settings/mcp/dialog.rs`
- `app/jaco/src/features/settings/prompts/dialog.rs`
- `app/jaco/src/features/settings/provider.rs`
- `app/jaco/src/features/settings/shortcuts/dialog.rs`
- `app/jaco/src/features/settings/skills.rs`
- `app/jaco/src/features/settings/skills/rows.rs`

**Implementation flow**

1. picker row 视觉改用 `ListItem`；其余三处保留既有无边框 selected/hover 视觉；所有 delegate、
   业务 children、selection owner 和 defer tests 保持。
2. `settings/layout.rs` 让 outer body 的 `v_flex` 自己 scroll。
3. `settings/mcp.rs::render_server_list` 让带 gap 的 `v_flex` 自己 scroll。
4. provider secret/Base URL 与 MCP URL 设置明确 content type，不改变值、mask toggle 或 validation。
5. 对全部 Scrollable 表面及普通 Input 做小窗口、长文本、focus、selection、soft-wrap 回归；只有观察到目标版本行为变化时，才给调用方补明确尺寸约束。
6. 对 controlled picker 与 ComposerEditor 做交互回归，不将其重写为标准组件。

**Errors and lifecycle**

- List callback 只有在当前 list entity update 结束后才能更新 owner/同一 list。
- row click、keyboard confirm 与 separator 不得因视觉替换丢失。
- Input content type 不承担 URL/secret validation，也不改变 form 的 error lifecycle。

**Tests**

| Requirement | Test file/surface | Existing/proposed test | Assertions |
| --- | --- | --- | --- |
| picker defer | `components/picker.rs` | `confirm_callback_runs_after_list_update_finishes` | owner 同步同一 list 无 panic |
| temporary defer | `features/temporary/list.rs` | 同名 existing test | route/list 同步无 panic |
| completion defer | `composer_editor/completion.rs` | 同名 existing test | completion owner 更新无 panic |
| row interaction | four delegates/manual | `list_rows_preserve_selection_and_clicks` | picker 使用官方边框；其余三处无边框；mouse/keyboard/selected/separator 保持 |
| scroll | 14 surfaces/manual | `jaco_scrollable_layout_matrix` | gap/padding/max-height/independent scroll 正确 |
| picker/focus | settings/temporary/manual | `jaco_picker_focus_matrix` | 连续切换、搜索焦点、Up/Down/Enter/Escape 正确 |
| input | provider/MCP/composer/manual | `jaco_input_regression_matrix` | password toggle、URL、long text、IME、selection、completion 正确 |

**Validation**

```bash
cargo test --locked -p jaco confirm_callback_runs_after_list_update_finishes
cargo test --locked -p jaco
```

**Done condition**

- picker 重复 row visual 已删除；其余三处保留已确认的场景视觉；受控 picker、defer 与
  ComposerEditor 边界保留；Scrollable/Input 的自动化与人工回归通过。

### JACO-MARKDOWN-55：验证流式 Markdown 与代码块高亮

**Current status**

- 流式 revision、fence boundary、language feature 与 plain fallback 可在当前 `5b45bcb` target 上实施和留证。
- current-theme/cache 验收在当前 target 上不可达；这部分必须由包含 `UPSTREAM-TEXT-15` 的
  后继 hash-specific Jaco 计划继承。当前工作包在此之前保持 blocked。

**Prerequisites**

- `JACO-COMPONENT-50` 完成。
- `THEME-10` 已完成共享 code palette、editor chrome 与单一 `HighlightThemeStyle` 投影。
- 当前 target 的非主题切换证据继续以 `5b45bcb` 为基线。
- 主题切换子门的额外前置条件是：`UPSTREAM-TEXT-15` 已合入、workspace 已切到包含修复的
  新完整 SHA，且已按该 SHA 新建后继迁移批次；该子门只能在后继计划中执行。

**Evidence**

- `app/jaco/src/components/conversation_detail.rs::ensure_message_text_state` 对新消息创建
  `TextViewState::markdown`，对 append-only 流式内容调用 `TextViewState::push_str`。
- gpui-component `e416af7f` 会合并快速 append，并以 revision 丢弃过期 parse result。
- rendered `CodeBlock` 仍直接使用 `SyntaxHighlighter(lang)`；`78095154` 的 Markdown
  source injection 不是这条渲染路径，但共享的 highlighter 与 tree-sitter feature 仍需回归。
- `5b45bcb` 在 parse 时把 highlight theme 存入 `CodeBlock`，并缓存与 theme identity 无关的
  styles；因此只切换 `ActiveTheme` 时 background 会更新而 syntax 仍使用旧颜色。

**Files**

- 验证并在需要时修改 `app/jaco/Cargo.toml` 的 `tree-sitter-languages-basic` feature。
- 在 `app/jaco/src/components/conversation_detail.rs` 的现有测试模块补流式 Markdown 状态测试。
- 验证 `app/jaco/src/components/conversation_detail/message.rs`。
- 验证 `app/jaco/src/components/conversation_detail/tool_blocks.rs`。
- 不修改 `crates/app-theme` 或 gpui-component highlighter，不在 Jaco 新建 Markdown renderer、
  code palette 或主题同步器。

**API contract**

- 任意 append 分片顺序下，最后接受的 `TextViewState` source 与数据库消息 source 完全一致。
- Rust/JavaScript 等已启用语言保留语法高亮；unknown/no-language code fence 完整显示为 plain text。
- inline code、fenced code、普通段落和列表之间的样式范围不得串色。
- 主题切换后 rendered Markdown 必须消费共享的 plain/muted/syntax 内容色；Markdown
  `tokens.muted` 背景必须可读且不得退化为默认黑色。它与 editor background 的独立语义及
  双 surface 对比度由 `THEME-10` 自动测试保证；当前 theme 的读取与 styles cache 失效由
  gpui-component 上游保证，不由 Jaco 同步。
- 切换主题不得修改既有消息的 TextView source、revision 或 Markdown block structure，也不得
  依赖应用调用 `set_text` 或重新创建消息 state。
- 若失败源于颜色映射或对比度，由 `THEME-10` 修复；Jaco 不增加局部颜色 override。

**Implementation flow**

1. 用 `cargo tree -e features -p jaco` 证明 basic language features 自动带入 core `tree-sitter`。
2. 构造包含普通段落、inline code、Rust fence、unknown fence 和无语言 fence 的 Markdown fixture。
3. 将同一 fixture 按 fence delimiter、language tag 和代码行边界拆成多次 append，验证最终 source/文档没有旧 revision 覆盖。
4. [后继迁移] 在包含上游修复的 target 上，以同一个已渲染 fixture 依次切换 Material light/dark 与
   Aurora，检查代码块 background、syntax、plain fallback、选择和复制；切换过程不修改 fixture source。
5. [后继迁移] 若颜色/对比度 invariant 失败，回到 `THEME-10`；若 current-theme/cache 上游门失败，回到
   `UPSTREAM-TEXT-15`。两种情况都不得在 Jaco 复制 highlighter 或添加主题同步器。

**Errors and lifecycle**

- background parse 的 stale result 不得覆盖较新 revision。
- 主题切换只使当前 theme 的 syntax styles cache 失效，不得增加 parse revision、异步任务或应用回调。
- 未注册语言不是错误，不得丢消息或 panic。
- 语言 feature 缺失必须在依赖门失败，不能通过将所有代码块静默当 plain text 来宣告迁移完成。

**Tests**

| Requirement | Test/surface | Proposed evidence | Assertions |
| --- | --- | --- | --- |
| streaming revision | `conversation_detail.rs` | `streaming_markdown_keeps_latest_complete_source` | 分片 append 后最终 source 完整且无旧结果回写 |
| fence boundaries | same fixture | `streaming_markdown_preserves_split_fences` | delimiter/lang/code 跨 chunk 仍形成正确 code block |
| language features | Cargo graph + rendered fixture | dependency gate + manual highlight smoke | grammar 注册；Rust/JavaScript 显示非默认 syntax colors |
| plain fallback | rendered fixture | `unknown_and_untyped_code_blocks_render_plain_text` | 文本完整、无 panic、无错误串色 |
| shared theme consumption | conversation/tool detail manual smoke | successor target 上的 light/dark/Aurora matrix | 既有消息不修改 source 即使用当前 syntax palette；背景可读；Jaco 无 palette override/sync |

**Validation**

```bash
cargo tree --locked -e features -p jaco
cargo tree --locked -p jaco -i gpui-component@0.5.2
cargo test --locked -p jaco streaming_markdown
cargo test --locked -p jaco code_blocks
cargo test --locked -p jaco
```

**Done condition**

- 流式 Markdown 最终一致、fence 分片稳定、已注册语言保留高亮、未知/无语言完整 plain
  fallback；在包含上游修复的 target 上，三套代表主题下既有 Markdown 不修改 source 即消费
  当前共享 palette、背景可读，且无旧 revision 闪回。在此之前只能记录当前 target 的部分证据，
  不得把 `JACO-MARKDOWN-55` 标为完成。

### JACO-VERIFY-60：Jaco 发布门

**Current status**

- [阻断] `JACO-MARKDOWN-55` 的 current-theme/cache 子门在 `5b45bcb` 上不可达；本发布门必须
  由后继 hash-specific Jaco 计划继承并完成，当前文档不得回写新的 target SHA。

**Prerequisites**

- `JACO-MARKDOWN-55` 完成。
- `THEME-10`、`UPSTREAM-TEXT-15` 与后继 gpui-component SHA 对应的 skill-sync 工作包已完成；
  当前 `SKILL-70` 只 vendoring `5b45bcb`，不能替代后继同步。
- 依赖图与后继 hash-specific 迁移批次记录的 target SHA 一致，而不是继续声称 `5b45bcb`
  已满足主题生命周期契约。

**Evidence**

- 本次升级同时改变 window、Taffy、focus/popover、theme、Scrollable、Input 与通用 row 视觉；只通过既有单元测试不足以证明 Jaco 迁移完成。
- Jaco 已出现过 `ListState` 重入和临时窗口焦点回归，必须把生命周期与键盘路径作为显式发布门。

**Files**

- 只修改本计划列出的 Jaco 文件，以及为这些行为新增的 Jaco 定向测试。
- 当前 target 可实施的部分证据可以回填本文件；`JACO-VERIFY-60` 的完成状态与最终证据只能
  写入后继 hash-specific Jaco 计划，本文件永久保留 blocked/partial 状态。
- 验证发现的范围外问题单独建 issue，不在 VERIFY 阶段引入兼容层。

**API contract**

- 当前 hash-specific 计划记录的 dependency target SHA、22-theme inventory、ThemeToken 背景、
  owned titlebar、deferred List callback、受控 picker 与 TextView current-theme 契约必须同时成立。
- 任何 release blocker 必须回到拥有它的工作包修复，不允许在验证层添加 fallback 或吞错。

**Implementation flow**

1. 运行格式、build、test、clippy、dependency 与 residual gates。
2. 视觉检查 Material light/dark、Aurora 下的 main/settings/about/temporary/screenshot、theme grid、
   conversation Markdown/code block、composer、provider/MCP/prompt/shortcut dialogs；Markdown
   应让既有消息在不修改 source 的情况下消费当前共享 palette；跨 editor/Markdown 的静态映射
   由 `THEME-10` 自动门负责，TextView 的运行时刷新由 upstream test 负责。
3. 交互检查 project/model/effort/approval picker 连续切换、Up/Down/Enter/Escape、mouse selection、search/composer focus 切换，不得出现 entity re-entry。
4. 验证主窗口、设置窗口和关于窗口的 titlebar drag/double-click。
5. 运行 macOS/Linux/Windows CI；Linux Wayland/X11 与 Windows 至少有启动 smoke。

**Errors and lifecycle**

- 测试或 smoke 中的 entity re-entry、stale task update、focus 丢失、gradient 降级均视为失败，不能以重试或默认值掩盖。
- Computer Use 无法覆盖的平台由对应 CI artifact/manual smoke 补充证据。

**Tests**

| Requirement | Evidence | Assertions |
| --- | --- | --- |
| dependency | Cargo graph | Jaco 只连接后继计划记录的 GPUI/gpui-component 完整 source SHA |
| theme | Jaco asset/token tests | exact 22 inventory、全解析、Aurora gradient 保真 |
| lifecycle | three defer tests + timer tests | 无重入、无 stale update |
| interaction | Jaco UI matrix | picker、temporary、composer 的 keyboard/mouse/focus 正确 |
| Markdown/highlighter | upstream lifecycle tests + streaming tests + rendered fixture | 无 stale revision；known language 高亮；unknown/no-language plain fallback；既有消息随当前主题刷新且 Markdown 背景可读 |
| layout | Jaco UI matrix | main/settings/dialog/scroll 在小窗口与 resize 下正确 |
| platform | CI + startup smoke | macOS/Linux/Windows 通过 |

**Validation**

```bash
cargo fmt --all -- --check
git diff --check
cargo build --locked -p jaco
cargo test --locked -p jaco
cargo clippy --locked -p jaco --all-targets --all-features -- -D warnings
cargo tree --locked -i gpui@0.2.2
cargo tree --locked -i gpui-component@0.5.2
rg -n '\.bg\(cx\.theme\(\)\.' app/jaco/src -g '*.rs'
rg -n 'tokens\.[a-zA-Z0-9_]+\.opacity\(' app/jaco/src -g '*.rs'
```

**Done condition**

- 自动门、Jaco 视觉/交互矩阵和三平台 CI 全部通过；没有 gradient 丢失、TextView 旧主题
  cache、List 重入、焦点、titlebar、scroll 或 Taffy blocker。

## 6. No change surfaces

| Surface | 结论 | 验证方式 |
| --- | --- | --- |
| 数据库/schema/transaction | No change | `git diff -- app/jaco/migrations crates/jaco-db` 为空 |
| gpui-store/app state schema | No change | 不新增/迁移 store key、state persistence |
| 网络/auth/proxy/cache/retry | No change | 现有 provider/MCP/request 行为测试继续通过 |
| 对话/附件/agent 数据流 | No change | ChatInputSubmit、conversation persistence 与 attachment contract 不改 |
| 快捷键/temporary route 数据 | No change | 只回归焦点与列表交互，不修改 shortcut schema/dispatch |
| Fluent/macOS bundle i18n | No new user-facing strings | 不改 `.ftl` 或 `InfoPlist.strings` |
| 图标 | No new icon declaration | Aurora 是 runtime theme asset，不是 app icon |
| theme persistence | No migration | 继续使用 `preset:<themes[].name>`；Ayu internal name 不变 |
| task ownership/cancellation | No semantic change | timer owner、drop/detach 与 failure path 保持 |
| ComposerEditor architecture | Retain | token/IME/attachment/undo/completion owner 不变；不与 rendered Markdown `CodeBlock` 混为一条路径 |
| generated code palette | Owned by `crates/app-theme` | Jaco 只消费 `ActiveTheme`；editor/Markdown 共用内容色但使用不同场景 chrome/surface |
| TextView theme refresh | Owned by gpui-component | Jaco 不订阅主题、不遍历消息、不触发同值 `set_text` 或 reparse |
| controlled picker architecture | Retain | open orchestration、grouped search、footer、selection owner 不变 |

## 7. 执行交接审计

- [x] 文档标题与文件名由 target SHA 标识，正文写全 source/target SHA；未来迁移不会覆盖本文件。
- [x] 只包含 Jaco exact files、行为、测试和 No change surfaces；其他 app/crate/skill 由各自计划承载。
- [x] 每个工作包有 prerequisites、evidence、API contract、implementation flow、lifecycle、tests、validation 与 done condition。
- [x] JSON theme、Aurora、preview、自定义 ThemeToken 背景构成 Jaco 的主题资产与消费链路；
  generated Material 颜色链路由 `THEME-10` 独立承担。
- [x] ListItem 复用没有删除 Jaco picker、delegate、defer 与 focus 的领域/生命周期职责。
- [x] editor/highlighter 更新已按 Markdown source editor 与 rendered `TextView CodeBlock` 两条路径拆分；
  两者共享 `app-theme` 生成的内容 palette、保留不同 chrome/surface，并设置流式 revision、
  language feature、plain fallback 与消费回归门；`5b45bcb` 的 parse-time theme/cache 缺口已单列
  为 `UPSTREAM-TEXT-15`，没有下沉为 Jaco workaround。
- [x] database、store、network、persistence、i18n、icons、task cancellation 等系统表面已明确为变更或 No change。
