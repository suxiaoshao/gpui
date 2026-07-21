# GPUI `1a246efd` / gpui-component `5b45bcb` 依赖证据

本文只保存本迁移批次共享的上游与依赖证据；各 package 的具体处理见
[总计划](README.md)中的子计划索引。

## 1. 精确版本区间

| Dependency | Source | Target | 本地状态 | 决策 |
| --- | --- | --- | --- | --- |
| `gpui` / `gpui_platform` | Zed `1d217ee39d381ac101b7cf49d3d22451ac1093fe` | Zed `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | `Cargo.lock` 已更新；crate version 仍为 `0.2.2` | 以 source SHA 而非 crate version 判断迁移 |
| `gpui-component` / assets | `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` | `5b45bcb26b9343d91a123a4d5ed8a654360512e5` | `Cargo.lock` 已更新；crate version 仍为 `0.5.2` | UI crate/assets/macros 保持同一 SHA |
| Zed source identity | root 曾使用带 `?rev=` source | root 与组件库都使用 canonical Git URL，由 lockfile 固定 SHA | 已对齐 | 不恢复单侧 `?rev=`，避免两套 GPUI 类型 |
| Rust toolchain | repo 文档 `1.92+` | Zed target checkout 使用 `1.95.0`；上游 crate 未声明正式 `rust-version` | 当前环境 `1.97.0` | repo 支持/验证基线写 `1.95+`，不称上游 MSRV |
| tree-sitter features | 旧组件 feature 结构 | 新 core `tree-sitter` feature；语言 features 自动包含它，native parser deps optional | 本地 feature 名无需更改 | 用 package feature tree 防止意外扩散 |

上游没有用 crate version 表达完整 Git 区间变化，因此本批次以 compare range、commit、PR、公开源码、组件文档和 stories 作为迁移记录。

## 2. GPUI 变化与 owner

| Upstream change | Evidence | Semantic impact | Owner plan |
| --- | --- | --- | --- |
| Taffy `0.10.1 -> 0.12.1` | Zed `91fdd55889` | flex/root/scroll layout 可能在仍可编译时发生变化 | Jaco、Feiwen、HTTP、Novel 子计划 |
| auto-sized root 填满 viewport | Zed `b0da438545` | 不能机械保留或删除所有 `.size_full()` | 各 app 视觉回归 |
| `container_query` | Zed `49ad...` | 可按元素真实宽度布局，替代 viewport/chrome 常数 | Jaco 子计划 |
| `View` trait | Zed `74b5207744` | entity-backed props 可获得稳定身份；`Render`/`RenderOnce` 仍兼容 | gpui-form-gpui-component 子计划 |
| `WindowOptions::app_owns_titlebar_drag` | target public API | 调用 `start_window_move` 的 app 必须声明所有权 | Jaco、Feiwen 子计划 |
| focus listener frame fix | Zed `af7de9a03c` | 降低 draw 中焦点重定向问题，但不替代 app focus policy | Jaco/Feiwen 回归 |
| nested deferred popover fix | Zed `5e982c6bdc` | 改善嵌套 popover；不改变 `ListState` update 中 delegate 回调的重入边界 | Jaco 子计划保留 defer |

GPUI-owned delay 应使用 `cx.background_executor().timer(...)`；没有 GPUI context 的领域后台 retry 不强制迁移。

## 3. gpui-component 变化与 owner

| Upstream change | Evidence | Semantic impact | Owner plan |
| --- | --- | --- | --- |
| `ThemeToken` / `ThemeTokens` / gradient background | `ea6b194d`, #2484 | 背景不再等同于单一 `Hsla`；Deref 错误用法会静默丢 gradient | app-theme、Jaco、Feiwen、Novel |
| Aurora theme | #2487 | 本地 JSON snapshot 缺一份；preview 必须保留完整 Background | Jaco |
| GPUI/Taffy migration and wrapper removal | `03155566`, #2573 | Form/GroupBox/Checkbox/Radio 临时 wrapper 被上游删除 | 各 package 编译/布局回归 |
| Scrollable source/auto-height fixes | `dbf57ad9`, #2509；`52dfda33`, #2547 | source element 现在承载内容语义，wrapper 可能冗余或改变 gap/size | Jaco、Feiwen、HTTP |
| base component roles/aria | `f0abdd9f` | 复用官方组件可获得语义；不自动覆盖自定义 editor/picker | Jaco、Feiwen |
| `ComboboxState::set_selected_values` | `5b45bcb...`, #2576 | 用当前 delegate 进行 value 投影，不再需要旧 delegate/index cache | gpui-form-gpui-component |
| `InputContentType::Password` / `Url` | target source/docs | 明确语义输入可提供平台/可访问性提示 | Jaco、Feiwen、HTTP |
| `TextViewState` rapid-update coalescing | `e416af7f`, #2371 | 连续 `push_str` 会合并待解析更新并丢弃过期结果；直接影响流式 Markdown，而不只是编辑器输入 | Jaco |
| syntax highlighter / tree-sitter core refactor | `372446c0`, #2450；`3de68cd1`, #2567；`78095154`, #2557 | core parser 变为显式 feature；未知语言退化为 inert/plain text；Markdown source editor 的 fenced language injection 改进 | Jaco、HTTP |
| highlight consumers | target `crates/ui/src/input/element.rs`、`crates/ui/src/text/node.rs` | Input editor 每次 render 读取当前 `ActiveTheme.highlight_theme`；rendered Markdown `CodeBlock` 使用同一 schema，但 target 在 parse 时克隆 theme，而背景在 render 时读取 `Theme.tokens.muted` | app-theme 生成；TextView theme lifecycle 需上游修复；Jaco 只验证消费 |
| TextView theme snapshot/cache | target `text/state.rs:198-206,247-255,587-613`、`text/format/markdown.rs:398-402`、`text/node.rs:605-695` | 仅切换 ActiveTheme 不会触发 `set_text`/reparse；既有 CodeBlock 继续使用旧 `HighlightTheme` 和 styles cache，造成新背景配旧 syntax | `UPSTREAM-TEXT-15` release blocker；禁止在 Jaco 加局部 reparse/sync |
| editor color fallback | target renderer source | `editor_foreground`、`editor_line_number`、`editor_active_line_number` 尚无直接 renderer consumer；普通文本/行号实际使用 `Theme.foreground` / `Theme.muted_foreground` | app-theme 保持 colors/highlight 的 plain/muted 语义一致；应用不补 override |
| optional editor gutter | target `input/element.rs` | `editor_gutter_background = None` 时继承 editor background；只有显式 `Some` 才定义独立 gutter 表面 | app-theme 不生成同色 gutter override |

## 4. 上游复用结论

| Local implementation | Upstream capability | Decision | Owner |
| --- | --- | --- | --- |
| Feiwen 平台标题栏 shell | `gpui_component::TitleBar` | 删除平台 fork，保留业务内容组合 | Feiwen |
| Feiwen 手画 progress | `Progress` | 直接复用 | Feiwen |
| Jaco picker selectable row | `ListItem` | picker 视觉层复用；其余三类 row 保留既有无边框 selected 视觉；delegate/业务数据/defer 不变 | Jaco |
| Jaco controlled picker | 标准 Combobox 缺受控 open/group/footer 契约 | 保留 | Jaco |
| Jaco structured composer | 新 Input API 不覆盖 token/IME/skill/attachment/undo/completion | 保留 | Jaco |
| Jaco rendered Markdown code blocks | `TextViewState::markdown` -> `CodeBlock` -> `SyntaxHighlighter(lang)` | 保留官方渲染链路；单独验证语言 feature、流式 fence、主题色和 plain fallback | Jaco |
| app-owned Markdown/editor code palette | shared `ActiveTheme.highlight_theme` | 不新增；共享主题层一次生成 plain/muted/syntax，应用只消费 | app-theme |
| Jaco theme-change reparse/sync | target TextView parse-time theme snapshot | 禁止本地 workaround；上游把 theme 移到 render stage，并让 styles cache 按 theme identity 失效 | UPSTREAM-TEXT-15 |
| viewport theme-grid 推算 | `container_query` | 替换 | Jaco |
| `IntegerInput<N>: RenderOnce` | entity-backed `View` | 定向适配，不批量迁移 | gpui-form-gpui-component |
| `.bg(ThemeColor)` | `Theme.tokens` | 背景迁移；前景与颜色计算保留 Hsla | app-theme + affected apps |

## 5. 明确非目标与 No change

- 不采用 GPUI mobile/touch、`run_embedded`、native popup、exclusive zone、
  `request_attention`、`reduce_motion`、system-wake；当前产品没有对应需求。
- 不把全部 `RenderOnce` 改为 `View`，不机械删除全部 `.size_full()`。
- 不改变数据库 schema、Diesel migration、repository query、provider/MCP persistence 或 secret format。
- 不改变网络协议、认证、代理、缓存、超时、重试和离线策略。
- 不新增 icon 或用户可见 Fluent 文案。
- 不修改持久化 theme ID；theme preset 继续使用 `preset:<themes[].name>`。

## 6. 依赖验证

```bash
cargo tree --locked -i gpui@0.2.2
cargo tree --locked -d
cargo tree --locked -e features -p jaco
cargo tree --locked -e features -p feiwen
cargo tree --locked -e features -p http-client
cargo tree --locked -e features -p novel-download
```

完成证据：GPUI family 与 gpui-component family 各自只有一个 source SHA，feature graph 与 package 实际用途一致。
