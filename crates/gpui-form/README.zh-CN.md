# gpui-form

[English](README.md) | [简体中文](README.zh-CN.md)

`gpui-form` 让 GPUI 页面编辑一份类型化 Rust draft、验证它、为保存准备一个 snapshot，并安全地应用
canonical saved result。先从一张普通页面表单开始；递归树后续使用同一套 session 与 path 规则。

## 添加依赖与 imports

```toml
[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui-component.workspace = true
gpui-form.workspace = true
gpui-form-gpui-component.workspace = true
```

下方完整示例使用这个 prelude；应用持有的 save type 与 I/O method 会在调用处说明。

```rust,ignore
use std::{collections::HashSet, sync::Arc};

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::{
    form::field,
    input::{Input, InputState},
};
use gpui_form::{
    Form, FormRevision, FormSchema, PrepareError, Prepared, ValidationMessage,
    ValidationRequest, ValidationSink, Validator,
};
use gpui_form_gpui_component::{
    FormInput, FormIntegerInput, IntegerInputState,
};
```

## 一张完整的 Provider 表单

### 1. 描述 draft

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,

    #[form(validate(on_change, on_submit))]
    retry_limit: u32,

    enabled: bool,
}
```

`FormSchema` 会创建可复用的静态 definition，例如
`ProviderDraft::NAME: FieldDef<ProviderDraft, String>`。definition 只包含 schema metadata 与 typed
access；它绝不持有 value、form entity、subscription 或 control state。

`required` 与 `validate(...)` 配置生成的 leaf schema 及其校验触发时机。

### 2. 向一次编辑 session 注入 validation

validator 属于 session，因此同一 model 可以在不同页面中配合不同的 application dependency 编辑。

```rust,ignore
struct ProviderValidator {
    reserved_names: Arc<HashSet<String>>,
}

impl ProviderValidator {
    fn new(reserved_names: Arc<HashSet<String>>) -> Self {
        Self { reserved_names }
    }
}

impl Validator<ProviderDraft> for ProviderValidator {
    fn validate(
        &self,
        model: &ProviderDraft,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<ProviderDraft>,
    ) {
        if request.includes(&ProviderDraft::NAME)
            && self.reserved_names.contains(model.name.trim())
        {
            out.at(ProviderDraft::NAME).error(
                "provider-name-reserved",
                ValidationMessage::key("provider-name-reserved"),
            );
        }

        if request.includes(&ProviderDraft::RETRY_LIMIT) && model.retry_limit > 10 {
            out.at(ProviderDraft::RETRY_LIMIT).error(
                "retry-limit-too-large",
                ValidationMessage::key("retry-limit-too-large"),
            );
        }
    }
}

let reserved_names = Arc::new(HashSet::from(["default".to_owned()]));
let runtime = Form::try_new_with_validator(
    ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    },
    ProviderValidator::new(reserved_names),
)?;
let form: Entity<Form<ProviderDraft>> = cx.new(|_| runtime);
```

`Form<M>` 持有这一次编辑 session 的 current draft、baseline、revision、validation report 与
form-owned async validation work。
`#[form(required)]` 负责 name 的缺失值规则；注入的 validator 则加入依赖本页面 catalog 的
业务规则。

### 3. 让页面持有 control 与一条 observation

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

impl ProviderPage {
    fn new(
        reserved_names: Arc<HashSet<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let runtime = Form::try_new_with_validator(
            ProviderDraft { name: String::new(), retry_limit: 3, enabled: true },
            ProviderValidator::new(reserved_names),
        )?;
        let form = cx.new(|_| runtime);

        let name_input = FormInput::new(
            &form,
            ProviderDraft::NAME,
            |window, cx| InputState::new(window, cx).placeholder("Provider name"),
            window,
            cx,
        );
        let retry_limit_input = FormIntegerInput::new(
            &form,
            ProviderDraft::RETRY_LIMIT,
            |window, cx| IntegerInputState::new(window, cx)
                .min(0u32).max(10u32).step(1u32),
            window,
            cx,
        )?;
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());

