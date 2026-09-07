# gpui-form-gpui-component：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/gpui-form-gpui-component`；工作包：WP-02。
- 本地编号使用 `gpui-form-gpui-component/F-*`、`gpui-form-gpui-component/L-*`、`gpui-form-gpui-component/ST-*`；公共决定/版本不在此重定义。
- 2026-09-07 已按根计划授权实施；具体依赖、调用和验证结果见根计划的[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/input.rs；src/lib.rs；tests/adapters.rs；README.md；README.zh-CN.md；docs/guide.md；docs/guide.zh-CN.md。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

保留 FormInput 单行能力，新增与原生 Textarea/Editor 匹配的 FormTextarea/FormEditor。复用现有 ControlBinding 与 writer 的值投影和事件路由；不得重引入 direction flag 或手工 FormEvent 订阅。

## 实施顺序与边界

1. 根计划中本 owner 的问题与技术门解除后，按固定版本/feature 更新 manifest；先落实依赖的生产方契约。
2. 执行上述本地迁移，删除已被替换的旧调用；不整文件覆盖 #205 分支，保留 main 后续变更。
3. 同步本 owner 当前使用文档及测试中的实际受影响示例。源码未受影响时保留原文，不制造格式 diff。
4. 执行下述最小充分验证，回填根计划的实施证据。若出现超出根契约的 API/行为变化，停止该项并记录问题。

## 验证与完成条件

- 候选命令：`cargo test -p gpui-form-gpui-component --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：来源控制不被同次写回、其他绑定收到值、程序写回/退役与延迟投影不覆盖较新输入；补最小新原生控制覆盖。
- 完成条件：目标依赖与必要调用迁移一致，旧路径不再作为有效实现，既有业务/安全边界不变；文档与实际代码相符。
- 实际命令、结果与平台/UI/E2E 边界由根实施记录统一记录；编译通过不等同手工验收。

## C-02 适配器目标契约

新适配器沿用现有 FormInput 的 new / try_new 与 Deref 结构，仅替换原生状态类型；不改变 String 业务值类型。
每个适配器保留 `subscriptions: Vec<Subscription>`、`_binding: ControlBinding`、`state: Entity<原生状态>`。
总路径 new 读初值并绑定；动态路径 try_new 返回现有 ResolveError。原生 change 通过 writer.defer_set，blur 通过
writer.defer_blur；外部值由 ControlProjection 静默投影，Retired 不重定向到新位置。

已核实 0.6.0 `InputState`、`TextareaState`、`EditorState` 均使用 `InputEvent`，`set_value`
为程序投影。`src/input.rs` 的 `text_control!` 生成三个独立公共适配器；完整声明以源代码为准，
公共用法已同步中英文 README/guide。新适配器回归验证中文多行写入、外部投影和 drop 后停止写回。
