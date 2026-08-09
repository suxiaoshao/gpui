# Jaco：迁移到 gpui-form 新破坏性 API 的实施计划

## 状态、边界与生产者门禁

- 状态：`Done`（2026-08-09）；已消费 `C-900`–`C-904`，实际 UI 操作测试按范围未执行。
- 关联 issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 本文：Issue #199 下 Jaco 的新一轮消费方迁移计划；不改写历史 `form-migration.md` 与
  `form-vnext-migration.md`。
- 所有者：`app/jaco`
- 本地编号：`E/D/F/L/ST/ERR/R/T-1200..1299`、`WP-1200..1209`。
- 实际 UI 操作测试：明确不执行；以自动化测试和静态残留扫描验收。

本文由上层 Issue #199 总计划引用，并保持为独立专题文档；本目录与根目录的 `README.md` 只登记状态和链接，
不混入本计划正文，也不改写两份历史迁移文档。

### 消费的核心生产者门禁

| 门禁 | Jaco 依赖的最终能力 | 开始消费的条件 |
| --- | --- | --- |
| `C-900` | `FormSchema`、无失败构造、total/dynamic path、运行时 occurrence、resolver 与不透明 `PathKey` | schema/path/topology fixture 通过 |
| `C-901` | `ChangeSet` 映射与 `FormEvent<M>`/`ModelChange`/`PathImpact` | 值/结构/退休 impact 与一次发布语义通过 |
| `C-902` | 不可 clone 的 binding、`ControlWriter`、`ControlProjection` 与四个内建 adapter | 自回传抑制、投影合并、动态退休 adapter 测试通过 |
| `C-903` | 快照 `Validator`、`ValidationTrigger::External`、`Prepared<M>`、`FormVersion`、`rebase_if_current` | validation/CAS/prepare fixture 通过 |
| `C-904` | 私有 `Transition` 与原子发布 | producer 聚焦门禁已通过；Jaco 迁移后共同推进到 `consumer-complete` |

**兼容策略：** Jaco 与新 Form 同轮 breaking 迁移，不保留旧 `typed` module import、`try_new*`、`value`、
`try_value`、旧 `FormEvent` pattern、裸 `FormRevision` CAS、`ControlLease` 或 cloneable binding 的兼容层。

## 目标与非目标

### 目标

1. 所有 Jaco Form 消费方只使用 `C-900`–`C-904` 的最终公开 API。
2. 用 `PathImpact` 取代当前按旧 `Committed { path }` / `ModelReplaced` 的事件过滤；只对真正受影响的
   应用投影执行副作用。
3. 保存型表单将 `Prepared` 携带的 `FormVersion` 原样保留到异步完成，并使用
   `rebase_if_current(version, canonical, cx)`；保存期间的新编辑不被覆盖。
4. 删除 Jaco 自建的 Form 事件 → 原生 setter、`ControlLease` 和 binding clone 路线，改用 core binding
   的 projector/writer 协议。
5. MCP 设置编辑器继续按 Form runtime `PathKey` reconcile 动态行；只在结构变化/退休时重建相关行。

### 非目标

- 不改 Jaco Conversation、agent run、MCP runtime、MCP OAuth runtime 或任何 `gpui-operation` 状态机。
- 不改数据库、provider/prompt/model catalog、secret 存储、MCP 配置 payload、UI 布局、Fluent key、图标。
- 不把 provider catalog 刷新变成 Form rebase，也不重做 provider total binding 的业务规则。
- 不修复 `gpui-component#2652`，也不在 Jaco 加 selection 补偿或方向布尔值。

## 已核实的当前事实

