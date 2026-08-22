# Jaco 依赖升级计划

- 状态：`In progress`（本地自动化通过；跨版本 E2E 与三平台/人工 smoke 待执行）
- Owner：`app/jaco`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)
- 本文只拥有 Jaco 的升级适配；workspace 版本、Git SHA 与 lockfile 由 root plan 统一拥有。

## Owner scope

Jaco 保留完整 `gpui-component`，不迁移到 `gpui-base`。本 owner 负责 Jaco 的组件初始化、
主题写入、输入控件、标题栏、组件 re-export 兼容和 app 级回归验证；`app-theme` 与
`gpui-form-gpui-component` 的公共修复由对应 crate owner 交付。

## 精确依赖与已知命中

`app/jaco/Cargo.toml` 当前直接依赖：

- `app-assets`（54）、`app-theme`（55-58）、`gpui`（59）、`gpui-form`（60）、
  `gpui-form-gpui-component`（61）、`gpui_platform`（64）、`gpui-component`（65）、
  `gpui-component-assets`（66）、`window-ext`（76）、`gpui-tokio`（100）；
- feature `tree-sitter-languages-basic/full` 在 25-45 行直接转发 `gpui-component` 的
  tree-sitter features；dev dependency 在 119 行开启 `gpui/test-support`。

本 owner 还承接以下 root 已审计的 direct 目标：

| Dependency | Current | Target | Jaco 风险面 |
| --- | --- | --- | --- |
| `thiserror` | 2.0.19 | 2.0.20 | app error derive 与 source chain |
| `time` | 0.3.54 | 0.3.55 | 配置/数据库时间序列化与本地格式化 |
| `async-trait` | 0.1.91 | 0.1.92 | provider/tool async trait object |
| `http` | 1.4.2 | 1.5.0 | MCP/provider request 类型 |
| `xcap` | 0.9.7 | 0.9.8 | Windows/macOS 屏幕捕获与附件 |
| `winresource` | 宽范围 `0.1` | 完整版本 `0.1.31` | Windows build script/resource |

`rig 0.41.0 -> 0.42.0` 的实现 owner 是 `crates/jaco-agent`，但 Jaco 是最终 app consumer；root
workspace 的 `rmcp` 保持 `2.2.0`，不能在本应用升级到 3.x。独立 test server 才升级到
`rmcp 3.1.4`，并由 Jaco 2.2 client 做协议互通验收。

这些依赖继续指向 root plan 选定的统一 GPUI family；Jaco 不声明 `gpui-base`。升级后的
`gpui-component::init` 已内部调用 `gpui_base::init`，因此 `app/jaco/src/app.rs:154` 保持唯一
初始化入口，不额外调用 base init。`gpui-tokio` dependency key 则改指同一 Zed target 的上游
`gpui_tokio` package；现有 Rust imports 保持，应用自身继续显式提供实际使用的 Tokio features。

已确认的源码命中：

- `app/jaco/src/state/theme.rs:115` 通过 `Theme::global_mut(cx).apply_config(&config)` 直接修改
  global theme；新组件将 base theme 保存为独立投影，这里必须随后调用 `Theme::sync_base(cx)`。
- `app/jaco/src/features/settings/prompts/dialog.rs:94-101` 用
  `InputState::new(...).multi_line(true)` 构造正文，升级后应使用 `FormTextarea` +
  `TextareaState::new`，并在 250 行改为 `Textarea::new`；名称字段继续使用 `FormInput`。
- 同文件测试 helper `dialog.rs:724-731` 固定为 `Entity<InputState>`，要为 textarea 状态建立
  对应 helper/断言，保留 change/blur 的 form binding 覆盖。
- `app/jaco/src/app.rs`、`features/settings.rs` 与 `features/about.rs` 的 component TitleBar 窗口以目标
  `TitleBar::window_options()` 为 base，删除三处手写 `app_owns_titlebar_drag = true`；temporary/screenshot
  等不渲染 component TitleBar 的窗口保持原 options。
- `app/jaco/src/app/title_bar_menu.rs` 是上游 `menu::AppMenuBar` 的本地 fork；迁移三个 consumer 后删除本地
  actions、menu state、popup 递归和 trigger，只保留带 app icon 的 `title_bar_leading` wrapper。
- `app/jaco/src/features/home/sidebar/search.rs` 手写了 query Input、List delegate、selection、键盘和 confirm
  helpers。目标组件新增 `Command`，按
  [root 复用审计](../../../../../docs/dev/issue-205/accessibility-and-command-reuse-audit.md)
  做适配式替代；数据库 search、`refresh::Operation`、stale/error/retry 和 ConversationId 映射继续归 Jaco。
  必须使用 `filterable(false)`，否则项目或消息正文命中会被 title-only 本地过滤错误隐藏。
- `app/jaco/src/components/picker.rs` 的列表型 picker 与目标 `Combobox/SearchableList` 大量重叠；目标新增
  query accessor 后已能表达动态 catalog 下的 query 保持。该大迁移只记录解除条件，未另立 follow-up issue 前
  保持 `Defer`，不阻塞依赖升级；arbitrary-content/controlled-open popover 和领域 value projection 不能直接删除。
