# gpui-form 类型化表单核心重构实施计划

## 1. 状态与范围

- 文档位置：`crates/gpui-form/dev/typed-form-store.md`。
- 关联分支：`codex/175-jaco-shortcut-temporary-window`。
- 关联 issue：无独立 issue；这是跨 crate form 基础设施迁移，不属于 issue #175 的产品需求。
- 当前阶段：**核心源码、集成测试、宏 compile-fail harness 与 workspace 自动化门禁已完成；
  Jaco 已完成定向 Computer Use smoke，但临时窗口全局快捷键与有数据列表的完整交互仍需人工验证。**
- 当前代码状态：`crates/gpui-form` 已落地单一类型化 model/runtime、revision/CAS、类型化字段与
  projection、同步/异步/control validation、stable-ID array 和纯 `prepare_submit` 契约；active
  source 的旧 submit runtime、来源跳过、draft/codec/focus 状态与兼容 API 已清除。
- 2026-07-21 验证证据：crate tests、workspace build/test、严格 clippy、dependency tree、
  residual scan 与 `trybuild 1.0.118` compile-fail fixtures 均通过；隔离数据目录的 bundle 已完成
  home、provider 与 shortcut 定向 smoke。Computer Use 不能触发全局快捷键，且隔离库没有临时对话
  数据，因此临时窗口上下键/搜索焦点仍保留为人工验证缺口。
- 发布门禁：这是破坏性 workspace 内部迁移。`gpui-form`、`gpui-form-macros`、
  `gpui-form-gpui-component` 和 Jaco 调用方必须在同一迁移序列中完成，不能单独发布中间态。

### 目标

1. 让一个 `Entity<GeneratedFormStore>` 成为当前类型化业务值、baseline、revision 和数据验证
   runtime 的唯一 owner。
2. 让 `FormField<Form, T>` 始终以真实 Rust 类型读写，不在 core 中保存 String draft、codec、
   focus 或 touched 状态。
3. 建立不会发生 GPUI entity 重入的绑定契约：组件事件 defer 写入；每次 form value 事件都
   静默重投影到所有已挂载控件，包括发起写入的控件。
4. 明确定义 mount/change/blur/dynamic/submit 验证、required 语义、同步 bucket、异步任务
   generation、control issue 生命周期和 Garde 0.23.0 国际化边界。
5. 让 `prepare_submit` 只负责同一份 model snapshot 的同步验证、pending 检查和纯转换；持久化
   task、loading、retry 和 provider/database 错误由页面或应用 store 持有。
6. 用 `FormRevision` 与 `rebase_if_revision` 防止异步保存响应覆盖用户的新编辑。

### 非目标

- 不在 core 中实现具体 `InputState`、Select、Combobox、Checkbox 或 Switch 绑定；这些属于
  `gpui-form-gpui-component` 或应用。
- 不让 form 持有 `FocusHandle`、焦点/blur/touched、popup/query/highlight、IME 或错误可见性。
- 不让 form 持有 options/catalog、disabled、placeholder、accessibility 或 presentation。
- 不在 form 中启动数据库、HTTP/provider 持久化，不提供 loading、retry、attempts 或通知。
- 不新增数据库 schema、migration、网络协议、资源文件、图标或应用级 locale key。
- 本文不描述 derive macro 的解析与代码生成步骤，也不描述 Jaco 页面迁移；它们使用各自的
  实施文档，但必须遵守本文公开契约。

### 已确认的用户决定

以下内容均为 `[用户决定]`，实施阶段不得重新引入替代设计：

1. `[用户决定]` 允许一次性大规模重构并删除不必要 API、trait 和类型；不保留 deprecated
   compatibility wrapper。
2. `[用户决定]` form 不保存 draft；数字、enum 等必须由能原生表达该类型的具体组件 state
   处理，不用 String 模拟业务值。
3. `[用户决定]` form 不保存 focus/touched/blurred；一个字段可被多个控件消费，各控件保留
   自己的交互状态。
4. `[用户决定]` bound control handle 只保存 subscriptions 和原生 component entity；
   attachment 由 subscription closure 捕获，不作为额外公开字段。
5. `[用户决定]` options/catalog 更新由调用方通过原生组件 API 修改；上游 API 不支持时直接
   重建 bound control，form 不缓存 delegate 或 options。
6. `[用户决定]` 每次 form value 事件重投影所有控件，包括来源控件；不公开 origin-skip、
   authoritative-readback 或 source guard 协议。
7. `[用户决定]` async trigger subscription 由页面/应用 owner 持有；检查启动后，task 和
   generation 由 form 持有；所有活跃 form async validation 都阻止提交。
8. `[用户决定]` `required` 的缺失语义固定为：trim 后为空的 `String`、`None`、空 Vec/map/set、
   `false` bool；数字和 enum 没有内置缺失语义。
9. `[用户决定]` 相等的字段写入是完全 no-op；显式 `replace`/`reset`/`rebase` 和成功的
   `rebase_if_revision` 即使值相等也推进 revision；失败 CAS 完全没有副作用。
10. `[用户决定]` 删除 form-owned `SubmitRuntime`、busy、attempts、outcome 和 persistence task。
11. `[用户决定]` Jaco 同步业务验证优先迁移到 Garde；core 只提供通用 Garde adapter 与
    i18n/path 桥接，不依赖具体应用本地化实现。

### 兼容与重建策略

- workspace 一次性改完所有调用方；不保留 `FieldChangeSource`、`SubmitRuntime`、
  `SubmitOutcome`、`TransformContext`、`begin_async_validation`/
  `finish_async_validation` 等旧入口。
- 当前没有对外稳定版本兼容承诺，不提供数据迁移层。
- 没有数据库变化，因此不存在 rebuild/migration 选择。

## 2. 证据快照

### 当前仓库事实

| 证据 | `[当前事实]` | 目标处理 |
| --- | --- | --- |
| `crates/gpui-form/src/form.rs` | `FormStore` 暴露 `SubmitRuntime`、`is_submitting`、`submission_attempts`；`FormEvent::FieldChanged` 携带 `FieldChangeSource` | 删除 submit runtime 和来源字段；增加 revision、context、条件 rebase 与 whole-model event |
| `crates/gpui-form/src/submit.rs` | 保存 `Task<()>`、attempts、outcome，并定义 `SubmitError::Busy` | 删除所有持久化状态；只保留准备阶段错误 |
| `crates/gpui-form/src/transform.rs` | `SubmitTransform<Model, Output>` 有 `preview`、`transform_on_submit` 和 `TransformContext { submitted }` | 改为 associated `Output` 和单一纯 `transform` |
| `crates/gpui-form/src/trigger.rs` | `FieldChangeSource` 用于区分 Control/Programmatic/Reset/Rebase | 整体删除；所有值写入遵守同一投影契约 |
| `crates/gpui-form/src/field.rs` | field 只有一个 path；control 写入携带 source；async 由调用方手工 begin/finish token；identified item 选择第一个匹配项 | 增加 `validation_path` 和 Projection segment；高层 async API 保留 task；重复 ID 返回不可用且形成 structural issue |
| `crates/gpui-form/src/control.rs` | `FormControl` 固定 associated `Form`/`Config`，返回 `Entity<Self>`；attachment 直接同步 update form | trait 改为任意 Form + build closure，wrapper 本身 deref 原生 entity；public attachment 只暴露 deferred intent，weak/liveness 保持 crate-private |
| `crates/gpui-form/src/validation.rs` | `ValidationContext` 有冗余 `submitted`；Field scope 只匹配完全相等 path；runtime 只保存 generation，不保存 task；`RequiredValue for bool` 永远返回非空 | 删除 submitted；实现祖先/后代 scope；form 保留 Task；`false` 为 missing |
| `crates/gpui-form/src/path.rs` | path 只有 Field/Item，动态 projection 复用普通 field segment | 新增 `Projection`，同时区分 projected path 与真实 validation path |
| `crates/gpui-form/tests/derive.rs` | 测试 attempts 和低层 begin/finish async token | 按最终 revision、投影和高层 async 契约重写 |
| `crates/gpui-form/src/core.rs`、`pipeline.rs`、`view.rs` | 旧目录入口仍以孤立文件存在，但 `lib.rs` 已不加载 | 删除，避免两套架构继续误导实现者 |

### 已核实上游事实

| 上游 | `[上游事实]` | 本计划约束 |
| --- | --- | --- |
| GPUI，Zed target lock commit `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | `Entity` 同一 update 期间不能再次 update；`WeakEntity` 可在异步/延迟工作前 upgrade；未 detach 的 `Task` drop 会取消任务 | subscriber 不同步回写 emitter；用户事件和回调中的 form 写入使用 defer；form 必须保留 async validation task |
| gpui-component target commit `5b45bcb26b9343d91a123a4d5ed8a654360512e5` | manifest 的 Zed dependencies 使用无 query Git source；其 lock 固定上述 Zed commit | core、adapter 与应用必须解析到同一 GPUI crate identity；不得保留 root `?rev=` source |
| Garde `0.23.0` `Validate` | 带 context 的入口是 `validate_with(&Context)` | 非默认 context 不 fallback 到 `validate()` |
| Garde `0.23.0` `I18n` | 方法返回 `Cow<'static, str>`；例如 `length_lower_than(&self, min: usize)`、`email_invalid(&self, reason: InvalidEmail)` | provider 必须实现精确签名，不能传不存在的 actual 参数 |
| Garde `0.23.0` `with_i18n` | handler 只在当前线程和同步闭包栈内生效 | handler 不跨 `await`；异步验证不复用 thread-local handler |
| Garde `0.23.0` error/path | error 最终只保存字符串；公开 `Path` 支持 `Display`，内部 iterator 是 doc-hidden；Vec path 使用当前 index | 保存 `ValidationMessage::Localized`；不逆向解析字符串；generated mapper 把 index 映射到 stable ID |
| Validify `2.0.0` | `Modify::modify(&mut self)` 原地修改 value | `ValidifyTransform` 只修改一次 clone，不回写 form |

