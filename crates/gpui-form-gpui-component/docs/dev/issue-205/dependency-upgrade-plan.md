# gpui-form-gpui-component 依赖升级计划

- 状态：`In progress`（本地 adapter 与消费侧自动化通过；三平台 CI 待执行）
- Owner：`crates/gpui-form-gpui-component`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 是 `gpui-form` 与完整 `gpui-component` 控件的 adapter owner。它负责适配新版本将
单行 input、普通 textarea 与 code editor 分成具体 state 的 API；它保留完整组件依赖，不
改成 direct `gpui-base`，也不承担 Lestty UI。

## 精确依赖与已知命中

`crates/gpui-form-gpui-component/Cargo.toml:7-12` 的完整 direct audit：

| Edge | Current | Target / disposition |
| --- | --- | --- |
| normal `gpui` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |
| normal `gpui-component` | 0.5.2 @ `57a9903f...` | 0.5.2 @ `5e5a1a30...` |
| normal `gpui-form` | workspace path 0.1.0 | current |
| dev `gpui/test-support` | 同 normal GPUI source | 0.2.2 @ `e0931d5a...` |

没有其他 registry direct dependency；升级后不增加 `gpui-base`。

当前 `src/input.rs` 的硬编码边界：

- 4 行只导入 `InputEvent, InputState`；9-13 行 `FormInput` 保存 `Entity<InputState>`；
- 16-60 与 62-104 行的 `new`/`try_new` builder 都固定
  `Context<InputState> -> InputState`，但分别复制同一套 initial value、projection、change、
  blur 与 retired binding 生命周期；
- 106-112 行 `Deref<Target = Entity<InputState>>`；`src/lib.rs:11` 只导出 `FormInput`。

目标组件 re-export `InputState`、`TextareaState`、`EditorState` 以及 styled `Input`、
`Textarea`、`Editor`。`InputState::multi_line` 和 `InputState::code_editor` 不再是构造入口：
textarea 使用 `TextareaState::new`，code editor 使用 `EditorState::new().language(...)`。

直接消费者：

- Jaco prompt content 需要 `FormTextarea`；
- HTTP Client raw body editor 需要 `FormEditor`；
- 所有现有单行字段与 `FormIntegerInput` 继续使用 `FormInput`/`InputState`。

## 工作包

### ADAPTER-DEP-1：提供三个具体 adapter

- 保留 `FormInput` 公开 API，并新增 `FormTextarea` 与 `FormEditor`；各自持有具体
  `Entity<State>` 并实现具体 `Deref`。
- 用私有、受控的代码生成或 helper 共享 `new`/`try_new` 的 binding lifecycle；公共 API
  仍是三个具名类型，不暴露 type-erased state，也不让消费者传 mode boolean。
- 统一处理 `InputEvent::Change`、`Blur`、`Focus`、`PressEnter`，保持 defer_set/defer_blur
  次序和 retired occurrence 安全。

### ADAPTER-DEP-2：公开导出与消费者迁移门

- 从 `src/lib.rs` 导出三种 adapter；保持现有 `FormInput` 和 integer/select/combobox API。
- 先交付 adapter 与 tests，再由 Jaco/HTTP Client owner 迁移；禁止消费者复制 binding 代码。

### ADAPTER-DEP-3：测试三种 state 的等价 contract

- 对三种 adapter 分别覆盖 initial form value、state change 写回、form projection 更新 state、
  blur validation、retired projection 与 owner/window drop。
- Editor case 额外断言 language/highlighter 更新不破坏 binding；Textarea case 断言换行值完整写回。
- 保留 IntegerInput tests，确认单行 `InputState` 的 number APIs 不受影响。

## Focused verification

```text
cargo check -p gpui-form-gpui-component --locked
cargo test -p gpui-form-gpui-component --locked
cargo clippy -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
cargo test -p gpui-form --all-features --locked
cargo check -p jaco -p http-client --locked
```

## 完成条件

- 三种 text state 均有类型安全的 form adapter 和等价 lifecycle tests。
- crate 仍直接依赖完整 `gpui-component`，没有 direct `gpui-base`。
- Jaco prompt 与 HTTP raw body 不再调用已移除的 mode builders，其他单行消费者无需改动。
