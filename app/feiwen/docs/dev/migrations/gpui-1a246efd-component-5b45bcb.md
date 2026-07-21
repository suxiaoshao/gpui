# Feiwen GPUI / gpui-component 迁移：GPUI `1d217ee39d381ac101b7cf49d3d22451ac1093fe` → `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`；gpui-component `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` → `5b45bcb26b9343d91a123a4d5ed8a654360512e5`

## 1. 状态与范围

- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- 文档：`app/feiwen/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md`。
- 状态：owned titlebar、官方 `TitleBar`/`Progress`、URL content type、ThemeToken 背景与
  Scrollable 接入已完成；`cargo test -p feiwen` 的 67 个测试及 workspace 自动门通过。
- 按本轮约定，实际 UI/Computer Use 与三平台标题栏 smoke 未执行，也未打包。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` →
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` →
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 目标：修正自绘标题栏所有权契约；用官方 `TitleBar` 和 `Progress` 删除重复平台/UI
  实现；给抓取 URL 输入增加语义；把 Feiwen 自定义背景迁移到 `ThemeToken`；验证新版
  Scrollable/Taffy 布局与三个桌面平台。
- 非目标：不修改抓取、查询、DuckDB、路由状态和错误恢复语义；不重写表单或输入状态；不采用
  与上述目标无关的新组件。

执行顺序：`FEIWEN-10 -> FEIWEN-20 -> FEIWEN-30 -> FEIWEN-40`。

## 2. 证据快照与决定

| 变化 | 当前实现 | 目标决定 |
| --- | --- | --- |
| GPUI owned titlebar | `app/feiwen/src/main.rs` 打开透明自绘标题栏；本地标题栏调用 `start_window_move` | `WindowOptions::app_owns_titlebar_drag = true` |
| 官方标题栏 | `app/feiwen/src/app/titlebar.rs` 自行维护 drag、双击、Linux 菜单和 Windows/Linux 控件 | 官方 `gpui_component::TitleBar` 拥有平台 shell；Feiwen 只拥有路由、摘要和操作内容 |
| 官方进度条 | `app/feiwen/src/features/fetch.rs::progress_bar` 手画 track/fill | `gpui_component::progress::Progress`；输入为 `0.0..=100.0` |
| URL content type | 抓取 URL 使用普通 `Input` | `Input::content_type(InputContentType::Url)`；不改变 value、validation 或 masking |
| renderable theme token | 五个 Feiwen 文件直接把顶层 `Hsla` 传给 `.bg(...)` | 背景使用 `cx.theme().tokens.<role>.background`；文字和边框继续用 `Hsla` |
| Scrollable/Taffy | 高级查询 filter/sort 容器直接使用 `overflow_y_scrollbar()` | 保留直接滚动结构，不新增或删除 wrapper；只做新版布局回归 |

上游 API 已在目标 source 验证：

- GPUI `WindowOptions::app_owns_titlebar_drag`：应用调用 `Window::start_window_move` 时必须启用。
- `gpui_component::TitleBar`：提供拖动、平台菜单、双击、窗口控件、主题背景和可组合 child。
- `gpui_component::progress::Progress::{new,value}` 与 `Sizable::with_size`：百分比值自动 clamp
  到 `0..=100`，渲染 `ProgressIndicator` accessibility role，并使用 `tokens.progress_bar`。
- `gpui_component::input::InputContentType::Url` 与 `Input::content_type`：只设置 native/autofill
  与 accessibility 语义。
- `ThemeToken` 同时提供代表 `Hsla` 与可渲染 `Background`；背景必须使用后者以保留渐变。

## 3. 目标组成与 API 契约

```text
WindowOptions { app_owns_titlebar_drag: true }
  -> FeiwenTitleBar (route/title/summary/actions owner)
       -> gpui_component::TitleBar (drag/menu/window controls/theme owner)

FetchProgress
  -> progress_percent(&FetchProgress) -> f32
  -> Progress::new("fetch-progress").value(percent).with_size(px(8.))
```

`app/feiwen/src/main.rs` 新增私有 helper，固定可测试的窗口契约：

```rust,ignore
fn main_window_options(title: impl Into<SharedString>) -> WindowOptions;
```

