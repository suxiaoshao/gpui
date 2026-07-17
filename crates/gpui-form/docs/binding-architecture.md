# gpui-form component adapter architecture

状态：已实施。核心、caller-owned adapter 与 workspace 调用方均已迁移；公共使用方式见 [`../README.md`](../README.md)，完整实施记录见
[`external-state-synchronization-plan.md`](external-state-synchronization-plan.md)。

## 1. 结论

`gpui-form` 不拥有 component entity，也不负责创建或配置 component state。稳定边界是 typed
`FormFieldHandle`、component-specific bind function 和调用方持有的 `SubscriptionSet`。

```text
                 user draft
component event ──────────────> FormFieldHandle -> DraftFieldStore
      ^                                               |
      |              programmatic draft               |
      └──────── adapter subscriptions <───────────────┘

catalog/config command ───────────────> component-specific config API
focus/query/scroll/IME ───────────────> component entity only
subscription lifetime ────────────────> caller-owned SubscriptionSet
submit ───────────────────────────────> DraftFieldStore only
```

不引入 public binding handle、`ComponentBindingSet`、`FormComponentAdapter`
trait，也不在 `SubscriptionSet` 上增加组件库专用方法。这样应用自定义组件和其他组件库可以复用同一 core
协议，而不依赖 `gpui-form-gpui-component`。

## 2. 三个语义 owner

| 通道 | 内容 | 唯一 owner |
| --- | --- | --- |
| form draft | raw/typed draft、baseline、dirty、touched、parse/validation errors | generated form entity |
| component config | options、capability、disabled、placeholder、mask | app/controller/catalog projection |
| interaction | focus、open、query、highlight、scroll、IME、task | component entity |

上游 component state 可以在同一 Rust 类型中物理保存 value、options 和 interaction，但 component 中的
text/selection 只是 form draft 的 UI mirror；options 变化不是 form value 变化。

## 3. Core boundary

### `FieldCodec<Value>` 与 `DraftFieldStore`

codec 是纯 domain value/draft 转换；不得读取 component、catalog、repository 或 GPUI context。
`DraftFieldStore` 保存 baseline、draft、parse error 和 field meta。用户输入、程序化赋值、baseline 替换和
submit 都在 form 内完成，submit 不访问 UI。

### `FormFieldHandle<Form, Draft>`

derive 为每个 leaf 生成可复制 handle：

```rust
pub struct FormFieldHandle<Form, Draft> {
    form: WeakEntity<Form>,
    path: FieldPath,
    read: fn(&Form) -> Draft,
    write: fn(&mut Form, Draft, FieldChangeCause, &mut Context<Form>),
}
```

handle 暴露 `draft`、`set_user_draft`、带显式 cause 的 `set_draft` 和 typed `subscribe_in`。它不提供
component config setter，不公开整个 form，也不产生 catalog/store 依赖。

页面级 observer 使用 generated `<Form>Field` 和 runtime `FormStoreEvent<Field>`。dynamic required 使用纯
form `set_<field>_required(required, cx)`；视觉 required affordance 由 app 配置。

### `SubscriptionSet`

`gpui-form::SubscriptionSet` 是与组件库无关的通用生命周期容器：

```rust
pub struct SubscriptionSet {
    subscriptions: Vec<Subscription>,
}
```

它只提供 `new`、`push`、`extend`、`clear` 等集合语义，不知道 field、component、adapter 或同步方向。调用方
按实际 mount scope 保存一个或多个 set；clear/drop 即断开对应订阅。

## 4. Adapter boundary

调用方创建 component entity，component-specific adapter 安装双向订阅并返回一个 core
`SubscriptionSet`：

```rust
pub fn bind_select<Form, Value, Delegate, Owner>(
    field: FormFieldHandle<Form, Value>,
    state: &Entity<SelectState<Delegate>>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, ComponentBindError>;
```

调用方合并结果：

```rust
subscriptions.extend(bind_select(
    ProviderFormStore::kind_handle(&form),
    &kind_state,
    window,
    cx,
)?);
```

bind function 内部固定顺序：