| ID | 当前事实 | 证据 | 迁移后果 |
| --- | --- | --- | --- |
| `E-1200` | ChatInput 以 `Form::try_new` 构造，并按 `Committed` path/`ModelReplaced` 保存 settings、投影附件 | `components/chat/input.rs:218-332` | 构造与事件 impact 必须迁移 |
| `E-1201` | RunSettings、provider secret、Shortcut prompt/hotkey 直接持有 `ControlLease`、clone binding 并手订阅所有 `FormEvent` | `components/chat/run_settings.rs`、`features/settings/provider/forms/secret.rs`、`features/settings/shortcuts/dialog.rs` | 必须改为 custom binding projector/writer |
| `E-1202` | Prompt、Shortcut、Provider、MCP 保存流程保存 `FormRevision` 并调用 `rebase_if_revision` | `features/settings/{prompts,shortcuts,mcp}/dialog.rs`、`provider/forms.rs` | 改为 `FormVersion` CAS |
| `E-1203` | Provider 三个变体表单以及 RunSettings 只经过静态 child/field 路径 | `features/settings/provider/forms.rs`、`components/chat/run_settings.rs` | total `get/set` 属于机械迁移，binding 机制另行替换 |
| `E-1204` | MCP 已使用 `ItemPath`/`PathKey` 管理五类 row，却在 replace、row mutation 后以旧 API rebuild/读取整个 model | `features/settings/mcp/{form_state,dialog,form_rows,validation}.rs` | 保留运行时身份，改为 impact 驱动 reconcile |
| `E-1205` | Jaco Garde message 与业务 validation 仍属于应用层，不承担 topology 映射 | `features/settings/form_validation.rs`、`features/settings/mcp/validation.rs` | 保留本地化和业务规则；改接 snapshot validator |

## 所有权与行为契约

### `ST-1200`：Form、原生控件与应用状态

- **权威 Form：** 各 dialog/controller 持有一个 `Entity<Form<Draft>>`；它拥有 draft、baseline、validation、
  topology 和 version。
- **原生控件：** 内置 adapter 或 custom binding 拥有 Input/Select/Integer state、focus、IME、popup 与
  原生订阅；Jaco 不保存 `ControlLease`。
- **应用状态：** 保存 task、catalog、provider capability、ChatInput agent 状态、MCP/OAuth runtime 仍由原 owner
  管理，绝不放入 Form。
- **发布：** 页面重绘使用 `cx.observe(&form, ...)`；只有 config 保存、附件投影与 MCP row reconcile 等选择性
  副作用订阅 `FormEvent<M>` 并查询 impact。

### `D-1200`：事件只按语义影响触发应用副作用

目标规则：

```rust,ignore
cx.subscribe_in(&form, window, move |this, _, event: &FormEvent<ChatInputInput>, window, cx| {
    let FormEvent::ModelChanged(change) = event else {
        return;
    };
    if change.impact(&run_settings_path).value_changed() {
        this.save_chat_form_config(window, cx);
    }
});
```

- config 保存：仅 `run_settings_path` 的值 impact；附件不触发。
- 附件 UI state：仅 attachments target 的值 impact；整模型生命周期因影响全部值而同步。
- MCP row UI：仅 collection target 的 `structure_changed()` 或 `retired()` 触发 row reconcile；单字段写入由
  内建/custom binding 自己投影，不能全量 rebuild。
- `ValidationChanged` 只驱动页面错误渲染，不设置原生值、不保存 config、不重建 row。

### `D-1201`：保存采用 opaque `FormVersion`，不重读 live model

```rust,ignore
let prepared: Prepared<Draft> = form.update(cx, |form, cx| form.prepare(cx))?;
let prepared: Prepared<Output> = prepared.map(map_draft_to_output);
let version = prepared.version();
let output = prepared.into_parts().1;

// 页面所有者持有的 I/O 完成回调
form.update(cx, |form, cx| form.rebase_if_current(version, canonical_draft, cx));
```

`version` 只来自同一个 `Prepared`；错误或 `false` 保持现有 draft，继续走现有“保存完成但有新编辑”通知。
Provider 的 secret 规范化仍按现有空 secret/已保存 secret policy 构造 canonical draft。

### `D-1202`：自定义控件统一使用 binding projector/writer

适用 `CustomTokenBudgetInput`、provider secret、Shortcut prompt/hotkey，以及 RunSettings 的 picker 同步。

```rust,ignore
let (binding, writer) = path.bind_control_in(
    &form,
    &native_state,
    |state, projection, window, cx| match projection {
        ControlProjection::Value(value) => state.project_silently(value, window, cx),
        ControlProjection::Retired => state.set_retired(window, cx),
    },
    window,
    cx,
);
// adapter 持有 `binding`；原生回调只捕获 `writer`。
```

