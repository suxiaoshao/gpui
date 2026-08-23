# jaco-db 开发计划

这里登记 `crates/jaco-db` owner 的实施级计划。跨 owner 状态与共享契约由 workspace root plan 持有。

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#188](https://github.com/suxiaoshao/gpui/issues/188) Conversation rename 与 project batch archive | `In progress`；既有 schema 上的 rename 与原子 project soft-delete 已实现并通过 owner tests，随 root 等待人工 UI/远端 CI | [issue-188/README.md](issue-188/README.md) |
| [#189](https://github.com/suxiaoshao/gpui/issues/189) Message/context、Settings selected/activity与cost projections | `Implemented`；`WP-201`–`WP-205`已实施，本地DB手工迁移已由root完成 | [issue-189/README.md](issue-189/README.md) |
