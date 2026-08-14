# gpui-form-macros：FormSchema 目标生成更新计划

## 状态、边界与依赖

- 状态：`Done`（2026-08-09）；实际 UI 操作测试不适用于 macro，本轮未执行。
- 计划 ID：`issue-199`
- 架构来源：[gpui-form 目标设计草稿](../../../../gpui-form/docs/dev/issue-199/design-draft.md)
- 所有者：`crates/gpui-form-macros`
- 本计划定义宏的生成、诊断、测试及本 crate 对外文档最终拼写同步；不实现 core runtime、控件绑定
  或应用迁移。
- 本计划保持为独立专题文档；Issue README 只增加状态和链接，历史计划、代码与 manifest 不在计划创建阶段改写。
- 兼容策略：破坏性变更；不保留旧 attribute、`on_dynamic` 或任何兼容别名/垫片。

本计划消费 core 所有者定义的 `C-900`–`C-904`，不另建共享 C-ID：

| Core 契约 | 宏的消费方式 |
| --- | --- |
| `C-900` | `FormSchema`、definition、total/dynamic path、collection occurrence 与 optional/case resolver 所需的最终生成契约。 |
| `C-901` | mutation/change/typed event；宏不生成、匹配或路由事件。 |
| `C-902` | control binding/adapter；宏不引用该契约。 |
| `C-903` | snapshot validation、`ValidationTrigger::External`、field metadata 与 prepare/version 的生成边界。 |
| `C-904` | private Transition/atomic rollout；宏不命名内部状态，完成后参与残留/聚合验收门槛。 |

## 已核对事实

| ID | 分类 | 事实与证据 | 后果 |
| --- | --- | --- | --- |
| `E-1000` | 当前事实 | `src/derive/expand.rs` 同时遍历 `syn::DataStruct/DataEnum`、生成访问函数、关联常量与 `SchemaVisitor` 调用。 | 建立唯一语义模型，避免各 expansion 单元重复解释 attribute。 |
| `E-1001` | 当前事实 | `attributes.rs` 支持 `child/items/required` 和旧 `on_dynamic`；`required` 默认开启 mount/change/blur/submit。 | 改为 `on_external`、submit 默认及叶子/容器组合校验。 |
| `E-1002` | 当前事实 | `definition.rs` 生成 `ROOT`/const，`driver.rs` 生成 `__visit`，`validation.rs` 生成旧 trigger bit。 | 保留职责拆分，统一消费新的语义模型。 |
| `E-1003` | 当前事实 | `Option` child 经 `visitor.optional`，`Vec` item 经索引 closure，payload enum case 经 `visitor.case`；静态 `ChildDef/CaseDef` 已存在。 | runtime occurrence、resolver proof 与 item identity 全部留在 core。 |
| `E-1004` | 当前事实 | `tests/ui.rs` 只运行失败测试样例；现有样例覆盖 generic、container option、冲突 kind、非 Vec、旧 array、非法 trigger 和受限 enum。 | 更新 stderr，新增最终语法的测试样例；递归通过测试由 core consumer test 覆盖。 |
| `E-1005` | 当前事实 | `Cargo.toml` 仅依赖 `proc-macro2/quote/syn/trybuild`。 | 不改 manifest、lockfile、feature 或生成链。 |
| `E-1006` | 用户决定 | descriptor 永不持有 `Entity<Form<_>>`/`WeakEntity<Form<_>>`；调用显式传 Form。 | 宏不生成 form field、weak capture、control 或 event。 |
| `E-1007` | 用户决定 | 普通字段 total；item/case/optional 后 dynamic；inactive 是 `Ok(None)`，退休/错 session 是 `Err(ResolveError)`。 | 宏只保留 typed static edge，不能将 runtime 状态编码进 descriptor。 |

## 所有者决策

