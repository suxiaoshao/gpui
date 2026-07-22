# 类型化 bound control 实施计划

## 1. 状态与范围

- 文档位置：`crates/gpui-form-gpui-component/dev/typed-bound-controls.md`。
- 关联分支：`codex/175-jaco-shortcut-temporary-window`。
- 关联 issue：无独立 issue；这是跨 crate form 基础设施迁移，不属于 #175 产品需求。
- 当前阶段：**目标 API、依赖升级、源码迁移与自动化验证已完成；隔离数据目录的 Jaco bundle
  已完成输入校验、blur/change、picker 与 switch 的定向 Computer Use smoke。**
- 当前源码说明：`src/` 已提供只持有 subscriptions 与原生 state entity 的 `FormInput`、
  `FormSelect`、`FormCombobox`、`FormIntegerInput` owning handle；旧 `Config`、form-generic state、
  focus/error visibility 副本、delegate workaround 与 `bind_*` API 已删除。
- 2026-07-21 验证证据：2 个 integer unit tests、6 个 GPUI adapter integration tests、workspace
  build/test、严格 clippy 与 active-source residual scan 均通过；本地 Jaco bundle 已生成并完成
  provider 的 on-change/on-blur 校验、shortcut picker/switch 以及 home approval picker smoke，未出现
  entity 重入 panic。临时窗口全局快捷键与有数据列表的键盘流程仍需人工验证。
- 目标：用一个普通 Rust owning handle，在一次构造中把原生 state entity 与类型化
  `FormField<Form, T>` 双向绑定；form 始终是业务值与 submit 的唯一来源。
- 覆盖：text input、select、combobox、exact integer input，以及无状态
  `Checkbox`/`Switch` 的 controlled-element 用法。
- 非目标：应用布局、catalog/store、fallback、持久化、路由、页面 focus 策略、应用翻译资源。
- 兼容策略：这是破坏性重构。删除旧 `Form*State<Form, ...>`、`Form*Config`、
  `FormControlStatus`、`FormBoolState` 和 `bind_*` 风格 API，不保留兼容 wrapper。
- 协同门槛：Jaco 调用点由 `app/jaco/docs/dev/gpui-form-migration.md` 负责迁移；旧 API
  只有在这些调用点已迁移后才能从同一 changeset 中删除。

### 已确认的用户决策

1. Form 只保存类型化业务值，不保存 component 的 raw draft、focus、blur 或 error visibility。
2. Bound wrapper 只保存 `Vec<Subscription>` 与原生 `Entity<State>`，并 `Deref` 到该 entity。
3. `FormControl<T>` 不提供 `Config`；调用者用构造闭包配置原生 state。
4. Component event 必须 defer 写 form；`FormField::subscribe_in` 对所有 `FieldChanged` 与
   `ModelReplaced` 静默回投影到所有 bound instance，只忽略 `RuntimeChanged`。
5. 不跳过 origin echo，不增加 authoritative-value read-back API；silent setter 终止回路。
6. Input 转发 Change 与 Blur；Select 只消费 `Confirm(Option<Value>)`；Combobox 只消费
   `Change(Vec<Value>)`，不再同时消费 Confirm。
7. Options/delegate/config 由应用持有；adapter 不保存副本、不选择 fallback、不隐式执行
   dynamic validation。
8. Integer component 自身保存精确 `N` 与私有 editor text，所有解析、range、step 和算术均
   使用整数类型，不经过 `f64`。
9. 非法 integer policy 在 bound control 构造时返回 typed error；incomplete/invalid/
   overflow/out-of-range 编辑不写 form，只发布 lifecycle-scoped control issue。
10. `Checkbox` 与 `Switch` 使用 `FormField<bool>` 直接 controlled render，不制造无原生 state
    的 `FormBool` wrapper。
11. 依赖采用统一 Cargo source 策略：Zed git dependency 的 manifest 不写 `rev`，精确提交由
    提交到仓库的 `Cargo.lock` 固定，并在 CI 中使用 `--locked`。
12. Control 统一由 `FormField::attach_control(&self, cx: &mut App)` 创建 attachment；attachment
    clone 共享同一 private lease/liveness，最后一个 clone drop 后 control issue 失效。

## 2. 证据快照

### 2.1 当前仓库事实

| 分类 | 当前事实 | 证据 | 对目标实现的影响 |
| --- | --- | --- | --- |
| GPUI source | workspace 直接依赖带 `rev=1d217ee39d381ac101b7cf49d3d22451ac1093fe` | `Cargo.toml` `[workspace.dependencies]` 与 `[patch.crates-io]` | 必须改为与 upstream 完全相同的无 query git source |
| gpui-component | lockfile 固定 `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` | `Cargo.lock` | 缺少 combobox value-based setter |
| Core attachment | prototype 公开 immediate write、source ID 与 issue lifetime | `crates/gpui-form/src/control.rs` | 收紧为四个返回 `()` 的 deferred intent；weak token、source/control ID 只留在 core 内部 |
| Core trait | `FormControl` 仍有 associated `Form`/`Config`，并返回 `Entity<Self>` | `crates/gpui-form/src/control.rs` | 改成 plain wrapper + build closure 契约 |
| Input prototype | `FormInputState<Form>` 保存 attachment、focused、blurred 与 config | `src/input.rs` | 整体替换为 `FormInput` |
| Select/combobox prototype | 保存 delegate 与 value/index callbacks，并在 adapter 内生成 unavailable issue | `src/select.rs`、`src/combobox.rs` | 删除这些副本与 policy；直接消费/投影 upstream Value |
| Integer prototype | form-generic wrapper 直接包 `InputState`，仍借 upstream `NumberInputEvent` | `src/integer_input.rs` | 拆成独立 native typed state 与最小 bound wrapper |
| Bool/status prototype | 存在 `FormBoolState` 与 `FormControlStatus` | `src/bool.rs`、`src/status.rs` | 删除；由应用 controlled render 和 field query 替代 |

### 2.2 Upstream 事实

