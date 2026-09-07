# http-client：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`app/http-client`；工作包：WP-03。
- 本地编号使用 `http-client/F-*`、`http-client/L-*`、`http-client/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/main.rs；src/features/request/body/http_text.rs；src/features/request/response.rs；src/features/request/tests.rs。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

将请求体代码输入与响应编辑器迁移到独立 EditorState/Editor；有 Form 绑定的请求体使用 FormEditor。保留语法语言选择、只读响应、查找、行号、请求/响应原始内容及任务取消。继续全语言 feature。

## 验证与完成条件

- 候选命令：`cargo test -p http-client --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：编辑请求体、切语言、响应查找/只读；本地测试服务请求取消；不改变既有媒体/PDF范围。
