# gpui-form-gpui-component

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form-gpui-component` 将 typed `gpui_form::Form<M>` session 接入
`gpui-component` 的 stateful control。它提供文本、整数、单选与多选 adapter；业务字段值始终只由
Form 持有。

```toml
[dependencies]
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

## 绑定字段

定义 schema，为一次编辑创建一个 form，然后在页面中同时持有 form 和 adapter：

```rust,ignore
use gpui::{Context, Entity, IntoElement, Render, Subscription, Window, div};
use gpui_component::{form::field, input::{Input, InputState}};
use gpui_form::{Form, FormSchema};
use gpui_form_gpui_component::FormInput;

#[derive(Clone, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required)]
    name: String,
}

struct ProviderEditor {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
}

impl ProviderEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| Form::new(ProviderDraft {
            name: String::new(),
        }));

        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );

        // 这里只重绘页面拥有的 label、error 与 button；FormInput 与 Form 的同步
        // 不依赖它。
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Self { form, form_observer, name_input }
    }
}

impl Render for ProviderEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let feedback = ProviderDraft::NAME
            .errors(&self.form, cx)
            .first()
            .map(|issue| validation_text(issue, cx))
            .unwrap_or_default();

        div().child(
            field()
                .label("Provider name")
                .required(true)
                .description(feedback)
                .child(Input::new(&self.name_input)),
        )
    }
}
```

`FormInput` 将 native edit defer 到 Form，把相关 Form value change 静默投影回 native input，并在自身
整个生命周期内拥有这条同步关系。该 input 发起的写入不会立刻投影回它自己；绑定到相同 path 的另一个
control 仍会收到新值。

## 提交同一个 session

control 不会再收集第二份 draft。准备现有 Form，执行由应用拥有的 I/O，然后只有在保存版本仍是当前版本时
才 rebase：

```rust,ignore
let prepared = self.form.update(cx, |form, cx| form.prepare(cx))?;
let (version, request): (_, ProviderDraft) = prepared.into_parts();

let canonical_model = self.save_provider(request).await?;
self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_model, cx);
});
```

## 选择合适的 adapter

- `FormInput`、`FormTextarea`、`FormEditor` 分别通过 `InputState`、`TextareaState`、
  `EditorState` 绑定 `String`；单行使用 `Input`，多行正文使用 `Textarea`，代码使用 `Editor`。
- `FormIntegerInput` 绑定整数 primitive，同时将未解析展示草稿按稳定 editor key 保留在 Form。
- `FormSelect` 绑定 `Option<D::Item::Value>`。
- `FormCombobox` 绑定 `Vec<D::Item::Value>`。
- `Checkbox` 与 `Switch` 没有暴露 state entity，应作为 controlled element 渲染。

对 total path 使用 `new`。经过 collection item、enum case 或 `Option::Some` 的 path 是 dynamic path：先解析，
再将得到的 path 传给 `try_new`。dynamic adapter 会随其 location 一起退休；出现新 location 时应新建 adapter，
不能把旧 control 重定向过去。

页面 observer 只用于页面渲染。内置 adapter 不依赖页面级 `FormEvent` subscription 也能保持同步。catalog 和
delegate 由应用拥有；刷新 option 不会隐式改写 Form value 或选择 fallback。

完整的 total/dynamic path、全部 adapter、validation、option refresh 与 custom stateful adapter recipe
请见[使用指南](docs/guide.zh-CN.md)或其[英文版本](docs/guide.md)。


## 部分字段的映射编辑

`FormMappedInput` 沿用现有构造函数支持 `EditResult::Update`。
`EditResult::update(...)` 在接纳时把已解析意图应用到最新存储值，`Parsed` 仍替换整个字段。
被拒绝编辑的原文保存在 Form 草稿中，其他编辑器的草稿与待接纳输入继续受保护。
详见[映射指南](docs/guide.zh-CN.md)。

`FormMappedInput` 及其他有状态适配器提供来源推断的 `editor_status(cx)` / `discard_draft(cx)`。
查询仅返回已接纳的来源草稿与问题；显式弃稿也取消该来源待接纳输入，同路径其他编辑器状态保留。
详见[指南](docs/guide.zh-CN.md)。

## 编辑会话迁移（#248）

旧 Validator 入口已删除；with_rules 显式注册规则依赖并返回 Result。Form 持有未解析草稿与原生编辑票据。Total set 返回 Result；replace/reset/rebase 强制 DraftPolicy 并返回生命周期报告。刷新用 rebase_preserving_edits 原子保留本地编辑并报告冲突。数字构造函数新增稳定 editor key；FormMappedInput 提供存储/展示映射。签名、路径基线/dirty 和迁移详情见[指南](docs/guide.zh-CN.md)。