- 返回值保留 `WindowBackgroundAppearance::Blurred`、透明 titlebar、14px traffic-light 位置。
- 返回值显式设置 `app_owns_titlebar_drag: true`；不得通过 `is_movable = false` 绕过。

`FeiwenTitleBar` 保留当前构造函数、字段、route/summary/action helpers 和 `RenderOnce`，但其
`render` 只组合：

```rust,ignore
gpui_component::TitleBar::new()
    .h(FEIWEN_TITLE_BAR_HEIGHT)
    .pl(FEIWEN_TITLE_BAR_LEFT_PADDING)
    .child(/* existing leading + center + trailing content */)
```

- 保留 `FEIWEN_TITLE_BAR_HEIGHT = 44px`、`FEIWEN_TRAFFIC_LIGHT_INSET = 14px` 和
  `traffic_light_position()`。
- interactive route tabs/buttons 继续停止 mouse-down 传播，不能触发 window drag。
- 删除 `TitleBarState`、`ControlIcon`、`WindowControls`、`WINDOW_CONTROL_WIDTH` 以及本地
  drag/double-click/right-click/platform-control 实现。
- 官方非 macOS 控件尺寸作为目标行为；不得恢复整份平台 fork。

`app/feiwen/src/features/fetch.rs` 新增：

```rust,ignore
fn progress_percent(progress: &FetchProgress) -> f32;
```

- 分母使用 `page_count().max(1)`，分子使用
  `min(completed_pages, page_count)`，结果乘以 `100.0`；无成功页为 `0.0`，完成或越界为
  `100.0`。
- 删除 `progress_bar(progress, cx) -> Div`；`Progress` 只消费现有状态，不拥有 fetch task。
- URL 渲染固定为
  `Input::new(&self.url_input).content_type(InputContentType::Url).disabled(is_running)`。

背景映射固定如下：

| 文件/旧背景 | 新背景 |
| --- | --- |
| `features/fetch.rs`: `background` | `tokens.background.background` |
| `features/fetch.rs`: danger row 6% | `tokens.danger.background.opacity(0.06)` |
| `features/query.rs`: accent 35% | `tokens.accent.background.opacity(0.35)` |
| `features/query/advanced/render.rs`: `table_head` | `tokens.table_head.background` |
| `features/query/advanced/render.rs`: accent 18%/25%/solid | `tokens.accent.background.opacity(...)` / `tokens.accent.background` |
| `features/query/advanced/sort.rs`: `background` | `tokens.background.background` |

`TitleBar` 和 `Progress` 自己消费官方 token；应用不得在外层复制其 track、fill、titlebar 或
窗口控件颜色。

## 4. 上游复用与删除

| 本地实现 | 上游能力 | 决定 | 删除/保留 | 回归门 |
| --- | --- | --- | --- | --- |
| 平台标题栏 shell | `TitleBar` | Reuse directly | 删除本地平台实现；保留 Feiwen 业务内容 | 三平台 drag/menu/control smoke |
| 手画进度条 | `Progress` | Reuse directly | 删除 helper | 0/中间/100、aria、动画 |
| 高级查询滚动容器 | 新 Scrollable/Taffy 语义 | Retain | 保留直接 `overflow_y_scrollbar()`，不增加 wrapper | filter/sort 独立滚动且不溢出 |
| 路由 tabs、摘要、actions | 上游无 Feiwen 领域状态 | Retain | 保留 | 切换与 disabled/loading 状态不变 |

## 5. 工作包

### FEIWEN-10：窗口与官方标题栏

**Files**

- 修改 `app/feiwen/src/main.rs`。
- 修改 `app/feiwen/src/app/titlebar.rs`。

**Implementation flow**

1. 提取并使用 `main_window_options`，设置 owned-titlebar flag。
2. 将 `FeiwenTitleBar::render` 的外壳替换为官方 `TitleBar`，保持三个业务区域和交互阻断。
3. 删除本地平台状态、图标和 controls；收紧 imports。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| 窗口契约 | `app/feiwen/src/main.rs` | `main_window_owns_custom_titlebar_drag` | flag=true，blur/title/traffic-light 不变 |
| route contract | `app/feiwen/src/app/titlebar.rs` | 保留现有 route/title tests | index/title mapping 不变 |
| 平台行为 | manual/CI | `feiwen_titlebar_platform_smoke` | 见 FEIWEN-40 |

