# app-theme 依赖升级计划

- 状态：`In progress`（本地自动化通过；消费应用三平台/人工 smoke 待执行）
- Owner：`crates/app-theme`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 拥有现有完整组件应用的 Material You/system-accent 主题生成与应用。它不成为
Lestty 的主题层，也不在 normal dependencies 中声明 `gpui-base`；它通过
`gpui-component::Theme` 的公开投影 API 同步 base-owned scrollbar/resize handles。

## 精确依赖与已知命中

`crates/app-theme/Cargo.toml:12-17` 的完整 direct audit：

| Dependency | Current | Target / disposition |
| --- | --- | --- |
| `gpui` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |
| `gpui-component` | 0.5.2 @ `57a9903f...` | 0.5.2 @ `5e5a1a30...` |
| `material-color-utils` | 0.1.3 | current |
| `platform-ext` | optional workspace path | current |
| `serde_json` | 1.0.151 | current |
| `smol` | 2.0.2 | current |

升级继续继承完整组件，不声明 production `gpui-base`。测试 target 额外通过
`gpui = { workspace = true, features = ["test-support"] }` 开启 `TestAppContext` 与
`#[gpui::test]`，并以 `gpui-base.workspace = true` 作为 dev-dependency 读取 base theme global，
精确验证 `Theme::sync_base` 投影；两者都不改变 app-theme 的 normal dependency graph。
Lestty 仍是 workspace 中唯一 normal/direct `gpui-base` consumer。`current` 来自 2026-08-20
root audit，实施当天重审。

目标组件中 `Theme::change` 会自动重建 base projection，但直接
`Theme::global_mut(cx)` 后调用 `apply_config` 不会。已确认命中：

- `crates/app-theme/src/lib.rs:338-342` 的 `apply_fixed_system_accent_theme` 直接修改 global
  theme；必须在可变借用结束后调用 `Theme::sync_base(cx)`。
- `src/lib.rs:207-215` 的 `preview_theme` 只修改局部 `Theme` 值，不写 global，不应调用
  `sync_base`。
- Jaco 另有 `app/jaco/src/state/theme.rs:115` 的 direct global mutation，由 Jaco owner 修复。
- Feiwen 在 `app/feiwen/src/app/workspace.rs:137` 消费本 crate 的 fixed system-accent 路径，
  是公共修复的首要回归消费者。
- 目标 schema 将 `scrollbar_show` 改名为 `scrollbar_mode` 且提供 serde alias；本 crate 的
  material mapping 使用 `ThemeColor` scrollbar colors，不命中旧字段名，但要做反序列化回归。

## 工作包

### THEME-DEP-1：同步 base projection

- 在 `apply_fixed_system_accent_theme` 完成 `apply_config` 后调用 `Theme::sync_base(cx)`；确保
  mutable borrow 已释放，避免同时借用 global。
- 不直接写 `gpui_base::Theme`，由完整组件继续拥有 projection 规则。

### THEME-DEP-2：锁定行为测试

- 新增真实 `TestAppContext` 测试：初始化 `gpui_component`，通过 `Theme::apply_config` 应用
  固定 accent 配置并调用 `Theme::sync_base`，比较完整组件 theme 与公开 base projection 的
  semantic colors、radius、scrollbar mode 和 resizable handle 关键值。
- `gpui-base` 仅作为该 projection 回归的 dev-dependency test seam；不得暴露到 app-theme
  production API 或 normal dependency graph。
- `gpui/test-support` 仅由 dev-dependency 为测试 target 启用；normal `gpui` dependency 不开启
  该 feature。
- 保留 `preview_theme` 的纯值测试，证明 preview 不依赖 global 初始化。
- 验证旧 `scrollbar_show` 配置仍能通过 alias 读取，写出时使用新字段名。

### THEME-DEP-3：消费侧 smoke

- Feiwen：system accent 应用后 scrollbar 与 resize handle 立即更新。
- Jaco：自有主题路径与本 crate 路径都同步，并在 light/dark 切换后保持一致。

## Focused verification

```text
cargo check -p app-theme --all-features --locked
cargo test -p app-theme --all-features --locked
cargo clippy -p app-theme --all-targets --all-features --locked -- -D warnings
cargo test -p feiwen --locked
cargo test -p jaco --locked
```

## 完成条件

- 每个 global theme direct-mutation 路径都显式同步 base projection。
- preview 仍是无 global 副作用的纯值构造。
- app-theme 没有 normal/direct `gpui-base`；唯一 direct edge 是 projection 回归专用的
  dev-dependency。Jaco/Feiwen 的主题与窗口 chrome smoke 通过。
