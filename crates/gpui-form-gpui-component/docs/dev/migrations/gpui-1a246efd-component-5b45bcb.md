# gpui-form-gpui-component：GPUI `View` 与 Combobox value API 迁移

## 1. 状态与范围

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 工作包：`FORM-20`。
- 当前状态：依赖、typed form redesign 与定向 GPUI/component API 迁移均已完成并通过自动化验证。

本计划只负责 `gpui-form-gpui-component` 的 adapter/UI state 生命周期。`gpui-form`、
`gpui-form-macros` 的 typed value、validation 与 attachment API 不在本批次修改。

## 2. 证据与决定

| Current local use | Target upstream fact | Decision |
| --- | --- | --- |
| `IntegerInput<N>` 是 props + `Entity<IntegerInputState<N>>`，实现 `RenderOnce` | 新 `View` 明确支持“props + backing entity”身份；旧 render traits 仍兼容 | 只迁移 `IntegerInput<N>`，不批量改其他 component |
| `FormCombobox` 拥有 native entity + subscriptions | target `ComboboxState::set_selected_values` 通过当前 delegate 投影 value | 保留薄 owning wrapper，禁止恢复 cached delegate/index workaround |
| adapter 的 form->component 投影不应模拟用户事件 | `set_selected_values` 可更新选择且不发 Change/Confirm | 保持无 echo 契约 |

### No change

- 不修改 `gpui-form` public traits、derive macro output、validation policy 或 typed field storage。
- 不把 subscription 放回 form；继续由 owning adapter/newtype 持有。
- 不增加 component options cache；上游 state 能直接更新时由用户通过暴露的 state 操作。
- 不改变 database、network、persistence、icons、i18n 或 app global state。
- 不新增依赖或 feature。

## 3. API 契约

### `IntegerInput<N>`

保留已有 builder、`Focusable`、`Sizable`、`Disableable` 与 `Styled` API，删除其
`RenderOnce` impl，定向实现 target `View`：

```rust,ignore
impl<N: IntegerValue> View for IntegerInput<N> {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // existing NumberInput composition
    }
}
```

- backing entity 是唯一 identity owner。
- 两个不同 state entity 必须拥有独立 element/use_state 空间。
- 同一个 backing entity 不得在同一父路径下作为两个兄弟重复渲染；该状态仍表示同一个 control。
- `View` 迁移不能改变 native state、binding subscription 或事件方向。

### `FormCombobox<T>`

- struct 继续只持 native `Entity<ComboboxState<...>>` 与 subscriptions。
- form -> component 同步每次调用 state 当前 delegate 的 `set_selected_values`。
- items reorder/add/remove 后，后续 form projection 以新 delegate 为准。
- draft value 在当前 catalog 缺失时显示无选择；不得自动 fallback 到第一项。
- programmatic projection 不发 `Change`/`Confirm`，避免写回 echo。
- user selection 仍通过 native event 更新 typed form field。

## 4. FORM-20 实施包

**Files**

- 修改 `crates/gpui-form-gpui-component/src/integer_input.rs`。
- 保留并验证 `crates/gpui-form-gpui-component/src/combobox.rs`。
- 扩充 `crates/gpui-form-gpui-component/tests/adapters.rs`。

**Implementation flow**

1. 将 `IntegerInput<N>` 从 `RenderOnce` 改为 `View`，identity 返回 backing entity ID。
2. 保持现有 `NumberInput` composition 与 builder/trait 行为。
3. 添加两个独立 integer state 的身份与交互隔离测试。
4. 审核 Combobox binding 只使用 target `set_selected_values`，删除任何残留旧 delegate capture/index projection。
5. 扩充 reorder/remove/external form replace 场景，证明始终使用当前 delegate且无事件 echo。

**Errors and lifecycle**

- 创建 native state/binding 的错误类型和返回路径保持不变。
- subscriptions 随 adapter/newtype drop 取消；form 可以继续被其他页面或组件使用。
- 无 async task、retry、partial output 或 shutdown。
- component entity update 内不得同步 re-enter 同一 entity；现有单向 event boundary 保持。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| View identity | `tests/adapters.rs` | `integer_inputs_use_backing_entity_identity` | 两个独立 state identity/use_state/interaction 不碰撞 |
| typed integer flow | 同上 | existing integer binding test | component <-> form 仍为 typed `N`，无 string draft |
| current delegate | 同上 | `select_and_combobox_bind_values_and_use_current_items` | reorder/remove 后选择正确 |
| missing value | 同上 | `combobox_missing_form_value_does_not_fallback` | 无选择且 form value 不被覆盖 |
| no echo | 同上 | `combobox_programmatic_projection_does_not_emit_user_change` | form projection 不触发 user callback/writeback |

**Validation**

```bash
cargo fmt --all -- --check
cargo test --locked -p gpui-form-gpui-component --test adapters
cargo test --locked -p gpui-form-gpui-component
git diff --check
```

**Done condition**

- `IntegerInput<N>` 使用 backing entity identity；Combobox 只依赖 target current-delegate API；
  typed value、subscription ownership 与无 echo 契约均通过测试。

## 5. 交接审计

- [x] 只有一个明确 `View` 候选，没有机械迁移所有 render type。
- [x] Combobox catalog 更新与外部 form projection 行为已确定。
- [x] `gpui-form` / macros 不需要本批次改动。
- [x] 所有生命周期和 No change surface 已记录。
