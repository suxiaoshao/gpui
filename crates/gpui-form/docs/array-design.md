# Dynamic Array Design

本文记录 `gpui-form` dynamic array 的结构设计和当前需要修复的 dirty/default-value 建模问题。

## 当前问题

`FieldArrayStore::append/remove/move/swap/replace_item/replace` 会调用 `bump_revision()`，先把 array store
自身标记为 touched/dirty。但 generated helper 随后调用 `*_refresh_meta()`，该方法从
`FieldMeta::default()` 重新聚合 child row metadata，只保留 child 的 dirty/touched/blurred/validating snapshot。

结果是：如果只发生结构变化，而剩余或新增 child row 仍是 pristine，array 自身刚刚产生的 dirty 会被覆盖。
典型错误场景：

- append 一个空/default row 后，提交值 `Vec<T>` 长度变化，但 `form.meta().is_dirty` 可能仍是 false。
- remove 一个 pristine row 后，提交值变化，但 form 可能被认为 pristine。
- move/swap pristine rows 后，顺序变化，但 child meta 都没有 dirty。
- replace rows 后，如果新 child 都是 pristine，也可能丢失结构 dirty。

这不是 UI 展示问题，而是 array field 对“当前 Vec 是否等于默认 Vec”的建模缺失。

## 设计目标

- array dirty 由两部分组成：结构值变化和 child row 内部字段变化。
- append/remove/move/swap/replace 后，若当前 `Vec<T>` 与默认 `Vec<T>` 不同，array 和 form 必须 dirty。
- 如果用户通过相反操作让当前 `Vec<T>` 回到默认值，array dirty 应恢复 false；不能只记住“曾经发生过结构操作”。
- `reset_items` 表示重新建立默认基线，reset 后当前值等于默认值，dirty 为 false。
- child row 的 touched/blurred/validating 仍参与 array meta 聚合；合法性和错误从 array errors 与 child
  `FormValidationReport` 聚合，不写入 `FieldMeta`。
- 宏只生成类型安全 glue code；array meta 的语义由 `FieldArrayStore` 统一计算。

## 文件和模块结构

| 文件 | 计划 |
| --- | --- |
| `crates/gpui-form/src/core/array.rs` | 扩展 `FieldArrayStore`，持有 array default value 基线，并提供统一的 meta recompute API。 |
| `crates/gpui-form/src/core/meta.rs` | 不新增 array 专用字段；`is_pristine()` 和合法性都保持派生语义。 |
| `crates/gpui-form/src/core/group.rs` | 不改变 `FieldGroupStore` 的职责；继续提供 row 当前 value 和 child meta。 |
| `crates/gpui-form-macros/src/expand/fields.rs` | 初始化 array field 时传入初始 `Vec<T>` 作为 default baseline。 |
| `crates/gpui-form-macros/src/expand/arrays.rs` | array helper 在结构操作后把当前 row values 和 child metas 交给 `FieldArrayStore` recompute，不再手写 meta 聚合。 |
| `crates/gpui-form-macros/src/expand/accessors.rs` | `*_meta()` / `*_required()` API 不变；必要时只适配新的 `FieldArrayStore` 类型参数。 |
| `crates/gpui-form/tests/core.rs` | 增加 `FieldArrayStore` default/current meta 单元测试。 |
| `crates/gpui-form/tests/derive.rs` | 增加 generated array helper 的 structural dirty 回归测试。 |

禁止新增 `mod.rs`。

## 自定义类型结构

推荐把 array store 从“只保存 item 列表”扩展为带 value 基线的结构：

```rust
pub struct FieldArrayStore<Item, Value = Item>
where
    Value: Clone + PartialEq + 'static,
{
    path: FieldPath,
    items: Vec<FieldArrayItem<Item>>,
    id_generator: FormItemIdGenerator,
    array_revision: u64,
    default_values: Vec<Value>,
    meta: FieldMeta,
    required: bool,
    errors: Vec<FieldError>,
    subscriptions: SubscriptionSet,
}
```

`Value` 是提交层 row value，例如 generated `Vec<HeaderInput>` 中的 `HeaderInput`。普通 core 单测可以继续使用
`FieldArrayStore<String>`；generated form 对 nested row 使用
`FieldArrayStore<FieldGroupStore<HeaderInput, HeaderRowFormStore>, HeaderInput>`。

新增或调整的方法：

```rust
impl<Item, Value> FieldArrayStore<Item, Value>
where
    Value: Clone + PartialEq + 'static,
{
    pub fn set_default_values(&mut self, values: Vec<Value>);

    pub fn rebase_default_values(&mut self, values: Vec<Value>);

    pub fn replace_items(&mut self, items: impl IntoIterator<Item = Item>) -> Vec<FormItemId>;

    pub fn reset_items(&mut self, items: impl IntoIterator<Item = Item>) -> Vec<FormItemId>;

    pub fn refresh_meta_from_values(
        &mut self,
        current_values: impl IntoIterator<Item = Value>,
        child_metas: impl IntoIterator<Item = FieldMeta>,
    );
}
```

语义：

- `set_default_values`：初始化或保留旧默认基线时使用，不改变当前 dirty 语义。
- `rebase_default_values`：reset 使用，默认值变成当前 values，并清空结构 dirty。
- `replace_items`：replace helper 使用，重建 rows/ids/subscription 容器并记录一次结构操作，但不改变默认基线。
- `reset_items`：reset helper 使用，重建 rows/ids/subscription 容器并清空结构 revision。
- `refresh_meta_from_values`：唯一负责 array meta 计算。

