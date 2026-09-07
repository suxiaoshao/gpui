# gpui-tokio：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/gpui-tokio`；工作包：WP-02。
- 本地编号使用 `gpui-tokio/F-*`、`gpui-tokio/L-*`、`gpui-tokio/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：Cargo.toml；src/lib.rs。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

Q-02 已确认保留本地实现，只更新 GPUI 来源。保留 owned_runtime 与外部 handle 的分层、AbortOnDrop，以及后台等待 JoinHandle 的返回语义；不改为 Git 上游 crate。

## 验证与完成条件

- 候选命令：`cargo test -p gpui-tokio --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：现有实现缺少的关键回归可复用 #205 panic、drop abort、外部 handle 三类用例；选择单一测试 owner，避免在 HTTP Client 和本 crate 重复。

## C-03 已有边界

`init(cx: &mut App)` 创建并持有 runtime；`init_from_handle(cx: &mut App, handle: tokio::runtime::Handle)`
仅借用外部 handle。保持 `Tokio::spawn<C, Fut, R>(cx: &C, future: Fut) -> Task<Result<R, JoinError>>`，
其中 C: AppContext，Fut: Future<Output = R> + Send + 'static，R: Send + 'static。
GlobalTokio drop 仅关闭其 owned_runtime；GPUI Task drop 通过 AbortOnDrop 中止 Tokio 任务。
