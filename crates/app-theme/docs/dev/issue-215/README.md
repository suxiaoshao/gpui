# app-theme：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/app-theme`；工作包：WP-02。
- 本地编号使用 `app-theme/F-*`、`app-theme/L-*`、`app-theme/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/lib.rs；Cargo.toml。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

适配 Theme/ThemeRegistry/highlighter 的目标路径与 API；保留系统强调色、用户主题选择及高亮主题唯一 owner，不能通过改写文本强制重解析。调用方初始化顺序由四应用负责。

## 验证与完成条件

- 候选命令：`cargo test -p app-theme --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：主题切换、亮暗外观及现有语法着色；不新建主题 Global 或复制主题状态。
