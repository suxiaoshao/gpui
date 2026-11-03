# gpui-form 使用指南

[English](guide.md) | [简体中文](guide.zh-CN.md)

本指南说明 form 编辑 session 的公开契约。先以 provider settings 页面为例，再扩展到 optional data 与
recursive collection。声明语法见[宏使用指南]；具体的 `gpui-component` adapter 与 custom control 见
[component adapter 使用指南]。

[宏使用指南]: ../../gpui-form-macros/docs/guide.zh-CN.md
[component adapter 使用指南]: ../../gpui-form-gpui-component/docs/guide.zh-CN.md

代码片段沿用 [README] 中的依赖与 import，并省略 `start_save`、`reconcile_query_rows` 等与示例无关的
页面 method。

[README]: ../README.zh-CN.md

## 1. 所有权：一个 form session，每次访问都显式传入

每次编辑 session 创建一个 `Entity<Form<M>>`。它持有 current typed model、baseline、validation fact 与
dynamic field location，以及按 editor key 保留的未解析展示草稿。页面持有 save/load operation、button policy、error visibility、focus 与展示。native
control 持有 IME、cursor、selection、popup state 等交互状态。

schema descriptor 不持有 session，因此每个 descriptor 可以复用：

```rust,ignore
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ProviderDraft {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    #[form(validate(on_submit))]
    retry_limit: u32,
    enabled: bool,
}

let form = cx.new(|_| {
    Form::new(ProviderDraft {
        name: String::new(),
        retry_limit: 3,
        enabled: true,
    })
    .with_rules(ProviderValidator::new(reserved_names).rules()).expect("valid rules")
});
```

`Form::new` 无配置错误；`with_rules` 返回 Result，拒绝重复 ID、缺少依赖或失效的 occurrence。为 `name` 生成的 descriptor 是
`ProviderDraft::NAME` 这类静态值；每次读取或修改都显式传入 `&form`。

## 2. 普通字段使用 total path

root field descriptor，或只穿过 `#[form(child)]` field 组合出的 path，都是 `TotalPath<Root, T>`。它在整个
编辑 session 内存在，包括 `replace`、`reset` 或 `rebase` 之后：

```rust,ignore
let name = ProviderDraft::NAME;

let current: String = name.get(&form, cx);
let changed: bool = name.set(&form, "primary".into(), cx).unwrap();

let enabled = ProviderDraft::ENABLED;
enabled.set(&form, true, cx).unwrap();
```

`set` 返回 model 是否变化。普通等值写入是 model no-op。不要在页面中再保存一份 value；render 时重新读取
Form。

静态嵌套也保持这个性质：

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct AuthDraft { username: String }

#[derive(Clone, PartialEq, FormSchema)]
struct RedirectDraft { callback_url: String }

#[derive(Clone, PartialEq, FormSchema)]
struct ServerDraft {
    #[form(child)]
    auth: AuthDraft,
    #[form(child)]
    redirect: Option<RedirectDraft>,
}

let server_form = cx.new(|_| Form::new(ServerDraft {
    auth: AuthDraft { username: String::new() },
    redirect: None,
}));
let username = ServerDraft::AUTH.then(AuthDraft::USERNAME);
let current: String = username.get(&server_form, cx);
```

## 3. item、case 与 optional payload 使用 dynamic path

item、enum case 或 `Option::Some` 边界会产生 `DynamicPath<Root, T>`。它的 `try_get`、`try_set` 与 adapter
构造函数在 path 属于另一个 form session 或已经 retire 时返回 `ResolveError`。

resolve 未激活的 enum case 或不存在的 optional child 是正常情况，返回 `Ok(None)`，不是 error。起始 path
已经 retire 时才返回 `Err(ResolveError)`：

```rust,ignore
let condition = node
    .then(FilterNode::KIND)
    .case(FilterNodeKind::CONDITION)
    .resolve(&query_form, cx)?;

if let Some(condition) = condition {
    let value = condition.then(FilterCondition::VALUE);
    let current: String = value.try_get(&query_form, cx)?;
    value.try_set(&query_form, "Rust".to_owned(), cx)?;
}

