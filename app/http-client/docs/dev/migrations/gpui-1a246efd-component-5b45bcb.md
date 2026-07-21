# HTTP Client GPUI / gpui-component 迁移：GPUI `1d217ee39d381ac101b7cf49d3d22451ac1093fe` → `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`；gpui-component `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` → `5b45bcb26b9343d91a123a4d5ed8a654360512e5`

## 1. 状态与范围

- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- 文档：`app/http-client/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md`。
- 状态：HTTP Client 应用级适配与自动化验证已完成；按约定未执行 UI 验收。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` →
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` →
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 目标：为 request URL 输入声明 `InputContentType::Url`，并验证 GPUI/Taffy/Input 更新没有破坏
  request header、tabs、params/headers/body 的布局和双向 URL 投影。
- 非目标：不重构 HTTP 表单、URL/params 双向数据流、请求发送、header/body editor、tab 或
  popover；不做主题颜色迁移；不引入新状态或 wrapper。

执行顺序：`HTTP-10 -> HTTP-20`。

## 2. 证据与决定

- `app/http-client/src/features/request/url_input.rs` 在 render 中使用
  `Input::new(&self.input)`；form 从 params 更新 URL 时会重建 `Entity<InputState>`，但 render
  builder 每帧应用到当前 entity。
- 目标 gpui-component 提供
  `Input::content_type(InputContentType::Url)`；这是 native/autofill/accessibility hint，不改变
  text value、mask、change event 或 validation。
- `app/http-client/src/features/request.rs` 由 method select、URL input、send button 和 tab body
  构成；Taffy/root-fill 行为变化需要回归，但当前结构不需要预先新增尺寸 wrapper。
- 决定：唯一代码改动是 URL input render；其余文件仅作为受影响布局和数据流验收面，不做计划内
  结构修改。

## 3. 文件与 API 契约

**修改**

- `app/http-client/src/features/request/url_input.rs`。

**验证但不修改**

- `app/http-client/src/features/request.rs`：method/URL/send header 与 main flex layout。
- `app/http-client/src/features/request/tab.rs`：params/headers/body tab body 尺寸。
- `app/http-client/src/features/request/params.rs`：URL ↔ query params 投影与 popover。
- `app/http-client/src/features/request/headers.rs`：header rows。
- `app/http-client/src/features/request/body.rs` 及其现有 child modules：body tabs/editors。

目标 render 契约：

```rust,ignore
impl Render for UrlInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Input::new(&self.input).content_type(InputContentType::Url)
    }
}
```

- 从 `gpui_component::input` 导入 `InputContentType`。
- `UrlInput` 的字段、constructor、subscriptions 和 `HttpFormEvent` 不改。
- `SetUrlByParams` / `SetUrl` 重建 input 后仍获得 URL content type，因为语义位于 render builder。
- 不给 params key/value、header、body、method 或其他普通文本输入设置猜测性的 content type。

## 4. 工作包

### HTTP-10：URL input 语义

**Implementation flow**

1. 在唯一 URL input 的 render builder 增加 `InputContentType::Url`。
2. 保持 `InputEvent::Change -> HttpFormEvent::SetUrlByInput` 与 form/params 反向更新原样。
3. 不移动 content type 到 `InputState`，不因 form 更新重新设计 entity 生命周期。

**Tests**

| Requirement | Evidence | Assertions |
| --- | --- | --- |
| URL content semantics | upstream gpui-component tests + local manual smoke | native/accessibility URL hint 生效，value/change 不变 |
| URL ↔ params | existing/manual request flow | URL 输入更新 params；params 更新后重建 input 且仍显示正确 URL |

本地 `Input` 没有公开读取 content type 的 getter；不得为测试暴露 app-only state。该行为由上游
`content_types_map_to_accessibility_roles` 单测和本地 accessibility/native smoke 覆盖。

### HTTP-20：布局回归与完成门

**Layout matrix**

- 常规和窄窗口下 method select、URL input、send button 不重叠或溢出。
- Params、Headers、Body 三个 tab 可切换，内容尺寸和已有滚动行为不变。
- query-param add popover 可打开、输入、确认并关闭；长参数列表不破坏主布局。
- params 修改后 URL input 仍在同一 header 位置显示完整的新值。

**Validation**

```bash
cargo fmt --all -- --check
git diff --check
cargo test --locked -p http-client
cargo clippy --locked -p http-client --all-targets --all-features -- -D warnings
git diff -- app/http-client/src/features/request/url_input.rs
```

最后一个 diff 的预期功能变化只有 import 与 `.content_type(InputContentType::Url)`；若其他
HTTP Client source 出现修改，必须从本迁移中移除或单独立项。

**Done condition**

- URL native/accessibility 语义生效；URL/params 双向行为和 request layout matrix 通过；除
  `url_input.rs` 外无 HTTP Client 实现改动。

## 5. No-change surfaces

- 数据请求：No change。request send 仍为现有实现状态；不增加 endpoint、auth、timeout、retry。
- form/state：No change。`HttpForm`、`HttpFormEvent`、subscriptions 与 entity ownership 不改。
- validation/parsing：No change。继续由现有 `url::Url` params 路径处理；content type 不验证 URL。
- persistence/database/cache：No change。
- theme/colors/assets/icons：No change。
- i18n：No change。无新增用户可见文案。
- platform/window/titlebar：No change。HTTP Client 使用现有 titlebar，不调用 app-owned drag。
- dependencies：No manifest/lockfile edit；本计划消费已经锁定的两个目标 SHA。
