# 快捷键临时窗口运行时契约

本文固定 issue #175 的快捷键触发与临时窗口生命周期。产品行为保持“取得输入后自动创建并自动运行”，
变化只在于临时 Conversation 必须由临时窗口 owner 完成路由和 reveal。
实现已落地：快捷键现在只调用 `temporary_window::show_created_conversation`，由 lifecycle owner 查找/创建
Popup、路由 Conversation、启动一次 AgentRun 并安排 reveal；实际桌面窗口验收按用户要求跳过。

## 1. 当前问题边界

改造前 `state::hotkey` 已调用 `app::temporary_window::open_temporary_window`，但随后又自行 downcast
`WindowHandle<Root> -> TemporaryWindow` 并调用 `open_created_conversation`。这使 hotkey 层知道窗口 view
结构，且 `open_temporary_window` 已经安排 reveal 后才路由内容。修复目标不是再创建一种窗口，而是把
“路由 created Conversation + 准备 reveal”收回已经拥有 lookup/create/hide/reveal 的
`TemporaryWindowLifecycleState`。

## 2. 保持不变的输入路径

### SelectionOrClipboard

`GlobalHotkeyState` 继续异步读取 selection，空值时回退 clipboard；归一化成功后构造 text
`ContentPart`，失败继续显示现有 empty-input notification。

### Screenshot：模型直接接收图片

继续编码 PNG、创建 `ComposerAttachment`，以 `shortcut-input-screenshot` 为 title/text seed。编码或附件
创建失败沿用现有错误通知。

### Screenshot：OCR fallback

继续在 `smol::unblock` 中 OCR，回到 GPUI foreground 后走与 selection 相同的 text parts 完成函数。

输入采集、OCR、附件存储、prompt snapshot 和 provider/model resolution 均不在本 issue 重构。

## 3. 新的临时窗口入口

`app/jaco/src/app/temporary_window.rs` 新增 crate 内入口：

```rust
pub(crate) fn show_created_conversation(
    created: state::conversations::CreatedConversation,
    cx: &mut App,
) -> Option<WindowHandle<Root>>;
```

它通过现有 `with_lifecycle_state` 委托：

```rust
impl TemporaryWindowLifecycleState {
    fn show_created_conversation(
        &mut self,
        created: state::conversations::CreatedConversation,
        cx: &mut App,
    ) -> Option<WindowHandle<Root>>;
}
```

方法必须消费 `CreatedConversation`，确保同一个 `run_request` 无法被重复提交。`open_temporary_window` 和
`toggle_temporary_window` 继续保留给菜单/普通临时快捷键使用。

## 4. 顺序与 owner

`show_created_conversation` 的固定顺序：

1. `find_temporary_window(cx)`；不存在时调用现有 `create_temporary_window(cx)`，窗口仍以
   `show: false`、`WindowKind::PopUp`、`WindowLevel::ModalPanel` 创建。
2. 对返回的 `WindowHandle<Root>` 执行一次 `update`。
3. 在 lifecycle owner 内检查 `root.view()` 能 downcast 为 `TemporaryWindow`；失败记录结构化错误并返回
   `None`。
4. 调用 `TemporaryWindow::open_created_conversation(created, window, cx)`，完成 history reload、route、
   workspace sidebar refresh 和唯一一次 `runtime.start_run`。
5. 在同一个 window update 中调用现有 `prepare_temporary_window`，取得 `TemporaryWindowReveal`。
6. update 成功后调用现有 `schedule_temporary_window_reveal`；返回 window handle。

因此数据流固定为：

```text
hotkey input
  -> create_conversation (DB commit, run 尚未启动)
  -> defer temporary-window dispatch until the hotkey update ends
  -> temporary_window::show_created_conversation
  -> route created conversation
  -> start_run exactly once
  -> native reveal popup
```

不新增普通窗口 fallback，不调用 main window router，不改变 960x620、目标鼠标屏幕定位、失焦隐藏、
10 分钟延迟销毁和 macOS 前台 app restore。

## 5. hotkey 层收口

`state/hotkey.rs` 删除 `open_created_shortcut_conversation` 中对 `Root`、`TemporaryWindow` 和
`open_temporary_window` 的知识，改为单一完成函数：

```rust
fn finish_shortcut_trigger(
    &self,
    created: state::conversations::CreatedConversation,
    cx: &mut App,
) {
    cx.defer(move |cx| {
        let _ = temporary_window::show_created_conversation(created, cx);
    });
}
```

`finish_shortcut_trigger` 必须在当前 `GlobalHotkeyState` 更新闭包结束后再路由窗口；selection、OCR 和 screenshot
完成路径都可能从该更新闭包中进入，直接同步更新 temporary window 会重新借用 GPUI 的 global/window/entity，触发
`RefCell already borrowed`。defer 只负责跨越更新边界，不改变 Conversation 创建或 AgentRun 的业务顺序。

selection/clipboard、直接 image 和 OCR fallback 在 `create_shortcut_conversation` 成功后都只调用该函数。
`create_shortcut_conversation` 同时改为读取：

