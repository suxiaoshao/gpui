# Gupi：配置与启动运行时契约

所属 [应用计划](README.md)，根状态与未决问题见 [根计划](../../../../../docs/dev/issue-218/README.md)。本文拥有 L-110 至 115、ST-110 至 113、R/T-110 至 113，由 WP-101 至 WP-103 实施。声明为目标契约；上游 API 的事实与应用自定义类型分开说明。

## E-110：已核对的 API

- `crates/gpui-operation/src/{lib,message,repair,refresh}.rs`：完整 Operation 接受 Transition 消息；Ready 可 Refresh，问题态通过显式 Repair 恢复；Complete 只用于运行态。Operation 拥有 Task，不负责启动或磁盘回滚。
- `app/jaco/src/state/config.rs`：ConfigOperation 放在 Store 中，数据携带配置与源字节，修复选择包括重读、重试写入与备份覆盖。
- `app/jaco/src/features/home/root.rs`：JacoRoot 订阅资源状态，根据数据可用性呈现内容，保留 HomeView Entity。
- `crates/gpui-form-gpui-component/src/{input,select}.rs`：实际适配器为 FormInput、FormSelect，通过 new 接收 owner、typed path 和 control builder。最终泛型调用受依赖升级后的类型一致性检查约束。

## L-110 / ST-110：唯一配置权威

以下类型位于 `src/state/config.rs`；使用 `gpui_kit::Task` 和 repo `gpui_store::Store`，不再并列安装 Store<AppConfig>。

```rust
pub(crate) enum ConfigContents {
    Missing,
    Configured(AppConfig),
}
pub(crate) struct ConfigData {
    path: PathBuf,
    contents: ConfigContents,
    source_bytes: Option<Vec<u8>>,
}
pub(crate) type ConfigOperation =
    repair::Operation<ConfigData, ConfigProblem, ConfigRepair, Task<()>>;
pub(crate) type ConfigStore = Store<ConfigOperation>;

pub(crate) enum ConfigRepair {
    Reload,
    SaveDraft,
    WriteCommitted,
    BackupAndReset,
    BackupAndWrite,
}
```

Missing 必须对应 source_bytes=None；Configured 对应成功读入或写入的完整字节。初次文件不存在可返回 Ready(Missing)，表示读取事实可靠，不代表配置已应用。运行时主题和语言从唯一 Form 草稿投影，立即预览；持久化权威仍为 Configured。引导末页可显式检测草稿中的 Pi，完成按钮要求检测结果与当前命令匹配且成功；正常启动从 Configured 检测。运行中已有 Configured 后的 NotFound 返回错误，保留旧数据，不能完成为 Missing。

ConfigController 是唯一命令入口，持有 ConfigStore 及设置表单弱引用；根组件/主题持有订阅。Store 按应用寿命存活，完成回调更新同一个 Store，不由 Task 强引用 Store 形成环。features/settings.rs 的 SettingsView 只拥有编辑 Form；关闭窗口不销毁配置任务。

ConfigRepair 只记录操作意图；提交快照由运行任务持有，失败后释放。错误上下文保留写入来源、冲突及备份结果，不保存可重放的旧草稿。具体定义和准入见 L-114。

## L-111：操作准入、转移与表单结果

| 当前状态 | 用户/系统动作 | 合法转移与结果 |
| --- | --- | --- |
| Idle | 启动读取 | Load(task) → Loading → Complete |
| Ready(Missing) | 保存初始表单 | 校验通过后 Refresh(task) → Refreshing → Complete |
| Ready(Configured) | 保存草稿、写回当前配置、重读 | 先捕获操作输入，再 Refresh(task) → Refreshing → Complete |
| Unavailable / Degraded | 用户选修复 | 检查问题是否支持该选择，再 Repair { repair, task } → Repairing… → Complete |
| 任意运行态 | 编辑、保存、重读、写回、重置 | 控件禁用，handler 拒绝，不构造任务、不排队、不发非法 Transition |
| 成功完成 | 提交新数据 | Ready(new_data)，按操作种类更新表单 |
| 失败完成 | 保留可用数据 | 无旧 Data 为 Unavailable；有旧 Data 为 Degraded |

Ready→Refresh 是库的任务生命周期消息。应用将“保存/重读”等业务意图保留在任务闭包中，以决定完成时如何处理表单，不用 phase 名猜测操作种类。

Degraded(Missing) 仍无可用配置，初次保存失败可能落到此状态：根界面应展示初始设置的错误与修复，不能因为 Operation.data().is_some() 就放行。Degraded(Configured) 则保留已应用配置和当前界面。这是 Gupi 与直接复制 Jaco `has_data` 判断的关键差异。