- total custom path 不会因 reset/rebase/replace 退休；它接收 canonical `Value`。
- dynamic path 收到一次 `Retired` 后移除对应原生 row；排队旧写入静默 no-op。
- 自写不回传；同字段另一控件、程序 set 和 canonical rebase 会投影最新值。
- Shortcut prompt 与 Provider API mode 使用内建 `FormSelect`；hotkey、secret、token budget 和 Jaco 自有
  reasoning/approval picker 使用上述 custom binding，不再各自实现 Form event 投影。
- model picker 一次提交会同时规范化 model/reasoning/approval，使用 `RunSettingsInput` 的复合 total
  path/writer 做一次原子 model commit；不能拆成多个独立 writer 产生中间态或多 revision。

### `D-1203`：MCP 行协调保留运行时身份

`McpServerFormDraft` 继续持有单一 form；`McpServerFormComponents` 继续保存原生 row entity，但 map key
只使用 Form 的 `PathKey`。针对 args、env、env-vars、headers、env-headers：

1. 将当前扁平 `_controls: Vec<FormInput>` 拆成固定 total controls 与五类 typed row-control struct；每个 row
   struct 自己持有 `ItemPath`、对应 `FormInput` adapter 和渲染所需原生 entity，map drop 就会 drop binding。
2. subscription 查询相应 `TotalItemsPath` 的 impact；无 structure/retirement 直接返回。
3. 在同一 Form snapshot 枚举当前 `items(&form, cx)`，以 `ItemPath::key()` 对旧 row map 做 retain/reuse/new；
   先完整构建所有新增 row，再一次替换 map/order，任一构建失败不留下半安装 adapter。
4. 已移除 path 的 row-control 先 drop binding，再丢弃原生 entity；同父级 reorder 只改顺序并复用原 row/control。
5. `form_rows.rs` 的 GPUI id 通过 `ElementId::from(&PathKey)` 和 `ElementId::NamedChild` 派生；不得再格式化
   脱敏 `Debug`，也不要求 `PathKey` 提供 raw getter 或稳定 `Display`。
6. collection mutation 或动态控件构造返回 `ResolveError` 时，保留上一个完整组件容器并记录诊断；
   不出现半安装 row。

## 文件级改动地图

```text
app/jaco/
├── src/components/chat/input.rs                         # F-1200 [修改] 构造器、total get、impact 订阅与 Prepared 发送
├── src/components/chat/input/attachment_flow.rs         # F-1201 [修改] total descriptor get 调用
├── src/components/chat/run_settings.rs                  # F-1202 [修改] 复合/custom projector-writer 与 total path
├── src/features/settings/provider/forms.rs              # F-1203 [修改] 构造器、FormVersion Prepared/CAS 与 total get
├── src/features/settings/provider/forms/secret.rs       # F-1204 [修改] secret custom binding projector/writer
├── src/features/settings/provider.rs                    # F-1205 [修改] FormSelect/事件/CAS 消费；删除 total control 重绑
├── src/features/settings/prompts/dialog.rs              # F-1206 [修改] 构造器与 Prepared FormVersion CAS
├── src/features/settings/shortcuts/dialog.rs            # F-1207 [修改] 构造器、FormSelect/hotkey binding、total get 与 CAS
├── src/features/settings/mcp/form_state.rs              # F-1208 [修改] 构造、根 get、total items 与 adapter binding
├── src/features/settings/mcp/dialog.rs                  # F-1209 [修改] FormVersion request/CAS 与 impact 驱动的行协调
├── src/features/settings/mcp/form_rows.rs               # F-1210 [修改] 从不透明 PathKey 派生 ElementId；不格式化 Debug
├── src/features/settings/mcp/validation.rs              # F-1211 [语义不变] 保留应用 Garde 规则与私有 trigger
├── src/features/settings/form_validation.rs             # F-1212 [复核] message mapping 通常不变
└── src/** 内联测试                                      # F-1213 [修改] 各 owner 模块的消费契约 fixture
```

`form_state.rs` 文件及 provider `forms/{api_key,ollama,custom_openai}.rs` 继续作为 `FormSchema` model owner；
仅当 macro 编译诊断要求调整已声明的 target attribute 时才修改。未授权移动模块、修改 manifest、数据库、locale
或 asset。

## 所有者本地契约与错误行为

### `L-1200`：Provider 准备后的输出

将 `ProviderPreparedSubmit { revision: FormRevision, output }` 替换为：

