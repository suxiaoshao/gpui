# jaco：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`app/jaco`；工作包：WP-03、WP-04。
- 本地编号使用 `jaco/F-*`、`jaco/L-*`、`jaco/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/app.rs；src/foundation/assets.rs；src/features/settings/prompts/dialog.rs；src/components/chat/detail.rs；src/components/chat/detail/message.rs；src/components/chat/detail/attachments.rs；src/components/chat/input/composer_editor.rs；src/state/theme.rs。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。
- F-04：`src/features/home/sidebar/search.rs`；Command 替代边界见 L-02。
- F-05：`src/app/title_bar_menu.rs`、`src/features/home/shell.rs`、`src/features/settings.rs`、`src/features/about.rs`、`src/app.rs`、`src/app/temporary_window.rs`；菜单和窗口配置见 L-03/L-05。
- F-06：`src/components/picker.rs`、`src/components/chat/form.rs`、`src/components/chat/run_settings.rs`、`src/features/home/new_conversation.rs`，以及其使用的 `src/components/chat/model_picker.rs`、`src/components/chat/form/project_control.rs`；列表选择消费链见 L-04。
- F-07：`src/components/hotkey_input.rs`；只替换显示格式，见 L-06。
- F-08：`src/components/chat/input/composer_editor.rs` 及其 `blink_cursor.rs`、`element.rs`、`history.rs`；局部替换见 L-07/L-08。

迁移入口/资源/测试 import，保留先初始化组件再应用本地主题。Prompt 从 FormInput 多行迁至 FormTextarea，保留保存中关闭保护。保留 basic/full 语言 feature 集合；不整体重写 ComposerEditor，允许 L-07/L-08 对已被上游覆盖的内部能力做局部替换。

## 验证与完成条件

- 候选命令：`cargo test -p jaco --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：提示词输入/撤销/保存关闭；消息流式中文与 Markdown 高度；图标 fallback；审批等待不闪动。
- 新增复用项按根 R-09–R-11 和下述 L 项验证；优先改造已有产品回归，删除仅镜像被移除样板的测试，不要求旧外观/动画/键盘流程逐项相同。

## 已实施展示工作包

- L-01（已知迁移）：Prompt 的 FormTextarea 控制继续绑定同一 Form；保存任务、校验和 rebase 由当前 dialog 持有。
- ST-01：消息身份、内容、工具审批、附件可用性与请求用量沿用当前业务投影；MessageContent 只承载展示元素。
- Q-07 已确认：Message/Bubble/Attachment/Marker 替换相应外层展示；保留复制失败反馈和附件安全打开逻辑。
- Shimmer 仅由真实执行状态开启，等待审批/完成/错误/取消保持静态；复用 conversation-agent-processing 文案，不让所有未终止 run 都动画。
- Q-04 已确认迁移 MessageScroller 并采用上游行为：向上阅读暂停跟随，滚回底部恢复。删除“必须等下一次提交才能恢复”的旧限制；验证后续流式增长恢复跟随且向上阅读不被新消息抢滚动。
- Q-05：0.6.0 的 TextView 与 main 未发布修复分开处理；保留 TextViewState 的流式解析、缓存与重测所有权。
- 已实施：用户行 `Message(End)`、agent 行 `Message(Start)`；正文 `Bubble`、文件/图片 `Attachment`、状态 `Marker`。`active && running && !waiting_approval` 才启用 Shimmer。稳定行键沿用 `TimelineRowKey`；`MessageScrollerState` 负责 splice/remeasure/follow，页面观察该实体以更新 jump 按钮。

## WP-03：上游通用交互复用补充（2026-09-06）

用户已在 2026-09-07 授权并实施这些复用项。具体验证见根实施记录。按必要功能判断，采用上游默认
交互，不为保留旧外观重复构建控件。D-ID 的决定归根计划，本节拥有文件、API、业务适配和技术缺口。

### L-02：会话搜索使用 Command（D-07，F-04）