let redirect = ServerDraft::REDIRECT
    .some()
    .resolve(&server_form, cx)?;

if let Some(redirect) = redirect {
    redirect
        .then(RedirectDraft::CALLBACK_URL)
        .try_set(&server_form, "https://example.com/callback".into(), cx)?;
}
```

resolved path 的返回类型决定 value type。对 `set` 或 `try_set` 传入错误的 Rust type 会被类型系统拒绝。

## 4. collection 与 recursive typed tree

结构化 `Vec<T>` 使用 `#[form(items)]`。业务 model 只包含业务数据：每个 item occurrence 的 identity 由 Form
生成。`ItemPath` 只能通过枚举 Form 或 collection mutation 获得：

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct HeaderDraft { name: String }

#[derive(Clone, PartialEq, FormSchema)]
struct RequestDraft {
    #[form(items)]
    headers: Vec<HeaderDraft>,
}

let request_form = cx.new(|_| Form::new(RequestDraft { headers: Vec::new() }));
let headers = RequestDraft::HEADERS;
let header = headers.append(
    &request_form,
    HeaderDraft { name: String::new() },
    cx,
)?;

header
    .then(HeaderDraft::NAME)
    .try_set(&request_form, "Authorization".into(), cx)?;
```

collection method 包括 `items`、`try_items`、`append`、`insert_before`、`move_before`、`remove`、
`replace_all` 与 `ItemPath::move_to`。它们使用 typed item path，不使用 raw ID、business ID 或 index。
`ItemPath::key()` 在 item 保持 active 时是稳定的 opaque UI key。

这套方式可以扩展到 recursive filter tree，而无需向 model 添加 ID：

```rust,ignore
#[derive(Clone, PartialEq, FormSchema)]
struct QueryDraft {
    #[form(child)]
    filters: FilterGroup,
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterGroup {
    #[form(items)]
    children: Vec<FilterNode>,
}

#[derive(Clone, PartialEq, FormSchema)]
enum FilterNode {
    Condition(FilterCondition),
    Group(FilterGroup),
}

#[derive(Clone, PartialEq, FormSchema)]
struct FilterCondition { value: String }

let query_form = cx.new(|_| Form::new(QueryDraft {
    filters: FilterGroup {
        children: vec![FilterNode::Condition(FilterCondition {
            value: String::new(),
        })],
    },
}));
let children = QueryDraft::ROOT
    .then(QueryDraft::FILTERS)
    .then(FilterGroup::CHILDREN);
for node in children.items(&query_form, cx) {
    let condition = node
        .case(FilterNode::CONDITION)
        .resolve(&query_form, cx)?;

    if let Some(condition) = condition {
        let value = condition.then(FilterCondition::VALUE);
        render_condition(node.key(), value.try_get(&query_form, cx)?);
    }
}
```

同父级重排保留 item path。删除并重新插入 item、重建 case 或 optional payload、替换 collection、
replace/reset/rebase 整个 Form，以及跨父级移动，都会使旧 dynamic path retire。path 不会悄悄解析到之后位于
同一表面位置的 value。

## 5. 连接控件并渲染页面

普通 `gpui-component` control 使用内置 adapter：

```rust,ignore
struct ProviderPage {
    form: Entity<Form<ProviderDraft>>,
    form_observer: Subscription,
    name_input: FormInput,
    retry_limit_input: FormIntegerInput<u32>,
}

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
    "budget-editor",
    |window, cx| IntegerInputState::new(window, cx).min(0u32).max(10u32),
    window,
    cx,
)?;
let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
```

`new` 接收 total path。`try_new` 接收已经 resolve 的 dynamic path，且可能因 location 已 retire 而失败。
内置控件自行保存 binding 并订阅 form change；只有页面本身需要 rerender 时，才保存 `form_observer`。

render code 分别使用 schema metadata 与 live form fact：

```rust,ignore
let name = ProviderDraft::NAME;
let errors = name.errors(&self.form, cx);
let pending = self.form.read(cx).is_validating();

