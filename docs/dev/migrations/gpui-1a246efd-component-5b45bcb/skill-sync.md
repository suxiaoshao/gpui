# GPUI `1a246efd` / gpui-component `5b45bcb` skill 同步计划

## 1. 状态与范围

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 前置条件：本批次可实施的 package 子计划完成；本计划只同步标题固定的 `5b45bcb` API、
  文档与已确认的上游阻断，不把未来修复伪装成当前 target 能力。
- `UPSTREAM-TEXT-15` 合入后，包含修复的新 gpui-component SHA 必须建立后继迁移批次，
  并在该批次重新执行 skill diff/sync；不能在本文件原地更换 vendoring source。
- 范围仅限 `.agents/skills/gpui` 与 `.agents/skills/gpui-component-usage`。
- 当前状态：strict GPUI mirror 与目标 checkout byte-identical；gpui-component 文档 snapshot、
  attribution 和 repo-owned 消费规则已同步并通过定向 diff/residual 验证。

## 2. 来源边界

| Local path | Ownership | Sync policy |
| --- | --- | --- |
| `.agents/skills/gpui/**` | upstream mirror | 从 gpui-component target checkout 的 `skills/gpui/**` 整体复制并 byte diff；禁止本地编辑 |
| `gpui-component-usage/references/components/*.md`（除 `index.md`） | vendored upstream docs | 只同步 target SHA 实际变化的组件文档，保持第三方 license/attribution |
| `gpui-component-usage/SKILL.md` | repo-owned consumer workflow | 人工合并最终使用规则，不用上游 contributor skill 覆盖 |
| `references/components/index.md` | repo-owned task index | 人工更新能力索引 |
| `references/rules/*.md` | repo-owned rules | 写入本 workspace 的主题、状态、重入和组件复用规则 |

若 strict mirror 需要本地说明，说明写在本文或 `gpui-app-development`，不能改 `.agents/skills/gpui` 镜像内容。

## 3. 精确文件

整体同步：

- `.agents/skills/gpui/**`。

同步 target SHA 发生内容变化的 vendored component docs：

- `.agents/skills/gpui-component-usage/references/components/chart.md`；
- `.../combobox.md`；
- `.../hover-card.md`；
- `.../input.md`；
- `.../popover.md`；
- `.../settings.md`。

更新 repo-owned 文档：

- `.agents/skills/gpui-component-usage/references/third-party/gpui-component-docs.md`；
- `.agents/skills/gpui-component-usage/SKILL.md`；
- `.../references/components/index.md`；
- `.../references/rules/theme-and-size.md`；
- `.../references/rules/state-and-interaction.md`；
- `.../references/rules/primitives.md`。

## 4. 最终规则契约

repo-owned consumer layer 必须记录：

- `ThemeToken` 同时包含代表 `Hsla` 和可渲染 `Background`；背景 opacity 写在 `.background`；
- generated theme 语义与代码 palette 由共享主题层（当前为 `crates/app-theme`）或上游主题实现拥有；
  应用只消费 `ActiveTheme`，不得复制 palette、缓存第二份颜色或对 `HighlightThemeStyle` 二次赋值；
- Input editor 与 rendered Markdown `CodeBlock` 共用 `HighlightThemeStyle.syntax` 及 plain/muted
  内容语义，但 editor chrome 与 Markdown `Theme.tokens.muted` surface 可以不同；
- [当前 blocker / successor contract] rendered Markdown `CodeBlock` 最终必须在 render 时读取
  当前 active highlight theme，styles cache 必须按 theme identity 失效；`5b45bcb` 尚未提供该
  能力，当前 skill 只记录 blocker 与禁止 app workaround，不把它描述为可调用的现有 API；
  应用不得通过主题订阅、同值 `set_text` 或 reparse 刷新代码高亮；
- 当前目标版本若未消费某个 editor color 字段，应优先修复 gpui-component 上游；
  不以 app wrapper 建立第三个主题来源；
- `Scrollable` 的 source element、gap、padding、size 和 wrapper ownership；
- `ComboboxState::set_selected_values` 使用当前 delegate 做 value projection，不发用户 Change/Confirm；
- `ListState` delegate callback 不得同步 update 同一 entity，owner mutation 使用 `window.defer`；
- 自绘 titlebar window 的 `app_owns_titlebar_drag` 契约；
- `TitleBar`、`Progress`、`ListItem` 的优先复用边界；
- `InputContentType::Password` / `Url` 只用于已有明确语义的输入。

## 5. 实施流程

1. 从固定 `5b45bcb` target checkout 整体同步 `.agents/skills/gpui`。
2. 同步六份变更的 component docs，并更新 attribution SHA。
3. 根据已经落地的代码人工更新 repo-owned SKILL/index/rules。
4. 检查相对链接、Navigation 与 progressive-loading 路径。
5. 执行 recursive diff 与 stale API residual scan。

No runtime lifecycle、database、network、icons、i18n 或 dependency graph change。

## 6. 验证与完成条件

```bash
diff -ru \
  /Users/sushao/.cargo/git/checkouts/gpui-component-95ce574d8a0da8b8/5b45bcb/skills/gpui \
  .agents/skills/gpui
rg -n "c36b0c6|tokens\.[a-zA-Z0-9_]+\.opacity" \
  .agents/skills/gpui-component-usage
```

- mirror diff 预期无输出。
- strict mirror 中的 timer 示例以目标上游原文为准，不用 repo-owned residual gate 改写或过滤。
- component vendored docs 与 target SHA 对应源文件一致。
- repo-owned rules 与最终实现一致，不保留旧 SHA、错误 timer 或会丢 gradient 的 opacity 示例。
- repo-owned rules 明确 TextView current-theme/cache owner，不包含任何 app-side refresh workaround。
- attribution 与 mirror diff 仍指向 `5b45bcb`；未来 SHA 的同步证据只能写入后继迁移文档。
- 所有相对链接存在，且没有把本地规则写入 upstream mirror。