- 已核实：[Command](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/command/command.rs) 与 [CommandState](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/command/state.rs) 提供 query、selection、loading、键盘导航、确认、取消与虚拟列表；`on_confirm` 延后到 state 更新结束后执行。
- 删除 `ConversationSearchDelegate`、独立 `InputState`/`ListState` 的拼接、`on_search_move_up/down/enter`、`move_selection` 与 `select_first_if_any`/`move_selected`/`confirm_selected`/`item_count`。行内容用 CommandItem 的标准展示或自定义内容，不保留本地选择/点击状态机。
- 由 view 持有一个 `Entity<CommandState>`；`Command::new(&state).filterable(false).on_query(...)` 驱动现有查询，`set_loading` 投影 Operation 的执行状态。数据库仍匹配标题、项目和消息正文，禁止组件再次按标题过滤。
- 每次 render 捕获与 Command model 同一快照的 ConversationId 向量；延后确认时按该快照解析，不把路径当成持久化 ID。保留 workspace 查询、取消/过期结果保护、stale/error/retry、现有 Fluent 文案与打开会话操作。
- 加载/错误/重试使用组件 loading、header/footer/empty 扩展承载；不因采用 Command 删除恢复入口。组件负责 Escape 的查询清空与取消行为，owner 在 `on_cancel` 关闭搜索界面。
- R-09/T-09：覆盖正文或项目独有命中、连续查询的过期结果、空结果、失败重试、键盘确认正确会话；原实体重入测试改为确认回调能安全更新 owner 的产品回归。

### L-03：标题栏菜单使用 AppMenuBar（D-08，F-05）

- 已核实：[AppMenuBar](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/menu/app_menu_bar.rs) 的 `new`/`reload`、菜单动作上下文、焦点恢复、左右键/Escape、hover 和 popup 构造；此能力基线已有。
- 删除 `TitleBarAppMenuBar`、本地菜单 actions/key context、`TitleBarAppMenu`、trigger 与递归 `OwnedMenu` 转换。`title_bar_leading` 只接收上游菜单 entity 并组合应用图标。
- 同步 home/settings/about 的 entity 类型、构造与 reload 调用；沿用现有应用菜单定义与组件初始化，不维护第二套菜单状态。
- R-10/T-10：在实际显示 AppMenuBar 的 Windows/Linux 路径验证菜单动作作用于原焦点上下文、取消后焦点恢复、子菜单与键盘操作；macOS 原生菜单消费路径保留。

### L-04：列表型 Picker 使用 Combobox（D-09，F-06）

- 已核实：[Combobox](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/combobox.rs) 提供 `set_items`、`query`/`set_query`、按值选择、`render_trigger`、`footer`；[SearchableListDelegate](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/searchable_list/delegate.rs) 提供分组、自定义匹配/行、`is_item_enabled`、`on_will_change`。基线已有 Combobox，目标接口按上述源码迁移。
- 将模型/项目等列表选择迁至 `ComboboxState` + `Combobox`；删除 `PickerListDelegate` 的通用 ListDelegate 实现、手工选中行查找与过滤投影、`PickerListItem` 选择外壳、列表专用 `PickerPopover`。领域数据通过薄 delegate 提供分组、值与匹配；能使用上游集合 delegate 时直接使用。
- Form/run settings 继续拥有持久化值和可修改性；按稳定 Value 同步选中值，刷新 catalog 后保留查询并重投影选择，缺失值按现有领域策略处理。不得把 filtered index 写回业务值。
- 只读状态通过禁用项/选择前拦截保证不能写入，并在现有触发器或 footer 显示原因；选中回调只向原业务入口提交值。typed projection、错误恢复、状态刷新继续由 owner 负责。
- `PickerControl` 只持有 `ComboboxState<SearchableVec<SearchableGroup<PickerItem<T>>>>` 和订阅。仅监听 `ComboboxEvent::Change`，在 defer 中调用原业务入口；取消产生的 Confirm 不写回。刷新时按完整目录稳定 Value 投影，恢复 query，保留 entity。旧 `picker_content_popover` 已无消费者并删除；其他任意内容 Popover 保留。
- R-09/T-09：刷新/过滤后确认正确业务值，选中项隐藏或移除不串值，只读与加载期间禁止修改，错误后可重试；复用现有 picker 身份、回调重入及只读回归。

### L-05：TitleBar 窗口默认配置（D-10，F-05）