### 依赖证据

| 依赖 | 当前声明/锁定 | feature/source | 决定 |
| --- | --- | --- | --- |
| `gpui`/`gpui_platform`/`gpui_macros` | 当前 manifest/lock 为 `?rev=1d217ee...` | target manifest 使用无 query `https://github.com/zed-industries/zed`；lock 为 `1a246efd...` | [发布门槛] 由 adapter 计划 `DEP-00` 一次性升级并统一 source identity |
| `gpui-component`/assets | 当前 lock 为 `c36b0c6...` | target lock 为 `5b45bcb...` | [发布门槛] 与 Zed source 升级在同一 lockfile changeset 完成 |
| `garde` | `0.23.0` | workspace；`default-features = false`；`derive,url,email,pattern` | package/version/features 不变；随 `DEP-00` 提交同一份更新后的 lockfile |
| `validify` | `2.0.0` | `gpui-form` optional dependency | package/version/features 不变；随 `DEP-00` 使用更新后的 lockfile |
| `gpui-form-macros` | workspace | derive re-export | 同步迁移 API，不改依赖来源 |

### 明确不变的系统面

- UI：core 不渲染 element，不新增 UI state 或交互。
- 数据库：无 schema、migration、repository 或 transaction 变化。
- 数据获取：不新增 HTTP/provider endpoint、认证、缓存、分页、timeout 或 offline 策略。
- 图标与 assets：无变化。
- 应用 i18n 文件：无变化；core 仅定义 `ValidationMessage` 和 Garde provider 边界。
- 平台：无 macOS/Linux/Windows 特有分支。
- 依赖：[用户决定] 接受 Zed/GPUI 与 gpui-component 升级。root manifest source 与
  `Cargo.lock` 只按 adapter `DEP-00` 修改；feature、MSRV 和平台 bootstrap 不另行改变。

## 3. 冻结设计

### 3.1 单一 owner 与数据边界

generated store 只通过一个 doc-hidden runtime 保存共享可变状态：

```rust,ignore
#[doc(hidden)]
pub struct FormRuntime<Model, ValidationContext> {
    value: Model,
    baseline: Model,
    revision: FormRevision,
    validation_context: ValidationContext,
    validation: FormValidationRuntime,
}
```

- `value` 是当前唯一可提交业务值；`baseline` 只用于 dirty/reset。
- `revision` 只描述业务值版本，不描述 validation/focus/loading。
- `validation_context` 是同步 validator 的外部只读依赖。
- `validation` 保存同步 issue bucket、异步 task/generation/issue 和 control issue lease。
- adapter 与 transform 必须是无运行时依赖的 `Default` 类型；数据库/service/catalog 等依赖放在
  `ValidationContext` 或由页面持有。
- 组件私有不完整文本不是业务值；它通过 active control issue 阻止提交。

### 3.2 Revision 与 lifecycle

```rust,ignore
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormRevision(u64);

impl FormRevision {
    pub const INITIAL: Self = Self(0);
    pub const fn get(self) -> u64;
}
```

- constructor 从 `INITIAL` 开始；mount validation 不推进 revision。
- 非相等字段写入推进一次；相等字段写入完全 no-op。
- 每次显式 `replace`、`reset`、`rebase` 都推进一次，即使 value 比较相等。
- `rebase_if_revision(expected, value, cx)` 先比较 revision；不等时返回 `false`，不得修改
  value、baseline、revision、validation、task 或 controls；相等时执行一次完整 `rebase`，
  推进 revision 并返回 `true`。
- revision 使用 checked increment；理论溢出是不可恢复的内部 invariant violation，不允许
  wrap 到旧 token。
- validation run、pending/issue 变化、context 更新和 component interaction state 不推进。

whole-form lifecycle 的固定顺序：

1. 按 replace/reset/rebase 语义更新 current/baseline；
2. 推进 revision；
3. 取消所有 form-owned async validation task，并清除全部 required/structural/generated
   synchronous field bucket、唯一 adapter-wide bucket 和 async pending/issue；
4. 保留仍活跃的 control issue lease；控件收到新值后由 adapter 按自身 private state 清理或
   更新 issue；
5. 发出一个 `FormEvent::ModelReplaced { revision }` 并 `cx.notify()`；
6. 每个 mounted control 静默读取并投影新值；不合成逐字段 change validation。

### 3.3 Field、path 与投影

```rust,ignore
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldPathSegment {
    Field(Cow<'static, str>),
    Item(FormItemId),
    Projection(Cow<'static, str>),
}

pub struct FormField<Form, T>
where
    Form: FormStore,
{
    form: WeakEntity<Form>,
    field: Form::Field,
    path: FieldPath,
    validation_path: FieldPath,
    read: Arc<dyn Fn(&Form) -> Option<T>>,
    write: Arc<dyn Fn(&mut Form, T, &mut Context<Form>) -> Result<bool, FormFieldError>>,
}
```

- `path` 标识当前 control/async issue；`validation_path` 标识真实 model 中应执行规则的最近
  parent path。
- 真实 nested field 使用 `Field` segment，并把 validation path 推进到真实 child。
- `project_value(name, read, write)` 使用 `Projection(name)`，保留 parent 的
  `validation_path`；projection 上继续 projection 仍保留该真实 path。
- `FieldPath::Display` 对 Projection 使用 `::<name>`，确保不会与真实字段字符串冲突。
- `value`、`set`、`set_user_value`、`errors`、`is_validating` 和 `validate` 在 form 已释放时
  返回 `FormFieldError::FormReleased`；动态/identified/projection path 不再存在时返回
  `ValueUnavailable`。
- 只读 field query 使用 owned 返回值，签名固定为：

  ```rust,ignore
  pub fn value(&self, cx: &App) -> Result<T, FormFieldError>;
  pub fn errors(&self, cx: &App) -> Result<Vec<ValidationIssue>, FormFieldError>;
  pub fn is_validating(&self, cx: &App) -> Result<bool, FormFieldError>;
  ```

  query 不借出 runtime 内部 report/bucket，也不延长 form 或 issue 的生命周期。
- `set` 和 `set_user_value` 具有相同业务写入语义；后者只明确表达 stateless control/user
  callback 的来源，不进入事件 payload。
- 每次调用 generated accessor 都只创建一个便宜 handle；不会创建 child entity、复制业务
  值或安装 subscription。多个控件可消费同一字段。

identified array item 的 read/write 必须遍历并确认 **恰好一个** ID 匹配；0 个或大于 1 个
都返回 `ValueUnavailable`。generated structural traversal 同时把缺失、重复或不可转换 ID
写入 blocking structural bucket；禁止选择第一个重复项。

### 3.4 事件与控件同步

```rust,ignore
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormEvent<Field> {
    FieldChanged {
        field: Field,
        path: FieldPath,
        revision: FormRevision,
    },
    ModelReplaced {
        revision: FormRevision,
    },
    RuntimeChanged,
}
```

- `FieldChanged` 和 `ModelReplaced` 是 value event；`RuntimeChanged` 只表示 validation/context
  runtime 改变。
- `FormField::subscribe_in` 对每一个 `FormEvent::FieldChanged` 和
  `FormEvent::ModelReplaced` 调用 listener，而不是按来源或 path 跳过；它只忽略
  `FormEvent::RuntimeChanged`。listener 重新读取自己的 typed field；这样
  parent/projection setter 改变其他派生值时，所有 consumer 仍保持一致。
- 事件不包含 `Any`、draft、source ID 或 read-back value。
- 每个 control binding 在 value event 后都调用 native silent setter，包括发起用户写入的
  control。silent setter 不得发出 user event。
- 组件 event callback 不能在 emitter 的 active update 内 update 同一 entity。adapter 在
  callback 内只读取并 clone 必要值，然后调用 attachment 的 public `defer_*` intent API；
  core 内部用 `cx.defer_in`、weak form 和 crate-private liveness 完成安全写入。

### 3.5 Control attachment 与 `FormControl`

```rust,ignore
pub trait FormControl<T>: Deref<Target = Entity<Self::State>> + Sized
where
    T: Clone + PartialEq + 'static,
{
    type State: 'static;
    type Error;

    fn new<Form, Owner, Build>(
        field: FormField<Form, T>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, Self::Error>
    where
        Form: FormStore,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<Self::State>) -> Self::State;
}
```

- trait 不含 associated `Form`、`Config`，也不返回 `Entity<Self>`。
- component configuration 通过 `build` closure 或 deref 后的原生 state API 完成。
- adapter wrapper 的字段固定为 `subscriptions: Vec<Subscription>` 在前、原生
  `Entity<State>` 在后；core 不提供 `SubscriptionSet`。

