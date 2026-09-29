# Issue #182：`gpui::View` 迁移记录

## 状态与范围

- 状态：`历史记录`
- 审计状态：保留 2026-07-31 的共享与现役应用证据；本次已移除 Jaco 专用边界
- 跟踪 issue：[#182](https://github.com/suxiaoshao/gpui/issues/182)
- 根索引：[README.md](README.md)
- 判断标准：[view-migration-criteria.md](view-migration-criteria.md)
- 源码基线：`510cd2e371846faca429279d4877fdcfb808cd0e`
- 快照日期：`2026-07-31`
- 已审计源码范围：`app/` 和 `crates/` 下的生产 Rust 代码
- 不纳入迁移记录：仅用于测试的测试框架和第三方依赖源码

本记录保留固定源码基线的迁移判断；不是当前工作区的重新审计，也不自动授权后续实现。Jaco 专用候选与待定项已退役。

## 分类

| 分类 | 含义 |
| --- | --- |
| `必须迁移` | 当前证据满足标准，该边界必须迁移为直接 `View` |
| `倾向迁移` | 形态和同步证据指向迁移，但仍有一个明确的所有权、标识或生命周期事实需要确认 |
| `待定` | 当前代码存在相互竞争的数据权威，或标识/通知契约不明确 |
| `保留 Render` | 该类型仍是一个内聚的、由实体持有的长生命周期控制器 |
| `保留 RenderOnce/函数` | 该边界确实无状态，或者仅用于拆分现有所有者的渲染逻辑 |
| `现有 View` | 该边界已经遵循直接 `View` 模型，保留为参考证据 |

## 汇总

下方计数统计的是可渲染边界，而不是估算的文件数或实现工作包数。一个分组族中，每个需要独立迁移的组件分别计数；下表仅统计保留的应用与共享组件。

| 归属 | 必须迁移 | 倾向迁移 | 待定 | 现有 View | 保留项覆盖 |
| --- | ---: | ---: | ---: | ---: | --- |
| `app/feiwen` | 1 | 0 | 1 | 0 | 见下方汇总 |
| `app/http-client` | 0 | 0 | 4 | 0 | 见下方汇总 |
| `app/novel-download` | 0 | 0 | 0 | 0 | 见下方汇总 |
| 共享 crate | 0 | 0 | 0 | 1 | 见下方汇总 |
| **合计** | **1** | **0** | **5** | **1** | — |

## 必须迁移项实施进度

| 边界 | 稳定后备状态 | 当前状态 | 已验证的关键约束 |
| --- | --- | --- | --- |
| `NumericRangeInput` | `NumericRangeInputState` | `已迁移` | 后备身份契约、范围解析语义 |

实施验证：

- `cargo test -p feiwen --offline`：`69` 项通过。
- `cargo clippy -p feiwen --all-targets --all-features --offline -- -D warnings`：通过。
- `cargo fmt --all --check` 与 `git diff --check`：通过。

## 必须迁移项

### `app/feiwen`：`NumericRangeInputState`

- 分类：`必须迁移`
- 当前形态：
  [`NumericRangeInputState: Render`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/components/numeric_range_input.rs#L18)
  同时存储两个嵌套的 `InputState` 实体，以及 `min_label`、`max_label` 和
  `disabled`。
- 稳定的后备标识：
  每个条件会创建一个 `NumberValue::Range(Entity<NumericRangeInputState>)`，
  并将其保留在条件树中
  （[state.rs:100](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/state.rs#L100),
  [state.rs:362](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/state.rs#L362)）。
  同级范围条件不会共享该实体。
- 父级持有的属性：两个标签和 `disabled`。尤其是父级的 `searching` 值已经传到
  `render_number_value`，但范围分支忽略了它，直接渲染实体
  （[render.rs:532](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/render.rs#L532)）。
- 持久组件状态：最小值/最大值输入框实体，以及它们的文本、焦点、选择
  和编辑状态。
- 当前同步方式：搜索开始/结束时调用 `AdvancedQueryState::set_disabled`，
  递归遍历每个条件，然后调用 `NumericRangeInputState::set_disabled` 并通知子组件
  （[query.rs:260](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query.rs#L260),
  [state.rs:712](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/state.rs#L712),
  [numeric_range_input.rs:42](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/advanced/components/numeric_range_input.rs#L42)）。
  这是将父级展示属性镜像到子组件状态的行为。
- 要求的目标边界：一个由现有范围状态实体作为后备状态的临时
  `NumericRangeInput` 直接 `View`。它的 `entity_id()` 必须返回该范围状态
  的 ID；标签和 `disabled` 作为每次重建的属性。
- 迁移后移除的同步：范围专用的 `set_disabled` 字段/设置方法，以及仅用于镜像搜索
  状态的递归子组件更新。后备状态继续持有两个输入框。
- 定向验证：父级重新渲染时保留最小值/最大值文本和焦点；不通过子组件设置方法
  更新禁用状态；证明同级标识唯一；保留占位文案以及
  `Missing`/`Invalid`/`Reversed` 解析行为。

## 待定项

### 所有权或组件 API 关卡尚未解决

#### Feiwen 结果表格边界

- 证据：
  [`QueryView::render_results_table`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query.rs#L352)
  渲染一个稳定的 `TableState<ResultsTableDelegate>`，而搜索状态通过
  `set_loading` 和 `table.refresh` 被复制进去
  （[query.rs:324](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query.rs#L324),
  [results_table.rs:40](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/feiwen/src/features/query/results_table.rs#L40)）。
- 阻塞事实：锁定版本的 `DataTable` 只通过 `TableDelegate::loading` 使用加载状态；
  尚未确认存在渲染时加载状态属性。由于委托会对数据行排序，数据行也成为
  委托持有的数据。
- 解决后的验证：更新加载状态时不重置排序、列宽/顺序、选择或
  滚动；只存在一个数据权威；并且不再有 `set_loading`/`refresh` 镜像。

#### HTTP 客户端镜像族

以下四个边界具有强烈的迁移信号，但尚不能满足稳定标识、父级重建和
减少同步关卡。

| 边界 | 当前镜像与阻塞事实 | 所需验证 |
| --- | --- | --- |
| [`UrlInput`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/url_input.rs#L9) | 输入框变更会发送表单 URL 事件；参数/表单 URL 变更会替换整个 `InputState` 实体（[url_input.rs:43](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/url_input.rs#L43)）。这种替换既不具备稳定标识，也没有经过证明的通知/重建路径。 | 声明表单的数据权威；保留一个编辑器标识；防止反馈循环；参数重写 URL 时保留焦点/IME/选择。 |
| [`HttpParamsView`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/params.rs#L22) | 查询输入框编辑会重写 `HttpForm.url`；URL 事件会重建所有数据行输入框实体（[params.rs:121](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/params.rs#L121)）。数据行按索引标识，没有稳定的领域键。 | 定义数据行标识和数据权威；保留弹出层/新增输入框、Enter 导航、焦点、新增/删除/重排索引，以及外部 URL 替换行为。 |
| [`HttpTextView`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/body/http_text.rs#L59) | 编辑器变更会将文本写入正文表单；`SetTextType` 会调用 `InputState::set_highlighter`（[http_text.rs:98](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/body/http_text.rs#L98)）。尚未确立渲染时语法高亮器属性或父级重建路径。 | 决定语言是属性，还是显式保留的状态投影；保留文本、焦点、选择、搜索和语法高亮。 |
| [`XFormView`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/body/x_form.rs#L107) | 键/值编辑会写入正文表单；新增/删除会追加或重建按索引标识的子输入框（[x_form.rs:152](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/app/http-client/src/features/request/body/x_form.rs#L152)）。数据行标识和通知所有权不明确。 | 定义稳定的数据行标识或显式替换策略；保留新增/删除、Enter 导航、焦点、订阅，并防止使用过时索引的回调。 |

## 现有直接 `View` 参考

### `IntegerInput<N>`

- 分类：`现有 View`
- 来源：
  [`crates/gpui-form-gpui-component/src/integer_input.rs:347`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/crates/gpui-form-gpui-component/src/integer_input.rs#L347)
- 后备标识：
  `Entity<IntegerInputState<N>>`；`entity_id()` 返回的正是该 ID
  （[integer_input.rs:430](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/crates/gpui-form-gpui-component/src/integer_input.rs#L430)）。
- 渲染时属性：占位文案、前缀/后缀、外观、尺寸、禁用状态和
  样式。
- 持久状态：有类型的值和策略、编辑器实体以及编辑器订阅
  保留在 `IntegerInputState` 中。
- 现有证据：
  [`tests/adapters.rs:124`](https://github.com/suxiaoshao/gpui/blob/510cd2e371846faca429279d4877fdcfb808cd0e/crates/gpui-form-gpui-component/tests/adapters.rs#L124)
  检查后备标识和同级唯一性。
- 保留的验证契约：状态持久性、最新的构建器属性、禁用状态/样式/焦点
  行为，以及唯一标识。

## 经审计后保留的结论

| 范围 | 边界或分组 | 分类与原因 |
| --- | --- | --- |
| `app/feiwen` | `WorkspaceView`、`QueryView` 和 `FetchView`；`app/novel-download::WorkspaceView` | `保留 Render`：具有任务、订阅、焦点和事件协调职责的应用控制器。 |
| `app/feiwen` | `DragSortRow` | `保留 Render`：它是仅为满足拖放 API 的 `Entity<W: Render>` 契约而创建的单次拖放快照实体，并不是稳定的父级属性加状态组合。 |
| `app/feiwen` | `FeiwenTitleBar`、`Tag` 和 `Novel` | `保留 RenderOnce`：一次性展示。标题栏持有的实体是读取/回调目标，而不是它的后备状态。 |
| `app/feiwen` | 高级查询、抓取、表格单元格、标题栏，以及 `render_multi_combobox` 辅助函数 | `保留函数`：所有者局部拆分或配置现有有状态控件；没有独立标识，也没有参数到子组件的镜像。 |
| `app/novel-download` | `Workspace::render_state` | `保留函数`：在工作区所有者内进行纯粹的状态到元素转换。 |
| `app/http-client` | `HttpFormView`、`HttpTabView` 和 `HttpBodyView` | `保留 Render`：内聚的根视图/标签页/正文控制器，持有表单、控件、子实体和订阅，且没有孤立的父级属性。 |
| `app/http-client` | `FormDataView` | `保留 RenderOnce/函数`：这是一个单元类型/无状态边界；直接 `View` 没有后备状态。 |
| `app/http-client` | `HttpHeadersView` | `保留 RenderOnce/函数`：它只组合表单的请求头输入框实体，自身没有状态/属性分离。 |
| `app/http-client` | `From<&mut HttpTabView> for AnyElement` | `保留函数`：在已经持有的子实体之间进行所有者局部选择。 |
| 共享 crate | `FormInput`、`FormSelect<D>`、`FormCombobox<D>` 和 `FormIntegerInput<N>` | 不属于可渲染候选。它们是持有表单控件的适配器，其订阅和控件租约必须比临时 `View` 存活更久；渲染时会解引用到原生状态实体。 |
| 共享 crate | `gpui-store` 和适配器测试中仅用于测试的 `Render` 测试框架 | 不纳入生产迁移记录。 |

## 审计覆盖范围与限制

- 清点覆盖 `app/` 和 `crates/` 下的生产 Rust 代码。定向搜索发现了 45 个生产
  `Render` 实现、33 个生产 `RenderOnce` 实现、一个工作区内的直接
  `View`，以及 176 个单行 `IntoElement`/`AnyElement` 辅助函数声明。对于多行
  声明，通过其所有者文件和调用位置进行检查，而不是依赖单行计数。
- 审计显式追踪了：
  - 构建器风格的状态/实体持有者；
  - 每个生产 `Render`/`RenderOnce` 分组；
  - 接收或发现 `Entity`/`WeakEntity` 的函数；
  - 每个 `window.use_state`/`window.use_keyed_state` 使用位置；
  - 与渲染相关的父级到子组件 `set_*`、`sync_*`、委托投影、
    观察器和订阅路径。
- 不纳入仅用于测试的测试框架和依赖源码。只有当候选依赖某个锁定依赖的当前契约时，
  才会读取该依赖的 API。
- 原 `倾向迁移` 条目已全部定案；剩余 `待定` 条目仍被特意计入记录，因为它们是
  仅按声明编制迁移列表会遗漏的隐藏父子同步案例。
- 本记录不授权任何实现顺序、工作包、源码编辑、依赖变更或行为变更。如需重新分类，
  必须先更新这里的证据和汇总计数。
