# gpui-form：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/gpui-form`；工作包：WP-02、WP-05。
- 本地编号使用 `gpui-form/F-*`、`gpui-form/L-*`、`gpui-form/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；README.md；README.zh-CN.md；docs/guide.md；docs/guide.zh-CN.md。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

只迁移 GPUI 依赖来源/dev features 与 trybuild 候选；维持 Form/ControlBinding/动态路径/prepare-rebase 全部核心契约。例子若新增 FormTextarea/Editor 使用，保持中英文一致；不得将上游 history 改造扩散成 Form 重构。

## 验证与完成条件

- 候选命令：`cargo test -p gpui-form --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：复用已有绑定、动态退役、快照验证及 compile-fail；不机械重录 stderr 掩盖语义变化。