```rust,ignore
impl<Form, T> FormField<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn attach_control(
        &self,
        cx: &mut App,
    ) -> Result<ControlAttachment<Form, T>, FormFieldError>;
}

#[derive(Clone)]
pub struct ControlAttachment<Form, T> { /* opaque public handle */ }

impl<Form, T> ControlAttachment<Form, T> {
    pub fn defer_set_user_value<Owner>(
        &self,
        value: T,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where Owner: 'static;

    pub fn defer_blur<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where Owner: 'static;

    pub fn defer_set_issue<Owner>(
        &self,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where Owner: 'static;

    pub fn defer_clear_issue<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where Owner: 'static;
}
```

- `attach_control` 立即确认 form 仍存在且当前 field/path 可读取；失败时分别返回
  `FormReleased` 或 `ValueUnavailable`。成功时创建一个 crate-private `ControlId` 和一份共享
  lease/liveness；它不修改业务值、revision 或 validation report，也不发出 form event。
- `ControlAttachment::clone` 共享同一个 private `ControlId` 和 lease/liveness，而不是注册新的
  control。最后一个 clone drop 后 lease 无法再升级，该 control issue 立即视为 inactive；
  runtime 中的 weak lease 不会延长生命周期。
- `attach_control` 只是 public 构造/生命周期 API。对 attachment 而言，四个 `defer_*` 是唯一
  public mutation API，全部返回 `()`；它们只记录 intent，并把 form update 安排到当前
  Owner update 结束后。
- crate-private `ControlId`、weak attachment 和 shared liveness 实现 deferred cancellation；
  不从 `typed` 或 crate root 导出，不出现在 `ValidationSource` payload。
- component-event subscription 捕获一个 public attachment clone；每个 `defer_*` 内部只把
  weak lifetime/form 放进队列。wrapper drop 并释放最后一个 clone 后，queued intent 无法升级，
  因而成为 no-op。
- deferred field 操作返回 `ValueUnavailable` 时，core 内部使该 attachment lifetime 失效、
  清理其 control issue，并 `cx.notify()` 当前 Owner，使结构 owner 在下一次 render 中 drop
  或重建控件；`FormReleased` 只结束 intent，不制造错误或 owner 状态。
- form-to-control subscription 通常只捕获 field 和 weak native entity。拥有 lifecycle-scoped
  control draft issue 的 typed editor 可以捕获同一 attachment 的 clone，在 silent authoritative
  projection 成功后调用 `defer_clear_issue`；当前内置 adapter 中只有 exact integer control 使用
  这一例外，attachment 仍不成为 wrapper 字段。
- form-to-control 投影由 adapter 读取 `FormField::value`；遇到 `ValueUnavailable` 时只 notify
  owner 并停止投影，不能选择 fallback value。下一次 render 负责 drop/rebuild wrapper，释放
  最后一个 attachment clone 并使旧 control issue inactive。
- public attachment 不提供 immediate write、read-back、`downgrade`/`upgrade`、invalidate、
  source ID 或 origin token。成功 intent 后由正常 value event 完成统一静默投影。

### 3.6 `FormStore` 公开契约

```rust,ignore
pub trait FormStore: EventEmitter<FormEvent<Self::Field>> + Sized + 'static {
    type Model: Clone + PartialEq + 'static;
    type Output: 'static;
    type Field: FormFieldId;
    type ValidationContext: 'static;
    type ValidationAdapter:
        ValidationAdapter<Self::Model, Context = Self::ValidationContext>;
    type SubmitTransform:
        SubmitTransform<Self::Model, Output = Self::Output>;

    fn from_value(value: Self::Model, cx: &mut Context<Self>) -> Self
    where
        Self::ValidationContext: Default;

    fn from_value_with_validation_context(
        value: Self::Model,
        validation_context: Self::ValidationContext,
        cx: &mut Context<Self>,
    ) -> Self;

    fn value(&self) -> &Self::Model;
    fn baseline(&self) -> &Self::Model;
    fn revision(&self) -> FormRevision;
    fn validation_context(&self) -> &Self::ValidationContext;
    fn set_validation_context(
        &mut self,
        next: Self::ValidationContext,
        cx: &mut Context<Self>,
    );

    fn replace(&mut self, value: Self::Model, cx: &mut Context<Self>);
    fn reset(&mut self, cx: &mut Context<Self>);
    fn rebase(&mut self, value: Self::Model, cx: &mut Context<Self>);
    fn rebase_if_revision(
        &mut self,
        expected: FormRevision,
        value: Self::Model,
        cx: &mut Context<Self>,
    ) -> bool;

    fn validate(
        &mut self,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        cx: &mut Context<Self>,
    );
    fn prepare_submit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<Self::Output, SubmitError>;

    fn validation_report(&self) -> ValidationReport;
    fn is_dirty(&self) -> bool;
    fn is_valid(&self) -> bool;
    fn is_validating(&self) -> bool;
    fn is_validating_at(&self, path: &FieldPath) -> bool;
    fn errors_at(&self, path: &FieldPath) -> Vec<ValidationIssue>;
    fn first_error_path(&self) -> Option<FieldPath>;
}
```

- `from_value` 是带 `ValidationContext: Default` 约束的默认入口；它只能委托给
  `from_value_with_validation_context`，不得造成第二次 mount validation。
- `from_value_with_validation_context` 始终存在；它先安装 model/context，再执行一次 Form
  scope 的 mount validation。
- `set_validation_context` 只替换 context、发出 `RuntimeChanged` 并 notify；不推进 revision、
  不清理 issue/task，也不隐式运行任何 trigger。
- macro 展开需要访问的 `FormRuntime`、runtime getter 和 commit helper 只通过
  `gpui_form::__private`/`#[doc(hidden)]` 暴露；README/Guide 不把它们当用户 API。
- generated store 只保存第 3.1 节的单一 `FormRuntime`；adapter 和 transform 只通过
  associated type 指定，不保存实例。每次同步验证使用
  `Self::ValidationAdapter::default()`，提交转换使用
  `Self::SubmitTransform::default()`；运行时依赖只能来自 typed validation context 或调用方。

### 3.7 同步验证

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValidationTrigger { Mount, Change, Blur, Dynamic, Submit }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationScope {
    Form,
    Field(FieldPath),
    Group(FieldPath),
    ArrayItem { path: FieldPath, id: FormItemId },
}

pub struct ValidationContext<'a, C> {
    pub external: &'a C,
}

pub trait ValidationAdapter<Model>: Default + 'static {
    type Context: 'static;
    fn validate(
        &self,
        model: &Model,
        trigger: ValidationTrigger,
        scope: ValidationScope,
        context: ValidationContext<'_, Self::Context>,
        cx: &App,
    ) -> ValidationAdapterReport;
}
```

每次 adapter run 都临时调用 `Self::ValidationAdapter::default()`；form runtime 不保存 adapter
实例，也不允许 adapter 自身携带跨 run 的运行时依赖。

trigger 语义：

- Mount：constructor 安装 value/context 后恰好一次；只执行声明 `on_mount` 的规则。
- Change：非相等 typed write 已提交后执行；validator 读取新 model。
- Blur：具体控件报告可靠 final blur 时执行；不保存 blur 状态。
- Dynamic：调用方在 catalog/locale/外部依赖变化后显式调用。
- Submit：`prepare_submit` 对同一 snapshot 执行；required 总是参与，其他规则按
  `on_submit` 声明参与。

scope 的 includes 规则固定为：Field 包含自身、后代及祖先 group/array path，不包含兄弟
叶子；Group 和 ArrayItem 包含自己的子树及祖先；Form 包含全部数据 path。projection 的
validate/blur 使用 `validation_path`，而非技术 projection path。

一次非相等字段写入顺序固定为：

1. 修改 typed model；
2. 推进 revision；
3. 清除与写入 path 相交的 required/structural/generated synchronous issue；
4. 取消并清除与写入 path 相交的 async pending/issue；
5. 写入失效阶段不清除 adapter-wide form issue 或任何 active control issue；第 6 步若实际运行
   adapter，则按正常 adapter run 语义整批替换 adapter-wide bucket；
6. 对新 model 执行 Change + 字段 `validation_path` scope；
7. 发出一个 `FieldChanged` value event 并 notify。

相等写入在第 1 步比较后立即返回，后续步骤均不发生。

runtime 使用分离 bucket，不靠 message 文本识别错误：

```rust,ignore
struct FormValidationRuntime {
    field_batches: BTreeMap<FieldValidationBucket, Vec<ValidationIssue>>,
    adapter_batch: Vec<ValidationIssue>,
    async_entries: BTreeMap<AsyncValidationKey, AsyncValidationEntry>,
    control_issues: BTreeMap<ControlId, ControlIssueLease>,
}
```

- `ControlId` 和 `ControlIssueLease` 均为 crate-private；公开
  `ValidationSource::Control` 是不含 ID payload 的 unit variant。
- required/structural/generated field bucket 按 path + source 整批替换。
- 每次 adapter 被调用都整批替换唯一 adapter bucket，包括 Field scope；adapter report 内的
  上游顺序保持不变。
- async 以 `(path, source)` 替换；control 在内部以 crate-private `ControlId` 替换。
- 最终 report 顺序固定为 generated schema/path 顺序、adapter batch 原顺序、async key 顺序、
  control ID 顺序；本地化消息不参与 identity、排序或去重。
- 不参与当前 trigger/scope 的 field bucket 保持原样。

### 3.8 Required 精确语义

```rust,ignore
pub trait RequiredValue {
    fn is_missing(&self) -> bool;
}
```

内置实现仅包括：

- `String`: `trim().is_empty()`；
- `Option<T>`: `is_none()`；
- `Vec<T>`: `is_empty()`；
- `HashMap<K, V>`、`BTreeMap<K, V>`、`HashSet<T>`、`BTreeSet<T>`: `is_empty()`；
- `bool`: `!self`。

自定义类型可实现 trait。数字和 enum 不实现；在不支持类型上声明 `#[form(required)]` 必须
成为 compile error。required 总是在 Submit 执行，字段 attribute 中声明的 trigger 只增加
更早时机。错误 key 固定为 `gpui-form-error-required`。

