# app-assets 依赖升级计划

- 状态：`In progress`（本地自动化通过；消费应用人工 smoke 待执行）
- Owner：`crates/app-assets`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)

## Owner scope

本 crate 拥有现有应用的 SVG icon trait、app-local embedded assets 与
`gpui-component-assets` fallback。它继续服务使用完整 `gpui-component` 的应用；不服务
Lestty，也不迁移到 `gpui-base`。

## 精确依赖与已知命中

`crates/app-assets/Cargo.toml:7-10` 的完整 direct audit：

| Dependency | Current | Target / disposition |
| --- | --- | --- |
| `app-assets-macros` | workspace path，crate 0.1.0 | current，继续 path edge |
| `gpui` | 0.2.2 @ `1a246efd...` | 0.2.2 @ `e0931d5a...` |
| `gpui-component` | 0.5.2 @ `57a9903f...` | 0.5.2 @ `5e5a1a30...` |
| `gpui-component-assets` | 0.5.1 @ `57a9903f...` | 0.5.1 @ `5e5a1a30...` |

本 crate 没有其他 registry direct dependency，不增加 `gpui-base`。完整 SHA 由 root plan 与
lockfile 拥有，表中短 SHA 仅用于识别同一 source identity。

- `crates/app-assets/src/lib.rs:14` 的公开 `SvgIconNamed` 继承
  `gpui_component::IconNamed`，这是完整组件 API 边界，不能改成 base trait。
- `src/lib.rs:18-39` 先解析 app-local SVG，再 fallback 到
  `gpui_component_assets::Assets`；升级必须保留查找顺序和 missing asset 语义。
- `src/lib.rs:61-72` 实现 GPUI `AssetSource`；89-193 行测试同时覆盖 component icon trait 与
  embedded source。
- Lestty 若需要图标必须拥有自己的 asset source；依赖本 crate 会违反其“不拉入完整组件”门禁。

## 工作包

### ASSET-DEP-1：保持组件边界

- 更新继承的 workspace dependencies，但保留 `IconNamed` supertrait 和 component-assets fallback。
- 若上游 icon 路径或 asset API 变化，只在本 crate 适配一次；现有应用不复制 fallback。

### ASSET-DEP-2：验证资源解析

- 覆盖 app-local 命中、component fallback、缺失资源、`list` 与路径 normalization。
- 对 Jaco/Feiwen 各做一个实际 icon smoke，确认自有 icon 和 Lucide fallback 都能加载。

## Focused verification

```text
cargo check -p app-assets --locked
cargo test -p app-assets --locked
cargo clippy -p app-assets --all-targets --all-features --locked -- -D warnings
cargo tree -p app-assets --locked
```

通过条件：依赖图含完整 `gpui-component`/assets 且不含 direct `gpui-base` 声明；asset tests 与
两个消费应用的 icon smoke 通过。

## 完成条件

- 公开 trait 与 fallback 行为保持兼容。
- Jaco/Feiwen 资源加载无回归，Lestty 依赖图不出现本 crate。