field()
    .label("Provider name")
    .required(name.schema().is_required())
    .description(error_text(errors.first(), pending))
    .child(Input::new(&self.name_input));
```

页面决定 error 是否可见，以及 submit 失败后 focus 哪个可见 control。Form 只报告事实，不持有 touched state、
focus 或 layout。

自定义的有状态控件使用 [component adapter 使用指南]中的 `ControlBinding`、`ControlWriter` 与
`ControlProjection` 协议。控件静默投影 `Value`，从 native event 通过 writer 写回，并处理一次性的
`Retired` projection。它不手工订阅 `FormEvent`，也不使用本地的双向绑定布尔开关。

## 6. Validation

validator 面向单个 `ValidationRequest` 编写，它同时提供 current model 与 snapshot-bound path resolution：

```rust,ignore
impl ProviderValidator {
    fn rules(self) -> Vec<ValidationRule<ProviderDraft>> {
        vec![ValidationRule::new("ProviderValidator", move |request, out| self.validate(request, out))
            .dependency(ProviderDraft::ROOT)]
    }

    fn validate(
        &self,
        request: ValidationRequest<'_, ProviderDraft>,
        out: &mut ValidationSink<'_, ProviderDraft>,
    ) {
        let model = request.model();

        if request.includes(&ProviderDraft::NAME) && model.name.trim().is_empty() {
            out.at(ProviderDraft::NAME).error(
                "provider-name-empty",
                ValidationMessage::key("provider-name-empty"),
            );
        }
    }
}
```

递归 validation 只使用同一个 request 创建的 path。这些 path 可以继续类型化组合并传给 `out.at`，但不提供
mutation 或 control binding 方法，也不能逃逸出本次 validation snapshot：

```rust,ignore
fn validate_filter_nodes<'a>(
    request: &ValidationRequest<'a, QueryDraft>,
    nodes: Vec<ValidationItemPath<'a, QueryDraft, FilterNode>>,
    out: &mut ValidationSink<'_, QueryDraft>,
) {
    for node in nodes {
        if let Ok(Some(condition)) = node
            .clone()
            .case(FilterNode::CONDITION)
            .resolve(request)
        {
            let value = condition.then(FilterCondition::VALUE);
            if request
                .try_get(&value)
                .is_ok_and(|value| value.trim().is_empty())
            {
                out.at(value).error(
                    "filter-value-empty",
                    ValidationMessage::key("filter-value-empty"),
                );
            }
            continue;
        }

        if let Ok(Some(group)) = node.case(FilterNode::GROUP).resolve(request) {
            let children = group.then(FilterGroup::CHILDREN);
            if let Ok(children) = request.try_items(&children) {
                validate_filter_nodes(request, children, out);
            }
        }
    }
}

impl QueryValidator {
    fn rules(self) -> Vec<ValidationRule<QueryDraft>> {
        vec![ValidationRule::new("QueryValidator", move |request, out| self.validate(request, out))
            .dependency(QueryDraft::ROOT)]
    }

    fn validate(
        &self,
        request: ValidationRequest<'_, QueryDraft>,
        out: &mut ValidationSink<'_, QueryDraft>,
    ) {
        let children = QueryDraft::ROOT
            .then(QueryDraft::FILTERS)
            .then(FilterGroup::CHILDREN);
        validate_filter_nodes(&request, request.items(&children), out);
    }
}
```

业务 validation 默认 trigger 是 `Submit`。需要即时反馈时，使用规则的 `.triggers(ValidationTriggers { ... })` 选择
`Mount`、`Change` 或 `Blur`；schema metadata 只控制 required 规则。catalog、permission 或其他外部依赖变化后，显式请求 scope 化的 validation：

```rust,ignore
ProviderDraft::NAME.validate(&form, ValidationTrigger::External, cx);
```

异步 validation 也只会针对 total path 或当前已 resolve 的 dynamic path 被显式发起。相交写入、retirement、
lifecycle change 或更新的 validation run 发生后，Form 会取消或丢弃旧 result。Form own 的 pending async
validation 会让 `prepare` 不能成功。

## 7. Replace、reset、prepare 与 save

应用数据改变时使用 lifecycle method：

```rust,ignore
self.form.update(cx, |form, cx| form.replace(next_draft, DraftPolicy::Preserve, cx)?);
self.form.update(cx, |form, cx| form.reset(DraftPolicy::Discard, cx)?);
self.form.update(cx, |form, cx| form.rebase(saved_draft, DraftPolicy::Discard, cx)?);
```

- `replace` 安装新的 current draft，并保留 baseline。
- `reset` 把 baseline 恢复为 current draft。
- `rebase` 把一个 model 同时安装为 current draft 与 baseline。

每个 lifecycle operation 都是语义上的 model change，即使 Rust value 相等。total path 保持有效；旧 dynamic
path 及其 pending work 会 retire。

在应用 own 的 I/O 前 prepare value，并保存其 version，而不是裸 revision number：

```rust,ignore
let prepared: Prepared<ProviderDraft> = self.form.update(cx, |form, cx| {
    form.prepare(cx)
})?;