### 3.9 Garde adapter 与国际化

```rust,ignore
pub trait GardeI18nProvider<C>: 'static {
    type Handler<'a>: garde::i18n::I18n + 'a
    where C: 'a;

    fn handler<'a>(context: &'a C, cx: &'a App) -> Self::Handler<'a>;
}

pub trait GardePathMapper {
    fn map_garde_path(&self, path: &str) -> Result<FieldPath, GardePathError>;
}
```

`GardeAdapter<T, P>` 的约束固定为：

```rust,ignore
T: garde::Validate + GardePathMapper + 'static,
P: GardeI18nProvider<T::Context>,
```

同步调用顺序固定为：

1. `P::handler(validation_context, cx)` 创建 handler；
2. 在 `garde::i18n::with_i18n(handler, || value.validate_with(context))` 中验证；
3. 空 path 产生 form-level Garde issue；非空 path 先通过 `value.map_garde_path`；
4. generated mapper 仅消费公开 `Path::Display` 字符串，并用当前被验证 model 把 Vec index
   映射为 stable `FormItemId`；
5. scope 过滤使用映射后的 stable `FieldPath`；
6. Garde 最终字符串保存为 `ValidationMessage::Localized`；不再包进通用 key；
7. unknown field、malformed/out-of-bounds index、invalid ID、duplicate ID 都转换为
   `ValidationSource::Internal`、code `garde_path_mapping` 的 blocking form issue。

`GardePathError` 必须有结构化 variant：`UnknownField`、`InvalidIndex`、
`IndexOutOfBounds`、`InvalidItemId`、`DuplicateItemId`。禁止调用 doc-hidden path iterator，
禁止在最终 path 保留数组 index，禁止从最终字符串猜 Garde rule。

默认 provider 返回 `garde::i18n::DefaultI18n`。自定义 provider 实现 Garde 0.23.0 的准确
`I18n` 签名，handler 生命周期不能跨 `await`。locale 变化由应用更新 validation context 后
显式执行 Dynamic validation；core 不观察 locale global。

### 3.10 异步验证

```rust,ignore
#[derive(Clone, Debug, PartialEq)]
pub struct AsyncValidationIssue {
    pub code: Cow<'static, str>,
    pub message: ValidationMessage,
}

impl<Form, T> FormField<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn start_async_validation<F, Fut>(
        &self,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    ) -> Result<(), FormFieldError>
    where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static;

    pub fn cancel_async_validation(
        &self,
        source: &str,
        cx: &mut App,
    ) -> Result<(), FormFieldError>;
}
```

- 页面 subscription 决定 debounce/trigger/何时调用 start，并由页面持有；core 不自动观察
  field event 并发网络请求。
- start 在一次 form update 中 snapshot typed value、为 `(path, source)` 递增全局单调
  generation、清除旧 issue、drop 旧 Task、spawn 新 future、把返回 `Task<()>` 存入 runtime，
  然后发出 `RuntimeChanged`。
- completion 通过 weak form entity 回到主线程，只在 key 和 generation 仍一致时移除 pending、
  保存 0/1 个 issue 并发出 `RuntimeChanged`；stale completion 完全无副作用。
- 相交字段写入、同 key 新 start、显式 cancel、whole-form lifecycle 和 form drop 都通过 drop
  retained Task 取消旧工作。
- 所有 pending entry 阻止 submit；没有 required/optional async 二分。非阻塞远程提示属于
  页面 UI state，不使用此 API。
- retry/backoff/debounce 由页面 subscription 或 service 决定；form 只管理单 key 最新一次
  检查及其取消。

### 3.11 提交与转换

```rust,ignore
#[derive(Clone, Debug, PartialEq)]
pub enum SubmitError {
    Validation(ValidationReport),
    ValidationPending,
    Transform(TransformReport),
}

pub trait SubmitTransform<Model>: Default + 'static {
    type Output: 'static;

    fn transform(
        &self,
        model: &Model,
    ) -> Result<Self::Output, TransformReport>;
}
```

`prepare_submit` 在一次 `Context<Form>` update 内只 clone 一次 current model，并按以下顺序：

1. 对该 snapshot 执行 required/structural/generated synchronous field rules 与 adapter 的
   Submit + Form scope 验证，并更新对应 field bucket 和唯一 adapter-wide bucket；
2. 若数据 issue 或 active control issue 存在，返回 `SubmitError::Validation(report)`；
3. 若任意 async entry pending，返回 `SubmitError::ValidationPending`；
4. 调用 `SubmitTransform::default().transform(&snapshot)` 恰好一次；
5. 成功返回 output；失败返回 `SubmitError::Transform(report)`。

整个操作不修改 value、baseline 或 revision，不开始 persistence，不读取 component/database，
也没有 Busy/attempts/outcome。同步 validation report 更新和 observer notify 是唯一 runtime
side effect；transform success/failure 本身不写回 form。

`IdentityTransform<Model>` clone snapshot；`ValidifyTransform<Model>` clone snapshot 后调用一次
`validify::Modify::modify`。两者 `Output = Model`。

## 4. 目标模块结构与所有权

```text
crates/gpui-form/
  src/
    lib.rs          # crate docs、derive re-export、公开模块入口
    array.rs        # FormItemId、ToFormItemId、identified-array 基础契约
    control.rs      # crate-private control identity/liveness、public deferred attachment、FormControl trait
    error.rs        # ValidationMessage/Issue/Report/Source
    field.rs        # FormField typed lens、projection、subscription、async 高层入口
    form.rs         # FormRevision、FormEvent、FormRuntime、FormStore
    path.rs         # FieldPath 与 Field/Item/Projection segment
    schema.rs       # FormFieldId、FieldSchema、ValidationTriggers
    submit.rs       # SubmitError；不保存 runtime/task
    transform.rs    # SubmitTransform、Identity/Validify、TransformReport
    trigger.rs      # ValidationTrigger；删除 FieldChangeSource
    typed.rs        # 稳定 public re-export
    validation.rs   # scope、required、adapter、Garde、runtime、async entry
  tests/
    derive.rs
    form_store.rs
    submit.rs
    validation.rs
```

删除以下孤立或 legacy 文件，且不新增 `mod.rs`：

- `crates/gpui-form/src/core.rs`
- `crates/gpui-form/src/pipeline.rs`
- `crates/gpui-form/src/view.rs`

### 所有权表

| 资源 | 唯一 owner | 生命周期/取消 |
| --- | --- | --- |
| current model、baseline、revision、validation context | generated form entity 内 `FormRuntime` | form entity |
| 同步 report bucket、async task/generation/issue、control issue weak lease | `FormValidationRuntime` | whole lifecycle 清全部 field/adapter/async data bucket 并取消 task，但保留 active control weak lease；form drop 取消剩余 task |
| async trigger subscription、debounce/retry | 页面/controller | 页面 drop 后不再启动新检查 |
| bound component entity 与 binding subscriptions | adapter wrapper/页面 | wrapper drop 先解除 subscriptions，再 drop entity |
| 组件 private text、focus/IME/selection/popup | concrete component state | component entity |
| options/catalog/capability | 应用 store 或 component delegate | 应用显式更新/重建并触发 dynamic validation |
| persistence task/loading/retry/error | 页面/controller/应用 store | 与页面或全局 store 策略一致，不受 form wrapper drop 隐式控制 |

## 5. 上游复用审计

| 能力 | 决定 | 删除/保留边界 |
| --- | --- | --- |
| GPUI `Entity`/`WeakEntity`/`Subscription`/`Task`/`defer_in` | 直接复用 | 不自建 entity store、subscription set、task registry 或 event loop |
| Garde `Validate`/`validate_with`/`I18n`/`with_i18n`/Path Display | 适配复用 | 只保留 typed context、provider 和 stable-path mapper；不复制规则、不调用隐藏 iterator、不二次包装英文文本 |
| Validify `Modify` | 直接复用 | adapter 只负责 clone 后调用一次，不复制 modifier 逻辑 |
| 现有 `FormItemId`/`ToFormItemId` | 保留并强化 | 增加重复/缺失检测，不改 stable u64 identity |
| 现有 `ValidationMessage`/`Issue`/`Report` | 保留并整理 bucket | 消息不作为 identity；支持 key 与 localized 两种边界 |
| `SubmitRuntime`/`SubmitOutcome`/`Busy`/attempts | 删除 | persistence 完全外置 |
| `FieldChangeSource`/public `ControlId`/origin skip/read-back | 删除 | 所有 value event 对所有 control 重投影；identity/weak 只留在 crate-private runtime |
| `AsyncValidationToken` + begin/finish | 删除 | 由 start/cancel 高层 API 和 retained Task 替代 |
| `TransformContext`/preview/transform_on_submit | 删除 | 单一纯 `transform` |
| `ValidationContext.submitted` | 删除 | trigger 已表达 Submit |
| core focus/draft/codec/subscription wrapper | 删除/不引入 | 具体 component/adapter/page owner 负责 |

## 6. 工作包

**跨计划依赖图**