```rust
reasoning_selection: trigger.shortcut.settings_snapshot.reasoning_selection.clone(),
approval_mode: trigger.shortcut.settings_snapshot.tool_policy.approval_mode,
```

调用前先按 [run-settings.md](run-settings.md) 第 8 节完成 capability 校验。保留
`AgentRunTriggerKind::Shortcut`、`project_id: None`、空 `skill_requests` 与 prompt snapshot 行为。

## 6. 错误、恢复与并发

| 失败点 | 行为 | 持久化结果 | Run 状态 |
| --- | --- | --- | --- |
| selection/clipboard 为空、截图/OCR 失败 | 现有 notification；不调用 create | 无 Conversation | 未启动 |
| provider/model/prompt 不可用或 capability mismatch | 现有/明确错误 notification | 无 Conversation | 未启动 |
| `create_conversation` 失败 | 现有错误 notification；不打开窗口 | transaction 回滚 | 未启动 |
| lifecycle global 未初始化、window create/update/downcast 失败 | tracing error；不创建普通窗口 fallback | Conversation 已提交，可从临时历史恢复 | downcast 前未启动 |
| route 成功、native reveal 安排失败 | tracing error；后续快捷键可复用窗口 | Conversation 已提交 | 已启动一次，不重试 |

`CreatedConversation`只能沿单一`show_created_conversation`路径消费，不得clone`run_request`或在hotkey层补偿启动。
真正的active-run幂等由现有`ConversationRuntimeStore::start_run`负责；lifecycle必须检查其返回值并记录重复/拒绝，
测试覆盖同一conversation的第二次start不会创建第二个active run。selection/OCR的现有background task和foreground
`cx.update_global`保持；不新增锁、channel、retry、timeout或shutdown hook。

窗口create/update/downcast失败后的“Conversation已提交但Run未启动”是本issue明确接受的失败边界：不创建普通窗口
fallback、不自动重试，保留临时历史可恢复性，并记录结构化错误。native reveal安排失败则Run已经启动且不重试。

## 7. 自动化测试

### `state/hotkey.rs`

当前已覆盖 snapshot 中 reasoning/approval 的 request 组装、capability mismatch 前置拒绝，以及临时快捷键
注册诊断；实际 selection/screenshot/OCR 到 Popup 的桌面交互验收不在本次执行范围内。

- `shortcut_create_request_uses_snapshot_run_controls`：构造 snapshot 后断言 reasoning 与三种 approval 均
  进入 `CreateConversationRequest`，不存在 hard-coded Ask。
- `shortcut_trigger_rejects_capability_mismatch_before_creation`：当前 model capability 与 snapshot 不兼容时
  返回错误且 repository conversation 数量不变。
- selection parts、direct image 和 OCR result 的成功分支最终都调用同一 finish helper；通过提取纯完成
  helper/测试 seam 断言，不复制窗口逻辑。

### `app/temporary_window.rs`

- 当前自动化测试覆盖显示器选择、窗口尺寸回退、失焦隐藏和 `ModalPanel` 窗口级别等纯函数边界。
- `show_created_conversation` 的已存在窗口/首次创建、Root downcast/update 失败、route/start-run/reveal
  顺序和 `start_run(false)` 幂等性目前由实现路径固定，尚未抽出可测试 seam。
- 不对 native reveal timing 编写测试；窗口类型、屏幕定位、失焦、前台 app restore，以及上述 lifecycle 顺序由
  第 8 节隔离 bundle 验收覆盖，统一归入 WP-110。

## 8. 隔离桌面验收

1. 使用 `/tmp/jaco-issue-175-qa` 作为 `JACO_CONFIG_DIR` 和 storage data dir，避免读取用户真实 DB/config。
2. bundle 后通过 Computer Use 确认启动的是当前工作树 `target/release/bundle/macos/Jaco.app`。
3. 在另一 app 中触发 selection/clipboard Shortcut：Jaco 主窗口不得创建或置前；popup 中出现新
   Conversation 并立即运行。
4. 对支持 image input 的 fixture model 触发 screenshot，确认 attachment 与自动运行进入同一 popup。
5. 对不支持 image、可 OCR 的 fixture model 重复，确认 OCR text 进入同一 popup。
6. 失焦隐藏，再触发新 Shortcut：复用同一 temporary window，重新定位并路由到新 Conversation；前一个
   run 不被重复启动。
7. 分别以 Auto Approve、Ask、Full Access 和至少两种 reasoning 设置触发，核对创建的 AgentRun snapshot
   与 Shortcut snapshot 一致。

## 9. Residual scan

实施完成后必须满足：

```bash
rg -n "TemporaryWindow|open_temporary_window|open_created_shortcut_conversation|downcast::<TemporaryWindow>" \
  app/jaco/src/state/hotkey.rs
```

结果应为空；通用 notification helper对`Root`的downcast不在扫描范围内，菜单仍可引用
`open_temporary_window`。另检查`state/hotkey.rs`中不再有快捷键专用的`ToolApprovalMode::RequestApproval`，
但不得把其他合法默认值误删。
