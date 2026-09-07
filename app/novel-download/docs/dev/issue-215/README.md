# novel-download：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`app/novel-download`；工作包：WP-03、WP-05。
- 本地编号使用 `novel-download/F-*`、`novel-download/L-*`、`novel-download/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/main.rs；src/features/workspace.rs；Cargo.toml。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

迁移初始化、assets 包名和相关测试。普通依赖只调整已列候选及必要 API；保留下载控制、进度、失败及取消语义，不扩大抓取范围。

## 验证与完成条件

- 候选命令：`cargo test -p novel-download --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：组件图标与基本窗口启动；既有下载/取消测试，不访问真实抓取入口。