```text
adapter DEP-00
  -> core FORM-10..60
  -> macro MACRO-10..40
  -> adapter CORE-GATE -> CONTROL-10 -> CONTROL-20 + CONTROL-30
  -> JACO-FORM-10..60
  -> CONTROL-40 -> CONTROL-50
  -> JACO-FORM-70
  -> core FORM-70
```

`FORM-60` 只完成 core source/API 与 core crate tests，不等待或修改 downstream。
`FORM-70` 是唯一最终发布门槛；它只在 macro、adapter 与 Jaco 各自完成后执行 workspace、
跨包残留与公开状态收口。

### FORM-10：收敛模块、公开类型与 breaking surface

**Prerequisites**

- 冻结本文第 3 节契约。
- [发布门槛] 先完成 adapter 计划 `DEP-00`：gpui-component lock 为 `5b45bcb...`、Zed
  lock 为 `1a246efd...`、manifest 只有无 query source，并通过 `cargo tree -d --locked`。
- macro/adapter/Jaco 实施者确认这是 breaking migration，不要求旧 API 中间态继续工作。

**Evidence**

- `src/lib.rs` 当前只加载 flat modules；`src/core.rs`、`pipeline.rs`、`view.rs` 是孤立旧入口。
- `typed.rs` 仍 re-export SubmitRuntime、TransformContext、FieldChangeSource 和低层 runtime。

**Files**

- 修改：`src/lib.rs`、`src/typed.rs`、`src/error.rs`、`src/path.rs`、`src/trigger.rs`、
  `src/array.rs`、`src/schema.rs`。
- 删除：`src/core.rs`、`src/pipeline.rs`、`src/view.rs`。
- 新增：`tests/form_store.rs` 的 compile/runtime 基础 fixture。

**API contract**

- 新增 `FormRevision`、`FieldPathSegment::Projection`、`FieldPath::join_projection`。
- `FormFieldError` 最终仅保留 `FormReleased`、`ValueUnavailable`。
- `ValidationTrigger` 保留五个 variant；删除整个 `FieldChangeSource`。
- `typed` 只重导出第 3 节稳定 public API；macro helper 放 `__private`，不得从 Guide 暴露。

**Implementation flow**

1. 先增加新类型/variant/Display，更新当前内部调用到可编译状态。
2. 收紧 `typed.rs` re-export，删除 legacy symbol。
3. 删除三个未被 `lib.rs` 加载的孤立入口。
4. 增加 compile-fail coverage，证明旧 API 不再存在且 unsupported required 不能通过 derive。

**Errors and lifecycle**

- 纯类型和模块收敛，无异步或部分进度。
- 删除文件前通过 `rg` 证明 `lib.rs` 和其他 crate 没有 module 引用；若存在引用，先迁移该
  引用，不恢复 legacy module。

**UI/data/database/icons/i18n/dependencies**

- UI、数据库、数据获取、图标/assets、应用 i18n、平台：`No change`。
- 依赖：只消费已完成的 `DEP-00` lock，不在本工作包再次选择或修改版本/source。
- 数据：只增加 path/revision 类型，不改变持久化模型。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| Projection 不与真实字段冲突 | `tests/form_store.rs` | `projection_path_has_distinct_segment_and_display` | 手工 FieldPath | `Field("x") != Projection("x")`，Display 含 `::<x>` |
| revision 初始值稳定 | `tests/form_store.rs` | `form_revision_starts_at_initial` | 最小 derived model | 初始为 `INITIAL/0` |
| 旧 symbol 已删除 | crate doctest/compile-fail | `legacy_submit_and_source_api_is_unavailable` | compile-fail snippet | 旧 import 无法编译 |

**Validation**

- `cargo fmt --all --check`
- `cargo test -p gpui-form --all-features --locked form_revision_starts_at_initial`
- `rg -n "mod (core|pipeline|view)|SubmitRuntime|FieldChangeSource|TransformContext" crates/gpui-form/src`
  只允许计划实施期间尚未完成的下一 WP 命中；FORM-60 后必须为零。

**Done condition**

- public module tree 与第 4 节一致；孤立文件已删除；没有新 `mod.rs`。
- 新 revision/path 类型及 error variant 有测试；旧类型不再从 crate root/typed 重导出。

### FORM-20：实现 FormRuntime、revision/CAS 与 typed field lens

**Prerequisites**

- FORM-10。
- 与 macro 计划中 generated store/runtime access 工作包原子迁移；中间 commit 可以不对外
  发布，但每个推送点应保持 workspace 可编译。

**Evidence**

- `form.rs` 当前没有 revision/context/CAS，lifecycle 直接由宏生成实现。
- `field.rs` 当前携带 source、按 path 过滤 subscription，并让 identified item `.find` 第一项。

**Files**

- 修改：`src/form.rs`、`src/field.rs`、`src/path.rs`、`src/array.rs`、`src/schema.rs`、
  `src/typed.rs`、`tests/derive.rs`。
- 新增/修改：`tests/form_store.rs`。

**API contract**

- 实现第 3.1、3.2、3.3、3.4、3.6 节全部签名和语义。
- generated store 只持有一个 `FormRuntime<Model, Context>`；该 runtime 及宏需要的
  getter/commit helper 为 doc-hidden。`ValidationAdapter`/`SubmitTransform` 只作为 associated
  type 存在，generated store 不保存其实例。
- `FormEvent` 只有 FieldChanged/ModelReplaced/RuntimeChanged；value event 携带 revision，不携带
  source。
- `FormField::subscribe_in` 对所有 `FieldChanged`/`ModelReplaced` 回调，只忽略
  `RuntimeChanged`；不按 source、path 或 equal whole-model value 过滤。
- `project_value` 创建 Projection path 并保留 parent validation path。

**Implementation flow**

1. 把 common state 移入 `FormRuntime`，让 generated store 只持有一个 runtime；删除 generated
   adapter/transform instance 字段，由实际 validation/submit 路径调用 associated type 的
   `default()`。
2. 用 `FormStore` default method 实现 getters、dirty、whole-form lifecycle、CAS 和 context setter。
3. 提供一个 doc-hidden typed commit helper，统一 equal compare、revision、validation invalidation、
   Change validation、value event 和 notify 顺序。
4. 重写 `FormField` read/write closure，删除 source 参数和 control 专用写入路径。
5. 增加 validation path；真实 nested accessor 推进真实 path，`project_value` 使用 Projection。
6. identified item read/write 统计匹配数，仅 1 个时成功。
7. subscription 在任何 FieldChanged/ModelReplaced 后重新读取自己字段；不做 origin/path/equal
   skip，并显式忽略 RuntimeChanged。

**Errors and lifecycle**

- weak form upgrade 失败返回 `FormReleased`，不得 panic。
- projection/array item 0 或多值返回 `ValueUnavailable`，不得 fallback。
- failed CAS 必须在取消 task、清 issue、发 event 之前返回。
- lifecycle 清除全部 required/structural/generated field bucket、adapter-wide bucket、async
  pending/issue/task，但不主动销毁活跃 control lease。

**UI/data/database/icons/i18n/dependencies**

- UI、数据库、数据获取、图标/assets、应用 i18n、依赖、平台：`No change`。
- 数据流变化：所有业务值写入集中到 typed form；不改变 Model 序列化/数据库形状。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| 写入后验证读取新值 | `tests/form_store.rs` | `typed_field_write_updates_model_before_change_validation` | recording adapter | adapter 读到新值，event revision 已推进 |
| equal write no-op | `tests/form_store.rs` | `equal_field_write_is_noop` | event counter | revision/report/event/notify 不变 |
| whole lifecycle 总推进 | `tests/form_store.rs` | `whole_form_lifecycle_always_advances_revision` | equal value/baseline | replace/reset/rebase 每次 +1，且每次各发一个 ModelReplaced |
| stale CAS 零副作用 | `tests/form_store.rs` | `stale_rebase_if_revision_has_zero_side_effects` | pending async + report + event counter | 所有状态完全相等、无 event |
| 成功 CAS 防双响应 | `tests/form_store.rs` | `successful_conditional_rebase_invalidates_same_revision` | 两次同 expected | 第一次 true/+1，第二次 false |
| 所有 control 收到回投 | `tests/form_store.rs` | `every_bound_control_receives_originating_write_projection` | 两个 fake subscriber | 两者都收到相同 typed value，包括来源方 |
| parent/projection 变更全量回投 | `tests/form_store.rs` | `parent_and_projection_writes_reproject_every_subscriber` | parent、projection 与多个 subscriber | 每个 FieldChanged 后所有 listener 都重新读取，不按 event path 过滤 |
| model replace 全量投影一次 | `tests/form_store.rs` | `whole_form_reprojects_all_controls_once_even_when_equal` | equal whole value + 多字段 subscriber | 每个 listener 一次，无逐字段 change validation |
| runtime event 不投影 | `tests/form_store.rs` | `subscribe_in_ignores_runtime_changed` | validation/context update + listener counter | RuntimeChanged 不调用 field listener |
| projection path/validation path | `tests/form_store.rs` | `projected_path_uses_projection_segment_and_parent_validation_path` | condition projection | issue path 为 Projection，validate scope 为 parent |
| identified duplicate | `tests/form_store.rs` | `identified_item_rejects_missing_and_duplicate_ids` | 0/1/2 matches | 仅 1 成功，其余 ValueUnavailable |

**Validation**

- `cargo test -p gpui-form --all-features --locked --test form_store`
- `cargo test -p gpui-form --all-features --locked --test derive`
- `cargo fmt --all --check`