### FEIWEN-20：Progress 与 URL 语义

**Files**

- 修改 `app/feiwen/src/features/fetch.rs`。

**Implementation flow**

1. 添加纯 `progress_percent` 并用官方 `Progress` 替换手画进度条。
2. 为唯一抓取 URL `Input` 设置 `InputContentType::Url`；cookie 和页码输入保持不变。
3. 不改变 fetch task、interrupt/resume/retry、日志或状态流。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| progress domain | `features/fetch.rs` | `progress_percent_clamps_to_component_range` | 无成功页=0，middle正确，overrun=100 |
| existing flow | `features/fetch.rs` | 现有 fetch state tests | interrupted/resume/failure/success 不变 |
| URL semantics | upstream + manual | `fetch_url_content_type_smoke` | URL role/autofill hint；输入值与校验不变 |

### FEIWEN-30：ThemeToken 与 Scrollable 布局

**Files**

- 修改 `app/feiwen/src/features/fetch.rs`。
- 修改 `app/feiwen/src/features/query.rs`。
- 修改 `app/feiwen/src/features/query/advanced/render.rs`。
- 修改 `app/feiwen/src/features/query/advanced/sort.rs`。
- `app/feiwen/src/app/titlebar.rs` 的背景由官方 `TitleBar` 接管，不另写 app token。

**Implementation flow**

1. 按第 3 节映射所有 app-owned `.bg(cx.theme().*)`。
2. opacity 只调用 `ThemeToken.background.opacity`，禁止调用
   `ThemeToken::deref` 后的 `Hsla::opacity`。
3. 保留 filter/sort 的直接滚动 source、`flex_1`、`min_h_0`、padding/gap；不添加兼容 wrapper。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| token residual | source gate | `feiwen_background_token_gate` | app-owned background 不再读取顶层 Hsla |
| advanced scroll | manual | `advanced_query_scroll_smoke` | filter/sort 长内容独立滚动，header 固定，无裁切 |
| renderable background | source gate + manual | `feiwen_background_token_smoke` | 背景一路保持 `Background`；system-accent light/dark 绘制正确 |

### FEIWEN-40：验证与完成门

**Validation**

```bash
cargo fmt --all -- --check
git diff --check
cargo test --locked -p feiwen
cargo clippy --locked -p feiwen --all-targets --all-features -- -D warnings
rg -n '\.bg\(cx\.theme\(\)\.' app/feiwen/src -g '*.rs' | rg -v '\.tokens\.'
rg -n 'tokens\.[a-zA-Z0-9_]+\.opacity\(' app/feiwen/src -g '*.rs'
```

两个 `rg` 预期无输出。三平台 smoke：

- macOS：traffic lights 保持 14px；空白处 drag、双击标题栏、route/action 点击均正确。
- Linux：右键系统菜单、drag、双击 maximize、minimize/maximize/close 正确。
- Windows：系统 control hit area、drag、双击、minimize/maximize/close 正确。
- 全平台：fetch progress 的 0/中间/100、URL 输入、disabled 状态、system-accent light/dark 背景，
  以及高级查询 filter/sort 小窗口滚动均无回归。实际 gradient end-to-end 由 Jaco Aurora 发布门负责；
  Feiwen 通过 source gate 保证没有提前降级 `Background`。

**Done condition**

- 本地平台标题栏与手画进度条实现已删除；owned-titlebar flag、官方组件、URL 语义和 token
  背景完成；自动验证与三个平台 smoke 全部通过。

## 6. No-change surfaces

- 数据库：No change。DuckDB pool、schema、query、transaction 和 repository 方法不改。
- 网络：No change。URL、cookie、Reqwest client、timeout/retry/offline 行为不改。
- 状态与生命周期：No change。`FetchTaskState`、task ownership、interrupt/resume/retry 和 route
  state 不改；`Progress` 不创建 task。
- persistence：No change。无配置、数据库或文件格式迁移。
- i18n：No change。无新增用户可见文案或 Fluent key。
- icons/assets：No change。官方 `TitleBar`/`Progress` 不要求新增 app asset。
- accessibility：仅获得官方 TitleBar/Progress 语义及 URL content type；不扩大为全应用 a11y 重构。
- dependencies：No manifest/lockfile edit；本计划消费已经锁定的两个目标 SHA。