let (version, request) = prepared
    .map(SaveProvider::from)
    .into_parts();
self.start_save(version, request, cx);

// save 成功后的稍后时刻：
let applied = self.form.update(cx, |form, cx| {
    form.rebase_if_current(version, canonical_saved_model, cx)
});
```

`prepare` 对一个 snapshot 运行 submit validation，并拒绝 blocking issue 或 pending async validation。
`Prepared::map` 保留 session-bound `FormVersion`。`rebase_if_current` 返回 `false` 时，draft 未被改变：应由
应用展示“保存期间仍在编辑”的相应 UI。

## 8. 有选择地观察语义变化

页面重绘使用普通 entity observation。只有 owner 需要选择性 side effect（如 reconcile recursive row tree）时，
才订阅 `FormEvent`：

```rust,ignore
let subscription = cx.subscribe(&query_form, |_, _, event, cx| {
    match event {
        FormEvent::ModelChanged(change) => {
            let target = QueryDraft::ROOT
                .then(QueryDraft::FILTERS)
                .then(FilterGroup::CHILDREN);
            let impact = change.impact(&target);

            if impact.structure_changed() || impact.retired() {
                reconcile_query_rows(cx);
            } else if impact.value_changed() {
                cx.notify();
            }
        }
        FormEvent::ValidationChanged { .. } => cx.notify(),
    }
});
```

`ModelChangeKind` 区分 edit、replace、reset 与 rebase。`PathImpact` 回答选择的 target 的 value 是否变化、
structure 是否变化、是否 retire。event 不带 control origin 或实现 identity：控件已经自行处理同步，其他 owner
只消费与自己 target 有关的语义效果。

## 相关文档

- [README](../README.zh-CN.md)
- [宏使用指南]
- [Component adapter 使用指南]


## 编辑会话与破坏性迁移（#248）

旧 `Validator`、`with_validator`、`replace_validator` 已删除。改用
`with_rules(Vec<ValidationRule<M>>) -> Result<Self, RuleRegistrationError>` 和
`replace_rules(..., cx) -> Result<(), ...>`。每个规则显式声明 `RuleId`、一个或多个
`.dependency(path)` 子树、可选动态 `.instance(path)` 和 `.triggers(ValidationTriggers)`。
同实例不允许重复 ID。执行请求提供完整快照；输出路径不决定调度。每次原子替换该 owner
的全部旧结果，空结果和跨 trigger 均可清错。已启动规则在依赖编辑后复核；不相关编辑不
影响它。External 全量复核；依赖有未解析草稿时清除旧同步结果并等待解析。
Garde 通过 `GardeValidator::new(context).into_rule("garde", Model::ROOT)` 注册。

原生 writer 用 `defer_set` 接纳已解析值，或用
`defer_draft(display, code, message, window, cx)` 提交展示草稿。绑定以稳定 `EditorKey`
区分编辑者；低层 writer 可用 `with_editor_key(key, cx)` 重连。卸载不删除 Form 草稿。
同路径不同编辑器互不清稿；成功解析只清来源编辑者。dirty 包含未解析草稿；可解析的等价
格式按存储值比较。原生入口在 defer 前同步登记票据；未接纳编辑阻止 prepare，即使最终
值相等，旧保存凭据也失效。绑定/窗口销毁结算未接纳票据，已接纳草稿仍保留。

`DraftChanged(ModelChange<M>)` 携带 revision 与路径影响；模型和草稿联合事务共享一次
revision/notify。只改错误文案不推进 revision。`SessionChanged` 表示模型未变的基线/
拓扑生命周期变化；只有实际模型变化发 ModelChanged。穷举 FormEvent 必须新增这两分支；
普通草稿/校验事件不调用原生 value setter。

Total path 有 `baseline(&form, cx)`、`is_dirty(&form, cx)`；dynamic/item path 有
`try_baseline -> Result<Option<T>, ResolveError>`、`try_is_dirty`。新增 occurrence 的
基线为 None，退休路径报错。容器 dirty 包含增删、顺序和子草稿。Total path/descriptor
的 `set` 改为 `Result<bool, ResolveError>`，使结构写入可以原子拒绝 WouldRetireDrafts。
动态 set/remove/replace_all/跨父移动同样保护草稿。明确弃稿后再删结构：
`form.discard_drafts(typed_subtree, cx)?`。标量程序写入默认保留展示草稿。

`replace(value, policy, cx)`、`reset(policy, cx)`、`rebase(value, policy, cx)` 强制
指定 `DraftPolicy::Preserve/Discard`，返回 `Result<LifecycleReport, LifecycleError>`。
Preserve 保留有效草稿，退休动态草稿则整次拒绝；存在待接纳原生编辑时返回 EditsPending。
Discard 清稿并结算票据。Preserve reset 不保证 clean；普通刷新使用保留编辑的 rebase。

`rebase_preserving_edits(remote, cx)` 原子计算 B/L/R 三方合并，安装远端基线并返回冲突路径
和版本。普通字段、直接 child 的叶子独立合并；opaque、集合、Option、枚举/case 作为整体
单元。双边修改默认留本地并报冲突；有草稿且远端变化，即使 L=B 也冲突。保留本地子树
保留其身份；无法对应的远端基线对动态路径返回 None。不按下标匹配，不提供通用 keyed
collection 框架。可用 `form.is_current(report.version())` 检查报告新鲜度，但不代表磁盘锁。
`rebase_if_current(prepared.version(), saved, cx)` 仍强制版本保护，拒绝草稿/未接纳输入。
迟到保存结果不能拿旧基线强行套刷新合并；保存期间编辑的合并需要实际提交快照作祖先。

### 字段级领域合并

字段具有业务 key 或领域合并规则时，声明 `#[form(merge = function_path)]`。
函数签名为 `fn(&T, &T, &T) -> FieldMerge<T>`，依次接收旧基线 B、本地存储值 L、
远端值 R。输入是完整的类型值，包括应用保留的未知内容或 Unreadable。函数必须是纯函数，
不访问 Form、GPUI context，不执行 I/O 或发事件。例如，不透明 map 可按 key 合并独立变更，
同 key 冲突时保留本地：

