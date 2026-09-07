# gpui-heatmap：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/gpui-heatmap`；工作包：WP-02。
- 本地编号使用 `gpui-heatmap/F-*`、`gpui-heatmap/L-*`、`gpui-heatmap/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/lib.rs；README.md。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

迁移 GPUI/组件依赖及被影响的 theme/element API；保留活动日期、强度映射、hover/点击语义。只改确实失效的 import/API，不重新设计组件。

## 验证与完成条件

- 候选命令：`cargo test -p gpui-heatmap --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：既有热力图测试及 Jaco 用量视图消费者；无行为改动不新增成套 UI 测试。