```rust,ignore
pub(super) struct ProviderPreparedSubmit {
    pub(super) version: FormVersion,
    pub(super) output: ProviderSettingsFormOutput,
}
```

`ProviderSettingsForm::prepare` 在消费 output 前通过 `Prepared::version()` 取得 `version`；成功完成后调用
`rebase_if_current`。应用代码不再以裸 revision 做 CAS 比较。

`ProviderEditorState::validated_revision` 仍是同步验证结果对应的 `FormRevision`，不是异步 CAS token，应保留。
成功 rebase 后 total binding 会自行投影 canonical value，删除当前 provider 页面重新创建全部 total
原生控件的步骤。

### `L-1201`：MCP 保存请求

把每个 request 的 `revision: FormRevision` 字段替换为 `version: FormVersion`。dialog 仍只序列化现有
`McpServerFormInput` 派生的 config output；`FormVersion` 只是内存控制事实，不持久化、不写日志，也不发送到
MCP runtime。

### `ERR-1200`：已退休的动态行

来自动态 MCP path 或陈旧 remove/move 回调的 `ResolveError` 是生命周期结果：不通知、不按索引回退、不重写整模型。
处理方丢弃/忽略该行，仅记录 debug 日志。其他 `MutationError` 保留完整的当前组件树，并走现有诊断/UI 错误路径。

### `ERR-1201`：prepare 与 CAS 冲突

`PrepareError` 仍表示行内字段校验/提交拒绝。`rebase_if_current == false` 表示 I/O 已成功但存在更新的本地编辑；
不得覆盖 draft，并沿用现有“保存完成但仍在编辑”的通知。Provider、Prompt、Shortcut 和 MCP 持久化错误保持现有
所有者与消息行为。

## 工作包

### WP-1200：锁定生产者契约并替换构造与 total API

**前置：**`C-900`、`C-903`。

1. 替换 `F-1200`–`F-1212` 中的 `gpui_form::typed` import 与 `try_new*`。
2. 将 total `.value` 改为 `.get`；只有本地分支确实需要时才保留 `.set` 返回值。
3. 非 submit 的整模型读取改用根 descriptor 的 total `get`；仅在 submit/save 边界使用 `Prepared`。
4. 让所有 derive model 通过 `C-900` macro 门禁。

**测试：**`T-1200`、`T-1201`。
**完成条件：**Jaco 活动源码不再出现 `Form::try_new*`、total `.value` 或 `gpui_form::typed` import。

### WP-1201：迁移 FormEvent impact 与 ChatInput/provider total 投影

**前置：**`WP-1200`、`C-901`、`C-902`。

1. 用 settings 与 attachments 的 `PathImpact` 订阅替换 ChatInput 旧事件匹配。
2. 将 provider/run-settings total binding 改为 core-owned adapter 投影；catalog refresh 继续不修改 Form。
3. 普通 `cx.observe` 只用于页面重绘。

**测试：**`T-1202`、`T-1203`。
**完成条件：**应用原生 setter 不再宽泛订阅 `FormEvent`；无关 model/validation 变化不会保存 settings
或重置 controls。

### WP-1202：替换所有 custom binding 与 CAS 保存路径

**前置：**`WP-1200`、`C-902`、`C-903`。

1. 将 run settings token budget、provider secret、Shortcut prompt/hotkey 迁移到 binding/projector/writer 所有权。
2. 将 Provider、Prompt、Shortcut、MCP request 类型与 save callback 从 `FormRevision` 迁到 `FormVersion`。
3. 在最终 `C-903` API 下保留现有 `GardeValidator` context replacement；validation context 变化继续显式触发
   External validation，绝不隐式修改 model。

**测试：**`T-1204`、`T-1205`。
**完成条件：**不再存在 `ControlLease`、旧 `bind_control`、clone generic binding、`rebase_if_revision` 或以
`FormRevision` 承担的 Form CAS。

### WP-1203：MCP 快照 validation 与 impact 驱动的行协调

**前置：**`WP-1200`、`C-900`–`C-903`。

1. 保持 Jaco `garde::Validate` 规则和私有 MCP validation trigger 不变；消费 `C-903` core `GardeValidator`
   的 snapshot 行为，不增加 app-owned `Validator<McpServerFormInput>`。