**Done condition**

- revision/CAS/lifecycle 的每条冻结语义都有独立断言。
- field event 不再携带 source；所有 FieldChanged/ModelReplaced 都让 mounted consumer 统一回投影，
  RuntimeChanged 不触发 field projection。
- generated store 只有一个共享 runtime owner；adapter/transform 无实例字段。
- nested/identified/projection handle 不创建平行业务值或 child entity。

### FORM-30：实现 attachment 生命周期与最小 `FormControl` trait

**Prerequisites**

- FORM-20。
- `gpui-form-gpui-component` 的绑定工作包必须同步采用 public `defer_*` intent 和
  all-control projection；core 不为旧 adapter 保留 source/weak API。

**Evidence**

- `control.rs` 当前 trait 固定 `type Form`/`type Config` 并返回 `Entity<Self>`。
- 当前 attachment 让跨 crate 调用方直接同步 update form，无法从 API 层阻止重入。

**Files**

- 修改：`src/control.rs`、`src/field.rs`、`src/form.rs`、`src/typed.rs`。
- 修改：`tests/form_store.rs`。

**API contract**

- 精确实现第 3.5 节 `FormControl<T>`、
  `FormField::attach_control(&self, cx: &mut App) -> Result<ControlAttachment<Form, T>, FormFieldError>`
  和 `ControlAttachment: Clone`。
- 对 attachment 而言，四个 public `defer_*` 是唯一 mutation API，返回值均为 `()`；
  `attach_control` 只创建/验证 lifetime，不写业务值。
- weak attachment、`ControlId` 和 shared active flag 仅为 crate-private。
- active control issue 只在 private lifetime 可 upgrade 且 active 时进入 report；公开
  `ValidationSource::Control` 不含 ID。

**Implementation flow**

1. `attach_control` 先 upgrade form 并读取 field/path：分别把失败映射为 `FormReleased` /
   `ValueUnavailable`；成功后创建一个 crate-private `ControlLifetimeState { active }`、
   `ControlId` 和 weak issue lease，不发 event、不改业务值/revision/report。
2. attachment clone 共享同一个 ID/lifetime；runtime 只持有 weak lease。四个 public intent 立即
   创建 private weak capture，并通过 `cx.defer_in(window, ...)` 安排 form 操作。
3. component-event subscription 捕获 attachment clone。form-to-control subscription 通常只捕获
   field + weak native entity；拥有 lifecycle-scoped control draft issue 的 typed editor 可以再
   捕获同一 attachment clone，在 silent authoritative projection 后调用 `defer_clear_issue`；当前
   内置 adapter 中只有 exact integer control 使用这一例外。wrapper 字段仍只有
   `Vec<Subscription>` 和 `Entity<State>`。
4. deferred set/blur/set-issue 在 immediate helper 前确认 typed field 仍可读取；clear-issue
   即使 path 已消失也可按 private ID 清理。所有 immediate helper 保持 crate-private，外部
   无法绕过 defer。
5. queued intent upgrade 失败时 no-op；返回 `ValueUnavailable` 时内部关闭 shared active flag、
   清理 issue 并 notify Owner；`FormReleased` 静默结束。
6. set/clear issue 走 private ControlId bucket并 emit RuntimeChanged；公开 report source 只显示
   unit `Control`。
7. 重写 `FormControl<T>` 为 Deref + generic constructor 契约，不引入 config 或 attachment
   wrapper field。

**Errors and lifecycle**

- public intent 返回 `()`；`FormReleased` 在 deferred closure 内变为 no-op，不向已释放 owner
  传递无意义错误。
- `ValueUnavailable` 的 internal invalidation 必须幂等；重复 stale intent 不产生 issue/event。
- 最后一个 attachment clone drop 后 private lease 无法 upgrade；report/is_valid 立即忽略旧 issue，
  queued private weak work 也不得访问 form。先 drop 某一个 clone 不得提前失活。
- form-to-control 读取遇到 `ValueUnavailable` 时，adapter notify Owner 并停止投影；render
  根据结构状态 drop/rebuild wrapper，不选择 fallback。
- Drop 只释放同步资源；没有 async Drop。

**UI/data/database/icons/i18n/dependencies**

- UI：core 只定义 trait/lifetime，不实现具体组件、键盘、焦点或 accessibility。
- 数据库、数据获取、图标/assets、应用 i18n、依赖、平台：`No change`。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| attach 立即验证可用性 | `tests/form_store.rs` | `attach_control_rejects_released_or_unavailable_field` | released form + disappearing projection | 分别返回 FormReleased/ValueUnavailable，不创建 active issue lease |
| clone 共享唯一 lease | `tests/form_store.rs` | `control_attachment_clones_share_one_lease_until_last_drop` | attachment + 两个 clone + issue | 中间 clone drop 后 issue 仍 active；最后一个 drop 后 report/is_valid 忽略 issue |
| 四种 intent 必定 defer | `tests/form_store.rs` | `control_attachment_intents_run_after_owner_update` | test executor/event counter | 当前 update 内 form 不变，下一 turn 才 set/blur/issue/clear |
| queued work 安全 | `tests/form_store.rs` | `queued_attachment_intent_is_noop_after_control_drop` | test executor/defer | callback 不写 form、不 panic |
| ValueUnavailable 内部处理 | `tests/form_store.rs` | `unavailable_deferred_intent_invalidates_lifetime_and_notifies_owner` | disappearing projection + notify counter | issue 清除、后续 intent no-op、Owner notified |
| 权威投影清理私有 issue | `crates/gpui-form-gpui-component/tests/adapters.rs`（adapter WP） | `authoritative_projection_clears_incomplete_editor_issue` | fake integer editor + attachment clone | silent setter 后 defer_clear_issue；wrapper 不存 attachment 字段 |
| public surface 收窄 | compile-time assertion/doctest | `attachment_exposes_only_deferred_mutation_intents` | exact attach signature + old immediate/weak imports | attach_control 可构造且 attachment 可 clone；四 mutation 方法 output 为 `()`；weak/ID/immediate APIs 不可见 |

**Validation**

- `cargo test -p gpui-form --all-features --locked --test form_store attachment`
- `cargo test -p gpui-form-gpui-component --all-features --locked`（与 adapter 工作包一起）
- `cargo fmt --all --check`

**Done condition**

- core 无 config/subscription wrapper/native component state。
- attachment clones 共享一个 private lease；最后一个 clone drop 可证明阻止 stale queued work
  和 stale control issue，且跨 crate adapter 无法同步 update form 或管理 weak/source ID。
- 拥有 lifecycle-scoped control draft issue 的 typed editor 可以通过 subscription closure 捕获
  attachment 清理 private issue；当前内置 adapter 中只有 exact integer control 使用这一例外，
  wrapper 字段仍保持 `subscriptions` + native entity 的最小结构。

### FORM-40：重建同步验证、required、scope 与 Garde adapter

**Prerequisites**

- FORM-20；FORM-30 的 control issue lease 可用。
- macro 计划已生成字段 trigger/schema、required traversal、structural traversal 和
  `GardePathMapper`。

**Evidence**

- 当前 Field scope 只比较相等 path，无法覆盖 parent/descendant。
- 当前 `ValidationContext` 重复保存 `submitted`。
- 当前 bool required 实现返回 `false`，导致必须同意的控件永远不报 required。
- Garde 0.23.0 的 exact `I18n`/`validate_with`/Path Display 已核实。

**Files**

- 修改：`src/validation.rs`、`src/error.rs`、`src/schema.rs`、`src/form.rs`、`src/field.rs`、
  `src/typed.rs`。
- 新增：`tests/validation.rs`。
- 修改：`tests/derive.rs`。

**API contract**

- 实现第 3.7、3.8、3.9 节所有类型、bucket、trigger/scope、required 和 Garde 契约。
- 删除 `ValidationContext.submitted`、`RequiredValue::is_empty_value` 旧名。
- `set_validation_context` 不验证；constructor mount 恰好一次。
- unknown external path 和 Garde mapping error 是 blocking internal form issue。

**Implementation flow**

1. 把 runtime 从单一 Vec/HashMap 改成分离的 deterministic buckets。
2. 实现 path 关系函数并让 Field/Group/ArrayItem scope 包含 subtree + ancestors。
3. 实现精确 typed write invalidation：只清除相交的 required/structural/generated synchronous
   field bucket，取消并清除相交 async entry；失效阶段保留 adapter-wide batch 和所有 active
   control issue，随后 Change adapter run 可以按正常规则整批替换 adapter-wide batch；
   whole-form lifecycle 才执行完整 data-level clear。
4. 实现 `RequiredValue::is_missing` 精确内置集合，并让 macro 在 unsupported type 上失败。
5. 每次 run 都临时构造 `Self::ValidationAdapter::default()`，让其 report 整批替换唯一
   adapter-wide bucket；generated store/runtime 不保存 adapter instance，并删除 submitted flag。
6. 重写 Garde provider 调用、Localized message 和 stable path mapping error。
7. 保证一次 validation run 最多 emit 一个 RuntimeChanged + notify；未参与 bucket 不变。

**Errors and lifecycle**

- validator panic 不被吞掉或转成字符串；按 Rust/GPUI 默认 panic 策略暴露实现错误。
- Garde path mapping 失败转 typed internal issue，不 panic、不忽略、不 fallback 到 index。
- locale/context 变化不自动清旧报告；调用方显式 Dynamic validation 后整批替换目标报告。
- active control issue 与同步 data bucket 分开，sync validation 不会误删它。

