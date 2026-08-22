# Novel Download 依赖升级计划

- 状态：`In progress`（本地自动化与 Windows bundle 通过；三平台/人工 smoke 待执行）
- Owner：`app/novel-download`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

Novel Download 保留完整 `gpui-component`，不迁移到 `gpui-base`。本 owner 负责应用启动、
下载表单、进度/取消状态与窗口行为的消费侧升级验证。

## 精确依赖与已知命中

`app/novel-download/Cargo.toml:24-30` 直接依赖 `gpui`、`gpui_platform`、
`gpui-component`、`gpui-component-assets`、`gpui-form`、`gpui-form-gpui-component` 与
`gpui-operation/tracing`；64 行的 dev dependency 开启 `gpui/test-support`。升级后继续继承
root 统一版本，不增加 `gpui-base`。

非 GPUI direct 目标是 `futures 0.3.33 -> 0.3.34` 与
`thiserror 2.0.19 -> 2.0.20`。`reqwest 0.13.4`、`scraper 0.27.0`、`async-stream 0.3.6`、
`async-compat 0.2.5`、`smol 2.0.2` 等在 2026-08-20 root audit 中保持 current；实施当天重审。

- `app/novel-download/src/main.rs:25` 调用 `gpui_component::init(cx)`；目标组件已经 transitively
  初始化 base，应用不得再调用 `gpui_base::init`。
- `main.rs:76,83-92` 使用 component assets、`WindowOptions` 和 `Root`，因此必须保留完整组件。
- `features/workspace.rs:63-64,85-88,233` 的 source control 是单行
  `FormInput`/`InputState`/`Input`，不命中 `.multi_line` 或 `.code_editor` 移除；保持该模型。
- `features/workspace.rs:654-657` 的 test setup 直接调用 `gpui_component::init`，继续作为唯一
  测试初始化入口。

## 工作包

### NOVEL-DEP-1：消费统一依赖

- 保留完整组件、assets、form 和 operation 依赖；确认只有一个 Zed 与 longbridge Git 身份。
- 不新增 direct `gpui-base`，不更改生产和测试中的组件 init 边界。

### NOVEL-DEP-2：表单与任务回归

- 在 form adapter 升级后重跑 source input 的 change/blur/validation 行为。
- 保持运行 snapshot、取消和 window removal 的 task ownership 语义，不因 GPUI Task API 变化
  引入 detach 或兜底分支。

### NOVEL-DEP-3：窗口 smoke

- 启动应用并验证 Root、输入焦点、下载进度、取消与关闭窗口。
- 检查完整组件的 dialog/notification/scrollbar 视觉与交互未回退。

### NOVEL-DEP-4：stream 与 error cohort

- 更新 futures 后覆盖 worker stream 的 item 顺序、取消、terminal completion 与 window drop。
- 更新 thiserror 后覆盖 unsupported source、HTTP/parser/I/O problems 的 source chain 和本地化映射。

## Focused verification

```text
cargo check -p novel-download --locked
cargo test -p novel-download --locked
cargo clippy -p novel-download --all-targets --all-features --locked -- -D warnings
cargo tree -p novel-download --duplicates --locked
```

重点保留 `features/workspace.rs` 中 form validation、running snapshot、cancel、window removal 与
localized terminal-problem tests，并做一次真实 UI 下载/取消 smoke。

## 完成条件

- 应用与测试都只初始化完整组件，没有 direct `gpui-base`。
- source form 和任务生命周期无回归。
- futures 0.3.34 与 thiserror 2.0.20 的 stream/error focused cases 通过。
- package 验证与三平台 CI 通过。
