# Issue #182：`gpui::View` 迁移记录

## 状态与范围

- 状态：`实施中`
- 审计状态：`已完成基线清点；分类结论仍为草稿`
- 跟踪 issue：[#182](https://github.com/suxiaoshao/gpui/issues/182)
- 根索引：[README.md](README.md)
- 判断标准：[view-migration-criteria.md](view-migration-criteria.md)
- 源码基线：`510cd2e371846faca429279d4877fdcfb808cd0e`
- 快照日期：`2026-07-31`
- 已审计源码范围：`app/` 和 `crates/` 下的生产 Rust 代码
- 不纳入迁移记录：仅用于测试的测试框架和第三方依赖源码
- 实现状态：`12/12` 个必须迁移边界已完成；原倾向迁移项已全部定案，待定项尚未实施

本记录按照判断标准记载当前工作区的审计结果和实施进度。用户已授权并完成全部必须迁移项；原三个倾向迁移项中，两个已升级并完成迁移，`ComposerEditor` 已定案为保留 `Render`。待定项不能由本记录自动授权实施。

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

下方计数统计的是可渲染边界，而不是估算的文件数或实现工作包数。一个分组族中，每个需要独立迁移的组件分别计数；因此三个运行设置选择器计为三个。

| 归属 | 必须迁移 | 倾向迁移 | 待定 | 现有 View | 保留项覆盖 |
| --- | ---: | ---: | ---: | ---: | --- |
| `app/jaco` | 11 | 0 | 3 | 0 | 见下方汇总 |
| `app/feiwen` | 1 | 0 | 1 | 0 | 见下方汇总 |
| `app/http-client` | 0 | 0 | 4 | 0 | 见下方汇总 |
| `app/novel-download` | 0 | 0 | 0 | 0 | 见下方汇总 |
| 共享 crate | 0 | 0 | 0 | 1 | 见下方汇总 |
| **合计** | **12** | **0** | **8** | **1** | — |

## 必须迁移项实施进度

| 边界 | 稳定后备状态 | 当前状态 | 已验证的关键约束 |
| --- | --- | --- | --- |
| `NumericRangeInput` | `NumericRangeInputState` | `已迁移` | 后备身份契约、范围解析语义 |
| `ChatForm` | `ChatFormState` | `已迁移` | 后备身份契约；嵌套控件句柄稳定性为显式构造契约 |
| `HotkeyInput` | `HotkeyInputState` | `已迁移` | 后备身份契约、录制与清除、设置草稿持久化 |
| `ModelSelector` | `ModelControlState` | `已迁移` | 后备身份契约 |
| `ReasoningSelector` | `ReasoningControlState` | `已迁移` | 后备身份契约 |
| `ApprovalSelector` | `ApprovalControlState` | `已迁移` | 后备身份契约 |
| `PickerPopover` | `ListState<D>` | `已迁移` | 列表后备身份契约 |
| `DetailBlock` | `DetailBlockState` | `已迁移` | keyed 状态跨父级重渲染保持、同级键隔离 |
| `CopyButton` | `CopyButtonState` | `已迁移` | 无单独身份测试；纳入 Jaco 整体编译与回归测试 |
| `PrimaryAction` | `PrimaryActionControlState` | `已迁移` | 后备身份契约 |
| `ChatInput` | `ChatInputController` | `已迁移` | 控制器后备身份契约、三个父级位置属性 |
| `LoadedSkillContentView` | `ScrollHandle` | `已迁移` | 完整路径 keyed state、路径碰撞隔离 |

实施验证：

- `cargo test -p jaco --offline`：`355` 项通过。
- `cargo test -p feiwen --offline`：`69` 项通过。
- `cargo clippy -p jaco -p feiwen --all-targets --all-features --offline -- -D warnings`：通过。
- `cargo fmt --all --check` 与 `git diff --check`：通过。

## 必须迁移项

### `app/feiwen`：`NumericRangeInputState`

- 分类：`必须迁移`
- 当前形态：
  [`NumericRangeInputState: Render`](../../../app/feiwen/src/features/query/advanced/components/numeric_range_input.rs#L18)
  同时存储两个嵌套的 `InputState` 实体，以及 `min_label`、`max_label` 和
  `disabled`。
- 稳定的后备标识：
  每个条件会创建一个 `NumberValue::Range(Entity<NumericRangeInputState>)`，
  并将其保留在条件树中
  （[state.rs:100](../../../app/feiwen/src/features/query/advanced/state.rs#L100),
  [state.rs:362](../../../app/feiwen/src/features/query/advanced/state.rs#L362)）。
  同级范围条件不会共享该实体。
- 父级持有的属性：两个标签和 `disabled`。尤其是父级的 `searching` 值已经传到
  `render_number_value`，但范围分支忽略了它，直接渲染实体
  （[render.rs:532](../../../app/feiwen/src/features/query/advanced/render.rs#L532)）。
- 持久组件状态：最小值/最大值输入框实体，以及它们的文本、焦点、选择
  和编辑状态。
- 当前同步方式：搜索开始/结束时调用 `AdvancedQueryState::set_disabled`，
  递归遍历每个条件，然后调用 `NumericRangeInputState::set_disabled` 并通知子组件
  （[query.rs:260](../../../app/feiwen/src/features/query.rs#L260),
  [state.rs:712](../../../app/feiwen/src/features/query/advanced/state.rs#L712),
  [numeric_range_input.rs:42](../../../app/feiwen/src/features/query/advanced/components/numeric_range_input.rs#L42)）。
  这是将父级展示属性镜像到子组件状态的行为。
- 要求的目标边界：一个由现有范围状态实体作为后备状态的临时
  `NumericRangeInput` 直接 `View`。它的 `entity_id()` 必须返回该范围状态
  的 ID；标签和 `disabled` 作为每次重建的属性。
- 迁移后移除的同步：范围专用的 `set_disabled` 字段/设置方法，以及仅用于镜像搜索
  状态的递归子组件更新。后备状态继续持有两个输入框。
- 定向验证：父级重新渲染时保留最小值/最大值文本和焦点；不通过子组件设置方法
  更新禁用状态；证明同级标识唯一；保留占位文案以及
  `Missing`/`Invalid`/`Reversed` 解析行为。

### `app/jaco`：`ChatForm`

- 分类：`必须迁移`
- 当前形态：
  [`ChatForm: Render`](../../../app/jaco/src/components/chat/form.rs#L67)
  将自身描述为纯视觉外壳，但在同一个实体中存储了 `ChatFormControls`、
  `skill_completion_placement`、预绘制边界和订阅。
- 稳定的后备标识：现有 `Entity<ChatForm>` 由 `ChatInputController`
  和快捷键对话框持有。状态侧需要保留边界、事件发送，以及长生命周期的
  观察/订阅句柄。
- 父级持有的属性：控件槽位的组合和启用状态，以及技能补全位置。
  目前该位置通过
  `ChatInputController::set_skill_completion_placement` 写入子实体
  （[input.rs:321](../../../app/jaco/src/components/chat/input.rs#L321),
  [form.rs:128](../../../app/jaco/src/components/chat/form.rs#L128)）。
- 持久组件状态：测量后的边界，以及拖放处理、弹出层、嵌套控件
  观察和 `ChatFormUiEvent` 分发所需的生命周期。
- 要求的目标边界：将稳定的表单后备状态与临时 `ChatForm` 直接 `View`
  拆开；用当前控件和位置重建 `View`，并返回后备状态的
  实体 ID。
- 迁移后移除的同步：已存储的展示控件和位置设置方法链。仍需用于
  观察嵌套控件实体的订阅保留在后备状态中；如果
  控件句柄集合可能变化，则必须重新协调这些订阅。
- 定向验证：事件传递、外部路径拖放、项目和运行设置弹出层、
  附件操作、预绘制边界、技能补全位置，以及父级
  重新渲染时标识保持稳定。

### `app/jaco`：`HotkeyInput`

- 分类：`必须迁移`
- 当前形态：
  [`HotkeyInput: Render`](../../../app/jaco/src/components/hotkey_input.rs#L23)
  先通过构建器风格的 `default_value`、`Sizable` 和 `Styled` 方法配置，
  然后将整个值放入实体。
- 稳定的后备标识：现有快捷键实体在每个使用位置持久存在，并独占
  录制会话。
- 父级持有的属性：元素 ID、样式、尺寸，以及初始值/外部值策略。这些内容
  应表达在临时 `HotkeyInput` 值上，而不是在构造实体时冻结。
- 持久组件状态：当前草稿按键、外层/捕获焦点句柄、按键
  拦截和焦点订阅。
- 隐藏同步证据：
  [`bind_hotkey`](../../../app/jaco/src/features/settings/shortcuts/dialog.rs#L43)
  通过 `set_hotkey` 将表单变更复制到快捷键实体，并使用双向保护标记将
  组件变更复制回表单。
- 要求的目标边界：由稳定快捷键状态实体作为后备状态的直接 `View`
  快捷键组件。表单仍是已声明的领域数据权威；录制/焦点仍为组件状态。
  表单控件绑定可以保留显式的值投影，但尺寸、样式、ID 和其他展示字段
  不得继续作为长生命周期的镜像状态存在。
- 定向验证：录制焦点和拦截、清除/停止操作、表单驱动的值变更、无反馈循环
  的组件驱动编辑、样式/尺寸刷新，以及同级标识唯一性。

### `app/jaco`：运行设置选择器函数族

以下三个函数分别独立迁移：

| 当前函数 | 后备标识 | 渲染时属性 |
| --- | --- | --- |
| [`render_model_selector`](../../../app/jaco/src/components/chat/run_settings.rs#L1077) | `Entity<ModelControlState>` | `enabled` |
| [`render_reasoning_selector`](../../../app/jaco/src/components/chat/run_settings.rs#L1200) | `Entity<ReasoningControlState>` | `enabled` |
| [`render_approval_selector`](../../../app/jaco/src/components/chat/run_settings.rs#L1295) | `Entity<ApprovalControlState>` | `enabled` |

- 分类：三个 `必须迁移` 边界。
- 持久组件状态：选择/打开状态、选择器/列表状态、适用时的能力
  数据、焦点，以及由运行设置控制器持有的打开状态变更处理器。
- 当前更新路径：表单/控制器更新每个控件状态并通知它；
  `ChatForm` 另外观察这些实体，以便重新运行其函数调用。
- 要求的目标边界：每个选择器对应一个具名的直接 `View`，返回相应控件状态的
  ID，并将当前启用状态作为属性接收。表单到控件的投影仍是显式的
  控制器绑定，而不是渲染时写入。
- 定向验证：选中标签、禁用/只读资源状态、打开状态/焦点协调、
  模型刷新、推理令牌预算页脚、审批选择，以及三个
  同级选择器各自具有不同的标识。

### `app/jaco`：`picker_popover`

- 分类：`必须迁移`
- 当前形态：
  [`picker_popover`](../../../app/jaco/src/components/picker.rs#L486)
  是一个可复用函数，其 `PickerPopoverConfig` 同时包含一个稳定的
  `Entity<ListState<D>>`，以及 `open`、触发器、尺寸、占位文案、
  页脚和回调
  （[picker.rs:457](../../../app/jaco/src/components/picker.rs#L457)）。
- 稳定的后备标识和状态：列表实体持有查询、选择、
  焦点、委托状态和列表生命周期。
- 父级持有的属性：其他所有配置字段，尤其是受控的 `open` 和
  布局/展示方式。
- 要求的目标边界：一个由列表实体作为后备状态的泛型具名直接
  `View`。`picker_content_popover` 仍保留为无状态组合辅助函数。
- 定向验证：搜索/焦点/选择持久性、受控打开状态变更、页脚/内容
  布局，以及禁止通过多个同级选择器 `View` 渲染同一个列表实体。

### `app/jaco`：`detail_block`

- 分类：`必须迁移`
- 当前形态：
  [`detail_block`](../../../app/jaco/src/components/chat/detail/tool_blocks.rs#L29)
  是一个在渲染期间发现带键的 `Entity<DetailBlockState>`，并将其与当前
  对话条目、可选文本状态、是否可以审批以及回调
  组合的函数。
- 稳定的后备标识：带键状态使用对话条目 ID，并持久
  持有 `expanded`。
- 父级持有的属性：条目快照、文本状态、可否审批标志和
  审批回调。
- 要求的目标边界：一个由 `DetailBlockState` 作为后备状态的具名直接
  `View`；其键/实体对每个对话条目必须保持唯一。
- 定向验证：展开状态持久性、详情 Markdown、审批操作、在同一个键下
  替换条目，以及同级标识。

### `app/jaco`：`copy_button`

- 分类：`必须迁移`
- 当前形态：
  [`copy_button`](../../../app/jaco/src/components/chat/detail/message.rs#L470)
  创建一个带键的 `Entity<CopyButtonState>`，并将其与当前复制载荷、
  标签、ID 和回调组合。
- 稳定的后备标识和状态：复制时间戳和定时器驱动的通知
  以按钮 ID 为键；定时器会显式通知该实体 ID。
- 父级持有的属性：复制文本、标签、回调和元素 ID。
- 要求的目标边界：一个由 `CopyButtonState` 作为后备状态的具名直接
  `View`。
- 定向验证：复制成功/失败、已复制超时重置、定时器结束后的通知、载荷
  刷新，以及每个消息操作都有唯一状态。

### `app/jaco`：主操作内联边界

- 分类：`必须迁移`
- 当前形态：`ChatForm::render` 使用
  [`Entity<PrimaryActionControlState>`](../../../app/jaco/src/components/chat/form/controls.rs#L67)。
  内联渲染主操作。它尚未成为具名组件或函数，因此仅按声明进行清点
  无法发现它。
- 稳定的后备标识和持久状态：提交任务和代理运行状态
  来源属于 `PrimaryActionControlState`。
- 父级持有的属性：`can_submit`、`disabled_reason` 和控件启用状态。
  `ChatInputController::sync_chat_form_projection` 目前计算前两项并将其写入子状态
  （[input.rs:443](../../../app/jaco/src/components/chat/input.rs#L443)）。
- 要求的目标边界：提取一个由 `PrimaryActionControlState` 作为后备状态
  的具名直接 `View`；由所有者渲染时提供当前是否可提交、禁用
  原因和启用状态。
- 迁移后移除的同步：作为父级持有镜像的 `can_submit` 和 `disabled_reason` 字段，
  以及相应的 `primary_action_state.update` 投影。
- 定向验证：发送/停止/正在停止模式、提交任务加载状态、
  资源禁用提示、操作分发，以及可提交性属性变更时
  状态保持持久。

### `app/jaco`：`ChatInputController` 渲染边界

- 分类：`必须迁移`
- 已确认的后备标识：每个新会话页、临时会话窗格和会话详情页都长期持有一个
  独立的 `Entity<ChatInputController>`，并且只在一个位置渲染它。
- 父级持有的属性：技能补全弹层位于表单上方或下方；该位置由三个父级的布局语义
  决定，不属于控制器任务、订阅或业务状态。
- 目标与实施：新增以控制器实体为后备状态的 `ChatInput` 直接 `View`；
  `entity_id()` 返回控制器 ID，并在父级每次渲染时接收当前位置。
  `ChatInputController` 不再实现 `Render`，但继续拥有编辑器、表单、运行设置、
  技能目录、提交状态和订阅。
- 已移除的同步：控制器中的位置字段、默认值、设置方法，以及三个父级构造阶段的
  位置写入。父级改为直接构造带位置属性的临时 `ChatInput`。
- 定向验证：同一控制器以不同位置属性重建时保持后备身份；新会话、临时窗口和
  会话详情三条渲染路径继续通过 Jaco 回归测试。

### `app/jaco`：已加载技能内容边界

- 分类：`必须迁移`，仅迁移 `SkillContentPanelState::Loaded` 分支。
- 稳定后备状态：每个已加载技能内容区的 `Entity<ScrollHandle>`，用于保留局部
  滚动位置和滚动边界行为。
- 父级持有的属性：内容、内容摘要和将剩余滚动距离传给父级列表的回调。
- 目标与实施：提取 `LoadedSkillContentView` 直接 `View`，返回滚动状态实体 ID；
  `SkillCatalogEntryView` 继续保留 `RenderOnce`，失败和未展开分支继续保留为
  无状态组合。
- 标识修正：原 `skill_row_id` 会把所有非 ASCII 字母数字清洗为 `-`，不能保证
  不同路径的键唯一。迁移后使用完整技能路径的 `ElementId::Path` 加具名子键，
  同时用于滚动 keyed state 和展开按钮。
- 定向验证：完整路径不同但旧清洗结果会碰撞的两个技能仍获得不同状态键；Jaco
  回归测试覆盖滚动内容的构造和父级列表渲染路径。

## 待定项

### 所有权或组件 API 关卡尚未解决

#### 项目选择器边界

- 证据：
  [`ProjectControlState`](../../../app/jaco/src/components/chat/form/project_control.rs#L126)
  持久持有打开状态/列表交互状态，而父级工作区、选择和可用性
  由
  `NewConversationPage::sync_project_picker`
  复制到它的选择器中
  （[new_conversation.rs:556](../../../app/jaco/src/features/home/new_conversation.rs#L556)）。
- 阻塞事实：当前列表委托同时持有查询/选择机制和复制而来的项目
  投影。拟议的 `View` 必须确立唯一数据权威，并消除投影设置方法，而不是转移它们。
- 解决后的验证：项目刷新、选中值、不可用项目、
  打开状态/焦点、添加项目页脚，以及稳定的列表标识。

#### 提供商列表边界

- 证据：
  [`ProviderSettingsPage::sync_provider_list`](../../../app/jaco/src/features/settings/provider.rs#L662)
  将权威数据行和选中索引写入稳定的提供商 `ListState`，随后通过
  提供商列表面板渲染该状态。
- 阻塞事实：尚不明确数据行是作为渲染时属性属于父级，还是作为权威数据属于
  可搜索委托。如果直接 `View` 仍在渲染期间调用 `set_rows`，就无法通过
  “减少同步”关卡。
- 解决后的验证：查询/筛选状态、选中的提供商、资源刷新、列表
  焦点/滚动，以及空列表/只读状态。

#### 提供商模型列表边界

- 证据：
  [`ProviderSettingsPage::sync_model_list`](../../../app/jaco/src/features/settings/provider.rs#L673)
  将数据行、禁用状态和选择写入稳定的模型 `ListState`。
- 阻塞事实：同一个委托将父级投影与本地搜索/选择机制组合；
  所有权和“不在渲染期间写入”的路径尚未确立。
- 解决后的验证：筛选、启用/禁用变更、选择、模型修改、
  焦点/滚动，以及同级列表标识。

#### Feiwen 结果表格边界

- 证据：
  [`QueryView::render_results_table`](../../../app/feiwen/src/features/query.rs#L352)
  渲染一个稳定的 `TableState<ResultsTableDelegate>`，而搜索状态通过
  `set_loading` 和 `table.refresh` 被复制进去
  （[query.rs:324](../../../app/feiwen/src/features/query.rs#L324),
  [results_table.rs:40](../../../app/feiwen/src/features/query/results_table.rs#L40)）。
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
| [`UrlInput`](../../../app/http-client/src/features/request/url_input.rs#L9) | 输入框变更会发送表单 URL 事件；参数/表单 URL 变更会替换整个 `InputState` 实体（[url_input.rs:43](../../../app/http-client/src/features/request/url_input.rs#L43)）。这种替换既不具备稳定标识，也没有经过证明的通知/重建路径。 | 声明表单的数据权威；保留一个编辑器标识；防止反馈循环；参数重写 URL 时保留焦点/IME/选择。 |
| [`HttpParamsView`](../../../app/http-client/src/features/request/params.rs#L22) | 查询输入框编辑会重写 `HttpForm.url`；URL 事件会重建所有数据行输入框实体（[params.rs:121](../../../app/http-client/src/features/request/params.rs#L121)）。数据行按索引标识，没有稳定的领域键。 | 定义数据行标识和数据权威；保留弹出层/新增输入框、Enter 导航、焦点、新增/删除/重排索引，以及外部 URL 替换行为。 |
| [`HttpTextView`](../../../app/http-client/src/features/request/body/http_text.rs#L59) | 编辑器变更会将文本写入正文表单；`SetTextType` 会调用 `InputState::set_highlighter`（[http_text.rs:98](../../../app/http-client/src/features/request/body/http_text.rs#L98)）。尚未确立渲染时语法高亮器属性或父级重建路径。 | 决定语言是属性，还是显式保留的状态投影；保留文本、焦点、选择、搜索和语法高亮。 |
| [`XFormView`](../../../app/http-client/src/features/request/body/x_form.rs#L107) | 键/值编辑会写入正文表单；新增/删除会追加或重建按索引标识的子输入框（[x_form.rs:152](../../../app/http-client/src/features/request/body/x_form.rs#L152)）。数据行标识和通知所有权不明确。 | 定义稳定的数据行标识或显式替换策略；保留新增/删除、Enter 导航、焦点、订阅，并防止使用过时索引的回调。 |

## 现有直接 `View` 参考

### `IntegerInput<N>`

- 分类：`现有 View`
- 来源：
  [`crates/gpui-form-gpui-component/src/integer_input.rs:347`](../../../crates/gpui-form-gpui-component/src/integer_input.rs#L347)
- 后备标识：
  `Entity<IntegerInputState<N>>`；`entity_id()` 返回的正是该 ID
  （[integer_input.rs:430](../../../crates/gpui-form-gpui-component/src/integer_input.rs#L430)）。
- 渲染时属性：占位文案、前缀/后缀、外观、尺寸、禁用状态和
  样式。
- 持久状态：有类型的值和策略、编辑器实体以及编辑器订阅
  保留在 `IntegerInputState` 中。
- 现有证据：
  [`tests/adapters.rs:124`](../../../crates/gpui-form-gpui-component/tests/adapters.rs#L124)
  检查后备标识和同级唯一性。
- 保留的验证契约：状态持久性、最新的构建器属性、禁用状态/样式/焦点
  行为，以及唯一标识。

## 经审计后保留的结论

| 范围 | 边界或分组 | 分类与原因 |
| --- | --- | --- |
| `app/jaco` | 根视图/页面/对话框控制器，包括 `JacoRoot`、`HomeView`、`NewConversationPage`、`ConversationDetailPage`、设置页面、编辑对话框、`TemporaryWindow`、`ImagePreview` 和 `ScreenshotOverlayView` | `保留 Render`：每个控制器都内聚地持有任务、订阅、焦点、事件和可变控制器状态；排除上述候选后，未发现孤立的渲染时属性镜像。 |
| `app/jaco` | `TitleBarAppMenuBar`、`TitleBarAppMenu`、`HomeSidebar`、`ConversationSearchView`、`TemporaryHotkeyControlState` 和 `SkillDetailDialog` | `保留 Render`：长生命周期的交互/控制器实体。`TemporaryHotkeyControlState` 有带键实体，但没有父级渲染时属性。 |
| `app/jaco` | `ComposerEditor` | `保留 Render`：`disabled` 参与焦点、IME、编辑动作和补全控制，技能目录快照参与令牌与补全协调；这些都是编辑器持久行为状态。直接 `View` 无法删除 `set_skill_entries` 或安全移除禁用状态，只会重写实体事件接线。 |
| `app/jaco` | `Resource*`、`CriticalResourcesView`、选择器/列表行、时间线行、侧边栏行、设置布局/行类型，以及提供商面板/行类型 | `保留 RenderOnce`：数据快照或一次性组合。嵌套的 `TextView`、列表或输入框实体是独立控件，并不是包装器的唯一后备状态。 |
| `app/jaco` | `SettingsShell` 和 `SettingsNav` | `保留 RenderOnce`：搜索输入框是嵌套控件，而不是外壳/导航的后备标识；将两个包装器的 ID 都设为它的 ID 还会带来标识冲突风险。 |
| `app/jaco` | `picker_content_popover`、标题栏辅助函数、`markdown_view`、提供商/MCP 输入行辅助函数，以及类似的所有者局部渲染函数 | `保留函数`：围绕现有直接 `View`/原生控件进行纯拆分；没有独立状态或同步。 |
| `app/jaco` | `temporary_hotkey_control` 和 `app_http_proxy_input` | `保留函数`：围绕没有父级属性的内聚带键状态；直接 `View` 不会增加属性/状态分离。 |
| `app/jaco` | `ComposerEditorElement` | `保留 Element`：它实现底层布局请求、预绘制和绘制边界。 |
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