| ID | 决策 | 排除的方案 | 后果 |
| --- | --- | --- | --- |
| `D-1000` | derive 只生成静态 definition、typed 访问函数与 schema traversal driver。 | 宏生成 session/token、runtime resolver、message、Transition 或 control。 | 与 `C-900` 单向依赖。 |
| `D-1001` | `#[form(child)]` 只接受 nested `T` 或 `Option<T>`；`#[form(items)]` 只接受 `Vec<T>`；未标注是 leaf；`required/validate` 只允许 leaf。 | 静默忽略容器 metadata，或自动把所有 struct 当 child。 | 无效 schema 在 derive span 失败。 |
| `D-1002` | trigger 固定为 `on_mount/on_change/on_blur/on_external/on_submit`；未显式选择 trigger 的 leaf validation 与 `required` 只在 submit 运行。 | `required` 隐式开启 mount/change/blur，或保留 `on_dynamic`。 | 业务验证默认不在构造和普通 set 时运行。 |
| `D-1003` | 只支持 unit 或单 payload tuple enum variant、非泛型 named struct，variant 不接受 `#[form(...)]`。 | named/multi-payload/generic 的类型擦除兼容。 | 诊断稳定，definition 不需类型擦除。 |
| `D-1004` | `DeriveModel` 是唯一 attribute 解释点；每个 known option/trigger 在同一 field 上最多一次。 | 各 expansion module 重新扫描 raw attribute。 | 消除语法分裂，duplicate 有确定诊断。 |

## 文件地图

```text
crates/gpui-form-macros/
├── src/derive.rs                         # F-1000 修改：入口解析并转交语义模型
├── src/derive/attributes.rs              # F-1001 修改：语法、重复项和 span 诊断
├── src/derive/model.rs                   # F-1002 修改：DeriveModel/field/variant 的唯一语义表示
├── src/derive/expand.rs                  # F-1003 修改：struct/enum 编排和最终约束
├── src/derive/expand/definition.rs       # F-1004 修改：ROOT、descriptor、typed 访问函数
├── src/derive/expand/driver.rs           # F-1005 修改：无状态 schema traversal
├── src/derive/expand/validation.rs       # F-1006 修改：leaf metadata 和 External trigger
├── tests/ui.rs                           # F-1007 修改：trybuild 失败用例分组；通过用例由 core 消费方测试样例持有
├── tests/ui/vnext/fail/*.rs/.stderr      # F-1008 修改/新增：稳定语法诊断
├── Cargo.toml                            # F-1009 不修改：无新增依赖/生成步骤
└── README*.md、docs/guide*.md            # F-1010 实现后核对：最终 grammar/API 中英文对应
```

`crates/gpui-form/src/{schema,path,validation,topology}.rs` 与
`crates/gpui-form/tests/ui{.rs,/vnext/pass/*.rs}` 是 `C-900`/`C-903` 的生产者/消费者门禁，
不属于 macro owner。macro crate 不增加反向运行时依赖。

## 目标契约

### L-1000：唯一的 derive semantic model

```rust,ignore
enum ModelKind {
    Struct { fields: Vec<SchemaField> },
    Enum { variants: Vec<SchemaVariant> },
}
struct DeriveModel { ident: syn::Ident, kind: ModelKind }
struct SchemaField {
    ident: syn::Ident,
    ty: syn::Type,
    span: proc_macro2::Span,
    kind: FieldKind,
    validation: LeafValidation,
}
enum FieldKind { Leaf, Child, Items }
struct LeafValidation { required: bool, triggers: TriggerSelection }
```

这些类型只在宏调用期间存活，不导出、不序列化、无运行时状态。parser 保存每个 token span，
再执行 `D-1001` 的 type/metadata 校验；递归 Child/Item 是否实现 `FormSchema` 由生成代码的 Rust
trait check 决定，宏不按类型名称猜测。

### L-1001：静态 descriptor 与 resolver 前提

对 named struct `Root` 生成唯一 const，getter 是 private associated function：

```rust,ignore
impl Root {
    pub const ROOT: ::gpui_form::RootDef<Self> = ::gpui_form::RootDef::__new();
    pub const LEAF: ::gpui_form::FieldDef<Self, T> = /* read/read_mut */;
    pub const CHILD: ::gpui_form::ChildDef<Self, ChildOrOptionChild> = /* ... */;
    pub const ITEMS: ::gpui_form::ItemsDef<Self, Item> = /* ... */;
}
impl Enum {
    pub const PAYLOAD: ::gpui_form::CaseDef<Self, Payload> = /* ... */;
}
```

