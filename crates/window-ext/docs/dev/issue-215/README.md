# window-ext：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/window-ext`；工作包：WP-02、WP-05。
- 本地编号使用 `window-ext/F-*`、`window-ext/L-*`、`window-ext/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

GPUI 来源与 thiserror 候选更新；保留原生窗口获取、macOS/Windows 特殊操作及错误语义。上游无法证明等价时不删除本地实现。

## 验证与完成条件

- 候选命令：`cargo check -p window-ext --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：平台主机验证直接消费者；未运行的平台注明，不以另一平台编译代替。