```rust
use std::collections::BTreeMap;
use gpui_form::{FieldMerge, FormSchema};

type Overrides = BTreeMap<String, String>;

#[derive(Clone, PartialEq, FormSchema)]
struct Settings {
    #[form(merge = merge_overrides)]
    model_overrides: Overrides,
}

fn merge_overrides(base: &Overrides, local: &Overrides, remote: &Overrides)
    -> FieldMerge<Overrides>
{
    let mut candidate = remote.clone();
    let mut conflict = false;
    for key in base.keys().chain(local.keys()) {
        let (b, l, r) = (base.get(key), local.get(key), remote.get(key));
        if l == b { continue; }
        conflict |= r != b && l != r;
        match l {
            Some(value) => { candidate.insert(key.clone(), value.clone()); }
            None => { candidate.remove(key); }
        }
    }
    if conflict { FieldMerge::conflicted(candidate) }
    else { FieldMerge::merged(candidate) }
}
```

hook 覆盖该字段默认的递归或整体合并，schema 遍历、校验和类型路径保持原有规则。
leaf、`child`、`items` 字段都可声明。应把 hook 放在实际合并边界：现有遍历在集合、Option、
枚举整体单元处停止，不会自动调用它们内部字段的 hook。重复声明会报错，函数和返回类型由
Rust 检查。详见[宏指南](../../gpui-form-macros/docs/guide.zh-CN.md#字段合并声明)。

存在待接纳原生编辑票据时，整个刷新在调用任何 hook 前返回 `EditsPending`。
无草稿字段的候选与普通字段在同一事务中安装；`conflicted` 把字段的 `PathKey` 加入去重的
`LifecycleReport::conflicts()`，报告 version 为安装后的版本。业务 key 是应用数据，
不能伪装成动态 PathKey。

任一未解析草稿与 hook 字段相交时，Form 跳过该 hook，保留整个本地字段、原始草稿文本和
活动动态身份。R 与 B 不同时报告字段冲突，即使 L=R；R=B 时只保留编辑。其他字段继续合并。
这有意推迟了该字段内远端独立 key 的更新；用户修正或显式放弃草稿后需再次刷新。
hook 不能授权弃稿。

没有草稿时，候选等于 L 就保留该字段动态身份；候选不同则退休它的全部动态后代，
调用方从新活动路径重建 binding，其他字段身份不受影响。不按下标、值或业务 key 对应身份。
字段存储基线始终是原始 R，因此未保存的候选仍然 dirty。候选=R 时动态基线对应安装后的拓扑；
R=B 且候选=L 时保留旧基线对应；其他情况 `try_baseline` 可返回 `None`。
应用可以读取 total 字段基线再按业务 key 投影，不能把 `None` 当成领域 Unset。
hook 仅用于 `rebase_preserving_edits`，`rebase_if_current` 的保存安装保护保持不变。

### 控件映射

`ControlMapping<S,D>` 用应用上下文快照 format，并把展示值、当前存储值、可选基线和
同一上下文交给 parse，返回 `EditResult::Parsed/Draft`。通过
`bind_mapped_control_in` / `try_bind_mapped_control_in` 和
`MappedControlWriter::defer_edit` 接入。`MappedControlBinding::refresh(context, cx)`
仅重投影无草稿编辑器，不改模型/revision，不重解析草稿；排队前完成 parse，后续上下文
不会重新解释旧输入。不同展示/映射类型复用旧稿 key 返回 MappingBindError::EditorType。
组件适配器提供 FormMappedInput，数字构造函数新增稳定 editor key 参数。继承、Unset、
Unreadable 与业务验证仍由应用定义；焦点、光标、选择和 IME composition 仍由原生控件持有，
只有已提交的原生 Change 进入 Form。


映射可在用户改回有效基线显示时恢复继承状态，避免把继承值写成覆盖值：

```rust,ignore
#[derive(Clone, PartialEq)]
enum Slot { Unset, Set(i32), Unreadable(String) }
struct NumberMapping;
impl ControlMapping<Slot, String> for NumberMapping {
    type Context = i32; // application's inherited value snapshot
    fn format(&self, stored: &Slot, inherited: &i32) -> String {
        match stored {
            Slot::Unset => inherited.to_string(),
            Slot::Set(n) => n.to_string(),
            Slot::Unreadable(raw) => raw.clone(),
        }
    }
    fn parse(&self, display: String, current: &Slot, baseline: Option<&Slot>, inherited: &i32)
        -> EditResult<Slot, String>
    {
        if let Some(base) = baseline.filter(|b| self.format(b, inherited) == display) {
            return EditResult::Parsed(base.clone());
        }
        if self.format(current, inherited) == display {
            return EditResult::Parsed(current.clone());
        }
        match display.parse() {
            Ok(n) => EditResult::Parsed(Slot::Set(n)),
            Err(_) => EditResult::draft(display, "number", "Enter an integer"),
        }
    }
}
```
