# gpui-form-macros：依赖升级计划

## Root Hub 与 owner 边界

- Plan ID：`issue-205`
- 状态：`In progress`（本地 trybuild/proc-macro 自动化通过；三平台 CI 待执行）
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[Workspace 依赖升级总计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Owner directory：`crates/gpui-form-macros`
- Root-owned surfaces consumed：`S-17`、`S-19`
- Owner-local IDs：`F-GFM-01`–`F-GFM-02`、`R-GFM-01`、`T-GFM-01`–`T-GFM-02`、`WP-GFM-01`
- Owns：proc-macro direct/dev dependency 声明和 trybuild 编译诊断 gate。
- Does not own：`gpui-form` runtime、adapter、workspace `Cargo.lock` 或 Issue #199 的既有 FormSchema 契约。

## 精确依赖目标

| Dependency | Current | Issue #205 target | Features/kind | Classification | Local use |
| --- | --- | --- | --- | --- | --- |
| `proc-macro2` | `1.0.107` | 保留 `1.0.107` | runtime proc-macro | Retained | `src/derive/attributes.rs`、`src/derive/expand*.rs` |
| `quote` | `1.0.47` | 保留 `1.0.47` | runtime proc-macro | Retained | `src/derive/expand*.rs` |
| `syn` | `3.0.3` | 保留 `3.0.3` | `full`, `extra-traits` | Retained | derive parser/model |
| `trybuild` | `1.0.118` | `1.0.120` | dev | Compatible | `tests/ui.rs:3` 与 `tests/ui/vnext/**` |

所有依赖继续来自 crates.io；不得借本次升级改变 derive grammar、generated API 或 `*.stderr` 诊断契约。

## Owner-local 目标与文件

```text
crates/gpui-form-macros/
├── Cargo.toml                                      # F-GFM-01 [Modify] trybuild 1.0.118 -> 1.0.120
├── tests/ui.rs                                     # F-GFM-02 [Verify only] trybuild harness
└── tests/ui/vnext/**/*.stderr                      # [Retain] 不盲目 bless 新诊断
```

`R-GFM-01`：新版 trybuild 必须继续按现有 pass/fail 集合验证 Rust 诊断；任何 snapshot 差异都要逐项确认是
预期的 Rust/tooling 输出变化，不能为了让测试通过而批量覆盖。

## WP-GFM-01：升级 trybuild 并锁定宏诊断

1. 将 `F-GFM-01` 的 dev dependency 精确更新为 `trybuild = "1.0.120"`，保留其余声明和 features。
2. 由 Cargo 更新 root lockfile，不修改 proc-macro source 或现有 FormSchema public contract。
3. 运行完整 UI suite；若 `*.stderr` 变化，先记录具体 fixture、旧/新诊断与根因，仅在确认为预期后纳入同一 WP。

完成条件：manifest 只出现目标版本 diff，所有 vnext fixture 仍得到预期 pass/fail 结果，无未解释 snapshot churn。

## Focused Validation 与 handoff

| T-ID | Command | Expected evidence |
| --- | --- | --- |
| `T-GFM-01` | `cargo test -p gpui-form-macros --all-features --locked` | unit/trybuild fixtures 全部通过 |
| `T-GFM-02` | `cargo clippy -p gpui-form-macros --all-targets --all-features --locked -- -D warnings` | proc-macro 与 test harness 无 warning |

若需要修改宏语义或 Issue #199 的 owner 文档，本 WP 立即停止并升级为独立行为迁移；聚合验证与 lockfile
completion evidence 由 root owner 记录。