1. 从 form draft 做 initial projection；form 已释放则立即失败；
2. 创建私有 `Rc<Cell<ComponentSyncState>>`；
3. 在局部 `SubscriptionSet` 中安装 component -> form 和 form -> component；
4. 两侧全部成功后返回；失败时局部 set drop，调用方已有订阅不变。

方向 guard 只由两个 closure 捕获，不成为 public handle：

```rust
enum ComponentSyncState {
    Idle,
    PublishingUserDraft,
    ApplyingFormDraft,
}
```

adapter 只负责 genuine user event -> form draft 和 programmatic form event -> component mirror。它不创建
state，不配置 options/disabled/required，不选择 catalog fallback，不参与 validation/submit。

## 5. Event and update rules

```text
component event callback
  -> read component draft once
  -> guard = PublishingUserDraft
  -> field.set_user_draft(...)
  -> form emits field events
  -> same adapter sees guard and skips mirror write
  -> guard = Idle
```

```text
form setter / replace_from_value
  -> form emits field event
  -> guard = ApplyingFormDraft
  -> state.update(...) writes mirror
  -> any programmatic component echo is ignored
  -> guard = Idle
```

GPUI 的同一 entity 不能在 active update scope 内再次 update。adapter 在进入另一侧 update 前完成当前侧读取，
并靠方向 guard 打断同步回路。禁止把无条件 `defer_in` 当统一解决方案。

## 6. Configuration flow

```text
catalog reload
  -> app computes component options/capabilities
  -> simple config uses component API
  -> select/combobox items use adapter-specific config command
  -> command reads form draft, replaces items, reprojects selected mirror
  -> no form event and no business fallback
```

配置命令接收 field handle 和 component state，而不是 binding handle：

```rust
set_select_items(
    ProviderFormStore::kind_handle(&form),
    &kind_state,
    ProviderDelegate::new(kinds),
    window,
    cx,
)?;
```

当前 pinned `SelectState`/`ComboboxState` 的 items 和 programmatic selection setters 不发布用户事件，因此该
命令无需访问 direction guard。draft 不在新 options 时，只清空 UI selection/展示 unavailable，不修改 form、不选
fallback。最终 submit resolver 决定拒绝或解析。

## 7. Lifetime and ownership

调用方 controller/view 保存：

- form entity；
- component entities；
- 一个或多个 `SubscriptionSet`；
- catalog/store subscriptions；
- product-specific derived presentation state（仅在无法即时派生时）。

替换 component entity 时，调用方 clear/drop 对应 mount scope 的 set。adapter 返回订阅但不再持有它们；core
`SubscriptionSet` 也不依赖任何组件库。

## 8. 自定义组件和其他组件库

自定义 adapter 放在应用或对应组件库的 adapter crate：

```rust
fn bind_custom_control<Form, Owner>(
    field: FormFieldHandle<Form, CustomDraft>,
    state: &Entity<CustomState>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Result<SubscriptionSet, CustomBindError>;
```

它遵守相同 initial projection、方向 guard、原子返回规则即可。无需实现 core trait，也无需依赖
`gpui-form-gpui-component`。

## 9. 删除的旧边界

- state-owning generic component binding；
- form leaf store 内的 component entity；
- component construction derive attributes；
- generated component-state accessors；
- submit-time component readback；
- public component binding handle 和 handle aliases；
- `ComponentBindingSet` / universal adapter trait；
- component-specific `SubscriptionSet::bind_*` extensions；
- form/catalog automatic rebase 或 conflict protocol。

不保留兼容层，workspace 调用方与 core/adapter 在同一阶段迁移。

## 10. Acceptance

- core public API 不出现 component state/config；`SubscriptionSet` 保持组件库中立；
- adapter bind functions 返回订阅，调用方决定 lifetime scope；
- adapter 可单独测试两个方向各同步一次；
- items/disabled 更新不产生 form field event；
- submit 在没有 mounted component 时结果相同；
- clear/drop caller set 后两侧不再同步；
- app-local custom adapter 不依赖 `gpui-form-gpui-component`；
- nested-update regression tests 覆盖 adapter input、model/reasoning/approval picker、project picker 和 temporary list；
  select/combobox adapter 通过同一 handle/guard contract 与 workspace 编译、测试路径验证。
