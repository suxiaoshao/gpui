# gpui-form 开发入口

状态：第一阶段 form runtime、derive、group、array、validation/transform 和 submit 已落地；breaking
architecture refactor 与 workspace 迁移已完成。目标消费者 API 见 [`../README.md`](../README.md)，实施记录见
[`external-state-synchronization-plan.md`](external-state-synchronization-plan.md)。

## 当前结论

- form 是 raw/typed draft、baseline、dirty/touched/errors、validation、transform 和 submit 的唯一 owner；
- application 是 component options/config 和 product fallback policy 的 owner；
- component entity 是 focus/query/highlight/scroll/IME/task 的 owner；
- component-specific adapter 只创建 genuine user value event 与 programmatic form value mirror 的订阅，调用方用
  core `SubscriptionSet` 持有生命周期；
- submit 只读取 form-owned draft；
- component config/catalog 更新不 replace、rebase 或 hydrate form；
- derive 保留 generated field enum，页面级变化统一使用 runtime `FormStoreEvent<Field>`；
- dynamic required 使用不带 `Window` 的纯 form setter，不进入 component binding；
- group/array 使用 `group(store = ...)` / `array(store = ...)` 结构属性；
- old state-owning binding API 无兼容层，workspace callers 已同步迁移并删除旧实现。

## 文档结构

| 文档 | 职责 |
| --- | --- |
| `README.md` | docs 索引和文档状态。 |
| `external-state-synchronization-plan.md` | breaking refactor 的完整计划、work packages 和验收标准。 |
| `binding-architecture.md` | 三通道 owner、field handle、caller-owned subscriptions 和事件方向。 |
| `macro-generation-boundary.md` | derive 只生成 form-domain glue 的目标边界。 |
| `number-input-design.md` | raw number draft、codec 与 component policy 分离。 |
| `array-design.md` | dynamic array 结构、stable row identity 和 dirty/default 语义。 |
| `validation-routing.md` | validation report 到 leaf/group/array 的路径路由。 |
| `validation-pipeline-strengthening-plan.md` | required/custom context/transform/array error pipeline。 |
| `meta-and-submit-state.md` | field/form meta 保存事实、派生查询和 final report。 |
| `submit-handler-design.md` | sync/async submit closure、task ownership 和 lifecycle。 |
| `phase-1-development-plan.md` | 历史实施记录；不是当前 API 或新代码参考。 |

## 已完成实施顺序

1. core 引入纯 `FieldCodec` / `DraftFieldStore` / `FormFieldHandle`；
2. derive 删除 component construction，生成 codec leaf、typed handle、field identity、generic event glue 和纯
   required setter；
3. adapter crate 实现 app-created state 的 component-specific bind functions，每次原子返回 core
   `SubscriptionSet` 供调用方合并；
4. workspace callers 同步迁移并删除旧 API；
5. Jaco catalog/store 和 final submit resolver 按独立 owner 接入。

每一步必须先让 form 在无 mounted component 时能够完整 validation/submit，再验证 UI mirror。不得通过 fallback、
额外 mirror 或无条件 defer 掩盖 ownership/event-direction 问题。

## 跨主题边界

- `gpui-form` 不依赖 `gpui-component`、`gpui-store`、DB、keychain、network 或 app config；
- `gpui-form-gpui-component` 不加载 catalog、不选择 fallback、不参与 submit；
- `gpui-store` 发布 app-owned immutable snapshots，不向 form 自动写值；
- labels/descriptions/placeholders/icons/i18n 由 app render 层管理；
- app 在明确丢弃 dirty draft 时才调用 `replace_from_value`。

## 验证基线

```bash
cargo fmt --all
cargo test -p gpui-form
cargo test -p gpui-form-macros
cargo test -p gpui-form-gpui-component
cargo check -p jaco
cargo test -p jaco --no-run
cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component \
  --all-targets --all-features -- -D warnings
git diff --check
```
