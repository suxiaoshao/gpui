# Number input design

状态：已实施。number 使用 form-owned `String` draft、纯 `NumberCodec<N>` 和 app-owned
`InputState`；完整 ownership 见 [`binding-architecture.md`](binding-architecture.md)。

## 1. Why raw draft belongs to the form

number 编辑存在无法立即表示成 Rust number 的合法中间态，例如 `"-"`、`"+"` 和 `"1."`。即使最终 typed
value 相同，`"012"` 与 baseline `"12"` 也可能代表尚未 normalize 的用户编辑。

因此：

- form 的 authoritative draft 是 `String`；
- codec parse 成功时得到 `N`；
- parse 失败时保留 raw draft 并保存 internal field error；
- dirty 比较 raw draft 与 raw baseline；
- submit 解析 form draft，不读取 `InputState`。

## 2. Public integration

form schema 使用 adapter crate 提供的纯 codec：

```rust
type PortCodec = gpui_form_gpui_component::NumberCodec<u16>;

#[derive(FormStore)]
#[form(store = "ServerFormStore")]
pub struct ServerInput {
    #[form(codec = "PortCodec", validate(on_blur, on_submit))]
    pub port: u16,
}
```

application 创建并配置 input state，再连接 field handle：

```rust
let port_state = cx.new(|cx| {
    InputState::new(window, cx)
        .mask(InputMask::Integer)
        .min(0.0)
        .max(u16::MAX as f64)
        .step(1.0)
});

let port_binding = bind_number(
    ServerFormStore::port_handle(&form),
    port_state.clone(),
    window,
    cx,
);
```

render 仍直接使用 upstream component：

```rust
NumberInput::new(&port_state)
```

## 3. Codec versus component policy

`NumberCodec<N>` only owns conversion:

```text
N -> N::to_string() -> String baseline/draft
String draft -> parse::<N>() -> N or FieldCodecError(code = "parse")
```

`NumberInputPolicy` is component configuration and is applied by the app when
creating `InputState`:

| 类型 | 建议 component policy |
| --- | --- |
| signed small integers | allow sign, integer mask, exact min/max, step 1 |
| unsigned small integers | no sign, integer mask, min 0, exact max, step 1 |
| `i64` / `u64` / `isize` / `usize` | integer mask; avoid lossy `f64` bounds/step for large values |
| `f32` / `f64` | allow sign/decimal; app chooses min/max/step |

adapter 可提供 `NumberInputPolicy::for_type::<N>()` 作为 convenience，但 policy 不进入 form field，不由
derive 自动应用，也不参与 dirty/validation/submit。

app-specific range、token budget、capability clamp 和 DB/config 约束属于 validator 或 final submit resolver，
不能只依赖 UI mask/min/max。

## 4. Data flow

用户输入：

```text
InputEvent::Change
  -> bind_number reads text once
  -> FormFieldHandle::set_user_draft(String)
  -> DraftFieldStore stores raw text
  -> codec parse + configured validation trigger
```

programmatic value：

```text
form replace/set/normalize
  -> NumberCodec::draft_from_value
  -> typed field event
  -> bind_number writes InputState mirror once
```

submit：

```text
DraftFieldStore::prepare_submit
  -> NumberCodec::parse(saved String)
  -> validation/transform/final report
```

## 5. Required tests

- signed intermediate `"-"` survives as dirty draft and yields parse error；
- `"012"` stays dirty against baseline `"12"` even when both parse to `12`；
- invalid draft is identical with or without a mounted component；
- form setter updates input mirror exactly once；
- input user event updates form exactly once；
- reset/replace restores raw baseline and clears parse error；
- large integer policy never performs lossy `f64` arithmetic；
- component min/max/items changes do not mutate form draft。

## 6. Other surfaces

- core dependency on `gpui-component`: none；
- DB/schema/config: no change；
- icon/assets: no change；
- i18n: parse error uses the existing form error resolver；
- lifecycle: adapter 返回 subscriptions，app 在 mounted scope 的 `SubscriptionSet` 中持有。