宏不生成 `Some`/`case` runtime 状态。`C-900` 用最终
`ChildDef<Self, Option<T>>` 和 `CaseDef<Enum, Payload>` 提供
`.some().resolve(&form, cx)`/`.case(CASE).resolve(&form, cx)`，因而后续 path 的 Rust 值类型由
descriptor 泛型确定。

### L-1002：traversal 与 validation metadata

```rust,ignore
impl ::gpui_form::FormSchema for Root {
    fn __visit(&self, visitor: &mut dyn ::gpui_form::__private::SchemaVisitor) {
        // leaf -> field; child -> child; Option -> optional;
        // Vec -> items; enum -> case/unit_case
    }
}
```

driver 只提供静态 edge/current model shape：不得读取 Form、分配 occurrence、构造 `PathKey`、产生 event 或
issue。`C-900` topology builder 消费该 driver；读取 snapshot 不得触发隐式 identity 分配。

最终 leaf metadata 由 `C-903` 定义：

```rust,ignore
::gpui_form::FieldSchema::new(
    name,
    required,
    ::gpui_form::ValidationTriggers { mount, change, blur, external, submit },
)
```

`#[form(required)]` 默认生成 `submit: true` 及其余 false；显式 `validate(...)` 仅开启列出的
trigger，`on_external` 映射 `ValidationTrigger::External`。

### ERR-1000：derive 诊断

| 输入 | 要求 |
| --- | --- |
| generic/union/tuple struct | 指向类型，说明仅支持非泛型 named struct 或受限 enum。 |
| child/items 冲突、重复 option、容器 leaf metadata | 指向后出现 token，说明互斥或仅 leaf 可用。 |
| 非 `Vec` items、非 `T/Option<T>` child | 指向 type/attribute；不生成 fallback leaf。 |
| `on_dynamic`、未知/重复 trigger | 指向 trigger；列出最终 token，不提供兼容别名。 |
| enum variant attribute、named/multi payload variant | 指向 variant；不生成类型擦除 accessor。 |

每个新增/修改诊断必须同步 `.stderr`。inactive/retired resolver 不是 derive error，分别由 `C-900`
返回 `Ok(None)`/`ResolveError`。

### ST-1000：生成物生命周期

- **权威：**一次 proc-macro 调用的 `DeriveModel`。
- **读取者：**definition、driver、validation expansion；三者不得重读 raw attributes。
- **输出：**关联常量、访问函数和 `__visit` token；没有 Entity、subscription、task 或持久化。
- **运行时边界：**Form session、topology、resolver、validation snapshot 和 binding 都属于 `C-900`–`C-904`。

## 工作包

### WP-1000：建立单一语义模型

**文件：**`F-1000`–`F-1003`

**前置：**`D-1000`–`D-1004`、core `WP-900` 已冻结 schema/validation 隐藏签名；不要求 `C-900`/`C-903` 已 producer-ready

1. 入口先拒绝不支持 container/generic 形状，再构造 `L-1000`。
2. parser 记录 kind、required、最终五个 trigger 与 span，执行重复、互斥、leaf/container 校验。
3. struct/enum expand 只消费 `DeriveModel`，删除旧 `dynamic` 字段及 raw attribute 二次解释。

**完成条件：**每个输入只解析一次；无效输入在确定 span 失败；活跃宏源码无 `on_dynamic`。

### WP-1001：生成最终 schema 输出

**文件：**`F-1004`–`F-1006`

**前置：**`WP-1000`、core `WP-900` 的 public/hidden trait 骨架

1. 按 `L-1001` 输出 ROOT、leaf/child/items/case const 和 typed 访问函数，保证 const 不含 Form entity。
2. 按 `L-1002` 输出 traversal；optional/case 仅给 core resolver 以静态 edge，items 仅报告 model 顺序。
3. 输出 `external` bit 和 submit 默认；删除旧 dynamic bit 输出。
4. 不引用 `FormEvent`、`ControlBinding` 或 `gpui_operation`。