目标 gpui-component commit：
[`5b45bcb26b9343d91a123a4d5ed8a654360512e5`](https://github.com/longbridge/gpui-component/commit/5b45bcb26b9343d91a123a4d5ed8a654360512e5)，
对应 [PR #2576](https://github.com/longbridge/gpui-component/pull/2576)。当前到目标的完整 compare 为
[`c36b0c6...5b45bcb`](https://github.com/longbridge/gpui-component/compare/c36b0c6ae6d14c33473f6610a27c3abc584afdf9...5b45bcb26b9343d91a123a4d5ed8a654360512e5)。
该 git 区间没有一份覆盖全部 54 个提交的完整 release migration guide，因此 compare、相关 PR、
manifest、源码与 lockfile 是本计划的迁移记录。

| 上游事实 | 源码/提交证据 | 本地决定 |
| --- | --- | --- |
| `InputEvent::{Change, Focus, Blur, PressEnter}` | target `crates/ui/src/input/state.rs` | 只消费 Change/Blur；silent `InputState::set_value` 用于投影 |
| `SelectEvent::Confirm(Option<Value>)` | target `crates/ui/src/select.rs` | 字段类型固定 `Option<Value>`，只消费 Confirm |
| `SelectState::set_selected_value` 使用当前 delegate，找不到时清空，且 setter 不发 Confirm | target `crates/ui/src/select.rs` | 删除本地 delegate/value-index map |
| `ComboboxEvent::Change(Vec<Value>)` 每次 toggle 发出，Confirm 在关闭时再次发出 | target `crates/ui/src/combobox.rs` | 只消费 Change，避免重复写 |
| `ComboboxState::set_selected_values` 使用当前 delegate，忽略缺失 value、保序、同步 snapshot、不发 event | PR #2576 / commit `5b45bcb...` | 直接复用，删除 captured delegate workaround |
| `Checkbox`/`Switch` 是没有公开 state entity 的 `RenderOnce` element | target checkbox/switch source | 不实现 `FormBool` |
| Upstream `NumberInput` 使用 `f64` range/step，但公开 `NumberInputEvent::Step` 与 presentation API | target `crates/ui/src/input/number_input.rs` | 只复用 UI/action；typed parse/policy/arithmetic 留在本 crate |
| target manifest 的所有 Zed dependencies 都是无 `rev` git source | target root `Cargo.toml` | workspace direct dependencies 与 patch replacement 必须采用相同 source identity |
| target lockfile 把 Zed source 固定到 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba` | target `Cargo.lock` | 首次兼容验证使用该精确 lock SHA |

### 2.3 依赖证据表

| 依赖 | 当前 source/version | 目标 source/version | 破坏性/行为变化 | 受影响位置 | Features/MSRV/平台 | 迁移决定 |
| --- | --- | --- | --- | --- | --- | --- |
| `gpui-component` | git lock `c36b0c6ae6d14c33473f6610a27c3abc584afdf9`, crate `0.5.2` | git lock `5b45bcb26b9343d91a123a4d5ed8a654360512e5`, crate 仍为 `0.5.2` | 新增 combobox value setter；区间还包含 accessibility、input 与 layout 变更 | adapter、所有 workspace UI crates | edition 2024；target 未声明额外 rust-version；tree-sitter 依赖被 feature 化，macOS/Windows native feature 有变化 | 升级整个同源 repo lock revision；不复制 upstream setter |
| `gpui-component-assets` | 同源 lock `c36b0c6...` | 同源 lock `5b45bcb...` | 与 UI crate 必须同一 repo revision | workspace assets consumers | features 不变 | 与 gpui-component 一起更新 |
| `gpui` | manifest/lock 带 `?rev=1d217ee...` | manifest `git=https://github.com/zed-industries/zed`，lock `#1a246efd...` | source identity 改变；target GPUI 含 Taffy 0.12 等 framework 变化 | 所有 workspace members | `gpui_platform` 保留 `font-kit,x11,wayland,runtime_shaders`；需 macOS/Linux/Windows CI | manifest 不 pin，lock 精确 pin，所有 source string 必须完全一致 |
| `gpui_platform`/`gpui_macros` | 同一带 rev source或 crates.io patch replacement | 同一无 query git source，lock `#1a246efd...` | 若任一 source 带 query，会产生第二套 `Entity/App/Window` 类型 | platform crates、macro transitive | 原 features 保持 | workspace 与 patch replacement 同步去掉 rev |
| Taffy | 由旧 GPUI 解析 | 随 target Zed lock 升级到 0.12 系列 | layout behavior 可能变化 | 全部 UI | 纯 Rust，平台无新增 bootstrap | 由 GPUI lock 管理；运行 UI smoke，不增加 direct dep |

### 2.4 已验证的 source 冲突

将 workspace GPUI 写成带 `?rev=` 的 direct dependency、同时使用 target gpui-component 的无
query Zed dependency，即使二者最终解析到相同 SHA，`cargo tree -d --locked` 仍显示两份 GPUI source，
定向 check 会出现
`Entity`、`App`、`Window`、`EventEmitter` 类型不匹配。因此禁止“direct dependency 带 rev、
upstream dependency 不带 rev”的组合，也禁止用同 git source 的 patch 假装统一它们。

### 2.5 明确无变化的系统面

| 系统面 | 决定 | 证据 |
| --- | --- | --- |
| Database/schema | 无变化 | adapter 不访问数据库或持久化 |
| HTTP/provider/auth/cache | 无变化 | adapter 只处理本地 entity/event |
| Icons/assets | 无变化 | 使用现有 gpui-component presentation，无新增 asset |
| App routes/windows | 无变化 | focus/route 仍由调用页面负责 |
| Async validation/network cancellation | 无变化 | 本 crate 只触发同步 change/blur 与 control issue；async validation 属于 core/app |
| Localization resources | 本 crate不打包 locale；只发稳定 key/params | 翻译 owner 是应用；Jaco locale 变更由 Jaco migration plan 覆盖 |

## 3. 设计决策

### D-01：最小 owning wrapper

每个 stateful adapter 的 exported type 是普通 Rust struct，字段顺序固定为 subscriptions 在前、
native entity 在后。它不带 `Form` 泛型，因为 form field 与 attachment 只被 subscription closure
捕获。

后果：同一 field 可创建任意数量实例；每个实例有独立 component interaction state 和 binding
lifetime，但共享同一业务值。

### D-02：始终静默回投影

Component event 通过 attachment defer 一次 typed form write。Form event 不暴露或依赖 source/control
ID；`FormField::subscribe_in` 对每个 `FieldChanged` 与 `ModelReplaced` 回调，不按 path、origin 或
value equality 过滤，只忽略 `RuntimeChanged`。因此 equal-value whole-form lifecycle 与其他 path
导致的 projection 变化也会重新读取当前 field。每个绑定实例执行 silent setter；公开 API 不提供
authoritative-value read-back，silent setter 是回路终点。

拒绝方案：origin skip + read-back reconcile。它需要更多公共 API，并使正确性依赖 skip 与 normalize
分支保持同步。

### D-03：公开 deferred intent，内部 weak lifetime

Component event subscription 持有 `ControlAttachment`，只调用其四个窄 intent：
`defer_set_user_value`、`defer_blur`、`defer_set_issue`、`defer_clear_issue`。Attachment 在 core
内部把 intent 转成 weak、可取消的排队工作；adapter 和应用都看不到 weak attachment、source ID 或
control ID。Attachment 实现 `Clone`，所有 clone 共享同一个 private lease/liveness；最后一个 clone
drop 后 issue inactive。普通 form projection subscription 只捕获 typed field 与自己的
`WeakEntity<State>` 并 defer silent setter。只有拥有 lifecycle-scoped control draft issue 的 typed
editor 可以额外捕获同一个 attachment clone，唯一用途是在 programmatic projection 成功后调用
`defer_clear_issue`；当前内置 adapter 中只有 exact integer control 使用这一例外。Wrapper 字段仍
只有 subscriptions 在前、native state entity 在后；drop 后排队任务不得写 form、更新 component
或延长 control issue。

### D-04：事件与值契约

- `FormInput`：`String`；Change 调 `defer_set_user_value`，Blur 调 `defer_blur`，
  Focus/PressEnter 忽略。
- `FormSelect<D>`：`Option<<D::Item as SearchableListItem>::Value>`；只处理 Confirm。
- `FormCombobox<D>`：`Vec<<D::Item as SearchableListItem>::Value>`；只处理 Change，Confirm 忽略。
- Select/combobox 不承诺 `on_blur`，因为 upstream composite control 没有可靠 final-blur API。

### D-05：application-owned options

Adapter 不存 delegate、items、availability、resolve/value_at callback 或 dynamic-validation policy。
Items 变化后，应用必须调用原生 `set_items` 并立即用当前 form value 调原生 value setter，或重建
wrapper；不能等待 value event，因为 options refresh 不写 form，也不保证产生 `FieldChanged`。
缺失 value 只影响 native projection；form value 不变，也不自动产生 option-unavailable control issue。

### D-06：typed integer 与错误语义

`IntegerInputState<N>` 是本 crate 唯一自定义 native state。它持有 typed `N`、私有
`Entity<InputState>`、内部 subscriptions 和 typed policy。Raw text 只存在于 editor。

- 非法 policy 是开发者构造错误：`NonPositiveStep` 或 `ReversedRange`，在安装 binding 前返回。
- 用户编辑错误是 control issue：Incomplete、InvalidSyntax、Overflow、OutOfRange。
- 编辑错误不改变 form；合法 edit 才 clear issue 并 defer `N`。
- programmatic form change 静默覆盖 raw editor 与 typed projection；只有 field read、weak entity
  upgrade 与 silent projection 都成功后，才清除该 control 旧的 editor issue。由 form 的业务验证
  判断 application-written value 是否满足业务范围。
- step 越界或 checked overflow 为 no-op，不 clamp、不产生另一份业务值。

### D-07：无状态 bool 不伪装 stateful API

删除 `FormBoolState`。Checkbox/Switch 从 `FormField<bool>::value` controlled render，并在 click
调用 `FormField::set_user_value`。页面已有的 form observer负责 rerender；组件没有公开 focus handle，所以 bool
field 不声明依赖原生 `on_blur` 的规则。

### D-08：lockfile 是 git commit 的唯一 pin

根 manifest 的 Zed dependencies 与 crates.io patch replacements 使用完全相同的无 query git URL。
`Cargo.lock` 必须提交 gpui-component `5b45bcb...` 与 Zed `1a246efd...`。所有 build/test/CI 使用
`--locked`；如果 lock source 或 dependency tree 出现第二份 GPUI，立即停止，不进入 adapter 实现。

### D-09：field write 只失效数据级相交 bucket

一次非相等 typed field write 只清除与写入 path 相交的 required、structural、generated
synchronous field bucket，并取消/清除相交的 async validation；adapter-wide issue 与所有 active
control issue 保留。随后执行 Change validation，最后发出 `FieldChanged` 与 notify。相等 field
write 是完整 no-op。Control issue 只能由对应 attachment intent、private lease 结束或 typed integer
成功 programmatic projection 后的 `defer_clear_issue` 清理。

## 4. 目标架构与 API

### 4.1 文件树

```text
Cargo.toml                                      # 修改：统一 Zed source
Cargo.lock                                      # 修改：精确锁定 target commits
crates/gpui-form/src/control.rs                 # 修改：FormControl + deferred attachment intents
crates/gpui-form-gpui-component/
  README.md                                     # 已更新：英文目标 API
  README.zh-CN.md                               # 已更新：中文镜像
  docs/guide.md                                 # 已更新：英文完整指南
  docs/guide.zh-CN.md                           # 已更新：中文镜像
  dev/typed-bound-controls.md                   # 本实施计划
  src/
    lib.rs                                      # 重写 exports
    error.rs                                    # 新增：binding 与 integer policy 构造错误
    input.rs                                    # 重写：FormInput
    select.rs                                   # 重写：FormSelect<D>
    combobox.rs                                 # 重写：FormCombobox<D>
    integer_input.rs                            # 重写：typed state、wrapper、element
    integer_input/error.rs                      # 重写：user edit error
    integer_input/parse.rs                      # 重写：exact parse
    integer_input/policy.rs                     # 重写：typed policy
    bool.rs                                     # 删除
    status.rs                                   # 删除
    number.rs                                   # 保持删除
    binding.rs                                  # 保持删除
  tests/adapters.rs                             # 整体重写
```

仓库规则禁止新增 `mod.rs`；`integer_input.rs` 继续作为子模块入口。

### 4.2 Core 构造与生命周期 API

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

impl<Form, T> ControlAttachment<Form, T>
where
    Form: FormStore,
    T: Clone + PartialEq + 'static,
{
    pub fn defer_set_user_value<Owner>(
        &self,
        value: T,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where
        Owner: 'static;

    pub fn defer_blur<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static;

    pub fn defer_set_issue<Owner>(
        &self,
        code: impl Into<Cow<'static, str>>,
        message: ValidationMessage,
        window: &Window,
        cx: &mut Context<Owner>,
    )
    where
        Owner: 'static;

    pub fn defer_clear_issue<Owner>(&self, window: &Window, cx: &mut Context<Owner>)
    where
        Owner: 'static;
}
```

这四个 intent 都返回 `()`。它们在调用时复制需要跨 update scope 的 owned payload，并由 core
内部执行 weak lifetime 检查、typed field mutation、`ValueUnavailable` 处理与结构 owner notify。
Weak attachment、source/control ID 和 immediate attachment mutation 都不是公开契约；adapter 不自行
实现 upgrade，也不增加 value read-back。`FormField::attach_control` 是唯一 public 创建入口；
`ControlAttachment` 实现 `Clone`，clone 共享同一 private lease/liveness，最后一个 clone drop 后
control issue inactive。四个 `defer_*` 是 attachment 唯一 public mutation intent。

### 4.3 Exported wrapper

```rust,ignore
pub struct FormInput {
    subscriptions: Vec<Subscription>,
    input: Entity<InputState>,
}

pub struct FormSelect<D>
where
    D: SearchableListDelegate + 'static,
{
    subscriptions: Vec<Subscription>,
    select: Entity<SelectState<D>>,
}

pub struct FormCombobox<D>
where
    D: SearchableListDelegate + 'static,
{
    subscriptions: Vec<Subscription>,
    combobox: Entity<ComboboxState<D>>,
}

pub struct FormIntegerInput<N>
where
    N: IntegerValue,
{
    subscriptions: Vec<Subscription>,
    input: Entity<IntegerInputState<N>>,
}
```

四个类型都实现 `Deref<Target = Entity<...>>` 和对应的 `FormControl<T>`；不提供
`input_state()`、`select_state()`、`combobox_state()` 或 matching-element helper。
调用者直接使用 deref 与 upstream `Input::new`/`Select::new`/`Combobox::new`，integer 使用
`IntegerInput::new`。

### 4.4 错误 API

```rust,ignore
#[derive(Debug)]
pub enum FormControlError {
    Field(FormFieldError),
    IntegerPolicy(IntegerInputPolicyError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerInputPolicyError {
    NonPositiveStep,
    ReversedRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerInputError<N> {
    Incomplete,
    InvalidSyntax,
    Overflow,
    OutOfRange { min: Option<N>, max: Option<N> },
}
```

`FormControlError` 与 `IntegerInputPolicyError` 定义在 `src/error.rs`；
`IntegerInputError<N>` 定义在 `src/integer_input/error.rs`。`FormControlError` 实现
`Display`、`Error`、`From<FormFieldError>` 与 `From<IntegerInputPolicyError>`。Policy
error 是 developer-facing constructor error；`IntegerInputError<N>` 不从 constructor
返回，而是映射为 validation issue。

### 4.5 Integer native state 与 element

```rust,ignore
pub trait IntegerValue:
    sealed::Sealed + Copy + Eq + Ord + Display + FromStr + 'static
{
    const ZERO: Self;
    const ONE: Self;
    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegerInputPolicy<N> {
    min: Option<N>,
    max: Option<N>,
    step: N,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerInputEvent<N> {
    Change(Result<N, IntegerInputError<N>>),
    Blur,
}

pub struct IntegerInputState<N>
where
    N: IntegerValue,
{
    editor_subscriptions: Vec<Subscription>,
    editor: Entity<InputState>,
    value: N,
    policy: IntegerInputPolicy<N>,
}
```

`IntegerInputState<N>`：

- `new(window, cx) -> Self`，默认 value 为 `N::ZERO`、无 min/max、step 为 `N::ONE`；
- builder `min(N) -> Self`、`max(N) -> Self`、`step(N) -> Self`；
- read-only `value() -> N`、`policy() -> IntegerInputPolicy<N>`、
  `editor() -> &Entity<InputState>`；
- silent `set_value(N, window, cx)`：同步 typed value 与 canonical text，不发
  `IntegerInputEvent`；
- `validate_policy() -> Result<(), IntegerInputPolicyError>`；
- 实现 `EventEmitter<IntegerInputEvent<N>>` 与 `Focusable`，focus handle 直接委托给内部 editor；
- 内部订阅 `InputEvent::{Change, Blur}` 与 `NumberInputEvent::Step`，不保存 focused/blurred bool。

`IntegerInput<N>` 是 `RenderOnce + IntoElement + Focusable + Sizable + Disableable + Styled`
的 presentation wrapper。它从 `Entity<IntegerInputState<N>>` 构造，并把 `placeholder`、`prefix`、
`suffix`、`appearance`、`size`、`disabled` 与 style 原样转交 upstream `NumberInput`；算术仍由
typed state 处理。

### 4.6 Ownership map

| 可变事实 | 唯一 owner | 修改路径 |
| --- | --- | --- |
| typed business value/baseline/report | generated form store | `FormField`、replace/reset/rebase/submit validation |
| binding subscriptions | plain `Form*` wrapper | constructor 创建，wrapper drop 释放 |
| control issue lifetime | shared private attachment lease | `attach_control` 创建；subscription 捕获 clone；最后一个 clone drop 后 inactive |
| Input focus/IME/selection | upstream `InputState` | native component API |
| Select/combobox query/popup/highlight/selection projection | upstream state | native event/setter |
| integer typed projection/raw editor/policy | `IntegerInputState<N>` | editor event、typed silent setter、checked step |
| options/delegate/catalog/disabled/placeholder | application/native component | construction closure；items setter 后立即按 form 重投影，或 rebuild |
| persistence/loading/retry | application service/page | `prepare_submit` 之后 |

### 4.7 End-to-end flow

**Component → form**

1. Constructor 调 `field.attach_control(cx)`，并把共享同一 lease 的 attachment clone 捕获到所需
   subscriptions；wrapper 不增加 attachment 字段。
2. Native entity emits supported user event。
3. Subscription 不 update emitter，只复制 event payload，并调用 attachment 的窄 deferred intent。
4. Core 在下一个 update scope 检查内部 weak lifetime；control 已卸载时无副作用结束。
5. 合法 payload 调 `defer_set_user_value`；Input/integer Blur 调 `defer_blur`；integer invalid
   payload 只调 `defer_set_issue`，恢复合法时先调 `defer_clear_issue` 再写 typed value。
6. 非相等 typed write 修改业务值并推进 revision，只清理相交的 required/structural/generated
   synchronous field bucket 和 async validation；adapter-wide/control issue 保留。随后执行
   `on_change`，最后发 `FieldChanged` 与 notify；相等写是完整 no-op。

**Form → component**

1. Field subscription 对所有 `FieldChanged` 与 `ModelReplaced` 执行，不按 path、origin 或 equality
   过滤；只忽略 `RuntimeChanged`。
2. 普通 callback 只捕获 typed field 与 weak native entity并 defer，不捕获 attachment。
3. 重新从 field 读取权威值，并调用 silent native setter；Setter 不发 user event，因此无递归。
4. Typed integer callback 可额外捕获同一个 `ControlAttachment` clone；只有 field read 与 silent
   native projection 均成功后，才调用 `defer_clear_issue` 清除该 control 旧的 editor issue。

**Drop/path disappearance**

1. Wrapper drop 释放 subscriptions 及其中的 attachment clones；最后一个 clone drop 后 lease 与
   control issue inactive。
2. Core 内部 weak intent 与 form projection 的 weak entity 都无法再生效，不再写 form 或 component。
3. Identified/projected path 先消失但 view 尚未 drop 时，read/write 返回
   `ValueUnavailable`；callback `cx.notify()`，结构 owner 下一次 render 释放或重建 control。

## 5. Upstream 复用与删除审计

| 本地实现 | 上游能力/证据 | 语义差异 | 决定 | 文件 | 回归测试 |
| --- | --- | --- | --- | --- | --- |
| captured select delegate/value-index callbacks | `SelectState::set_selected_value` uses current delegate | upstream missing value clears native selection | Reuse directly；删除 callbacks | `select.rs` | current delegate after reorder/remove |
| captured combobox delegate/index mapping | PR #2576 `set_selected_values` | upstream omits missing values and is silent | Reuse directly；删除 mapping | `combobox.rs` | reorder/add/remove + form unchanged |
| adapter config structs | native constructor/builder/state APIs | native API already owns configuration | Delete | input/select/combobox exports | compile/use examples |
| adapter focus/blur/show-error/status | native focus + form report | form field may be rendered by many controls | Delete | `input.rs`、`status.rs` | two controls share report; blur trigger only |
| origin-echo skip/read-back reconciliation | silent native setters | always projection has one extra silent setter | Delete skip；不新增 read-back | all adapters/core | origin+peer reproject once, no loop |
| `FormBoolState` | controlled Checkbox/Switch element | no native state entity to own | Delete | `bool.rs` | guide example + Jaco render tests |
| `f64` number policy | no upstream exact integer state | cannot represent full u64/i128 exactly | Retain custom typed state only | integer tree | primitive boundaries, >2^53 |
| upstream NumberInput presentation/actions | `NumberInput` APIs | arithmetic event is untyped | Adapt：复用 UI/action，typed state处理 event | `integer_input.rs` | checked increment/decrement |

删除顺序要求：先迁移调用点，再删除 `Form*Config`、`Form*State`、`FormControlStatus`、
bool/status legacy modules，最后收紧 `lib.rs` exports；禁止反向增加兼容别名。

## 6. 工作包

**跨计划依赖图**

```text
DEP-00
  -> core FORM-10..60
  -> macro MACRO-10..40
  -> CORE-GATE -> CONTROL-10 -> CONTROL-20 + CONTROL-30
  -> JACO-FORM-10..60
  -> CONTROL-40 -> CONTROL-50
  -> JACO-FORM-70
  -> core FORM-70
```

`CONTROL-40` 只等待 Jaco 的具体调用点迁移完成；全应用 legacy residual 与最终发布收口属于
`JACO-FORM-70` 与后续 core `FORM-70`：前者完成应用残留，后者执行最终 workspace/public-status
release gate；两者必须在 adapter 文档/exports 已由 `CONTROL-50` 定稿后顺序执行。

### DEP-00：统一 GPUI source 并升级 gpui-component

**Prerequisites**

- 用户已确认 lockfile-pin 策略。
- 这里确认的是设计选择，不是未来命令权限。实际执行任何联网依赖解析、更新 `Cargo.lock` 或其他
  受仓库权限规则约束的命令前，仍必须当次申请提权；不能把本轮文档批准当成执行授权。

**Evidence**

- 2.2/2.3 固定 upstream commit、lock SHA、feature 与平台差异；2.4 记录带 query 与无 query
  Zed source 会产生两套 GPUI 类型的复现结果。
- 当前 root manifest、`Cargo.lock` 与 target gpui-component manifest/lock 的具体差异见 2.1。

**Files**

- 修改 `Cargo.toml`：workspace `gpui`/`gpui_platform` 与 `[patch.crates-io]`
  `gpui`/`gpui-macros` 全部删除 `rev`，保留完全相同的 Zed git URL；features 不变。
- 修改 `Cargo.lock`：longbridge repo 全部 package 锁到 `5b45bcb26b9343d91a123a4d5ed8a654360512e5`；
  Zed repo 全部 package 锁到 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。

**API contract**

- 不新增或修改 Rust public API；本工作包只固定 Cargo source identity 与 lockfile contract。
- 根 manifest 的 `gpui`、`gpui_platform`、`gpui-macros` 与 crates.io patch replacement 必须使用
  完全相同、无 query 的 `https://github.com/zed-industries/zed` git source；已有 features 保持不变。
- `Cargo.lock` 必须把 longbridge repo 固定到 `5b45bcb26b9343d91a123a4d5ed8a654360512e5`，
  把所有 Zed packages 固定到 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。

**Implementation flow**

1. 先删除 root manifest 中全部 Zed dependency/patch replacement 的 `rev`，统一 source identity；
   禁止只改 lockfile。
2. 在当次提权批准后依次执行
   `cargo update -p gpui-component --precise 5b45bcb26b9343d91a123a4d5ed8a654360512e5` 与
   `cargo update -p gpui --precise 1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`；禁止人工编辑 lockfile。
3. 扫描 `Cargo.lock` 中所有同源 longbridge 与 Zed packages，确认整组 package 分别固定到目标 commit，
   不只检查命令中点名的两个 package。
4. 用 `cargo tree -p gpui-form-gpui-component -d --locked` 检查 duplicate GPUI。
5. 用 lock source 搜索确认所有 Zed package 只有
   `git+https://github.com/zed-industries/zed#1a246...`。
6. 运行 dependency focused check；随后按 core、macro 计划完成其 breaking API，再回到本计划的
   CORE-GATE/CONTROL 工作包。

**Errors and lifecycle**

- Source identity 或 lock SHA 不符合预期是构建期阻断，不进入运行时，也不增加 conversion、fallback
  或兼容层。
- Manifest 与 lockfile 必须作为一个原子 changeset 更新；任一验证失败都保留失败证据并停止后续工作包。

**UI/data/database/icons/i18n/dependencies**

- Features：`gpui_platform` 原四个 feature 保持不变；不启用新的 tree-sitter feature。
- MSRV：仓库仍使用 Rust 1.92+；target 未声明更高 `rust-version`。
- 平台 bootstrap：无变化；最终必须通过 macOS/Linux/Windows CI。
- UI：本包不主动改变产品布局或交互，但 dependency bump 必须执行 7.3 的 UI smoke。
- Form data、数据库/schema、网络、icons/assets 与应用 i18n：均无变化。

**Tests**

- 本包没有独立业务单元测试；duplicate-source regression 由 dependency tree、定向 compile 与后续
  workspace/UI smoke 共同覆盖。

**Validation**

```bash
cargo tree -p gpui-form-gpui-component -d --locked
cargo check -p gpui-form-gpui-component --all-targets --all-features --locked
```

**Done condition**

- 完成：dependency tree 中只有一份 GPUI source，定向 check 进入本 crate 源码错误或通过。
- 停止：出现第二份 GPUI、target commit 缺少 `set_selected_values`，或 Zed lock 不是
  `1a246...`；不得用 adapter conversion、unsafe 或 type alias 绕过。

### CORE-GATE：消费 core 的 FormControl 与 deferred attachment contract

**Prerequisites**

- DEP-00 完成。
- `crates/gpui-form/dev/typed-form-store.md` 的 FORM-10 至 FORM-60 已完成。
- `crates/gpui-form-macros/dev/form-store-derive.md` 的 MACRO-10 至 MACRO-40 已完成，adapter
  tests 可以使用最终 generated fields。

**Evidence**

- 4.2 固定 adapter 消费的唯一 public contract；core 计划 3.5 与 FORM-30 是 attachment 实现和
  lifecycle 测试的唯一来源，FORM-60 提供 core source/API crate-test gate。
- 旧 prototype 的 immediate attachment/source API 见 2.1，只用于证明删除范围，不得保留兼容导出。

**Files**

- 本 gate 不修改 `crates/gpui-form`；core 文件与测试的唯一 owner 是 core 计划 FORM-30 与
  FORM-60。

**API contract**

- 编译确认 `FormField::attach_control(&self, cx: &mut App)`、`ControlAttachment: Clone`、
  4.2 的 `FormControl<T>` 与
  `ControlAttachment::{defer_set_user_value,defer_blur,defer_set_issue,defer_clear_issue}` 已存在，
  四个 intent 都返回 `()`。

**Implementation flow**

- 先编译 `gpui-form` 的最终 generated-field、attachment 与 `FormControl<T>` contract，再编译本 crate
  的旧 prototype，收集且只保留由 adapter 尚未迁移造成的编译错误。
- 确认 weak attachment、source/control ID、immediate mutation 与错误恢复仅为 core 私有实现，
  adapter 无需 read-back、Config 或 focus API。
- 对照 4.2 逐项检查唯一 attachment 创建入口、shared clone lease、四个 deferred intent 的返回类型和
  visibility；任一缺失或重复 public surface 都停止后续 CONTROL 工作包。

**Errors and lifecycle**

- Internal weak upgrade 失败是正常 drop/cancel，不记录 issue。
- Form/path 错误由 intent 内部转换为 liveness/`ValueUnavailable` 处理并 notify 结构 owner；adapter
  不接收 `Result`，也不自行恢复。
- Attachment clone 必须共享一个 private lease/liveness；任一 clone drop 不影响其余 clone，最后一个
  clone drop 才使 issue inactive。Typed write 不得顺带清除 adapter-wide 或 active control issue。

**UI/data/database/icons/i18n/dependencies**

- UI 与业务数据：本 gate 不创建 control 或改变 field；只验证最终 core/macro contract 可被消费。
- 数据库/schema、网络、icons/assets、应用 i18n 与平台代码：均无变化。
- 依赖：只消费 DEP-00 已固定的 lock；本 gate 不再修改 manifest 或 lockfile。

**Tests**

以下测试由 core 计划拥有，本 gate 继承其完成证据：

| 要求 | 测试文件 | 测试名 | 断言 |
| --- | --- | --- | --- |
| deferred lifetime | `crates/gpui-form/tests/derive.rs` | `queued_control_intent_expires_with_last_attachment` | drop owning subscription 后 intent 无写入，issue inactive |
| shared clone lease | 同上 | `control_attachment_clones_share_one_lease_until_last_drop` | drop 单个 clone 后 issue仍 active；drop 最后一个后 inactive |
| public creation | 同上 | `form_field_attach_control_returns_shared_attachment` | 唯一创建入口返回 typed attachment；stale path 返回 typed error |
| typed write ordering | `crates/gpui-form/tests/derive.rs` | `control_write_projects_before_change_validation` | API 返回 unit；revision 先推进；validator/report 看到新值；event/notify 最后发生 |
| issue invalidation | 同上 | `typed_write_preserves_adapter_and_control_issue_buckets` | 只清相交 required/structural/generated sync + async；adapter/control保留 |
| equal no-op | `crates/gpui-form/tests/derive.rs` | `equal_control_write_is_a_complete_no_op` | revision、issues、async task、validation、event、notify 均不变 |

**Validation**

- `cargo test -p gpui-form --all-features --locked` 通过。
- `cargo check -p gpui-form-gpui-component --all-targets --all-features --locked`。

**Done condition**

- Core 的继承测试通过，`attach_control`、Clone lease 与四个 deferred intent 签名/语义和 4.2
  完全一致。
- Adapter check 的失败只允许来自尚未迁移的 adapter 源码，不允许缺少、重复或冲突的
  core/macro API；否则停止 CONTROL-10。

### CONTROL-10：实现最小 FormInput 与共享同步模式

**Prerequisites**

- CORE-GATE。

**Evidence**

- 2.2 已核实 `InputEvent::{Change,Focus,Blur,PressEnter}` 与 silent
  `InputState::set_value`；D-01 至 D-04 固定 wrapper、回投影、defer 与事件契约。
- 2.1 的 Input prototype 显示现有 attachment/focus/config 副本必须整体移除。

**Files**

- 新增 `src/error.rs`。
- 整体重写 `src/input.rs`。
- 开始重写 `tests/adapters.rs` 的共享 fixture。

**API contract**

- 实现 4.3 的 `FormInput`、`Deref<Target=Entity<InputState>>`、
  `FormControl<String, State=InputState, Error=FormControlError>`。

**Implementation flow**

- Constructor 调 `field.attach_control(cx)`，build 原生 state，silent 初始化，然后创建 form
  subscription 与 input subscription；attachment 只由 component subscription closure 捕获。
- Form subscription 对所有 `FieldChanged`/`ModelReplaced` defer silent `set_value`，不按 path 或
  equality 过滤；只忽略 `RuntimeChanged`。
- Input Change 调 `defer_set_user_value`；Blur 调 `defer_blur`；Focus/PressEnter 无操作。

**Errors and lifecycle**

- Constructor 初值读取失败返回 `FormControlError::Field`，不安装半套 subscription。
- Component→form queued work 的 weak lifetime 与 runtime `ValueUnavailable` 由 attachment intent
  内部处理；form→component 严格只捕获 field + weak entity。Drop 后两条路径均无副作用。
- 不 fallback、不把 path disappearance 吞成默认值。

**UI/data/database/icons/i18n/dependencies**

- UI：继续直接渲染 upstream `Input`，placeholder、masked/multiline、disabled、focus 与 style
  仍由 native state/element 配置；adapter 不新增 presentation API。
- 数据：form `String` 是唯一业务值；IME、selection 与 raw interaction state 只在 `InputState`。
- 数据库/schema、网络、icons/assets 与应用 i18n：均无变化。
- 依赖/平台：只消费 DEP-00 与 CORE-GATE，不新增 crate、feature 或平台分支。

**Tests**

| 要求 | 测试文件 | 测试名 | 场景 | 断言 |
| --- | --- | --- | --- | --- |
| one-call bind | `tests/adapters.rs` | `form_input_constructs_from_typed_field` | String form | native initial equals form |
| change ordering | 同上 | `form_input_change_stores_value_before_change_validation` | validator reads field | report sees new String |
| blur | 同上 | `form_input_blur_runs_blur_validation_without_focus_mirror` | blur-only rule | issue appears, wrapper has no focus fields |
| always reproject | 同上 | `form_input_origin_and_peer_receive_silent_projection` | two inputs one field | form write once; both values equal; no second event |
| drop/defer | 同上 | `dropping_form_input_cancels_queued_component_write` | queued change then drop | form unchanged; no issue remains |
| programmatic | 同上 | `programmatic_field_write_reprojects_every_input` | two inputs | both silently update |
| whole-model event | 同上 | `form_input_reprojects_equal_model_replacement` | equal-value `ModelReplaced` | native setter仍执行一次 |
| projection dependency | 同上 | `form_input_reprojects_after_other_path_field_change` | projected field depends on sibling | sibling `FieldChanged` 后读取新 projection |
| runtime event | 同上 | `form_input_ignores_runtime_changed` | validation/context runtime update | native setter不执行 |

**Validation**

```bash
cargo test -p gpui-form-gpui-component --test adapters --locked form_input
cargo clippy -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
```

**Done condition**

- `FormInput` 的实例字段只有 subscriptions 与 `Entity<InputState>`，且 `Deref`/constructor
  签名与 4.2/4.3 一致。
- 初值、Change、Blur、origin+peer 回投影、programmatic write 与 drop/defer 测试全部通过。
- Equal `ModelReplaced` 与其他 path `FieldChanged` 会投影，`RuntimeChanged` 不投影。
- Input event callback 内没有 emitter update、同步 form update、focus/error visibility 副本或 fallback。

### CONTROL-20：直接绑定 Select/Combobox Value

**Prerequisites**

- CONTROL-10。

**Evidence**

- 2.2 已核实 Select 只需消费 `Confirm(Option<Value>)`，Combobox 必须只消费
  `Change(Vec<Value>)`，两种 native value setter 都静默且读取当前 delegate。
- PR #2576 / `5b45bcb...` 提供 `set_selected_values`；5 章明确 captured delegate/index
  workaround 的删除边界与回归测试。

**Files**

- 整体重写 `src/select.rs`：`FormSelect<D>` 只绑定 `Option<Value>`，只消费 Confirm。
- 整体重写 `src/combobox.rs`：`FormCombobox<D>` 只绑定 `Vec<Value>`，只消费 Change。

**API contract**

- 两者 wrapper 字段仅 subscriptions + native entity，Deref 到 native entity。
- 投影分别调用 `set_selected_value`/`set_selected_index(None)` 与 `set_selected_values`。
- `FormSelect<D>` 实现
  `FormControl<Option<<D::Item as SearchableListItem>::Value>, State=SelectState<D>, Error=FormControlError>`；
  `FormCombobox<D>` 实现
  `FormControl<Vec<<D::Item as SearchableListItem>::Value>, State=ComboboxState<D>, Error=FormControlError>`。
- Select 的唯一 component→form event 是 `Confirm(Option<Value>)`；Combobox 的唯一
  component→form event 是 `Change(Vec<Value>)`，Confirm 不属于 binding contract。

**Implementation flow**

- 先按当前 typed field value silent 初始化 native selection，再安装 component event subscription 与
  form value-event subscription；任一初始化错误在安装 subscription 前返回。
- 两者沿用 CONTROL-10 的 value-event contract：所有 `FieldChanged`/`ModelReplaced` 都重新读取
  field 并静默投影，只忽略 `RuntimeChanged`。
- Component event 只复制 owned value 并调用 attachment deferred intent；form event 重新读取 field，
  再 defer 当前 native value setter，setter 不发 user event。

**选项与验证边界**

- 不提供 adapter `set_delegate`/`set_items` wrapper；调用者用 native API。调用 `set_items` 后必须
  立即读取当前 form value 并调用 native value setter，或立即重建 wrapper，不能等待 value event。
- 缺失 value 不写 form、不 fallback、不自动 issue；应用显式 dynamic validation。
- Select/combobox 不注册 blur subscription。

**Errors and lifecycle**

- Constructor 初值读取失败返回 `FormControlError::Field`，不留下半套 subscription。
- Component write 只走 core deferred intent；form projection 只捕获 typed field 与 weak native
  entity。Wrapper drop 或 path disappearance 的处理与 CONTROL-10 相同。
- 当前 delegate 无法解析 value 只影响 native projection；不得清空、替换或规范化 form value，
  也不得制造 adapter-owned availability issue。

**UI/data/database/icons/i18n/dependencies**

- UI：search/query/popup/highlight 与 item rendering 继续由 native Select/Combobox state 管理；
  catalog 更新由应用调用 native API 或重建 wrapper。
- 数据：form 分别只保存 `Option<Value>` 与 `Vec<Value>`；adapter 不复制 options/delegate。
- 数据库/schema、网络、icons/assets 与应用 i18n：均无变化；dynamic validation 文案仍由应用持有。
- 依赖/平台：必须已锁定 gpui-component `5b45bcb...`；不新增其他依赖、feature 或平台分支。

**Tests**

| 要求 | 测试文件 | 测试名 | 场景 | 断言 |
| --- | --- | --- | --- | --- |
| select event | `tests/adapters.rs` | `select_confirm_writes_option_value_once` | counting form event | Some/None各一次 typed write |
| select current delegate | 同上 | `select_options_refresh_immediately_reprojects_current_value` | reordered/removed items，无 form event | setter立即按新 delegate；form value不变 |
| combobox event | 同上 | `combobox_change_writes_values_and_confirm_is_ignored` | emit Change then Confirm | form只因 Change 写一次 |
| combobox current delegate | 同上 | `combobox_options_refresh_immediately_reprojects_current_values` | reorder/add/remove，无 form event | native selection立即按 value/顺序解析；missing omitted |
| origin projection | 同上 | `select_and_combobox_reproject_origin_silently` | event counters | origin state得到 silent setter；无递归 event |
| reentrancy | 同上 | `selection_bindings_never_update_emitter_inside_event_scope` | real GPUI events | 无 already-being-updated panic |

**Validation**

```bash
cargo test -p gpui-form-gpui-component --test adapters --locked select
cargo test -p gpui-form-gpui-component --test adapters --locked combobox
cargo clippy -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
```

**Done condition**

- `FormSelect`/`FormCombobox` 字段仅为 subscriptions + native entity，且无 delegate、items、
  resolve/value-index 或 availability policy 副本。
- Select Confirm 与 Combobox Change 各只产生一次 typed write；Combobox Confirm 不写 form。
- Reorder/add/remove 后投影使用当前 delegate，origin+peer 均静默同步，全部 reentrancy 测试通过。
- Options refresh 在同一次调用路径立即按当前 form value 重投影或重建，不依赖后续
  `FieldChanged`/`ModelReplaced`。

### CONTROL-30：实现 exact IntegerInput native state

**Prerequisites**

- CONTROL-10。

**Evidence**

- 2.2 已核实 upstream `NumberInput` 的 range/step 使用 `f64`，无法精确覆盖完整 Rust integer
  primitive；5 章因此只保留 exact typed state，同时复用 upstream presentation/action。
- D-06、4.4 与 4.5 已固定 constructor error、edit event、issue key、checked arithmetic 和公开类型。

**Files**

- 整体重写 `integer_input.rs` 与 `integer_input/{error,parse,policy}.rs`。

**API contract**

- 实现 4.4/4.5 的 errors、sealed `IntegerValue`、typed policy、state、event、element 与
  `FormIntegerInput<N>`。
- 支持 `i8/i16/i32/i64/i128/isize/u8/u16/u32/u64/u128/usize`。
- `FormIntegerInput<N>` 实现
  `FormControl<N, State=IntegerInputState<N>, Error=FormControlError>` 并
  `Deref<Target=Entity<IntegerInputState<N>>>`；wrapper 字段仍只有 subscriptions 与 native entity。
- `IntegerInputState<N>`、`IntegerInputEvent<N>`、`IntegerInputPolicy<N>`、
  `IntegerInputPolicyError`、`IntegerInputError<N>` 与 `IntegerInput<N>` 的公开方法、trait 实现和
  visibility 必须与 4.4/4.5 一致。

**Implementation flow**

1. Native state 内创建 `InputState`，关闭其 `f64` step，订阅 Change/Blur/Step。
2. ASCII optional sign + digits 做 shape check，再 `FromStr` 区分 overflow，再 typed range check。
3. Change 发 `IntegerInputEvent::Change(Result<N, IntegerInputError<N>>)`；不在 native state
   访问 form。
4. Component subscription 捕获 `attach_control` 返回的 attachment clone；对 Ok 依次调用
   `defer_clear_issue` 与 `defer_set_user_value`，对 Err 调
   `defer_set_issue` 且不写 form。
5. Form projection closure 捕获 field + weak integer state，并额外捕获同一个 attachment clone；
   对所有 `FieldChanged`/`ModelReplaced` 重新读取 field，silent `set_value` 成功后才调用
   `defer_clear_issue`。它忽略 `RuntimeChanged`，且不基于 application-written value 重新制造
   editor issue。
6. Step 用 checked typed arithmetic；overflow/range failure no-op。

**Issue 映射**

| 编辑错误 | code | message key | 参数 |
| --- | --- | --- | --- |
| `Incomplete` | `integer_input_incomplete` | `gpui-form-error-integer-incomplete` | 无 |
| `InvalidSyntax` | `integer_input_invalid` | `gpui-form-error-integer-invalid` | 无 |
| `Overflow` | `integer_input_overflow` | `gpui-form-error-integer-overflow` | 无 |
| 只有下界 | `integer_input_out_of_range` | `gpui-form-error-integer-min` | `min` |
| 只有上界 | 同上 | `gpui-form-error-integer-max` | `max` |
| 同时有上下界 | 同上 | `gpui-form-error-integer-range` | `min`,`max` |

参数统一用 `Display::to_string()`；adapter 不生成最终本地化句子。

**Errors and lifecycle**

- `NonPositiveStep`/`ReversedRange` 是 constructor error，必须在安装 binding 前返回；不得构造半成品。
- Incomplete/syntax/overflow/range 是 mounted control issue：invalid edit 保留 raw text 与最后一个
  form `N`，恢复合法时 clear issue 后写值，wrapper drop 后 issue 自动失效。
- Programmatic form write 静默覆盖 stale editor；只有 field read、weak entity upgrade 与 silent
  projection 都成功后才清除旧 editor issue。Application-written domain violation 只由 form
  business validation 报告。Step overflow/range failure 是 no-op，不 clamp。
- Field read、weak entity upgrade 或 silent projection 未成功时不得 clear issue；两个 subscription
  捕获的 attachment clone 共享一个 lease，wrapper drop 释放最后的 clones 后 issue inactive。

**UI/data/database/icons/i18n/dependencies**

- UI：`IntegerInput` 复用 upstream `NumberInput` 的外观、size、disabled、prefix/suffix 与 focus；
  不复用其 `f64` 业务数值。
- 数据：form 只保存精确 `N`；raw text、parse 状态与 typed policy 只在 native state。
- 数据库/schema、网络与 icons/assets：均无变化。
- i18n：adapter 只产生表中稳定 key/字符串参数，不新增 locale bundle；应用负责最终翻译。
- 依赖/平台：不新增 numeric/parser crate，不新增 feature 或平台分支；只消费 DEP-00 后的 Input/NumberInput API。

**Tests**

| 要求 | 测试文件 | 测试名 | 场景 | 断言 |
| --- | --- | --- | --- | --- |
| primitive coverage | `src/integer_input.rs` unit tests | `every_standard_integer_primitive_parses_exact_boundaries` | min/max literals | exact round trip |
| >2^53 | 同上 | `u64_above_two_pow_53_never_uses_f64` | `9_007_199_254_740_993` | exact N/text |
| policy errors | 同上 | `invalid_integer_policy_fails_before_binding` | step 0, reversed range | exact typed variant，无 subscription |
| edit classification | 同上 | `integer_edit_distinguishes_incomplete_syntax_overflow_and_range` | empty/sign/letters/MAX+1/range | exact error variant |
| invalid lifetime | `tests/adapters.rs` | `invalid_integer_text_preserves_form_and_blocks_submit_until_drop` | mounted integer | form旧值；issue active；drop 后 inactive |
| valid projection | 同上 | `valid_integer_edit_round_trips_canonical_text_to_origin_and_peer` | two controls | form typed N；两端 canonical；一次 user write |
| programmatic overwrite | 同上 | `programmatic_integer_write_replaces_stale_invalid_editor` | invalid raw then field set | raw被覆盖；旧 editor issue清除；业务范围由 form validator决定 |
| projection failure | 同上 | `failed_integer_programmatic_projection_preserves_editor_issue` | stale path或dropped entity | 未成功投影时不调用 clear issue |
| shared lease | 同上 | `integer_subscriptions_share_one_attachment_lease` | drop one subscription then wrapper | 单 clone drop不失效；wrapper drop 后 issue inactive |
| step | 同上 | `integer_step_uses_checked_typed_arithmetic` | boundaries/signed/unsigned | no clamp/no overflow/f64 |
| blur | 同上 | `integer_blur_runs_form_blur_validation_and_keeps_invalid_text` | invalid raw + blur rule | raw保留；blur issue更新 |

**Validation**

```bash
cargo test -p gpui-form-gpui-component --locked integer
cargo clippy -p gpui-form-gpui-component --all-targets --all-features --locked -- -D warnings
```

**Done condition**

- 所有标准 signed/unsigned integer primitive 的边界与 `u64 > 2^53` 均 exact round trip，代码路径
  不把 typed value、range、step 或 arithmetic 转为 `f64`。
- Constructor policy、四类 edit error、issue lifetime、programmatic overwrite、checked step、Blur 与
  origin+peer canonical projection 测试全部通过。
- Integer 只有 form projection closure 额外捕获 attachment，且只在成功 silent projection 后
  `defer_clear_issue`；其他 adapter 的 projection closure 均为 field + weak entity。
- `FormIntegerInput<N>` 仍只持有 subscriptions + `Entity<IntegerInputState<N>>`，native state 不访问 form。

### CONTROL-40：删除 legacy surface 并收紧 exports

**Prerequisites**

- CONTROL-20、CONTROL-30 与 `JACO-FORM-20..60` 已完成；列出的 adapter 调用点均已迁移到
  新 API。Jaco 的最终 residual gate 不属于本前置。

**Evidence**

- 2.1 列出旧 bool/status/config/form-generic wrapper；5 章已经逐项完成 upstream 复用、保留或删除决定。
- `app/jaco/docs/dev/gpui-form-migration.md` 固定所有应用调用点的 owner 与替换顺序；本包不重新设计 Jaco UI。

**Files**

- 删除 `src/bool.rs`、`src/status.rs`；保持 `binding.rs`、`number.rs` 删除。
- 重写 `src/lib.rs`。

**API contract**

- `src/lib.rs` 只导出：`FormControlError`、`FormInput`、`FormSelect`、
  `FormCombobox`、`FormIntegerInput`、`IntegerInput`、`IntegerInputState`、
  `IntegerInputEvent`、`IntegerInputError`、`IntegerInputPolicy`、
  `IntegerInputPolicyError`、`IntegerValue`。
- 删除所有 `Form*Config`、`Form*State<Form,...>`、`FormControlStatus`、
  `input_state`/`select_state`/matching-element helper 与 bool wrapper test。

**Implementation flow**

- 先确认 Jaco active source 已迁移，再删除模块和 export，最后用 residual gate 阻止兼容 alias
  或旧 free binding API 回流。

**受影响 Jaco 调用点**

- `app/jaco/src/components/run_settings.rs`
- `app/jaco/src/features/settings/mcp/form_state.rs`
- `app/jaco/src/features/settings/prompts/dialog.rs`
- `app/jaco/src/features/settings/provider.rs`
- `app/jaco/src/features/settings/shortcuts/dialog.rs`

这些文件的具体字段所有权、render 与验证迁移由 Jaco 实施计划固定；本 work package 只确认
这些 adapter 调用点已经不依赖旧 export。应用其余 legacy 清理由后续 `JACO-FORM-70` 负责。

**Errors and lifecycle**

- 这是有意的 compile-time breaking change；旧 import/constructor 失败时必须迁移调用点，不得新增
  deprecated alias、双实现或运行时 fallback。
- 删除 legacy wrapper 必须同时释放其 subscription/status/control-issue 生命周期；最终 owning
  wrapper 的 drop 语义仍由 subscriptions + core attachment lifecycle 保证。

**UI/data/database/icons/i18n/dependencies**

- UI：只删除 adapter-owned helper/status/bool wrapper；Jaco 的视觉布局和原生 component rendering
  由调用点迁移保持。
- 数据：不迁移或转换业务值；generated form field 仍是唯一来源。
- 数据库/schema、网络、icons/assets、应用 i18n、平台与依赖：均无变化，继续使用 DEP-00 lock。

**Tests**

- 运行完整 `tests/adapters.rs`，确认只通过新 exported surface 构造所有 stateful controls。
- Residual scan 覆盖 adapter crate Rust source；旧符号只能出现在迁移文档的历史说明中。

**Validation**

```bash
rg -n 'Form(Input|Select|Combobox|IntegerInput|Bool)(State|Config)|FormControlStatus|bind_(input|number|select|combobox|bool)' \
  crates/gpui-form-gpui-component --glob '*.rs'
cargo test -p gpui-form-gpui-component --all-features --locked
cargo check -p jaco --all-targets --all-features --locked
```

**Done condition**

- Adapter residual command 无 active source 命中；文档中只允许历史迁移说明明确引用旧名。
- `lib.rs` 只导出 4.3/4.4/4.5 列出的目标类型，所有 adapter 与 Jaco compile/test 通过。
- 没有 compatibility wrapper、alias、旧 matching-element helper 或第二套 bool/status API。

### CONTROL-50：同步公共文档与 compile examples

**Prerequisites**

- CONTROL-40。

**Evidence**

- README/Guide 当前明确标记为 target design preview；4 章与 CONTROL-10/20/30/40 是最终签名和行为的
  唯一校对来源。
- 用户已确认 English 为默认入口、中文为语义镜像，README 只做项目介绍/最短示例，完整行为放 Guide。

**Files**

- 核对本轮已写的 `README.md`、`README.zh-CN.md`、`docs/guide.md`、
  `docs/guide.zh-CN.md` 与最终代码签名。

**API contract**

- 四份文档中的 imports、constructor、event、error、attachment lifecycle 与 exported names 必须与
  CONTROL-10/20/30/40 的最终 public API 完全一致；English 是默认入口，中文是等价镜像。
- README 只承载项目介绍与最短可编译示例；Guide 承载 attachment、value-event projection、options
  refresh、integer issue 与 custom adapter 的完整 contract。

**Implementation flow**

- 如果实现签名与已确认设计冲突，停止并更新设计/计划，不允许只修改示例掩盖差异。
- 实现与全套测试完成后删除四份公共文档的 design-preview/source-not-started 提示，并只按已实现
  signature 更新 import、constructor、event、error 与 lifecycle 描述。

**Errors and lifecycle**

- 示例必须区分结构上保证存活时的 `expect` 与 projected/dynamic path 的正常 `Result` 处理。
- Guide 必须写明 `FormField::attach_control(&self, cx: &mut App)` 的唯一 public 创建入口、
  `ControlAttachment: Clone` 的 shared-lease/final-drop 语义与四个唯一 public mutation intent。
- Guide 必须说明 deferred intent、drop cancellation、`ValueUnavailable`、integer control issue、
  successful-programmatic-projection 才 clear，以及 typed write 的精确 issue-bucket invalidation；不得公开
  core-private weak/source/control ID。

**UI/data/database/icons/i18n/dependencies**

- UI/data：文档只描述 CONTROL-* 已实现行为，不引入新 UI、状态 owner 或 API。
- 数据库/schema、网络、icons/assets、locale bundle 与平台代码：均无变化。
- i18n：英文默认、中文镜像；integer message key 的最终翻译仍由应用负责。
- 依赖：示例以 DEP-00 的 locked API 为准，不新增 dev dependency；无法直接 doctest 的 GPUI fixture
  放入现有 integration test。

**Tests**

- 把 README/Guide 的核心构造、Select、Combobox、IntegerInput 与 custom FormControl 片段
  覆盖为 compile test 或等价 integration test。
- Compile/integration coverage 必须断言 custom adapter 通过 `field.attach_control(cx)` 创建 attachment，
  component subscription 捕获 clone，普通 projection 只捕获 field + weak entity；只有拥有
  lifecycle-scoped control draft issue 的 typed editor projection 才可额外捕获同一 attachment clone，
  并仅在成功 silent projection 后 clear，当前内置 adapter 中只有 exact integer 使用该例外。
- 行为文档测试必须覆盖：全部 `FieldChanged`/`ModelReplaced` 都回投影且只忽略 `RuntimeChanged`；
  options refresh 在 native `set_items` 后立即按当前 form value 调 value setter；typed write 只失效
  相交 required/structural/generated sync bucket 与 async validation，并保留 adapter-wide/control issue。
- English 是默认文档；中文文件必须保持相同章节、API、行为与链接。

**Validation**

```bash
cargo test -p gpui-form-gpui-component --doc --locked
cargo test -p gpui-form-gpui-component --test adapters --locked documentation_examples_compile
rg -n '^## ' crates/gpui-form-gpui-component/{README.md,README.zh-CN.md,docs/guide.md,docs/guide.zh-CN.md}
rg -c '^```' crates/gpui-form-gpui-component/{README.md,README.zh-CN.md,docs/guide.md,docs/guide.zh-CN.md}
test -f crates/gpui-form-gpui-component/docs/guide.md
test -f crates/gpui-form-gpui-component/docs/guide.zh-CN.md
test -f crates/gpui-form-gpui-component/dev/typed-bound-controls.md
test -f crates/gpui-form/docs/guide.md
test -f crates/gpui-form/docs/guide.zh-CN.md
test -f crates/gpui-form-macros/docs/guide.md
test -f crates/gpui-form-macros/docs/guide.zh-CN.md
git diff --check -- crates/gpui-form-gpui-component
```

逐项检查四份文档中的相对链接目标存在；不能只比较链接文字。

**Done condition**

- Public docs 不再声称实现尚未开始，所有示例与最终 exports 编译一致。
- README EN/ZH 章节和最短示例语义一致；Guide EN/ZH 章节、API、事件、错误、ownership、链接与
  code fence 一一对应，English 仍为默认入口。
- Public docs 与 compile coverage 明确固定 attachment 创建/Clone lease、全 value-event 投影、
  control-draft typed editor capture 例外（当前内置仅 exact integer）、options 立即重投影和精确
  issue-bucket invalidation，不允许退回旧语义。
- 文档 compile coverage、链接检查、围栏检查与 `git diff --check` 全部通过。
- 完成后依次交给 `JACO-FORM-70` 执行全应用 legacy residual，再由 core `FORM-70` 执行最终
  workspace/public-status release gate；`CONTROL-50` 不反向等待这两个工作包。

## 7. 跨包验证

### 7.1 依赖与 source identity

```bash
cargo tree -p gpui-form-gpui-component -d --locked
rg -n 'git\+https://github.com/zed-industries/zed' Cargo.lock
rg -n 'git\+https://github.com/longbridge/gpui-component' Cargo.lock
```

期望：只有一个 Zed source SHA `1a246...`，只有一个 longbridge source SHA `5b45...`；
不得出现同 repo 同 SHA 但 `?rev=` 不同的第二个 source。

### 7.2 Focused validation

```bash
cargo fmt --all --check
cargo test -p gpui-form --locked
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form-gpui-component --locked
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component \
  --all-targets --all-features --locked -- -D warnings
git diff --check
```

### 7.3 Workspace validation

Jaco migration完成后执行仓库默认基线：

```bash
cargo build --locked
cargo test --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

CI 必须在 macOS、Linux、Windows 三个平台通过。Dependency bump 后补 UI smoke：

1. Input 连续输入、blur 与两个实例同步；
2. Select/combobox item reorder/add/remove 后使用当前 delegate；
3. Integer incomplete、invalid、上下键、边界与 `u64 > 2^53`；
4. 页面 drop 或重建后，已排队事件不再写 form；
5. 全流程无 `already being updated`、`RefCell already borrowed` 或意外失焦。

本轮只改文档，因此本轮完成时只要求链接/术语检查与 `git diff --check`；上述 cargo/UI
命令属于后续实现完成证据，不能写成已经执行。

## 8. 执行交接审计

- 所有重大产品/API/依赖选择已经由用户确认，没有把候选方案留给实现者。
- 每个 exported wrapper 的字段、泛型、Deref target、constructor 与 event contract 已固定。
- Form/component/options/integer editor/subscription/persistence owner 已唯一化。
- Component event、form projection、drop、path disappearance 与 reentrancy 顺序已固定。
- Select/combobox 直接复用已核实的 upstream value setter；本地 delegate workaround 明确删除。
- Integer 保留的 custom responsibility 仅为 upstream `f64` API 无法满足的 exact typed gap。
- Dependency gate 精确到 manifest source、lock SHA、验证命令与停止条件。
- Database、network、icons/assets、routes、i18n ownership 与 platform surface 均有明确 no-change/
  responsibility decision。
- 每项行为都有具体测试文件、测试名、fixture 与 assertion；没有“实现时再研究”的 broad task。
- 源码与 focused/workspace 自动化 evidence、定向 Computer Use UI evidence 已完成；跨平台 CI 与
  临时窗口全局快捷键/有数据列表流程尚未在本地环境执行，不能据此宣称所有平台发布门禁完成。