**UI/data/database/icons/i18n/dependencies**

- i18n：只增加/修正 provider 和 `ValidationMessage::{Key, Localized}` 边界；不修改 locale 文件。
- UI、数据库、数据获取、图标/assets、依赖、平台：`No change`。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| scope 关系 | `tests/validation.rs` | `field_scope_includes_ancestors_and_descendants_not_siblings` | nested paths | exact includes matrix |
| write invalidation 顺序 | `tests/validation.rs` | `typed_write_invalidates_only_intersecting_owned_buckets_before_change_validation` | required/structural/generated/adapter/async/control 全 bucket fixture + recording adapter | Change 前只清相交前三类并取消相交 async；非相交与 active control 保留；adapter-wide 不被 invalidation 清理，但参与 Change 时整批替换 |
| deterministic batch | `tests/validation.rs` | `validation_batches_replace_deterministically` | 多来源重复消息 | bucket 整批替换、顺序稳定、消息不参与 identity |
| required String/Option | `tests/validation.rs` | `required_detects_trimmed_string_and_none` | derived fixture | 空值报 key，非空清除 |
| required collections/bool | `tests/validation.rs` | `required_detects_empty_collections_and_false_bool` | Vec/maps/sets/bool fixture | 精确 missing matrix |
| unsupported required | `tests/derive.rs` compile-fail | `required_numeric_and_enum_are_compile_errors` | trybuild/doctest | 清晰 derive diagnostic |
| mount exactly once | `tests/derive.rs` | `constructor_runs_mount_validation_exactly_once` | recording adapter/context | model/context 已安装，计数 1 |
| context setter 无验证 | `tests/validation.rs` | `set_validation_context_only_replaces_and_notifies` | counter adapter | revision/report/task 不变，adapter 0 次 |
| adapter 只按 Default 临时构造 | `tests/validation.rs` | `validation_default_constructs_stateless_adapter_per_run` | Default/validate counters + typed context | 每次 run 各 default/validate 一次；运行时依赖来自 context；form 不保留 adapter |
| adapter field scope bucket | `tests/validation.rs` | `adapter_batch_is_replaced_on_scoped_run` | custom adapter | form + field issues没有旧批残留 |
| Garde default context | `tests/validation.rs` | `garde_uses_validate_with_for_default_context` | garde model | expected issues/path |
| Garde custom i18n signature | `tests/validation.rs` | `garde_custom_i18n_preserves_localized_message` | complete test I18n provider | exact localized text，无二次 key |
| stable array path | `tests/validation.rs` | `garde_array_indices_map_to_stable_item_ids` | reordered array | final path 使用 Item ID，不含 index |
| mapping failures block | `tests/validation.rs` | `garde_path_mapping_failures_become_internal_issues` | unknown/out-of-range/duplicate | 每种 typed reason，submit invalid |

**Validation**

- `cargo test -p gpui-form --all-features --locked --test validation`
- `cargo test -p gpui-form --all-features --locked --test derive`
- `cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings`

**Done condition**

- 五种 trigger、四种 scope、所有 required 内置类型和 Garde mapping/i18n 都有测试。
- report 只通过 bucket 替换；没有 submitted flag、英文 Garde 二次包装或 index path fallback。

### FORM-50：用 retained task/generation 实现高层异步验证

**Prerequisites**

- FORM-20、FORM-40。
- 页面/adapter 使用 defer 后再调用 start；core 不为同步 nested update 兜底。

**Evidence**

- 当前 `begin_async_validation`/`finish_async_validation` 把 task retention 和 stale token 责任
  暴露给调用方，runtime 只保存 generation。
- GPUI `Task` drop 会取消未 detach 工作；因此 form 必须保留最新 task。

**Files**

- 修改：`src/validation.rs`、`src/field.rs`、`src/form.rs`、`src/error.rs`、`src/typed.rs`。
- 修改：`tests/validation.rs`、`tests/derive.rs`。

**API contract**

- 实现第 3.10 节 `AsyncValidationIssue`、`start_async_validation`、
  `cancel_async_validation`。
- 删除 public `AsyncValidationToken`、begin/finish API。
- generation 为 runtime 内全局 checked monotonic u64；key 重启不复用旧 generation。

**Implementation flow**

1. `AsyncValidationEntry` 保存 generation、`Option<Task<()>>`、`Option<ValidationIssue>`。
2. start 在同一 form update 中 snapshot、更新 generation、drop 旧 task、spawn 并 retain 新 task。
3. completion 使用 weak form + key/generation CAS；current 才更新 entry 和 notify。
4. typed write 按 path 交集取消；whole lifecycle 全取消；cancel source 只影响该 field key。
5. report/is_validating/prepare_submit 从 runtime entries 派生，不保存重复 bool。

**Errors and lifecycle**

- service future 返回 `Err(AsyncValidationIssue)` 是验证失败；future 自身不公开 transport error
  variant，调用方在 closure 中映射为用户可理解 issue 或把非验证错误留在应用 state。
- stale completion、form released、explicit cancel 均不恢复旧 issue。
- drop Task 是取消机制；不 detach、不在 Drop 中 await。

**UI/data/database/icons/i18n/dependencies**

- 数据获取：core 不实现 endpoint/auth/timeout/retry/cache；closure owner 负责。
- UI、数据库、图标/assets、应用 i18n、依赖、平台：`No change`。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| 新 start 取消旧 task | `tests/validation.rs` | `starting_same_async_key_replaces_task_and_generation` | controllable futures/drop probe | 旧 task drop，generation 不同，旧 issue 清除 |
| stale completion 无效 | `tests/validation.rs` | `stale_async_completion_has_zero_side_effects` | two completion channels | report/pending/event 只反映最新 |
| typed write 取消相交 | `tests/validation.rs` | `typed_write_cancels_intersecting_async_validation` | parent/child/sibling paths | parent/child取消，sibling保留 |
| lifecycle 全取消 | `tests/validation.rs` | `whole_form_lifecycle_cancels_all_async_tasks` | multiple keys | 全部 drop，pending false |
| form drop 取消 | `tests/validation.rs` | `dropping_form_cancels_retained_async_tasks` | drop probe | task 被取消，无 stale update |
| 页面 subscription drop | `tests/validation.rs` | `dropping_trigger_subscription_keeps_started_validation_owned_by_form` | subscription + pending future | 不再启动新任务，已启动仍 pending |
| 所有 pending 阻止提交 | `tests/submit.rs` | `any_active_async_validation_blocks_submit` | 不同 trigger/source | 均返回 ValidationPending，transform 0 次 |

**Validation**

- `cargo test -p gpui-form --all-features --locked --test validation async`
- `cargo test -p gpui-form --all-features --locked --test submit active_async`
- `cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings`

**Done condition**

- caller 不再能手工 finish stale token；form 保留并取消所有最新 task。
- pending 只有一个派生事实源，submit/field/form 查询完全一致。

### FORM-60：简化 submit/transform，完成 core source/API 与 crate tests

**Prerequisites**

- FORM-10 至 FORM-50。
- 不等待 macro、component adapter 或 Jaco；本工作包不修改任何 downstream crate 或应用文件。

**Evidence**

- 当前 `submit.rs` 和 `transform.rs` 仍包含 persistence task、Busy、attempts、outcome、preview、
  TransformContext。
- 旧 `tests/submit.rs` 断言旧 runtime 行为。

**Files**

- 修改：`src/submit.rs`、`src/transform.rs`、`src/form.rs`、`src/typed.rs`、`src/lib.rs`、
  `tests/submit.rs`、`tests/derive.rs`。
- 不修改 macro、adapter、Jaco 或其他 workspace member；公开文档的“实施完成”状态只由
  `FORM-70` 在最终 gate 通过后更新。

**API contract**

- 精确实现第 3.11 节 SubmitError/SubmitTransform/prepare_submit；
  `SubmitTransform<Model>: Default + 'static`。
- `IdentityTransform`、feature-gated `ValidifyTransform` 均使用 associated Output。
- 删除 `SubmitRuntime`、`SubmitOutcome`、`SubmitError::Busy`、`TransformContext`、preview、
  transform_on_submit、is_submitting、submission_attempts、last_outcome 和 task setter。

**Implementation flow**

1. 先改 transform trait/identity/Validify 与 `FormStore` associated-type contract；generated store
   不保存 transform instance。Macro 代码生成由 `MACRO-40` 修改。
2. 重写 `prepare_submit` 为单 snapshot 固定顺序；一次 submit validation 更新 report，只有在
   validation/pending 均通过后才构造 `Self::SubmitTransform::default()` 并调用一次
   `transform`。
3. 删除 submit runtime 字段、trait getter、re-export 与旧测试。
4. 用 core crate 内手写/derived fixture 更新 submit、transform 与 legacy compile-fail tests；不修改
   downstream 调用点。
5. 运行 core source residual scan、core package tests、clippy 与 feature tree；跨 workspace residual、
   文档状态和 release validation 留给 `FORM-70`。

**Errors and lifecycle**

- Validation 和 ValidationPending 不启动 transform/persistence。
- Transform failure 不写入 validation bucket、不改变业务值/revision。
- persistence cancellation/retry/partial success 不属于 core；调用方根据自己的 task owner 处理。
- 保存成功但用户继续编辑时 CAS 返回 false；调用方显示“已保存但仍有新编辑”等产品反馈，
  不强制 rebase。

**UI/data/database/icons/i18n/dependencies**