任务开始前在同一次 owner update 中检查状态、生成输入、构造任务、安装转移；中间不 await。所有配置读写互斥，无需额外读写 generation。完成后先转移到稳定状态，再通知订阅与更新表单。

| 操作 | 成功后的表单 | 失败后的表单 |
| --- | --- | --- |
| 保存草稿 | 以实际提交值 rebase；运行期间禁止编辑 | 保留草稿 |
| 主动重读 | 若 dirty，开始前确认舍弃；成功才用读入值 rebase | 保留草稿 |
| 当前内存写回 | 草稿和值/基线都保留 | 保留草稿 |
| 备份重置 | 明确确认后以新配置 rebase | 保留草稿，显示已完成的备份等部分结果 |

保存失败后继续编辑，使用同一个保存入口提交当前表单。问题态的保存使用 Repair { repair: SaveDraft, task }，不能向 Degraded 发送 Refresh；按 L-114 检查冲突与备份要求。

## L-112 / ST-112：Pi 数据与页面投影

```rust
pub(crate) struct PiProbeData {
    command: PathBuf,
    version: String,
}
pub(crate) type PiOperation =
    refresh::Operation<PiProbeData, ProbeFailure, Task<()>>;
```

根视图分别持有已应用命令和设置草稿的两个 Pi owner；每个 owner 独立持有完整 Operation、任务和错误。启动及重试只使用已应用 owner，设置页的检测只使用草稿 owner；草稿检测不会替换启动状态或使恢复设置页消失。首次检查使用 Load，Ready 使用 Refresh，Unavailable 使用 Retry，Degraded 使用 Refresh。同一输入重复点击只在已结算状态准入。命令变化通过 Cancel 丢弃旧 Task，立即为新输入从 Idle 开始；不得将旧命令版本作为新命令的有效 Data 保留。

Operation 的 Cancel 恢复之前的稳定态并丢弃 Task，GPUI/Tokio 的 abort-on-drop 链切断旧完成回调，不另加 generation 或取消错误。检查开始即显示 loading。

查找与创建进程通过 spawn_blocking 执行；应用级 Semaphore 最多放行两项，名额由实际阻塞任务持有至结束。15 秒 deadline 从 controller 提交时建立，覆盖名额排队、查找、启动及读输出。接收通道由探测 future 持有：取消或超时会关闭接收端；后台任务在查找前后检查取消与 deadline，取消后不再启动，无法交接或已超时的 Child 通过 kill_on_drop 终止。阻塞任务不直接回调 owner。已交接 Child 的失败路径显式 kill/wait，收尾额外限制 2 秒，失败写日志并 Drop 兜底。取消只保证停止等待并触发终止，不保证同步系统调用立即返回或操作系统已回收全部资源。界面不直接启动子进程。Pi 检查的成功条件仅为命令/版本契约，实际 RPC 兼容性由 #219 验证。

features/startup.rs 的根视图持有已有页面 Entity 并订阅配置、布局、Pi；正常业务外壳归 features/home.rs，共用恢复布局归 components/recovery.rs。每次 render 从这些权威数据决定内容；不维护可修改的 StartupRoute。对 Configured 的读取成功与布局可恢复条件满足后，仅当已应用 owner 的成功结果对应当前已应用命令且无运行任务或错误时，才放行主界面；运行中配置重读失败保留已应用界面及设置问题提示。

## L-113 / ST-113：受控退出

采用应用级 Running/Draining 退出阶段，它仅表达应用退出，不复制配置 Operation。Quit 将阶段置为 Draining，拒绝新配置/Pi 操作，显示退出进度；重复 Quit 只激活现有窗口。

已有配置写入继续保留在其 Operation 中直至 Complete，禁止通过 Cancel 或销毁 owner 中断。已应用与草稿两种版本探测同时执行 Cancel；退出不等同步阻塞返回，也不等待取消后的 Complete。已持有或晚交接的子进程沿 Drop 链触发终止。布局恢复任务也需结算，然后保存布局，最后调用 cx.quit。退出协调任务由根视图的 quit_task 持有，布局恢复任务由 layout_task 持有；协调者观察运行操作是否结束，不将同一个 Task 从 Operation 取出并复制到其他 owner。应用不使用 detach 维持业务任务。

布局保存失败只写日志，静默允许退出，不弹通知或要求确认。用户主动提交的配置保存失败在设置页保留错误和草稿；用户随后明确退出时正常退出，不因该错误拦截，不弹二次确认、不自动重试、不额外持久化草稿。写入未返回时按 L-114 保持运行所有权，不伪造回滚。系统强制终止不在普通 Quit 的完成保证内。

## 补充验证