不推荐只在宏里保存旧 dirty bit 再 OR 回去；那会让 append 后 remove 回默认值的场景仍保持 dirty。

## Meta 计算规则

`refresh_meta_from_values` 按下面规则生成 array field meta：

```text
structural_dirty = current_values != default_values
child_dirty = any(child_meta.is_dirty)

meta.is_dirty = structural_dirty || child_dirty
meta.is_default_value = !structural_dirty
meta.is_touched = array_revision > 0 || any(child_meta.is_touched)
meta.is_blurred = any(child_meta.is_blurred)
meta.is_validating = any(child_meta.is_validating)
```

`meta.is_pristine()` 由 `!meta.is_dirty` 计算。array 是否存在 blocking error 不进入 `FieldMeta`；
generated store 通过 `current_validation_report` 聚合 array-level errors 和 child row reports。

`array_revision` 继续表示结构操作次数。它可以让 append/remove 后 touched 保持 true，即使当前值后来回到默认值。

## 数据流

初始化：

```text
domain Vec<T>
  -> generated child store for each T
  -> FieldGroupStore<T, ChildStore>
  -> FieldArrayStore<FieldGroupStore<T, ChildStore>, T>
       default_values = initial Vec<T>
  -> refresh_meta_from_values(current initial Vec<T>, child metas)
```

结构编辑：

```text
app calls generated headers_append/remove/move/swap/replace
  -> FieldArrayStore mutates items / ids / indexes / subscriptions
  -> macro refreshes row paths
  -> macro collects current row values from FieldGroupStore::value()
  -> macro collects child field_meta()
  -> FieldArrayStore::refresh_meta_from_values(...)
  -> parent form refresh_meta aggregates array meta
```

child row 编辑：

```text
child component emits event
  -> child generated store updates draft/meta
  -> parent observe syncs FieldGroupStore value/meta
  -> FieldArrayStore::refresh_meta_from_values(current row values, child metas)
  -> parent form refresh_meta
```

reset：

```text
app calls generated headers_reset_items(values)
  -> rebuild children and subscriptions
  -> FieldArrayStore::reset_items(groups)
  -> FieldArrayStore::rebase_default_values(values)
  -> refresh_meta_from_values(values, child pristine metas)
  -> parent form dirty false
```

## 所用组件

`gpui-form` 不提供 array row UI 组件。

- Row input 组件继续由 child form fields 决定，例如 `InputState`、`SelectState`、`ComboboxState` 或 app-defined
  `FormComponentBinding`。
- Add/remove/move/swap 按钮由接入 app 使用 `gpui-component::Button` 或自己的 row action 组件实现。
- Array design 只影响 store/meta/subscription，不新增 GPUI element 或 view 组件。

## 全局数据管理

无全局数据管理变更。

- `FieldArrayStore` 是打开的 generated form store 内部状态。
- `FormItemId` 只在当前 form entity 生命周期内稳定，不写入全局 registry。
- 不引入 `Global`、`gpui-store`、跨窗口共享 store 或 app-level cache。

## 数据库变更

无数据库变更。

- `default_values` 是表单打开时的内存基线，不持久化。
- `FormItemId` 不进入 domain output，不写数据库。
- 接入 app 的 DB/config/keychain 写回仍由 app validator 和 submit/save flow 决定。

## 数据获取方式

无网络或数据库读取。

- 初始 `Vec<T>` 来自接入 app 传入的 domain input。
- child row current value 来自 `FieldGroupStore::value()`。
- child metadata 来自 `FieldGroupStore::field_meta()`。
- submit output 继续由 generated `draft()` / `output()` 从当前 row values 组装。

## Icon

无 icon 变更。

- `gpui-form` 不新增 Lucide icon 或 app asset。
- array add/remove/reorder icon 属于接入 app；crate 只提供 helper 和 metadata。

## i18n

无新增 i18n key。

- 结构 dirty 不产生用户可见错误文案。
- array validation error 仍输出 message key + params，由接入 app resolver 翻译。
- row action 文案，例如 Add/Remove/Move，不属于 `gpui-form`。

## 新增依赖

不新增依赖。

- 只使用现有 std、`gpui`、`gpui-component`、proc-macro stack。
- 不引入 diff 库、UUID、serde 或数据库相关依赖。

## 测试计划

Core tests：

- `FieldArrayStore` append pristine/default row 后 structural dirty 为 true。
- remove pristine row 后 structural dirty 为 true。
- move/swap pristine rows 后 dirty 为 true。
- append 后 remove 回原始 `Vec<T>`，dirty 回 false，touched 保持 true。
- `rebase_default_values` 后 dirty false。

Generated derive tests：

- `headers_append(HeaderInput::default())` 让 `headers.meta().is_dirty` 和 `form.meta().is_dirty` 为 true。
- remove pristine row 让 array/form dirty。
- move/swap pristine rows 让 array/form dirty。
- 结构编辑回到默认 values 后 array/form dirty false。
- child row dirty 仍能传播到 array/form。
- reset items 重新建立默认值并清空 dirty。

## 非目标

- 不在本阶段实现通用 array UI。
- 不把 dynamic array row action 下沉为 crate-owned button/icon/layout。
- 不把 array dirty 状态写入 app 数据库或 config。
- 不通过“保留旧 dirty bit”绕过根因；必须基于当前 `Vec<T>` 与默认 `Vec<T>` 的比较建模。
