# Issue #199：Field 描述符与内部消息 Transition 改造

## 文档定位与原计划所有权

- 状态：`Done`（`WP-100`–`WP-104` 已实施并通过自动化/残留门禁）
- 跟踪 Issue：[#199](https://github.com/suxiaoshao/gpui/issues/199)
- 计划 ID：`issue-199`
- 根计划：[Issue #199 根计划](../../../../../docs/dev/issue-199/README.md)
- 所有者目录：`crates/gpui-form`
- 专题文档：`crates/gpui-form/docs/dev/issue-199/field-descriptors-and-internal-transitions.md`
- 所有者索引：[gpui-form 开发计划](../README.md)
- 所引用的根计划 ID：`S-01` 至 `S-12`；`C-01`（core ↔ macro）、
  `C-02`（core ↔ adapter）、`C-03`（core ↔ Jaco）；`ERR-01`
  （`FieldAccessError`）、`ERR-02`（`FieldMutationError`）、`ERR-03`
  （adapter/control 构造边界），以及 `ERR-04`（`SubmitError`）。
- 所有者编写的本地 ID/范围：`E/D/F/L/ST/R/T-100..199`、
  `WP-100..109`。
- 分配的 WP：`WP-100` 至 `WP-104`。
- 负责：`gpui-form` 公共 runtime——静态与定位描述符、表单状态变更/事件、schema/path 解析、
  验证 runtime、提交准备、导出及其定向测试。
- 不负责：`#[derive(FormModel)]` 展开与生成命名（`C-01`，macro owner）；原生 control wrapper
  及其订阅（`C-02`，adapter owner）；或 Jaco 页面状态、持久化、operation 与可见恢复（`C-03`）。

## 实施结果（2026-08-02）

- `FormState`、静态 `FormField`、`PartialFormField`、`ControlBinding`、非泛型 `FormEvent` 与
  `PreparedSubmit` 已按 C-01–C-03 落地；descriptor 内不再保存 form entity。
- `FormRuntime` 与 validation runtime 的权威变更由 core-private `gpui_operation::Transition`
  message/effect 实现；macro、adapter 与 Jaco 只调用领域 façade。
- `cargo test -p gpui-form --all-features --locked`、合并 clippy 与 active-source residual scan 通过。

## 所有者本地证据

| E-ID | 分类 | 主张 | 证据 | 计划结论 |
| --- | --- | --- | --- | --- |
| E-100 | 当前事实 | `FormField` 保存 `WeakEntity<Form>` 与 `Arc` 读/写闭包；所有同步读取和写入都会升级该弱 entity，且可能返回 `FormReleased`。 | `src/field.rs:39-107,123-170` | 用独立于 form 的描述符和显式 `&Entity<Form>` 操作替换 entity-bound field。 |
| E-101 | 当前事实 | derive 当前生成 `*_field(&Entity<Self>)` 和 `*_in(FormField<...>)` 函数，而 runtime event 是 `FormEvent<Field>`。 | `../gpui-form-macros/src/derive/expand.rs:300-426,569-618` | Core 必须暴露 `C-01` 消费的静态描述符与非泛型 event 契约；此处不实施 macro 迁移。 |
| E-102 | 当前事实 | `FormRuntime` 已拥有当前值、baseline、单调 revision、验证 context、验证 runtime、生命周期替换与 CAS rebase。 | `src/form.rs:32-114,197-265` | 在修改公共 trait/name 和提交返回类型时保留这一唯一权威。 |
| E-103 | 当前事实 | 现有 validation runtime 分离 generated、adapter、control 和保留的 async-task bucket；相交写入会使 generated/async state 失效，whole-model replacement 清除 data validation 但保留 control issue。 | `src/validation.rs:188-328`; `src/field.rs:138-170,263-352` | 保留 bucket 所有权和 retained-task generation 语义；通过 total 或 partial descriptor 操作暴露它们。 |
| E-104 | 当前事实 | `ControlAttachment` 目前是唯一的 deferred callback 边界，但它持有从 descriptor 继承弱 form handle 的 `FormField`。 | `src/control.rs:57-215` | 依 `C-02` 将其改名/改造为 `ControlBinding`，仅将弱 form capture 移至该边界。 |
| E-105 | 当前事实 | `prepare_submit` 验证 clone 的 model 后返回 `Output`；transform 是 `Default` 构造、可失败并报告 `TransformReport`。 | `src/form.rs:173-195`; `src/submit.rs`; `src/submit/transform.rs` | 返回原子的 `PreparedSubmit { revision, output }`，并使静态 submit transform 不可失败。 |
| E-106 | 当前事实 | 既有公开 README 和 guide 是 v2 预览文档：静态 `FormModel` descriptor、total/partial availability、非泛型 event、`PreparedSubmit` 与 `ControlBinding`。 | `README.md:1-198`; `docs/guide.md:41-460` | 实现文档化目标，不添加 v1-to-v2 adapter。 |
| E-107 | 当前事实 | Core 测试覆盖分布于 derive、validation、nested validation、corrective transaction/validation/Garde、submit 及 UI compile-fail suite。 | `tests/{derive,validation,nested_validation,corrective_transactions,corrective_validation,corrective_garde,submit,ui}.rs` | 仅在新公共契约要求时合并/重命名；保留行为覆盖而非只更新名称。 |
| E-108 | 用户决定 | descriptor 永不持有 `Entity`/`WeakEntity`；静态 associated constant 零分配；每个同步操作接收 `&Entity<Form>`。 | Issue #199 设计决定 | 这是 breaking core API，而非可选模式。 |
| E-109 | 用户决定 | `within` 保持 availability；`item` 和 `project_value` 产出 partial descriptor；静态 validation/transform policy 保持 associated，`WeakEntity` 限于 control/async/subscription 边界。 | Issue #199 设计决定 | 在 `L-100` 至 `L-106` 确立精确的 total/partial 与生命周期划分。 |
| E-110 | 用户决定 | 表单状态变化在 core 内部使用 `gpui_operation::Transition` 和私有 owned message/effect；公开表面继续使用 `set`、`validate`、`reset`、`rebase`、`prepare_submit` 等领域方法，不要求调用方发送消息或导入 `Transition`。 | Issue #199 后续设计讨论；Issue 本身已包含迁移到 `gpui-operation` 的范围 | 本次只补充三个 form crate 的中文开发计划；不修改 Issue、公开 README/guide、根计划或 Jaco 文档。 |
| E-111 | 当前事实 | `gpui_operation::Transition<Message>` 可独立表达“状态 + owned message → output”，不负责 GPUI `Context`、任务创建、emit 或 notify；`refresh`/`repair` family 面向单一 fallible resource operation，不匹配表单的多字段、分桶和按 key 并发验证。 | `../gpui-operation/src/transition.rs`、`../gpui-operation/src/{refresh,repair}.rs`、`../gpui-operation/dev/message-driven-transitions.md` | Core 只复用 `Transition` trait；继续拥有专用的表单/验证 runtime，不套用 `refresh::Operation` 或 `repair::Operation`。 |

## 所有者本地决策

| D-ID | 决策 | 证据 | 被拒绝的实质替代方案 | 结论/所有者 |
| --- | --- | --- | --- | --- |
| D-100 | 用 `FormState` 替换 `FormStore` 并保留一个生成的根 `FormRuntime`；不使用 child form entity 或页面拥有的 draft copy。 | E-102；S-01、S-12 | 保留 form-bound field handle 或引入 per-group entity。 | `src/form.rs`、`src/lib.rs`、`C-01`、`ST-100`。 |
| D-101 | 生成的 `State::FIELD` 是只有静态 access metadata 的 schema-level `const FormField<State, T>`；取用它不执行 allocation、subscription、value capture 或 entity capture。 | E-106、E-108；S-02、S-09 | 为每个 form 构造 descriptor 或在其中保留 weak entity。 | Core 提供 descriptor representation；macro 在 `C-01` 下负责生成。 |
| D-102 | Total descriptor 使用不可失败的 `value`、`set`、`validate`、`errors` 和 `is_validating`，均接收 `&Entity<Form>`；dynamic availability 使用 `PartialFormField` 且只提供对应的 `try_*`。 | E-108、E-109；S-03 | 为普通同步代码保留 `Result<_, FormReleased>`。 | 从 descriptor access 删除 `FormReleased`；将 unavailable path 映射为 `ERR-01`/`ERR-02`。 |
| D-103 | `within` 保持 availability；`item` 与 `project_value` 始终生成 partial descriptor。定位/组合 descriptor 不保证公开 `Copy`。 | E-106、E-109；S-03、S-04 | 使所有 descriptor 都为 `Copy`，或将 projection 错误地设为 total。 | 静态 associated constant 保持零分配；dynamic composition 可使用适合其 capture 的 path/accessor 的私有 representation。 |
| D-104 | Form value mutation 保持为一个 root transaction：equal write 无副作用；changed write 提交一次 revision、使相交 state 失效、执行 scoped change validation、发出一次 `ValueChanged`，并只 notify 一次。 | E-102、E-103；S-05 | 让 nested descriptor 独立更新 ancestor form，或添加 origin echo suppression。 | `L-103`、`ST-101`、`R-100`。 |
| D-105 | `FormEvent` 没有 field-enum type parameter。仅有 `ValueChanged { path, revision }`、`ModelReplaced { revision }` 和 `ValidationChanged { scope }`；descriptor observer 为相关 value/model event 重投影，忽略仅验证 event。 | E-101、E-106；S-04、S-05 | 保留 `FormEvent<Field>` 和 root enum ID，或跳过 source control。 | `src/form.rs`、`src/field.rs`、`C-02`、`R-101`。 |
| D-106 | Validation adapter 和 submit transform 是静态 type-level policy，不是保留的 value，也不以 `Default` 构造。Validation 保持同步并返回 scoped bucket；submit transform 不可失败。 | E-105、E-109；S-06、S-08 | 存储 policy instance，或将 `TransformReport` 保留为 pseudo-validation failure。 | `L-104`、`L-105`、`ERR-04`；macro 通过 `C-01` 接收 associated type。 |
| D-107 | `prepare_submit` 在同步 submit validation 与 pending-async rejection 后，从同一 snapshot 产出 `PreparedSubmit { revision, output }`。 | E-105；S-08、S-12 | 分开返回 output，并让调用方稍后读取 revision。 | Jaco 经 `C-03` 消费不可变 save handoff；core 不拥有 I/O 或 busy state。 |
| D-108 | 使 public API 有意不兼容，并在同一 migration 中删除此 owner 的 legacy name/path；不添加 compatibility alias、conversion wrapper 或 `FormReleased` fallback。 | E-106、E-108；S-10 | 在 core 内暂存两套 API。 | 根 rollout owner 在 `C-01` 至 `C-03` 下协调 downstream compilation。 |
| D-109 | `FormRuntime` 与 `FormValidationRuntime` 保持唯一状态权威，并为多个私有 message 类型实现 `gpui_operation::Transition`。Transition 只执行合法性检查和原子状态变更并返回私有 effect；领域 façade 在同一次 entity update 中准备消息、合并 effect，并最多 emit 一个 `FormEvent`、notify 一次。 | E-102、E-103、E-110、E-111；D-104 至 D-107 | 复制一份本地 transition trait；公开统一 `FormMessage`/dispatch API；让 generated state、macro 或 adapter 实现/构造消息；把验证塞进 `refresh`/`repair` operation family；把整个 form 压成单一 phase enum。 | `F-100`、`F-102`、`F-107`、`F-121`、`L-102` 至 `L-107`、`ST-100` 至 `ST-103`、`WP-100` 至 `WP-104`。 |

## 所有者本地目标设计

### 文件与所有权树

```text
crates/gpui-form/
├── Cargo.toml                       # F-100 [修改，手写] 仅增加 workspace gpui-operation 依赖；不新增 feature
├── src/lib.rs                         # F-101 [修改，手写] v2 公共导出与 macro/private runtime bridge
├── src/form.rs                        # F-102 [修改，手写] 领域 façade、FormRuntime、修订、非泛型事件、effect 合并/发布与提交交接
├── src/form/transition.rs             # F-121 [新增，手写] 私有 root message/effect 与 Transition 实现
├── src/field.rs                       # F-103 [重写，手写] 静态/定位后的 total、partial 描述符操作及描述符订阅
├── src/schema.rs                      # F-104 [修改，手写] 静态 schema 元数据与精确运行时路径解析支持
├── src/schema/path.rs                 # F-105 [修改，手写] 不拥有 entity 的静态路径与定位路径组合
├── src/schema/array.rs                # F-106 [修改，手写] 稳定标识条目的可用性与身份辅助函数
├── src/validation.rs                  # F-107 [修改，手写] 私有验证消息、作用域 bucket、控件问题及 keyed async 状态转换
├── src/validation/report.rs           # F-108 [修改，手写] ERR-01/02/04 引用的报告类型
├── src/submit.rs                      # F-109 [修改，手写] PreparedSubmit 与最终 SubmitError 表面
├── src/submit/transform.rs            # F-110 [修改，手写] 静态且不可失败的 SubmitTransform 与恒等转换
├── src/control.rs                     # F-111 [修改，手写] 私有 lease 及面向 C-02 的公开 ControlBinding 弱引用/延迟边界
├── src/typed.rs                       # F-112 [修改，手写] 精简的 v2 类型导出；不保留旧类型别名
├── tests/derive.rs                    # F-113 [修改，手写] 生成静态描述符的消费契约
├── tests/validation.rs                # F-114 [修改，手写] 验证作用域、partial 访问、异步任务生命周期与错误
├── tests/nested_validation.rs         # F-115 [修改，手写] 嵌套组合、路径与事件行为
├── tests/corrective_transactions.rs   # F-116 [修改，手写] 单事务、修订、空操作与 CAS 行为
├── tests/corrective_validation.rs     # F-117 [修改，手写] bucket 失效与身份规则的回归覆盖
├── tests/corrective_garde.rs          # F-118 [修改，手写] 静态 Garde 策略与归一化路径覆盖
├── tests/submit.rs                    # F-119 [修改，手写] PreparedSubmit 与不可失败转换行为
└── tests/ui.rs and tests/ui/fail/*    # F-120 [修改，手写快照] C-01 后的 derive/API compile-fail 覆盖
```

`F-102` 至 `F-112` 与 `F-121` 仍是唯一的 core 实现权威。macro 消费公共声明，但不列入 core 文件；
原生控件 wrapper 在 `C-02` 下消费 `L-100` 至 `L-106`。`Cargo.lock` 仍由 root rollout 所有者在实施期
通过正常 Cargo 流程刷新，不在本 owner 中手工编辑；workspace 已有的 `gpui-operation` package/source/feature
身份不得改变。本所有者不负责生成、vendored、数据库、i18n、图标、打包或平台产物。

### 所有者本地契约

#### L-100：描述符、可用性与错误表面（`F-103`）

```rust
pub struct FormField<Form, T> { /* private static/located descriptor data; no Entity or WeakEntity */ }
pub struct PartialFormField<Form, T> { /* private availability-dependent located descriptor data */ }

#[doc(hidden)]
pub trait FormFieldParent<Child, T>
where Child: FormState {
    type Output;
    fn compose(self, child: FormField<Child, T>) -> Self::Output;
}

pub enum FieldAccessError {
    ValueUnavailable,
    MissingItem(FormItemId),
    DuplicateItem(FormItemId),
}

pub enum FieldMutationError {
    Access(FieldAccessError),
    ItemIdentityChanged,
}

impl<Form, T> FormField<Form, T>
where Form: FormState, T: Clone + PartialEq + 'static {
    pub fn value(&self, form: &Entity<Form>, cx: &App) -> T;
    pub fn set(&self, form: &Entity<Form>, value: T, cx: &mut App);
    pub fn validate(&self, form: &Entity<Form>, trigger: ValidationTrigger, cx: &mut App);
    pub fn errors(&self, form: &Entity<Form>, cx: &App) -> Vec<ValidationIssue>;
    pub fn is_validating(&self, form: &Entity<Form>, cx: &App) -> bool;
    pub fn bind_control(
        &self,
        form: &Entity<Form>,
        cx: &mut App,
    ) -> ControlBinding<Form, T>;
    pub fn subscribe_in<Owner>(
        &self,
        form: &Entity<Form>,
        window: &Window,
        cx: &mut Context<Owner>,
        listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Subscription
    where Owner: 'static;
    pub fn start_async_validation<F, Fut>(
        &self,
        form: &Entity<Form>,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    )
    where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static;
    pub fn cancel_async_validation(
        &self,
        form: &Entity<Form>,
        source: &str,
        cx: &mut App,
    );
    pub fn project_value<U>(
        self,
        name: &'static str,
        read: impl Fn(&T) -> Option<U> + 'static,
        write: impl Fn(&mut T, U) -> bool + 'static,
    ) -> PartialFormField<Form, U>;
}

impl<Child, T> FormField<Child, T>
where Child: FormState, T: Clone + PartialEq + 'static {
    pub fn within<Parent>(
        self,
        parent: Parent,
    ) -> Parent::Output
    where Parent: FormFieldParent<Child, T>;
}

impl<Form, T> PartialFormField<Form, T>
where Form: FormState, T: Clone + PartialEq + 'static {
    pub fn try_value(&self, form: &Entity<Form>, cx: &App) -> Result<T, FieldAccessError>;
    pub fn try_set(&self, form: &Entity<Form>, value: T, cx: &mut App) -> Result<(), FieldMutationError>;
    pub fn try_validate(&self, form: &Entity<Form>, trigger: ValidationTrigger, cx: &mut App) -> Result<(), FieldAccessError>;
    pub fn try_errors(&self, form: &Entity<Form>, cx: &App) -> Result<Vec<ValidationIssue>, FieldAccessError>;
    pub fn try_is_validating(&self, form: &Entity<Form>, cx: &App) -> Result<bool, FieldAccessError>;
    pub fn try_bind_control(
        &self,
        form: &Entity<Form>,
        cx: &mut App,
    ) -> Result<ControlBinding<Form, T>, FieldAccessError>;
    pub fn try_subscribe_in<Owner>(
        &self,
        form: &Entity<Form>,
        window: &Window,
        cx: &mut Context<Owner>,
        listener: impl FnMut(&mut Owner, &mut Window, &mut Context<Owner>) + 'static,
    ) -> Result<Subscription, FieldAccessError>
    where Owner: 'static;
    pub fn try_start_async_validation<F, Fut>(
        &self,
        form: &Entity<Form>,
        source: impl Into<Cow<'static, str>>,
        trigger: ValidationTrigger,
        validate: F,
        cx: &mut App,
    ) -> Result<(), FieldAccessError>
    where
        F: FnOnce(T) -> Fut + 'static,
        Fut: Future<Output = Result<(), AsyncValidationIssue>> + 'static;
    pub fn try_cancel_async_validation(
        &self,
        form: &Entity<Form>,
        source: &str,
        cx: &mut App,
    ) -> Result<(), FieldAccessError>;
}

impl<Form, Item> FormField<Form, Vec<Item>>
where Form: FormState, Item: Clone + PartialEq + 'static {
    pub fn item(self, id: FormItemId) -> PartialFormField<Form, Item>;
}
```

实际的私有表示必须分离静态 schema/access lens 与定位后的 path/lens；不得保留 `Entity<Form>`、
`WeakEntity<Form>`、值状态或订阅。静态关联常量必须能从函数指针以及静态 schema/path 数据以
`const` 方式构造。不得为定位、partial 或 projection 描述符添加或承诺公开的 `Copy` bound。
对于 `T` 为 `Vec<Item>` 的 identified-array 描述符，静态元数据包含一个可选函数项，其签名等价于
`fn(&T, usize) -> Option<FormItemId>`；macro 根据 `#[form(array(id = "..."))]` 提供该函数项。
这样无需 `Any`、不安全的类型擦除，也无需应用在每次调用 `item` 时传入闭包，即可让 ID accessor
保持类型安全且零分配。
`FormField::item` 只存在于携带 macro 所生成 ID-at-index 元数据的 `Vec<Item>` 描述符，返回
`PartialFormField<Form, Item>`；调用方绝不传递 ID getter。它在写入时验证已捕获的 stable ID。
`project_value` 始终返回 partial，并为验证保留最近的真实模型路径，而身份、异步与控件问题使用
projection path。

`ERR-01` 只由 partial 读取、查询和验证操作返回。`ERR-02` 在变更时包装它，并额外报告候选替换值
改变了不可变的 identified item ID。由于调用方提供强 entity，total 操作既没有错误类型，也没有
存活性分支。

#### L-101：schema 元数据与精确路径（`F-104` 至 `F-106`）

```rust
pub trait FormModelSchema {
    fn schema_at_path(&self, segments: &[FieldPathSegment])
        -> Result<&'static FieldSchema, FormSchemaPathError>;
}
```

保留 declared field、group、array container、direct identified-item root 与 nested leaf 的 exact resolver
行为。missing/duplicate/unconvertible item 仍是 availability/schema failure；绝不选择第一个 match、回退到
index 或使用 ancestor schema。static schema 从 descriptor（`field.schema()` 与 `field.path()`）获得，而非
从生成的 public field enum 获得。`within` 在不定义新 schema 的情况下组合 static total parent/child 与
partial parent；`item` 和 `project_value` 只创建 located descriptor。

#### L-102：状态、修订与事件契约（`F-102`）

```rust
pub enum FormEvent {
    ValueChanged { path: FieldPath, revision: FormRevision },
    ModelReplaced { revision: FormRevision },
    ValidationChanged { scope: ValidationScope },
}

pub trait FormState: EventEmitter<FormEvent> + Sized + 'static {
    type Model: Clone + PartialEq + StructuralValidate + FormModelSchema + 'static;
    type ValidationContext: ValidationContextValue;
    type ValidationAdapter: ValidationAdapter<Self::Model, Context = Self::ValidationContext>;
    type SubmitTransform: SubmitTransform<Self::Model>;
    // from_value, from_value_with_validation_context, value, baseline, revision,
    // validation context, replace/reset/rebase/rebase_if_revision, validation queries,
    // prepare_submit, and macro-only runtime glue
}
```

`FormRuntime` 仍是值、baseline、`FormRevision`、验证上下文与 `FormValidationRuntime`（`ST-100`）的
唯一所有者。`replace`、`reset` 与 `rebase` 即使值相等也推进修订，取消或清除数据派生的验证状态，
只发出 `ModelReplaced` 并通知。失败的 `rebase_if_revision` 是完整的空操作：值、baseline、修订、报告、
保留任务、事件流与通知均保持不变。删除现有字段 enum 关联类型和 `FieldChanged` variant。

`FormState` 与 generated state 的公开表面只保留上述领域方法。`gpui_operation::Transition`、内部 message、
effect 和验证状态枚举都不是 trait associated type，也不从 `lib.rs`/`typed.rs`/`__private` re-export；公开
调用方不需要依赖 `gpui-operation`。macro 所需的 doc-hidden runtime bridge 也只暴露领域级委托，由 core
在 bridge 内部构造私有 message。

#### L-103：根变更、effect 发布与描述符观察（`F-102`、`F-103`、`F-121`）

Core 保留一个领域 façade，由描述符 lens、描述符的实际事件路径和真实验证路径参数化。它必须：

1. 恰在一次 `Entity<Form>::update` 中解析 descriptor，clone 当前模型并应用纯 lens；partial 访问或身份失败
   在任何 authoritative state 写入前返回，候选值相等时返回 `FormTransitionEffect::Unchanged`；
2. 在 façade 层调用需要 `&App` 的 schema/validation policy，得到 owned 的候选值、精确路径和同步验证结果；
   `Transition` 本身不得接收 `App`、`Window`、`Context<_>` 或闭包 lens；
3. 把这些 owned data 组装为 `CommitFieldValue`，交给 `&mut FormRuntime` 的私有 `Transition` 实现；该实现
   原子提交一次值/revision，只使相交的 generated、structural、adapter 与异步状态失效，并安装 scoped
   validation result；
4. façade 合并 root/validation effect，然后统一应用：changed write 只 emit 一次
   `FormEvent::ValueChanged { path, revision }` 并 notify 一次；无变化或失败不 emit、不 notify。

`replace`、`reset`、`rebase`、`rebase_if_revision` 和 validation-context replacement 走同一 façade → 私有
message → transition → effect 流程。CAS 的 expected revision 必须在 transition 内再次检查；失败返回
`Unchanged`。`ModelReplaced`/`ValueChanged` effect 吸收同一 transaction 中的 validation change，不能再追加
第二个 `ValidationChanged` 或第二次 notify；只有纯验证变化才发布 `ValidationChanged`。

创建描述符订阅时接收显式的强 form；其保存的 GPUI 订阅仅可在回调生命周期边界保留弱 form 状态。
收到 `ValueChanged` 时，若事件路径与描述符路径相交则重投影；每个 `ModelReplaced` 都重投影；收到
`ValidationChanged` 时不重投影。不存在来源 payload、权威回读或回声跳过逻辑。

#### L-104：验证策略与运行时（`F-107`、`F-108`）

```rust
pub trait ValidationAdapter<Model>: 'static {
    type Context: ValidationContextValue;
    fn validate(
        value: &Model,
        trigger: ValidationTrigger,
        scope: &ValidationScope,
        context: &Self::Context,
        cx: &App,
    ) -> ValidationAdapterReport;
}
```

适配器由 `FormState::ValidationAdapter` 选定，并由 macro 生成的验证胶水代码静态调用，既不存储也不使用
`Default` 构造。`FormValidationRuntime` 仍是生成的、适配器、控件及保留的异步 bucket（`ST-102`）的权威来源。
一次有作用域的运行只替换经精确 scope/trigger 归一化选中的 bucket；它保留同级 bucket 及受生命周期约束的
控件 issue。无效的适配器 path 会在 scope filtering 前变为一个阻塞性的内部 issue。
同步 policy 调用仍在领域 façade 中完成，结果通过 `ReplaceSynchronousValidation` message 安装；
`set_validation_context` 通过 root transition 只替换并发布新的 context，不隐式运行动态验证。

异步 API 在公开的同步入口接收 `&Entity<Form>`。façade 先读取 typed snapshot、创建 task/attempt，再以
`StartAsyncValidation` 将 task handle 安装进 keyed state；只有 task 的完成回调捕获弱表单并发送 owned
`CompleteAsyncValidation`。相交变更、显式取消和整个 model 生命周期操作分别发送 invalidation/cancel message。
过期 completion 返回 `ValidationTransitionEffect::Unchanged`，没有 state/event/notify 效果；任何处于
`Running` 的保留条目都会阻止 `prepare_submit`。

#### L-105：提交（`F-102`、`F-109`、`F-110`）

```rust
pub struct PreparedSubmit<Output> {
    pub revision: FormRevision,
    pub output: Output,
}

pub trait SubmitTransform<Model>: 'static {
    type Output: 'static;
    fn transform(model: &Model) -> Self::Output;
}

pub enum SubmitError {
    Validation(ValidationReport),
    ValidationPending,
}

fn prepare_submit(&mut self, cx: &mut Context<Self>)
    -> Result<
        PreparedSubmit<
            <Self::SubmitTransform as SubmitTransform<Self::Model>>::Output,
        >,
        SubmitError,
    >;
```

`prepare_submit` 一同快照 value 和 revision，对该 model 快照执行同步提交验证，并通过 L-107 的验证 transition
安装结果；façade 仅在有效变化时发布一次 `ValidationChanged`/notify。随后它拒绝 report issue 和任何
`Running` async state，恰好调用一次关联 transform，并将其 output 与捕获的 revision 一同返回。
`prepare_submit` 不引入 submit phase/message-driven public API，也不把 transform 放进 transition。删除 transform
failure 和 `TransformReport`：内联的业务拒绝属于验证，而保存 failure 属于调用方的 operation。Core 不拥有
持久化 task、忙碌 state、retry、notification 或 `gpui-store` access。

#### L-106：控件绑定边界（`F-111`）

`ControlAttachment`、`FormControl`、`ControlId` 以及所有公开的 `attach_control` 入口均按
`C-02` 替换。精确的公开边界如下：

```rust,ignore
pub struct ControlBinding<Form: FormState, T> { /* private weak form, descriptor, id, lease */ }

impl<Form, T> Clone for ControlBinding<Form, T>
where Form: FormState, T: Clone + PartialEq + 'static;

impl<Form, T> ControlBinding<Form, T>
where Form: FormState, T: Clone + PartialEq + 'static {
    pub fn defer_set<Owner>(
        &self,
        value: T,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where Owner: 'static;

    pub fn defer_blur<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where Owner: 'static;

    pub fn defer_set_issue<Owner>(
        &self,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where Owner: 'static;

    pub fn defer_clear_issue<Owner>(
        &self,
        window: &Window,
        cx: &mut Context<Owner>,
    ) where Owner: 'static;
}
```

`FormField::bind_control(form, cx)` 是 total；partial 的 `try_bind_control(form, cx)` 先解析当前 path，且只返回
`ERR-01`。`ControlBinding` 拥有 lease/control-issue lifecycle，并且是唯一会为延迟的 component callback 捕获
`WeakEntity<Form>` 的 core API。它绝不暴露该弱 handle、control identifier、immediate form mutation 或 read-back。
丢弃最后一个 clone 会清除其 control issue；当 form、lease 或 partial path 不可用时，延迟 callback 会静默停止。

`ControlBinding::defer_*` 仍是公开领域 intent。其排队 callback 回到 core façade 后，才由 core 构造
`CommitFieldValue`、`SetControlValidationIssue` 或 `ClearControlValidationIssue` 等私有 message；binding、macro
与 component adapter 都不得存储、构造、匹配或 re-export transition protocol。

#### L-107：私有 message、验证状态与 effect 契约（`F-102`、`F-107`、`F-121`）

下面的名称和可见性是实现目标；字段可按模块拆分，但不能提升到公开或 doc-hidden macro surface：

```rust,ignore
#[must_use]
pub(crate) enum FormTransitionEffect {
    Unchanged,
    Notify,
    Publish(FormEvent),
}

#[must_use]
pub(crate) enum ValidationTransitionEffect {
    Unchanged,
    Changed(ValidationScope),
}

pub(crate) struct CommitFieldValue<Model> {
    candidate: Model,
    event_path: FieldPath,
    validation_path: FieldPath,
    validation: SynchronousValidationBatch,
}
pub(crate) struct ReplaceModel<Model> { /* value + mount validation batch */ }
pub(crate) struct ResetModel { /* mount validation batch */ }
pub(crate) struct RebaseModel<Model> { /* canonical value + mount validation batch */ }
pub(crate) struct RebaseModelIfRevision<Model> {
    expected: FormRevision,
    value: Model,
    validation: SynchronousValidationBatch,
}
pub(crate) struct ReplaceValidationContext<Context>(Context);

pub(crate) enum SynchronousValidationSource {
    Generated,
    Structural,
    Adapter,
}
pub(crate) struct SynchronousValidationUpdate {
    source: SynchronousValidationSource,
    issues: Vec<ValidationIssue>,
}
pub(crate) struct SynchronousValidationBatch {
    scope: ValidationScope,
    updates: Vec<SynchronousValidationUpdate>,
}
pub(crate) struct ReplaceSynchronousValidation(SynchronousValidationBatch);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AsyncValidationKey {
    path: FieldPath,
    source: Cow<'static, str>,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AsyncValidationAttempt(u64);
pub(crate) enum AsyncValidationOutcome {
    Valid,
    Invalid(ValidationIssue),
}
pub(crate) enum AsyncValidationState {
    Running { attempt: AsyncValidationAttempt, task: Task<()> },
    Settled { attempt: AsyncValidationAttempt, outcome: AsyncValidationOutcome },
}
pub(crate) struct StartAsyncValidation { /* key + attempt + task */ }
pub(crate) struct CompleteAsyncValidation { /* key + attempt + outcome */ }
pub(crate) struct CancelAsyncValidation { /* key */ }
pub(crate) struct InvalidateValidationPath { /* path */ }
pub(crate) struct SetControlValidationIssue { /* lease + path + issue */ }
pub(crate) struct ClearControlValidationIssue { /* lease */ }

// 每个 message 使用独立、精确的 Output；不定义公开统一 FormMessage。
impl<Model, Context> Transition<CommitFieldValue<Model>>
    for &mut FormRuntime<Model, Context>
{
    type Output = FormTransitionEffect;
    /* ... */
}

impl Transition<StartAsyncValidation> for &mut FormValidationRuntime {
    type Output = ValidationTransitionEffect;
    /* ... */
}
```

必须满足以下不变量：

- 所有 authoritative runtime 写入都经过上述私有 transition；descriptor/form/binding 的公开领域方法负责准备
  owned data 和最终应用 effect，不公开 `dispatch`、message 构造器或 `Transition` bound；
- transition 不接收 GPUI context，不 spawn task、不 emit、不 notify。`F-102` 的单一 effect applicator 将
  `Unchanged` 映射为零发布、`Notify` 映射为一次 notify、`Publish(event)` 映射为一次 emit 加一次 notify；合并时
  `ModelReplaced`/`ValueChanged` 优先于同一 transaction 的 `ValidationChanged`；
- form 可以同时 dirty、invalid 和 validating，因此不创建单一 `FormPhase`。同步 bucket 仍是独立 state；异步状态
  则按 `AsyncValidationKey { path, source }` 保存在 map 中，不同 key 可并发；
- map 中不存在 key 表示从未运行、已取消或已失效；`Running` 才计入 `is_validating`/pending-submit，`Settled::Invalid`
  才进入 report。新的同 key `Start` 必须先安装新 state，再 drop 被替换 task；不同 key 互不取消；
- completion 只在当前 entry 为同 key、同 attempt 的 `Running` 时转成 `Settled`。stale completion、失效 lease、
  重复 cancel 或等价 bucket replacement 都返回 `Unchanged`；cancel/invalidate 删除 entry，不恢复旧 settled result；
- `AsyncValidationAttempt` 与 `FormRevision` 分离：不相交字段变更不应让仍有效的异步 completion 失效；相交路径由
  `InvalidateValidationPath` 精确取消；
- 本 crate 不使用 `gpui_operation::refresh::Operation`、`repair::Operation`、operation phase、family task owner 或
  phase-to-UI mapping。这里只复用 `Transition` trait 和 message-driven state change 形式。

### 状态与数据流

#### ST-100：根类型化表单状态

- **权威来源：** macro 生成的 `FormState` entity 中的 `FormRuntime<Model, ValidationContext>` 字段。
- **初始化与生命周期：** `FormState::from_value[_with_validation_context]` 创建它；entity 销毁时丢弃它及保留的异步任务。
- **读取方：** 带显式 `&Entity<Form>` 的 `L-100` 描述符；`L-102` form 查询；`C-02` 控件；`C-03` 页面。
- **变更：** 唯一路径是公开领域方法 → L-107 私有 owned message → `Transition` 修改 runtime；`L-103` 描述符写入、`L-102` replace/reset/rebase/CAS 与上下文 setter 都遵循该路径。
- **发布与投影：** transition 只返回 effect；`F-102` 在 entity update 末尾最多 emit 一个 `FormEvent` 并 notify 一次。页面重新渲染 form 级反馈，控件在 `C-02` 下订阅并重投影。
- **持久化边界：** 无。页面拥有持久化，并通过 `C-03` 有条件地将规范保存模型返回给 `rebase_if_revision`。
- **重置与取消：** 已变更的描述符写入会取消相交的异步条目；生命周期替换会清除数据验证并取消所有保留的异步条目，但保留由 `ST-102` 管理的已挂载控件问题。

#### ST-101：描述符可用性与变更

- **权威来源：** `L-100` 中的 static schema/access metadata 加上 `ST-100` 中的 current model；没有 descriptor-owned mutable state。
- **初始化与生命周期：** generated associated constant 无需 form 即存在；located descriptor 由 `within`、`item` 或 `project_value` 创建，并正常 drop。
- **读取方：** synchronous page/control code 调用显式的 total 或 `try_*` method。
- **变更：** descriptor lens 只生成候选值/路径；`L-103` façade 把 owned candidate 交给 root transition，descriptor 自身不更新 entity、不构造 message。
- **发布与投影：** `ValueChanged` 携带实际的 located path；descriptor subscription 比较 path 并从 `ST-100` 重新读取。
- **持久化边界：** 无。
- **重置与取消：** item availability 会针对每个 current model 重新计算；whole-array replacement/rebase 可使旧 partial descriptor 再次不可用或可用，且不保留 retired-ID history。

#### ST-102：验证、控件与异步运行时

- **权威来源：** `ST-100` 内的 `FormValidationRuntime`；`ControlBinding` 仅保留 lease liveness，而 native control 在 `C-02` 下保留 focus/IME/temporary editor text。
- **初始化与生命周期：** 构造时为空，value/context 安装后 mount validation 运行一次；replacement 或 form drop 时结束保留的 task。
- **读取方：** report/form query 及 descriptor error/pending query；submit preparation。
- **变更：** generated static validation glue 只计算结果；`L-103`、显式 dynamic/blur call、async start/complete/cancel/invalidate 和 `L-106` control issue intent 均转换成 L-107 的私有 validation message。
- **发布与投影：** validation transition 返回 `Changed(scope)` 后，由 `F-102` 转成一次 `ValidationChanged` 加 notify；被 value/model transaction 吸收时不重复发布。value projection 忽略该 event。
- **持久化边界：** 无。
- **重置与取消：** scoped replacement 保留无关的 sibling bucket；stale task generation 和失效的 control lease 不能让 issue 复活。

#### ST-103：内部 message/effect 管线

- **权威来源：** message 和 effect 是一次领域调用内的瞬时 owned value，不是第二份 form state；唯一长期权威仍是 `ST-100`/`ST-102`。
- **初始化与生命周期：** façade 在一次 entity update 中创建 message，transition 同步消费；effect 在返回调用方前由 `F-102` 应用并丢弃。
- **读取方：** 只有 `F-102`、`F-107`、`F-121` 的 core-private 实现和同模块单元测试；macro、adapter、app 与公开文档都不可见。
- **变更：** message 不可变；transition 直接变更唯一 runtime，并返回 `Unchanged`/`Notify`/`Publish` 或 validation `Changed`。
- **发布与投影：** 只有 façade effect applicator 可访问 GPUI emit/notify；transition 和生成代码没有发布权限。
- **持久化边界：** 不持久化、不进入 `gpui-store`，也不序列化或跨 async task 传递 task handle 以外的 runtime state。
- **重置与取消：** 没有消息队列或 replay；async completion 是新的 owned message，并由 key/attempt 合法性决定是否生效。

### 边界实现

| 根契约 | Core 实现 | Consumer/result |
| --- | --- | --- |
| C-01 | `L-100`/`L-102` 定义 `FormField`、`PartialFormField`、`FormState`、static validation/transform policy、`PreparedSubmit` 和 non-generic `FormEvent`；`F-101` 保留只含领域委托的 macro-only runtime visibility。L-107 message/effect/`Transition` 实现保持 core-private。 | Macro 生成 named state + associated const descriptor，并针对精确的 v2 declaration 编译；不依赖 `gpui-operation`，不生成消息或 `Transition` impl。 |
| C-02 | `L-100`、`L-103` 和 `L-106` 接收显式 form entity，并提供 total/partial binding creation 加 deferred intent；core 在 binding callback 后才进入 L-107。 | Adapter 拥有 wrapper/subscription 和 native state；没有 form descriptor 拥有 weak entity，也不依赖/构造内部 transition protocol。 |
| C-03 | `L-102`/`L-105` 暴露 `PreparedSubmit` 和 `rebase_if_revision`；`ST-100` 不承担 persistence ownership。 | Jaco 捕获 prepared pair，拥有 save operation/error，并对 canonical saved model 执行 CAS-rebase。 |

## 所有者本地工作包

### WP-100：建立 core v2 公共基础类型与内部 transition 基础

**所有者**

`crates/gpui-form`

**前置条件与契约**

- `D-100` through `D-103`, `D-109`; `S-01` through `S-04`; `C-01`; `ERR-01`, `ERR-02`.

**文件 ID**

- `F-100` through `F-106`, `F-112`, `F-113`, `F-115`, `F-120`, `F-121`.

**实现顺序**

1. 在 `F-100` 增加 `gpui-operation = { workspace = true }`，不开额外 feature；建立 `F-121` 私有 message/effect/`Transition` 模块和 `F-102` 单一 effect applicator。不得 public/doc-hidden re-export message、effect 或 `Transition`；`Cargo.lock` 由 root rollout 在实施期通过 Cargo 刷新，不能手改。
2. 将 core trait/runtime-facing export 重命名为 `FormState`；将 entity-bound field representation 替换为 private static/located lens representation 及显式 entity total/partial method。现有 optional Validify feature 仍只服务 static infallible transform implementation。
3. 实现保持 availability 的 `within`、partial 的 `item`/`project_value`、精确的 identity error，以及不捕获 entity 的 descriptor-owned schema/path query。
4. 用 `FormEvent` 替换 field-enum event plumbing；从 active core export 中删除 `FormStore`、`FormFieldError`、`FormReleased`、`_field` compatibility entrypoint 和所有 alias。

**测试**

| R-ID | T-ID/文件 | 建议场景 | 固件/mock | 断言 |
| --- | --- | --- | --- | --- |
| R-100 | T-100 `tests/derive.rs` | Static descriptor 无需 form 即可复用，且 total operation 使用显式 entity。 | C-01 之后 derive 的 simple model。 | 没有 entity/weak handle constructor；`value/set/errors` 没有 `Result`。 |
| R-101 | T-101 `tests/nested_validation.rs` | Static `within`、item 和 projection availability composition。 | nested group 和 identified array model。 | Static parent 保持 total；item/projection 和 descendant 只暴露 `try_*`；missing/duplicate ID 返回 `ERR-01`。 |
| R-102 | T-102 `tests/ui.rs` | v1 field constructor/error 不存在，内部 transition protocol 不可访问。 | trybuild pass/fail fixture。 | 旧 `*_field`、`FormStore`、`FormFieldError` 和 field-generic event 用法以 v2 diagnostic 失败；外部也不能导入 message/effect/module 或要求 generated state 实现公开 `Transition`。 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form --all-features --locked --test derive --test nested_validation --test ui` | Static/partial API 和 generated consumer contract。 | 已锁定的 workspace dependency。 | 所有定向 core test 和 UI snapshot 通过。 |

**完成条件**

生成的静态描述符不包含 form 实例或弱句柄，普通同步字段操作不会失败；动态缺失是显式的，v1
公开符号没有仍然生效的 core 兼容路径，且新增的 transition 基础完全留在 core private surface。

### WP-101：保留单次根变更/事件事务

**所有者**

`crates/gpui-form`

**前置条件与契约**

- `WP-100`; `D-104`, `D-105`, `D-109`; `S-04`, `S-05`; `C-02`.

**文件 ID**

- `F-102`, `F-103`, `F-107`, `F-114` through `F-117`, `F-121`.

**实现顺序**

1. 将每次 total/partial descriptor write 和 replace/reset/rebase/CAS 路由到 `L-103` façade；façade 只准备 owned candidate/validation data，所有 authoritative write 经 `F-121` 的 message-specific `Transition` 完成。保留 equal-write、partial failure 和 failed-CAS 的 `Unchanged` 语义。
2. 实现 effect 合并/应用：发出 non-generic event variant 并使 subscription matching 识别 path；whole-form lifecycle 只发出 `ModelReplaced`，同一 transaction 的 validation effect 不再额外发布。
3. 将 deferred control event 后所需的所有 weak capture 移入 `ControlBinding`；binding 回调只进入领域 façade，删除 origin/source skip/read-back behavior，且不向 C-02 暴露 message/effect。

**测试**

| R-ID | T-ID/文件 | 建议场景 | 固件/mock | 断言 |
| --- | --- | --- | --- | --- |
| R-103 | T-103 `tests/corrective_transactions.rs` 加 `F-121` 单元测试 | Nested total/partial write、equal write 与 root message/effect。 | Event recorder 加 nested model。 | Changed value 返回一个 publish effect，对外观察一次 revision/value event/notify；equal 或 failed item write 返回 `Unchanged`，没有 state/event/task/report change。 |
| R-104 | T-104 `tests/nested_validation.rs` | Reprojection event selection。 | Descriptor subscription recorder。 | Relevant `ValueChanged` 和所有 `ModelReplaced`（包括 origin）都会 reproject；`ValidationChanged` 不会。 |
| R-105 | T-105 `tests/derive.rs` 加 `F-121` 单元测试 | further edit 后的 CAS。 | Prepared model lifecycle fixture。 | Failed CAS 在 transition 内返回 `Unchanged`，保持所有 `ST-100` 和 `ST-102` fact 不变；成功 lifecycle 只发布一个 `ModelReplaced` effect。 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form --all-features --locked --test corrective_transactions --test nested_validation --test derive` | Transaction/event/CAS invariant。 | 已锁定的 workspace dependency。 | Event/revision 和 reproject assertion 通过。 |

**完成条件**

没有 nested lens 执行另一次 entity update，且没有 value write 暴露 origin suppression、spurious validation event 或 duplicate notification。

### WP-102：完成验证和控件生命周期迁移

**所有者**

`crates/gpui-form`

**前置条件与契约**

- `WP-100`, `WP-101`; `D-106`, `D-109`; `S-06`, `S-07`; `C-02`; `ERR-01`, `ERR-03`.

**文件 ID**

- `F-103`, `F-107`, `F-108`, `F-111`, `F-114` through `F-118`, `F-121`.

**实现顺序**

1. 将验证适配器调用转换为带直接上下文/引用参数的静态关联策略；façade 保留精确的先路径解析、后作用域归一化顺序，把 owned report 交给 `ReplaceSynchronousValidation` transition，并保持相互独立的作用域 bucket。
2. 按 L-107 实现 keyed `AsyncValidationState` 与 start/complete/cancel/invalidate transition matrix。task 在 façade 创建；同 key replacement 先安装新 `Running` 再 drop 旧 task，不同 key 并发；completion 只接受当前 attempt，settled/absent/stale 路径不得发布 effect。
3. 让 total/partial explicit-entity API 贯穿 dynamic/blur/async 入口；只有启动后的 completion callback 使用弱引用，`AsyncValidationAttempt` 与 form revision 分离，相交路径失效由专用 message 表达。
4. 用最小的 `ControlBinding` 延迟意图 surface 替换公开的 `ControlAttachment`/`FormControl`，保持 control-issue lease 私有；set/clear/drop 都经私有 validation transition，最后一个 binding drop 清除 issue，失效 lease 返回 `Unchanged`。

**测试**

| R-ID | T-ID/文件 | 建议场景 | 固件/mock | 断言 |
| --- | --- | --- | --- | --- |
| R-106 | T-106 `tests/validation.rs` 加 `F-107` 单元测试 | 作用域验证保留同级/表单/控件 bucket，并覆盖同步 bucket message/effect。 | 带 valid/invalid nested path 的 recording adapter。 | 精确替换 scope/trigger；malformed path 为一个阻塞性的内部 issue；等价 replacement 返回 `Unchanged`，实际变化只返回一个 `Changed(scope)`。 |
| R-107 | T-107 `tests/corrective_validation.rs` 加 `F-107` 单元测试 | 同 key replacement、不同 key 并发、相交写入/生命周期取消和 stale completion。 | 受控 async future、key、attempt 和 path。 | 新 state 在旧 task drop 前已安装；只丢弃相交 task；matching completion 转为 `Settled`；stale/重复 completion 不改变内容、不发布；仅 `Running` 阻止提交。 |
| R-108 | T-108 `tests/corrective_garde.rs` | Static Garde policy/path mapping。 | Garde model/context。 | 不存储或以 default 构造 adapter instance；context 和精确 path report 保持正确。 |
| R-109 | T-109 `tests/validation.rs` 加 C-02 下的 adapter test | Binding lease/partial 消失和 control-issue transition。 | 失效 lease、已 drop 的 form、不可用 item。 | 延迟意图静默停止；有效 set/clear 只产生一次 validation effect；失效 lease 为 `Unchanged`；没有 synchronous descriptor weak upgrade。 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form --all-features --locked --test validation --test corrective_validation --test corrective_garde` | Bucket、static adapter、async lifecycle。 | 已锁定的 workspace dependency。 | 所有目标场景通过。 |

**完成条件**

验证一旦开始即完全由表单拥有，状态变化由私有 transition matrix 唯一落实，static policy value 绝不保留或以 default 构造，并且 `WeakEntity` 不存在于 `FormField`/`PartialFormField`，只存在于 binding/async/subscription callback 内部。

### WP-103：使提交准备具备原子性且仅由 core 负责

**所有者**

`crates/gpui-form`

**前置条件与契约**

- `WP-101`, `WP-102`; `D-107`, `D-109`; `S-08`, `S-12`; `C-03`; `ERR-04`.

**文件 ID**

- `F-102`, `F-107`, `F-109`, `F-110`, `F-119`, `F-121`.

**实现顺序**

1. 定义/导出 `PreparedSubmit`，收窄 `SubmitError`，并将 `FormState::prepare_submit` 改为从一个 snapshot 计算 submit validation、经 L-107 validation transition 安装结果，再构造同源 revision/output；不新增公开 submit message 或 phase。
2. 将 `SubmitTransform` 改为其 static infallible associated function；更新 identity 和 optional Validify transform，删除 `TransformReport` 及 transform failure mapping，并保持现有 `validify-transform` feature/dependency 作为内建 v2 grammar contract。除 WP-100 新增的 `gpui-operation` core 依赖与 root-owned lockfile 刷新外，不改 Validify feature/dependency。
3. 更新 focused test，以证明 validation/pending path 不调用 transform、validation effect 最多发布一次且 successful transform 只运行一次；将 persistence task/retry/error 完全交给 C-03 consumer。

**测试**

| R-ID | T-ID/文件 | 建议场景 | 固件/mock | 断言 |
| --- | --- | --- | --- | --- |
| R-110 | T-110 `tests/submit.rs` | Valid submit snapshot。 | 计数 static transform。 | Validation transition 安装 snapshot report；output 和 revision 与同一个 model 匹配；transform 恰好一次；无公开 submit phase。 |
| R-111 | T-111 `tests/submit.rs` | Validation 或 pending async 拒绝。 | Failing validator/`Running` async state。 | `ERR-04`、零次 transform 调用；report 只在 transition 产生实际变化时发布一次。 |
| R-112 | T-112 `tests/derive.rs` | CAS save handoff。 | `PreparedSubmit` 之后 edit。 | Consumer 可使用捕获的 revision；stale rebase 为 no-op。 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo test -p gpui-form --all-features --locked --test submit --test derive` | 原子提交和保存/CAS 边界。 | 已锁定的 workspace 依赖。 | Prepared snapshot 场景通过。 |

**完成条件**

Core 返回一个不可变的 prepared handoff，不提供 transform-report、busy 或 persistence 表面，且没有实现
可以将修订与 prepared output 分开读取。

### WP-104：认证 v1 移除并交接 core

**所有者**

`crates/gpui-form`

**前置条件与契约**

- `WP-100` 至 `WP-103`、macro `WP-200..203`、adapter `WP-300..302` 以及 Jaco
  `WP-400..405`；`D-108`；`S-10`、`S-12`；C-01/C-02 producer gate 与 C-03/C-04
  consumer-complete 证据。该工作包有意置于 root residual-certification wave；
  `WP-100` 已在 atomic breaking worktree 内执行公开 v1 移除。

**文件 ID**

- `F-100` through `F-121`.

**实现顺序**

1. 在 active core source/test 中搜索所有已移除的 v1 name，只删除 residual export/helper 和 stale test fixture，并认证 `WP-100` 未留下 compatibility surface。另行认证 message/effect/transition module 没有 public/doc-hidden re-export，且 active core 没有采用 `refresh::Operation`、`repair::Operation` 或自建同名 transition trait。保留计划中的 Validify feature/dependency，且不添加 alias。
2. 仅在 root plan 授权 final documentation status 后，才因整体 v2 公开 API 落地而更新 core README/guide；本次内部 `Transition` 决策不改变公开签名和使用方式，因此不新增 message/dispatch/phase 说明，也不单独触发公开文档修改。
3. 运行 focused core suite和依赖树检查，随后运行 root-owned macro/adapter/Jaco integration sequence；将任何 consumer-specific failure 报告给 root hub，而非在 core 中泄漏内部协议或添加兼容层。

**测试**

| R-ID | T-ID/文件 | 建议场景 | 固件/mock | 断言 |
| --- | --- | --- | --- | --- |
| R-113 | T-113 residual/boundary scan | Legacy surface 移除且内部 transition 不泄漏/不误用 operation family。 | Active core source、exports 和 test。 | 不保留 v1 active API；没有 message/effect/`Transition` public re-export，没有 `refresh::Operation`/`repair::Operation`，且 macro-support bridge 只暴露领域委托。 |
| R-114 | T-114 package validation | 完整 core crate behavior 与 dependency boundary。 | 已锁定的 all-features build。 | 格式化、check/test/doc/clippy、dependency tree 和 diff check 通过；`gpui-form` 直接依赖 workspace `gpui-operation`，不启用额外 feature。 |

**定向验证**

| 命令/手动场景 | 目的 | 所需环境 | 预期证据 |
| --- | --- | --- | --- |
| `cargo fmt --all --check` | 格式化 gate。 | workspace 的 Rust toolchain。 | 没有格式化 diff。 |
| `cargo check -p gpui-form --all-targets --all-features --locked` | Core 所有 target 编译。 | 已锁定的 workspace dependency。 | 公开领域 API 与私有 transition 实现均编译。 |
| `cargo test -p gpui-form --all-features --locked` | Core runtime 和 UI suite。 | 已锁定的 workspace dependency。 | 所有 core test 通过。 |
| `cargo test -p gpui-form --doc --all-features --locked` | 公开文档仍只消费领域 API。 | 已锁定的 workspace dependency。 | doctest 不需要导入消息或 `Transition`。 |
| `cargo clippy -p gpui-form --all-targets --all-features --locked -- -D warnings` | Core lint gate。 | 已锁定的 workspace dependency。 | 没有 warning。 |
| `cargo tree -p gpui-form --edges normal --depth 1 --locked` | 新依赖与 feature 边界。 | 已锁定的 workspace dependency。 | 仅出现 workspace `gpui-operation` normal dependency；没有额外 feature/family crate。 |
| `cargo tree -d --locked` | Shared GPUI source identity check。 | 已锁定的 workspace dependency。 | 未引入 duplicate GPUI identity。 |
| `git diff --check` | Whitespace check。 | Repository worktree。 | 输出为空。 |

**完成条件**

core 只暴露 explicit-entity v2 领域契约，内部 transition protocol 不可被下游观察或构造，且其 package-level
测试通过；下游集成证据和面向用户的文档完成状态仍由 root 负责。

## 定向验证与交接

| R-ID/需求 | 所有者/WP | 自动/手动证据 | 预期结果 | 外部前置条件 |
| --- | --- | --- | --- | --- |
| R-100 至 R-102 | WP-100 | `derive`、`nested_validation` 和 UI test | Static associated descriptor 和 total/partial API 按指定方式编译和运行。 | C-01 macro implementation。 |
| R-103 至 R-105 | WP-101 | transaction/nested/derive test | 单次 update/revision/event/notify 和 no-op CAS invariant。 | WP-100 完成后无。 |
| R-106 至 R-109 | WP-102 | validation/corrective/Garde 加 C-02 adapter test | Scoped bucket、async cancellation 和唯一的 weak binding boundary。 | C-02 adapter migration。 |
| R-110 至 R-112 | WP-103 | submit/derive test 加 C-03 consumer test | Prepared output/revision 具备原子性；transform 不会失败且只运行一次。 | C-03 page migration。 |
| R-113 至 R-114 | WP-104 | boundary/residual scan、fmt、check、core all-features/doc test、clippy、tree、diff | 没有 core compatibility layer、内部协议泄漏、operation-family 误用或 dependency identity regression。 | Root-controlled aggregate validation。 |

迁移后运行 core residual scan：

```sh
rg -n "FormStore|FormFieldError|FormReleased|ControlAttachment|FormControl|TransformReport|set_user_value|FieldChanged|FormEvent<|refresh::Operation|repair::Operation" crates/gpui-form/src crates/gpui-form/tests
```

它不得有 active-source match。另需定向检查 `src/lib.rs`/`src/typed.rs`/macro-support exports 不含 message/effect
或 `Transition` re-export。macro、adapter 和 Jaco 所有者计划拥有各自对应的 residual scan 和 package command。
本文档记录已经完成的 `WP-100` 至 `WP-104` 设计、实施与验证档案。Issue #199 下
`gpui-form` 后续设计、进度与状态统一由[总入口](README.md)跟踪；新的未定方案先记录在
[设计草稿](design-draft.md)，不得从本文档推断为已确认的新契约。
