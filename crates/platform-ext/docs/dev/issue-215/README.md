# platform-ext：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/platform-ext`；工作包：WP-05。
- 本地编号使用 `platform-ext/F-*`、`platform-ext/L-*`、`platform-ext/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；build.rs；winmd/。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

bindgen 保留 0.66.0，使用该版本已有的 --no-allow 替代生成后固定字符串裁剪。0.100.0 移除既有参数且生成 RuntimeType::NAME，与保留 windows-core 0.62.2/windows-future 0.3.2 不配套。winmd 作为既有输入保持，不手改 OUT_DIR 产物。

## 验证与完成条件

- 候选命令：`cargo check -p platform-ext --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：Windows 主机实际执行 build.rs 和 OCR/原生边界编译；macOS 检查不算 Windows 生成验证。