- `app/jaco/src/state/theme.rs` 在同步 component/base theme 后，还应把 Light/Dark 映射到
  `App::set_window_appearance`，System 映射为 `None`；系统主题观察与 `platform-ext` accent 继续保留。
- timeline 的 `pause_following_tail()` 与 composer blink 的 synced animation 是可选收薄点，但都有产品行为
  差异，本批只记录 spike/测试条件，不机械替换。
- `IndexPath`、`Disableable`、`Selectable`、`StyledExt`、`h_flex`、`v_flex` 等已迁到
  `gpui-base`，但目标 `gpui-component` 继续 re-export；Jaco 的现有
  `gpui_component::...` imports 不做无收益改写。

## 工作包

### JACO-DEP-1：消费统一依赖批次

- 保留上述直接依赖和 feature forwarding，更新后先编译默认与 full tree-sitter feature 图。
- 确认依赖图只有一个 `gpui`/`gpui_platform` Git 身份和一个 longbridge 组件身份。
- 消费 Zed `gpui_tokio` 并验证 MCP/network/time/drop-cancel；不得依赖 HTTP Client 提供 Tokio feature union。
- 不新增 `gpui-base` direct dependency，也不改变 `gpui_component::init` 调用语义。

### JACO-DEP-2：修复行为性 API 变化

- 在 Jaco 自有 global theme 写入后调用 `Theme::sync_base(cx)`；依赖 `app-theme` owner 对公共
  system-accent 写入做同样修复。
- 将显式 Light/Dark/System 选择同步到 native window appearance；不以此删除系统 accent observer。
- 等 `gpui-form-gpui-component` 提供 `FormTextarea` 后迁移 prompt content；不要把
  `InputState` 包装成兼容 shim，也不要复制 form binding 生命周期。

### JACO-DEP-3：删除重复 UI 基础设施并做窗口回归

- 用上游 `AppMenuBar` 替代本地 fork，并用 `TitleBar::window_options()` 统一三个 component TitleBar window；
  保持 `Root`、通知、dialog、通用 list/select 和自定义 composer 的完整组件实现。
- 会话搜索删除 app-local Input/List/键盘样板层，改由 `CommandState` + dynamic `CommandItem` 承担；保留外部
  search operation、错误重试和业务 identity。验证两段 Escape、选择更新和消息正文命中。
- Picker 只保存 consumer/contract inventory，本依赖批次保持 `Defer`，不进行 940 行级产品重构。
- 审计自绘与 icon-only 交互的 accessible name；不能以 target component 已写部分 role 作为完成证据。
- 验证自绘标题栏拖动区、系统 accent 切换、scrollbar/resize handle 与 theme radius/color
  同步；确认 re-export imports 仍编译。

### JACO-DEP-4：非 GPUI 与 agent consumer 回归

- 对 `thiserror`、`time`、`async-trait`、`http` 按 error chain、时间 round-trip、provider
  async dispatch 与 MCP request 边界运行 focused tests。
- Windows/macOS 分别验证 `xcap` 屏幕捕获；Windows CI 同时构建 `winresource 0.1.31` 的
  executable resources。
- 消费 Rig 0.42 migration：覆盖 unary/streaming、hooks、tool identity、持久化、error mapping
  与 cancellation；确认 app 不直接持有另一个 RMCP type universe。
- 使用独立 `rmcp 3.1.4` test server 跑 bearer、OAuth/refresh、tool list/call 与 shutdown E2E，
  证明 root `rmcp 2.2.0` client 的 legacy protocol negotiation 仍可用。

## Focused verification

```text
cargo check -p jaco --locked
cargo check -p jaco --no-default-features --features tree-sitter-languages-full --locked
cargo test -p jaco --locked
cargo test -p jaco features::home::sidebar::search::tests --locked
cargo test -p jaco-agent --all-features --locked
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
cargo tree -p jaco --duplicates --locked
cargo tree -i gpui-base --locked
cargo tree -p jaco -i gpui_tokio --locked
cargo tree -p jaco -i rmcp --locked
```

手工 smoke：启动 Jaco，切换 light/dark 与 system-accent 主题，打开 Prompt 编辑 dialog 并
验证多行输入、保存、blur validation；在 Windows/macOS/Linux 检查 titlebar 拖动、scrollbar
和 resize handle 颜色/圆角。会话搜索另验证 title/project/message-body 命中、键鼠选择、两段 Escape、
loading/error/retry，以及 Narrator/VoiceOver/Orca 对搜索框和高亮结果的播报。

## 完成条件

- Jaco 保持完整组件能力且没有 direct `gpui-base`。
- 两个 direct theme mutation 路径（Jaco 自有路径与共享 `app-theme` 路径）均同步 base 投影。
- Prompt textarea、tree-sitter feature 图、标题栏和核心测试全部通过。
- 本地 AppMenuBar fork 已删除，三个窗口使用上游 options helper；Zed `gpui_tokio` 的 drop-cancel/network/time
  回归通过且没有本地 path bridge。
- 会话搜索只保留业务 owner，通用交互已适配到 `Command`；没有二次过滤回归，并完成明确的 a11y 实测记录。
- 非 GPUI direct 目标、Rig 0.42 app consumer tests 与 RMCP 2 client/3.1.4 server E2E 通过。