- UI：core 无变化；Jaco loading/错误展示在其迁移计划验证，本工作包不修改 Jaco。
- 数据库：无 schema/query 变化；repository canonical response 由调用方传给 rebase/CAS。
- 数据获取、图标/assets、应用 i18n、依赖、平台：`No change`。

**Tests**

| Requirement | Test file | Proposed test name | Fixture/mock | Assertions |
| --- | --- | --- | --- | --- |
| 单 snapshot 与 Default 构造 | `tests/submit.rs` | `prepare_submit_default_constructs_transform_for_one_snapshot` | recording adapter + Default/transform counters | 两者看到相同 model；通过 validation 后 default 1 次、transform 1 次；form 不保存实例 |
| 错误顺序 | `tests/submit.rs` | `prepare_submit_checks_validation_before_pending_before_transform` | seeded issue + pending + counter | Validation 优先；无 issue 时 Pending；transform 0 次 |
| 纯 transform | `tests/submit.rs` | `transform_success_does_not_mutate_form_state` | custom output | value/baseline/revision/report/controls 不变 |
| transform 失败 | `tests/submit.rs` | `transform_failure_is_not_validation_state` | failing transform | 返回 Transform，validation report 不新增 |
| identity | `tests/submit.rs` | `identity_transform_clones_model` | typed model | output 等于 snapshot，非共享可变状态 |
| Validify | `tests/submit.rs` | `validify_transform_modifies_only_output_clone_once` | Modify counter/value | form 不变，output 规范化一次 |
| 无 submit runtime | compile/residual scan | `form_exposes_no_persistence_runtime` | old imports/methods | compile-fail/rg 无命中 |

**Validation**

- `cargo test -p gpui-form --all-features --locked --test submit`
- `cargo test -p gpui-form --all-features --locked`
- `cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings`
- `cargo tree -p gpui-form -e features --locked`
- `rg -n "SubmitRuntime|SubmitOutcome|SubmitError::Busy|submission_attempts|last_outcome|TransformContext|transform_on_submit" crates/gpui-form/src crates/gpui-form/tests`

**Done condition**

- `prepare_submit` 只做同步准备；core 没有持久化 state/task。
- core source/tests 中的 submit/transform legacy symbol 已删除，core package tests 与 clippy 通过。
- 未修改 downstream crate、应用或其公开状态；workspace 是否可发布只由 `FORM-70` 判断。

### FORM-70：执行最终 workspace、残留与公开状态 release gate

**Prerequisites**

- FORM-60。
- `crates/gpui-form-macros/dev/form-store-derive.md` 的 `MACRO-40`。
- `crates/gpui-form-gpui-component/dev/typed-bound-controls.md` 的 `CONTROL-50`。
- `app/jaco/docs/dev/gpui-form-migration.md` 的 `JACO-FORM-70`。

**Evidence**

- Core 的 breaking source/API 已在 FORM-10 至 FORM-60 完成并通过 crate tests。
- Macro、adapter 与 Jaco 分别拥有自己的实现、调用点迁移和局部残留 gate；只有全部完成后，
  workspace validation 与“实施完成”状态才有意义。

**Files**

- 审核并按最终实现同步：`README.md`、`README.zh-CN.md`、`docs/guide.md`、
  `docs/guide.zh-CN.md`、`dev/README.md`、`dev/typed-form-store.md`。
- 只在验证发现目标文档与最终 public API 不一致时修改上述 core 文档；不得在本 gate 修改
  macro、adapter 或 Jaco 源码来掩盖其未完成工作包。

**API contract**

- 不新增或改变第 3 节 public API；本工作包只证明 core、macro、adapter 和 Jaco 对同一契约完成
  迁移。
- 只有全部 validation、residual 和文档一致性 gate 通过后，才把公开 design-status 与本计划状态
  更新为实施完成。

**Implementation flow**

1. 确认四个 prerequisite 的完成证据与局部测试结果。
2. 运行第 7 节完整 workspace validation、dependency identity 与跨包 residual scan。
3. 对照最终 rustdoc/public exports 审核英文 README/Guide，再同步中文镜像；不改变已经冻结的行为。
4. 验证链接、章节、代码围栏和 `git diff --check`。
5. 全部通过后更新 core 公开状态；任一失败则保持“目标 API/实施未完成”状态并返回对应 owner 的
   工作包修复。

**Errors and lifecycle**

- 这是只读验证加 core 文档状态收口，没有运行时 partial success、retry 或 cancellation。
- Dependency duplicate、legacy residual、跨 crate 编译失败、平台 CI 失败或 EN/ZH 语义偏差均为
  release blocker；不得增加 compatibility wrapper、fallback 或跳过测试。

**UI/data/database/icons/i18n/dependencies**

- UI：只验证 Jaco/adapter 计划已完成的 smoke，不在 core 新增 UI。
- 数据、数据库/schema、网络、icons/assets、应用 i18n：`No change`；只验证对应计划没有留下旧
  form owner 或第二数据源。
- 依赖/平台：不再修改版本选择；验证 DEP-00 固定的 gpui-component `5b45bcb...`、Zed
  `1a246...`、单一 source identity 和 macOS/Linux/Windows CI。

**Tests**

| Requirement | Test/evidence | Assertions |
| --- | --- | --- |
| workspace contract | 第 7 节 workspace check/test/clippy | core、macro、adapter、Jaco 使用同一最终 API |
| dependency identity | `cargo tree -d --locked` + lock scan | 单一 GPUI source；exact target commits |
| legacy deletion | 第 7 节跨包 residual commands | active source 无旧 draft/bind/source/submit runtime API |
| public docs | headings/fences/link check + doc/compile examples | English 默认；中文语义镜像；示例匹配最终 API |
| runtime UI | Jaco/adapter 已记录 Computer Use smoke | 无重入、失焦或 stale projection 回归 |

**Validation**

- 执行第 7.1 至 7.3 节全部命令与对应 Jaco/adapter UI smoke evidence。
- `git diff --check`。

**Done condition**

- 所有 prerequisites、workspace validation、dependency identity、residual、公开文档和平台 CI 均通过。
- Core 文档状态可安全标记为实施完成；没有为了通过 gate 修改 downstream owner 的源码或引入兼容层。

## 7. 跨工作包验证

### 7.1 Core 聚焦命令

```bash
cargo fmt --all --check
cargo test -p gpui-form --all-features --locked
cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings
cargo tree -p gpui-form -e features --locked
cargo tree -d --locked
git diff --check
```

### 7.2 FORM-70 workspace release gate

以下命令只在 `FORM-70`、即 macro、adapter 与 Jaco 同步迁移后执行：

```bash
cargo check --workspace --all-targets --all-features --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --all-features --locked
cargo test -p jaco --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo tree -d --locked
```

CI 最终按 `.github/workflows/ci.yml` 覆盖 macOS、Linux、Windows。core 本身没有平台特有手工
smoke；具体 Jaco 表单的键盘、焦点和 UI 验证属于 Jaco/adapter 计划。

### 7.3 FORM-70 残留 API 与文档审计

```bash
rg -n "SubmitRuntime|SubmitOutcome|SubmitError::Busy|submission_attempts|last_outcome|TransformContext|transform_on_submit|begin_async_validation|finish_async_validation|FieldChangeSource|set_with_source|source_control_does_not_echo|is_empty_value" \
  crates/gpui-form crates/gpui-form-macros crates/gpui-form-gpui-component

rg --files crates/gpui-form/src | rg '/(core|pipeline|view)(\\.rs|/)'
```

两条命令在最终实现中都应无命中。应用仍可拥有自己命名的 loading/attempt counter；审计不应
误删与 form runtime 无关的产品状态。

### 文档一致性

- `README.md` 为默认英文项目介绍与最短完整示例；`README.zh-CN.md` 是语义一致的中文镜像。
- `docs/guide.md` 与 `docs/guide.zh-CN.md` 覆盖全部公开使用契约；代码 block/API 名必须一致。
- 实施计划只使用中文，并区分 `[当前事实]`、`[上游事实]`、`[用户决定]` 和目标设计。
- `docs/README.md` 与 `dev/README.md` 链接必须可解析；不把实施步骤混入 README。

## 8. 执行交接审计

- [x] 没有未解决架构问题；每项歧义已按用户确认的推荐方案冻结。
- [x] 每个新增 public type/trait/method 都有 owner、side effect、错误和生命周期契约。
- [x] revision、equal write、whole-form operation 和 CAS 的边界已精确定义。
- [x] 同步/异步/control validation bucket、替换范围、排序和 submit 阻塞条件已精确定义。
- [x] GPUI reentrancy 通过 public `defer_*` intent + crate-private weak lifetime 解决；外部无法
  绕过 defer，也不依赖来源跳过隐藏问题。
- [x] Garde 0.23.0 的 exact API、国际化生命周期和 stable path 限制已核实。
- [x] Required、Garde、Validify、GPUI task/entity 能力均优先复用上游。
- [x] UI、数据、数据库、网络、图标/assets、i18n、依赖和平台 surface 均已明确处理或写明
  `No change`。
- [x] 每项需求都映射到具体测试名、命令和 done condition。
- [x] 删除清单和残留 symbol scan 已固定，不给实现者留下兼容策略选择。
- [x] `FORM-60` 只完成 core source/API 与 crate tests；唯一最终 release gate 是
  `MACRO-40 -> CONTROL-50 -> JACO-FORM-70 -> FORM-70`，验证路径和命令已明确。