**完成条件：**recursive Vec、nested child、Option child 与 enum payload 都保留最终 Rust 类型；没有 runtime ID/event/transition token。

### WP-1002：稳定诊断与 core 消费 gate

**文件：**`F-1007`–`F-1008`；core 通过测试样例由 `C-900`/`C-903` owner 实现

**前置：**`WP-1001`

1. 更新现有失败用例 stderr，尤其 `invalid_trigger.rs`：拒绝 `on_dynamic`、列出 `on_external`。
2. 新增重复 trigger、容器 leaf metadata、child type shape 的失败测试样例。
3. 在 core `tests/ui/vnext/pass` 增加/更新 recursive fixture，编译消费 `get/set`、`items`、case/optional
   `resolve`、`try_get/try_set`，并让错误 value type 编译失败。

**完成条件：**trybuild 与 core 消费方测试样例都不使用字符串 path、业务 ID 或旧 API。

### WP-1003：对外文档与残留门禁

**文件：**`F-1010`；活跃 macro 源码/tests

**前置：**`WP-1002`、core `C-900`/`C-903` focused gate

1. 对照最终生成签名核对中英文 README/guide，只调整实际拼写和示例；不公开 topology、Transition、
   mailbox 或 runtime identity。
2. 扫描活跃宏源码/tests/docs，确认不再出现 `on_dynamic`、旧 constructor、旧 descriptor 名或
   兼容 alias；历史开发文档不在残留目标内。

**完成条件：**本 crate 对外文档与导出/生成 API 一致，残留扫描通过；不把目标文档误报为实现证据。

## 测试矩阵与命令

| R-ID | T-ID | 层级 | 场景与断言 |
| --- | --- | --- | --- |
| `R-1000` | `T-1000` | macro trybuild | `on_dynamic`、未知/重复 trigger 稳定失败且 span 正确。 |
| `R-1001` | `T-1001` | macro trybuild | child/items 冲突、容器 metadata、非 Vec items 与不支持 enum/model 无 fallback。 |
| `R-1002` | `T-1002` | core 通过测试样例 | root/child/items/case/optional descriptor 的最终类型正确，错误 `set` 类型被拒绝。 |
| `R-1003` | `T-1003` | core topology tests | macro driver 支持 core occurrence；reorder/rebuild 的 runtime 语义不由 macro 伪造。 |
| `R-1004` | `T-1004` | core validation tests | `on_external` 生效；未选择 mount/change/blur 的字段不会在构造/普通 set 执行业务验证。 |

```sh
cargo fmt --all
cargo test -p gpui-form-macros --locked
cargo test -p gpui-form --all-features --locked
cargo clippy -p gpui-form-macros -p gpui-form --all-targets --all-features --locked -- -D warnings
git diff --check
```

不运行实际 UI 操作测试；macro 没有 UI runtime。

## 验收

1. 活跃 derive 只接受最终 grammar，旧 `on_dynamic` 无兼容入口。
2. 每个 descriptor 为可复用 static const，绝不捕获 Form entity、值、subscription 或 runtime identity。
3. child/items/optional/case 的 typed output 让 core 精确区分 total、dynamic、inactive 和 retired。
4. validation metadata 使用 `External`、submit 默认和明确 non-submit opt-in。
5. macro 不依赖 operation、不构造 event/control binding，也不承担 runtime error。
6. trybuild、core pass/validation gate、format、clippy 与 `git diff --check` 通过；不进行实际 UI 操作测试。

## 实施证据

- 实现位置：当前工作区，尚未提交；`WP-1000`–`WP-1003` 已完成。
- `cargo test -p gpui-form-macros --locked` 通过；12 个 compile-fail fixture 全部通过。
- core compile fixture 同时覆盖 items、case、optional、dynamic `try_get/try_set`，错误 leaf value type
  由 compile-fail fixture 拒绝。
- 与 core/adapter 的聚合测试、producer Clippy、workspace check、格式和 diff-check 均通过。
- 未执行实际 UI 操作测试；macro 没有 UI runtime。
