# Novel Download GPUI / gpui-component 迁移：GPUI `1d217ee39d381ac101b7cf49d3d22451ac1093fe` → `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`；gpui-component `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` → `5b45bcb26b9343d91a123a4d5ed8a654360512e5`

## 1. 状态与范围

- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- 文档：`app/novel-download/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md`。
- 状态：workspace root 已使用 `ThemeToken::background`，领域 crawler timer 保持不变；
  `cargo test -p novel-download` 及 workspace 自动门通过。按约定未执行实际 UI 测试与打包。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` →
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` →
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 目标：让 workspace 背景使用可渲染的 `ThemeToken::background`，并对新 GPUI/Taffy
  下的输入、按钮和结果区做布局回归。
- 非目标：不迁移 crawler timer；不修改抓取协议、下载文件、状态机、input 语义或窗口 titlebar；
  不新增主题资产或颜色生成逻辑。

执行顺序：`NOVEL-10 -> NOVEL-20`。

## 2. 证据与决定

- `app/novel-download/src/features/workspace.rs` 的 root surface 当前使用
  `.bg(cx.theme().background)`，这会丢失 gpui-component 新 `ThemeToken` 的 renderable gradient。
- 目标 gpui-component 的 `ThemeToken` 同时持有代表 `Hsla` 与 `Background`；元素背景使用
  `cx.theme().tokens.background.background`。
- `app/novel-download/src/crawler/implement.rs` 的 `smol::Timer::after(duration)` 属于无 GPUI
  context 的领域重试等待，不是 GPUI entity/task 定时器；明确保留。
- 决定：只修改 workspace root 背景。crawler、window、input、button 和 fetch state 不做代码迁移。

## 3. 文件与 API 契约

**修改**

- `app/novel-download/src/features/workspace.rs`。

**明确保留且不修改**

- `app/novel-download/src/crawler/implement.rs`。
- `app/novel-download/src/main.rs`。

目标背景契约：

```rust,ignore
div()
    .track_focus(&self.focus_handle)
    .key_context("NovelDownload")
    .p_4()
    .size_full()
    .bg(cx.theme().tokens.background.background)
    .text_color(cx.theme().foreground)
```

- 背景读取 `ThemeToken.background`；foreground 仍使用顶层 `Hsla`。
- 不使用 `cx.theme().tokens.background.opacity(...)`；如未来需要透明度，必须调用
  `.background.opacity(...)`，避免经 `Deref<Target = Hsla>` 丢失渐变。
- `WorkspaceView` 字段、event、subscription、fetch task 和 focus path 全部不变。

保留 timer 契约：

```rust,ignore
smol::Timer::after(duration).await;
```

- owner 仍是 crawler retry loop；无 `Window`、`Context` 或 `BackgroundExecutor` 可注入。
- 不为了统一表面写法把领域 timer 迁到 GPUI executor，也不改变 retry duration/cancellation。

## 4. 工作包

### NOVEL-10：workspace ThemeToken

**Implementation flow**

1. 只把 workspace root 的 `ThemeColor::background` 读取改为
   `ThemeTokens::background.background`。
2. 保持 text foreground、padding、focus、key context、input/button 和状态结果顺序不变。
3. 全包扫描确认没有新增错误的 `ThemeToken::deref` 背景写法。

**Tests**

| Requirement | Evidence | Assertions |
| --- | --- | --- |
| renderable background | source gate + light/dark smoke | root surface 使用 `Background`，没有提前转换为代表 `Hsla` |
| interaction unchanged | manual smoke | 输入、发送、清空、重新 focus、loading/result 状态不变 |

### NOVEL-20：布局、timer 与完成门

**Layout/lifecycle matrix**

- 常规和窄窗口下 input、send button、状态文字与最近五条 chapter link 不溢出或消失。
- light/dark 主题下背景、foreground 和 link 可读；背景值一路保持 renderable `Background`。
- crawler retry 仍通过 `smol::Timer::after` 等待，duration 和错误传播不变。

**Validation**

```bash
cargo fmt --all -- --check
git diff --check
cargo test --locked -p novel-download
cargo clippy --locked -p novel-download --all-targets --all-features -- -D warnings
rg -n '\.bg\(cx\.theme\(\)\.' app/novel-download/src -g '*.rs' | rg -v '\.tokens\.'
rg -n 'smol::Timer::after' app/novel-download/src/crawler/implement.rs
```

第一个 `rg` 预期无输出；第二个必须命中 crawler timer，证明本迁移没有误删领域等待。

**Done condition**

- workspace root 通过 `ThemeToken.background` 绘制；交互/布局 smoke 通过；crawler timer 保持原样；
  除 `features/workspace.rs` 外无 Novel Download 实现改动。

## 5. No-change surfaces

- crawler/network：No change。解析、HTTP、retry、error mapping 与 `smol::Timer` 不改。
- file persistence：No change。下载目录、文件命名、append/write 行为不改。
- state/lifecycle：No change。`FetchState`、`WorkspaceEvent`、detached task 与 weak entity 更新不改。
- input/focus：No change。该输入是 novel ID，不声明 URL content type；发送后清空与 focus 不改。
- window/titlebar：No change。继续使用 native-compatible `TitlebarOptions`，没有 app-owned drag。
- database/cache：No change。
- i18n/icons/assets：No change。无新文案、图标或主题资产。
- dependencies：No manifest/lockfile edit；本计划消费已经锁定的两个目标 SHA。