2. 将 row rebuild trigger 移到 collection `PathImpact`；复用未变化的 `PathKey` row，只退休已删除的 dynamic row。
3. 五个 total collection `items` 改为无失败枚举；add/remove/move mutation 仍可失败，并按
   `ERR-1200` 处理 stale dynamic row callback。

**测试：**`T-1206`–`T-1208`。
**完成条件：**同父级 reorder 保留原生 row 身份；remove/reinsert/case/整模型生命周期不能复活
旧 row control 或 issue。

### WP-1204：残留清理与定向验证

**前置：**`WP-1201`–`WP-1203`。

1. 只更新 inline tests 与有意保留的 compile fixture；历史文档保持不变。
2. 执行残留扫描和下方验证矩阵；明确登记 desktop UI 操作测试按范围未执行。

## 自动化测试与验收

| ID | 层级 | 场景 | 断言 |
| --- | --- | --- | --- |
| `T-1200` | Jaco 编译/单元测试 | 全部静态 draft 构造器与 total descriptor | 无失败构造；保留 `get/set` 的 Rust 类型 |
| `T-1201` | Prompt/Provider/Shortcut GPUI | 提交 → 异步完成 → 并发编辑 | 仅匹配的 `FormVersion` 可以 rebase；旧完成回调保留新 draft |
| `T-1202` | ChatInput GPUI | 编辑 composer/attachments/run settings | 仅目标 impact 保存 settings/投影附件；validation 变化没有值副作用 |
| `T-1203` | RunSettings/provider GPUI | catalog 刷新与程序化 model 变更 | total control 投影 canonical 值；catalog 不重写 draft |
| `T-1204` | 自定义控件 GPUI | 来源写入、外部写入、释放 | 无自回传；投影最新外部值；已释放 writer 为 no-op |
| `T-1205` | validation | Garde/context 与提交错误 | 保留原有 Fluent 字段消息及字段局部性 |
| `T-1206` | MCP 单元测试 | 五个集合的 add/remove/reorder | reorder 保留 `PathKey`，重新插入使用新 `PathKey` |
| `T-1207` | MCP GPUI | 行退休时仍有排队的延迟原生回调 | 仅一次 `Retired`；不写入替换后的行 |
| `T-1208` | MCP validation | 嵌套行 issue 加非活动 transport/case | issue 只附着到存活的类型化字段；非活动 resolver 不是错误的过期失败 |
| `T-1209` | 残留扫描 | 活动 Jaco 源码/测试 | 除历史文档/有意的反向 fixture 外，不存在旧 API 名称 |

执行阶段命令（实际 UI 操作测试不在范围内）：

```text
cargo fmt --all
cargo test -p gpui-form --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p jaco --bin jaco --all-features --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p jaco --all-targets --all-features --locked -- -D warnings
cargo check --workspace --all-targets --all-features --locked
git diff --check
```

验收还包括对活动源码/测试的精确残留扫描：`Form::try_new`、`try_new_with_validator`、
`FormRevision`（非 Form 业务 revision 逐项分类）、`rebase_if_revision`、`ControlLease`、`bind_control`、
`FormEvent::Committed`、`FormEvent::ModelReplaced`、total `.value`、dynamic `try_value`。不得误删 Jaco
自身合法的 `gpui_operation::Transition`、数据库 ID、历史文档或 MCP runtime。

## 实施证据

- 实现位置：当前工作区，尚未提交；`WP-1200`–`WP-1204` 已完成。
- ChatInput、RunSettings、Provider、Prompt、Shortcut 与 MCP 设置表单已消费最终 typed path、impact、binding
  和 `FormVersion` 契约；MCP 五类集合按 `PathKey` 增量 reconcile，同父重排保留 row/control 身份。
- stale save completion 不覆盖新 draft；Prompt/Shortcut/MCP 不关闭仍有新编辑的 dialog，Provider 只在 CAS
  成功后推进 validated revision。
- `cargo test -p jaco --bin jaco --all-features --locked` 通过：362 项；Jaco check 与
  `cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings` 通过。
- 活动 Jaco 源码/测试旧 Form surface 精确扫描零命中；仅保留计划允许的 Provider 同步 validation
  `FormRevision`。
- 未修改 Conversation 或 MCP runtime；未执行实际 UI 操作测试。
