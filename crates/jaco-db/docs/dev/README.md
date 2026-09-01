# jaco-db 开发计划

这里登记 `crates/jaco-db` owner 的实施级计划。跨 owner 状态与共享契约由 workspace root plan 持有。

| Issue | 状态 | 入口 |
| --- | --- | --- |
| [#188](https://github.com/suxiaoshao/gpui/issues/188) Conversation rename 与 project batch archive | `In progress`；PR [#208](https://github.com/suxiaoshao/gpui/pull/208) 已提交，owner tests 通过，随 root 等待人工 UI/远端 CI 结果 | [issue-188/README.md](issue-188/README.md) |
| [#189](https://github.com/suxiaoshao/gpui/issues/189) Message/context、Settings selected/activity与cost projections | `Implemented`；`WP-201`–`WP-205`已实施，本地DB手工迁移已由root完成 | [issue-189/README.md](issue-189/README.md) |
| [#193](https://github.com/suxiaoshao/gpui/issues/193) Conversation recency fresh schema 与排序 | `In progress`；production 与本地自动化完成，等待 root 验收 | [issue-193/README.md](issue-193/README.md) |
| [#196](https://github.com/suxiaoshao/gpui/issues/196) stable attachment ID 与 prelinked entry batch | `Implemented locally`；DB owner 已实现 caller-assigned ID、ordered entry/attachment transaction 与 generated-file index，schema 保持不变 | [issue-196/README.md](issue-196/README.md) |