        Ok(Self { form, form_observer, name_input, retry_limit_input })
    }
}
```

root 上的 `FieldDef` 是目标 total-path 简写，因此 `FormInput::new` 不需要返回 path-resolution
result。有状态 control 的构造与 native event 同步见
[component adapter guide](../gpui-form-gpui-component/docs/guide.zh-CN.md)。

### 4. 渲染 schema metadata 与实时 form status

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let is_pending = self.form.read(cx).is_validating();
let feedback = errors
    .first()
    .map(|issue| validation_text(issue, cx))
    .unwrap_or_else(|| if is_pending { "Checking…".into() } else { String::new() });

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(feedback)
    .child(Input::new(&self.name_input));
```

页面还会读取 form-level dirty、valid、pending、report 与 revision state，用于渲染按钮和摘要。form
不决定何时显示 error，也不决定 submit 失败后 focus 哪个 control；active page 决定。

### 5. Prepare、保存并有条件地 rebase

`Prepared<M>::map` 会消费 prepared snapshot，因此在把它映射为 request 前先捕获 revision：

```rust,ignore
struct SaveProvider {
    name: String,
    retry_limit: u32,
    enabled: bool,
}

impl From<ProviderDraft> for SaveProvider {
    fn from(draft: ProviderDraft) -> Self {
        Self {
            name: draft.name,
            retry_limit: draft.retry_limit,
            enabled: draft.enabled,
        }
    }
}

fn save(&mut self, cx: &mut Context<Self>) -> Result<(), PrepareError> {
    let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
        form.prepare(cx)
    })?;

    let revision = prepared.revision();
    let request = prepared.map(SaveProvider::from);
    self.start_save(revision, request, cx); // 页面持有的 async operation
    Ok(())
}

fn save_finished(
    &mut self,
    submitted_revision: FormRevision,
    saved: ProviderDraft,
    cx: &mut Context<Self>,
) {
    let applied = self.form.update(cx, |form, cx| {
        form.rebase_if_revision(submitted_revision, saved, cx)
    });
    if !applied {
        self.show_saved_while_editing_notice(cx);
    }
}
```

`prepare` 验证同一个 snapshot，拒绝 blocking data/control issue 与 pending async validation，再连同
revision 捕获该 snapshot。保存、加载、retry 与 notification 仍属于 page 或 controller。CAS 失败绝不
覆盖保存期间产生的新编辑。

## Dynamic item 不需要 model ID

用 `#[form(items)]` 标记结构化 collection，但不要把 form 导航 identity 放进业务 draft：

```rust,ignore
#[derive(Clone, FormSchema)]
struct HeaderDraft {
    name: String,
}

#[derive(Clone, FormSchema)]
struct RequestDraft {
    #[form(items)]
    headers: Vec<HeaderDraft>,
}

let headers = RequestDraft::HEADERS;
let header = headers.append(
    &request_form,
    HeaderDraft { name: String::new() },
    cx,
)?;
let name = header.then(HeaderDraft::NAME);
name.try_set(&request_form, "Authorization".to_owned(), cx)?;
```

Form 生成 item 稳定的 session-local identity，并把它放在返回的 typed `ItemPath` 内。`items`、
`append`、`insert_before` 与 `replace_all` 产生这些 path；remove 与 move operation 消费或比较它们。
调用方不声明 `#[form(identity)]`、不构造 raw item ID，也不在 `RequestDraft` 中持久化 form identity。

dynamic enum 与 optional 位置必须在当前 session 中定位：

```rust,ignore
let payload = enum_path.try_case(form_entity.read(cx), EnumDraft::PAYLOAD)?;
let child = optional_path.try_some(form_entity.read(cx))?;
```

`try_case` 与 `try_some` 捕获当前 active incarnation，但返回的 `DynamicPath` 不保存 form entity。
纯静态 `.then(...)` 组合不需要 Form。case 经历 `A -> B -> A`，或 option 经历
`Some -> None -> Some` 后，旧 dynamic path 仍保持 retired。调用方不会看到或传递
`TopologyIndex`；每次 resolve、validation 或 mutation transaction 都由 Form 内部使用同一份私有
topology snapshot。

## 下一步

- [使用指南](docs/guide.zh-CN.md)：validation workflow、lifecycle replacement、optional/recursive
  path、由 runtime 定位的 collection topology 与 lifetime rule。
- [宏使用指南](../gpui-form-macros/docs/guide.zh-CN.md)：`FormSchema`、schema fragment、enum case、
  runtime item path 与编译期 diagnostic。
- [Component adapter 使用指南](../gpui-form-gpui-component/docs/guide.zh-CN.md)：native control、
  deferred binding、integer/select/combobox 行为与 custom adapter。