| 要求 | 测试 | 必须断言 |
| --- | --- | --- |
| R-110 Missing 与可用配置区分 | T-110，配置/启动状态测试 | Ready(Missing)、Degraded(Missing) 均不启动 Pi、不进入主界面；Configured 才提供设置投影 |
| R-111 准入与数据完整 | T-111，配置 controller 测试 | 运行期间所有竞争操作被拒绝；失败保留数据；修复使用合法消息；内存写回不清空 dirty 草稿 |
| R-112 Pi 输入归属 | T-112，fixture 进程测试 | 命令 A 改 B 后 A 不得放行 B；取消 A 后 B 进入检查；A 的迟到结果不得覆盖 B；后台启动不超过两项；取消不产生错误提示 |
| R-113 退出时所有权 | T-113，应用级测试/手工 | 写入中 Quit 不丢 Task；重复 Quit 不产生第二条退出链；已有提交任务结算及布局保存尝试结束后退出；保存失败不拦截或弹二次确认 |

上述测试补充应用计划 T-100 段已有场景，实施时合并相同覆盖，不重复建立平行测试集。

## L-114：写入失败后的再次保存

```rust
pub(crate) enum ConfigWriteSource { Draft, Committed }
struct PendingConfig {
    value: AppConfig,
    bytes: Vec<u8>,
    expected_source: Option<Vec<u8>>,
    source: ConfigWriteSource,
}
```

PendingConfig 位于 state/config.rs，只表示本次运行任务的不可变输入。每次保存都重新 prepare、校验并捕获当前草稿；失败后保留表单，不提供独立的重试写入按钮。错误只记录 Draft/Committed 来源与备份结果。发生冲突后，备份覆盖在用户确认时重新捕获当前来源：Draft 使用当前表单，Committed 使用已应用配置并保留草稿；不能恢复失败前的旧表单快照。

| 问题 | 可用动作 | 约束 |
| --- | --- | --- |
| 配置读取/解析失败 | Reload；明确的重新设置/备份重置 | 原文件存在时替换前备份；目录/权限不满足则停止 |
| 写入前失败，outcome=Unchanged | 保存当前草稿、写回已应用配置、Reload | 原有数据仍有效；每次写入重新取值并比较 expected_source |
| 外部版本冲突 | Reload、明确 BackupAndWrite | 不提供绕过版本检查的普通重试 |
| 备份成功后写入失败 | 保存当前草稿、写回已应用配置、Reload | 保留并展示已有备份结果；再次核对磁盘，不虚报整个操作未产生效果 |
| outcome=NeedsReconcile | Reload | 禁用保存/覆盖；先核对当前磁盘，结果明确后再开放提交 |

未返回的写入继续处于 Refreshing 或 Repairing…，Task 和旧数据由 Operation 持有，控件禁用。不能因 UI 等待超时直接发 Complete(Err) 后放开重试，留下后台旧写入继续竞争。提交返回但结果需核对时，Complete(Err(NeedsReconcile)) 转为问题态；下一步用户选 Reload 经 Repair 收敛实际磁盘结果。

Jaco `database/operation.rs` 的 Retiring 是专门的数据库会话退役阶段，证明需要收尾的业务资源应在收尾完成后才允许修复；Gupi 配置本阶段可用现有 repair 家族和上述错误上下文表达，不新增同构 Operation。若后续确有共享家族无法表达的生产状态，再基于具体资源设计，不能因“复杂”先扩充状态数。

## L-115：超时先于执行建立

Pi 探测在创建任务前确定 15 秒 deadline，统一约束排队、查找、创建进程和读输出，不在步骤切换或每次收到输出时重新计时。到期生成 ProbeFailure::Timeout；按 L-112 对已交接 Child 进行最多 2 秒的 kill/wait，再向 PiOperation 发送 Complete(Err)。清理失败保留日志诊断并 Drop 兜底，不虚报成功回收。

超时错误使首次探测成为 Unavailable；同一命令已有成功值时成为 Degraded。界面明确呈现超时并提供合法 Retry/Refresh。运行中重复触发仍被拒绝。该策略仅用于可终止的版本探测，不套用于不可撤销的文件提交。

后续 Pi RPC 自身返回的模型/工具错误按原意展示，宿主探测超时、进程退出和通信中断保持独立来源；“还在执行”本身不等于 Pi 已报错。本轮不实现第二阶段行为。

验证补充：T-111 覆盖冲突无普通重试、NeedsReconcile 只允许核对、备份成功后的失败保留结果；T-112 覆盖先建立 deadline、超时清理完成后发布错误；T-113 覆盖布局保存失败静默退出。合并到原测试场景，不增加重复门禁。
