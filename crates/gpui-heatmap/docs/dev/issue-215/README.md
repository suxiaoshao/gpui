# gpui-heatmap：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/gpui-heatmap`；工作包：WP-02。
- 本地编号使用 `gpui-heatmap/F-*`、`gpui-heatmap/L-*`、`gpui-heatmap/ST-*`；公共决定/版本不在此重定义。
- 2026-09-07 已按根计划授权实施；具体依赖、调用和验证结果见根计划的[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/lib.rs；README.md。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

迁移 GPUI/组件依赖及被影响的 theme/element API；保留活动日期、强度映射、hover/点击语义。只改确实失效的 import/API，不重新设计组件。

## 实施顺序与边界

1. 根计划中本 owner 的问题与技术门解除后，按固定版本/feature 更新 manifest；先落实依赖的生产方契约。
2. 执行上述本地迁移，删除已被替换的旧调用；不整文件覆盖 #205 分支，保留 main 后续变更。
3. 同步本 owner 当前使用文档及测试中的实际受影响示例。源码未受影响时保留原文，不制造格式 diff。
4. 执行下述最小充分验证，回填根计划的实施证据。若出现超出根契约的 API/行为变化，停止该项并记录问题。

## 验证与完成条件

- 候选命令：`cargo test -p gpui-heatmap --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：既有热力图测试及 Jaco 用量视图消费者；无行为改动不新增成套 UI 测试。
- 完成条件：目标依赖与必要调用迁移一致，旧路径不再作为有效实现，既有业务/安全边界不变；文档与实际代码相符。
- 实际命令、结果与平台/UI/E2E 边界由根实施记录统一记录；编译通过不等同手工验收。