- 已核实：[TitleBar::window_options](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/title_bar.rs) 同时设置透明标题栏和 `app_owns_titlebar_drag: true`。
- `src/app.rs` 通用窗口构造以此 helper 为 struct update base；`temporary_window.rs` 实际没有渲染 TitleBar，保留原生窗口配置；保留标题、bounds、尺寸、定位与需要的 traffic-light 参数。删除只重复默认组合的 helper 内容，同步相应断言。
- R-10/T-10：拖动/双击仅由正确 owner 处理，标题与窗口尺寸/位置正确，标题栏控件点击不触发窗口拖动；不以旧坐标或外观一致为验收条件。

### L-06：快捷键显示（D-11，F-07）

- 已核实：[Kbd::format / Kbd](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/kbd.rs) 提供平台修饰键与特殊键显示，基线已有。
- 删除 `format_keystroke_label` 的手写映射，字符串消费者用 `Kbd::format`，元素消费者可直接用 `Kbd`。`format_hotkey_label` 可保留为解析现有配置到上游显示的薄入口。
- `HotkeyInputState` 的录制、拦截、焦点管理、`keystroke_to_string` 与解析/持久化继续保留。R-10/T-10 验证录制配置往返及显示可读，不要求保留旧键名拼写。

### L-07：光标重复调度（D-12，F-08）

- 已核实：[gpui-pre 0.3.3 Animation](https://docs.rs/crate/gpui-pre/0.3.3/source/src/elements/animation.rs) 的 `repeat_synced()`、`with_max_fps()` 和减少动态效果时静态渲染行为。
- 目标删除 `BlinkCursor::blink` 的递归 timer 与相应 epoch 调度；重复部分由动画承担，owner 仅保留焦点/编辑活动导致的可见性策略。可接受上游共享相位，无需保留旧节拍。
- 已删除 `blink_cursor.rs`；仅活跃窗口中聚焦且可编辑时把 element 包入 1 秒 `repeat_synced().with_max_fps(2.)` 动画。输入后以 `cursor_hold_until` 保持 300ms 可见；失焦/退役移除动画。减少动态效果时 phase 0 保持可见，不创建计时任务或独立光标实体。
- R-11/T-11 覆盖聚焦/输入/失焦/退役、减少动态效果；验证动画限帧生效，避免替换后持续逐帧刷新。

### L-08：历史容器（D-13，F-08）

- 已核实：[UndoHistory<T>](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/base/src/undo_history.rs)，组件 `history` 模块公开 re-export；提供事务分组、容量、撤销/重做与新 push 清除 redo。
- 目标删除 `EditorHistory` 自管的 undo/redo Vec 和容量裁剪。保留 `EditorState` 的文本、token、选区、marked range 快照以及恢复规则；通过包含 before/after 的编辑事务适配上游，不能只替换容器而沿用单侧快照。
- 已采用 `UndoHistory<Edit { before, after }>`，最多 200 个事务、不启用时间合组。普通修改完成后记录；IME 首次标记时保存 base，提交/unmark 时只记录一次。相同快照不入栈，push 自动清 redo。undo 取倒序事务末项 before，redo 取正序末项 after；clear 仍是可撤销编辑，退出时随 owner 释放。
- R-11/T-11 覆盖普通文本、原子 token、中文组合输入、撤销/重做后选区恢复、撤销后新分支与容量边界；不要求继承无必要的旧分组方式。

### 明确保留与范围外线索

- D-04 的 Composer 原子 token/IME 协作仍无已确认完整替代；HotkeyInput 仍需要录制功能。局部替换不改变这两项业务能力。
- `Clipboard` 当前先写剪贴板并设置成功状态，再通知 `on_copied`；回调不能否决成功。保留 `detail.rs::copy_to_clipboard` 的失败通路与 `copy_button.rs` 的成功条件；仅提示时长/图标差异不构成保留理由。
- GPUI `App::set_window_appearance` 可令 macOS 原生 chrome 跟随应用主题，但它属于新增接入，当前没有对应整套本地代码可删除。本节仅记录线索，不加入执行工作包；主题跟随模式的状态来源与系统观察反馈需另行确定。
