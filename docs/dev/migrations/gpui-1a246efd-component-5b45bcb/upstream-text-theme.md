# gpui-component：TextView 代码高亮主题生命周期上游修复

## 1. 状态与范围

- 发现版本：gpui-component
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 关联迁移：`gpui-1a246efd-component-5b45bcb`。
- 工作包：`UPSTREAM-TEXT-15`。
- 当前状态：修复已在本地 gpui-component 工作区实现，并通过 `text::` 测试与 crate check；
  未创建 issue、未提交、未推送，也未纳入当前 workspace 的 `5b45bcb` 依赖，因此主题切换验收仍属于后继迁移。
- 实现 owner：`longbridge/gpui-component` 的 `TextView` / Markdown / highlighter 链路，
  不是 Jaco 或 `crates/app-theme`。

本计划只修复 rendered Markdown `CodeBlock` 对高亮主题的生命周期与 styles cache。
Material palette 生成仍由 `crates/app-theme` 负责，应用继续只消费 `ActiveTheme`。

该修复落地后 gpui-component source SHA 必然变化。按照本仓迁移文档规则，得到目标 SHA 后必须
新建对应 `gpui-<gpui-sha>-component-<new-sha>` 迁移批次并引用本计划；不能改写当前文件名或声称
`5b45bcb` 已包含修复。

## 2. 当前证据

- `crates/ui/src/text/state.rs::increment_update` 仅在初始内容、`set_text`、`push_str` 或
  Markdown extension revision 变化时，把当时的 `cx.theme().highlight_theme` 克隆进
  `UpdateOptions`。
- `set_text` 遇到相同文本会直接返回，因此布局重算不能借同值 `set_text` 刷新主题。
- `crates/ui/src/text/format/markdown.rs` 在 parse 阶段把该 theme 传给 `CodeBlock::new`。
- `crates/ui/src/text/node.rs::CodeBlock` 长期保存这份 `Arc<HighlightTheme>`，并把计算后的
  styles 缓存在与 theme 无关的 `Option<Vec<_>>` 中。
- 主题切换后，CodeBlock background 会在 render 阶段读取新的 `Theme.tokens.muted`，但 syntax
  styles 仍来自旧 theme；Input editor 则在每次 render 读取当前 active highlight theme。
- 上游源码已经留下“highlight theme 应移到 render stage”的 TODO；本计划采用该职责边界。

## 3. 已冻结设计

主题不再进入 Markdown parse 数据：

```text
TextViewState source
  -> Markdown parse
  -> CodeBlock { lang, text, theme-aware styles cache }

CodeBlock::render(current ActiveTheme)
  -> styles(current highlight theme)
       ├── same Arc identity: reuse cached styles
       └── changed Arc identity: recompute styles and replace cache
```

### 核心类型与 API

```rust,ignore
struct CachedCodeStyles {
    highlight_theme: Arc<HighlightTheme>,
    styles: Vec<(Range<usize>, HighlightStyle)>,
}

pub struct CodeBlock {
    lang: Option<SharedString>,
    styles: Arc<Mutex<Option<CachedCodeStyles>>>,
    state: Arc<Mutex<InlineState>>,
    pub span: Option<Span>,
}

impl CodeBlock {
    fn styles(
        &self,
        highlight_theme: &Arc<HighlightTheme>,
    ) -> Vec<(Range<usize>, HighlightStyle)>;
}
```

- 删除 `UpdateOptions.highlight_theme`。
- 删除 `format::markdown::parse`、`ast_to_document`、`ast_to_node` 与 `CodeBlock::new` 的
  `highlight_theme` 参数。
- `CodeBlock::render` 从当前 `cx.theme().highlight_theme` 取得 `Arc` 并传给 `styles`。
- cache hit 使用 `Arc::ptr_eq` 判断 active theme identity；`HighlightTheme` 通过不可变 `Arc`
  安装，主题切换会替换该 Arc，因此不需要 app-owned generation counter。
- theme identity 改变时只重新计算 syntax styles，不重新 parse Markdown、不修改 TextView source
  或 revision，也不让应用主动调用 `set_text`。
- thread-local `CODE_BLOCK_HIGHLIGHTERS` 继续只缓存 parser/highlighter state；它不拥有主题。

## 4. UPSTREAM-TEXT-15 实施包

**Prerequisites**

- 以 gpui-component `5b45bcb` 复现同一 `TextViewState` 在主题切换后 background 更新而 syntax
  styles 不更新。
- 按 gpui-component 的 issue/PR 模板记录通用复现，不暴露 Jaco 内部实现。

**Files（upstream repository）**

- 修改 `crates/ui/src/text/state.rs`。
- 修改 `crates/ui/src/text/format/markdown.rs`。
- 修改 `crates/ui/src/text/node.rs`。
- 在对应 upstream test module 增加主题切换与 cache invalidation tests。

**Implementation flow**

1. 从 `UpdateOptions` 和 Markdown parser 递归调用链删除 highlight theme 参数。
2. 从 `CodeBlock` 删除 parse-time `highlight_theme` 字段，把 styles cache 改为
   `CachedCodeStyles`。
3. 在 `CodeBlock::render` 读取当前 active highlight theme；同一 theme 复用 styles，theme Arc
   改变则使用同一语法树重新计算并替换 cache。
4. 保持 `Theme.tokens.muted` background、selection、copy、actions 与 code text state 原样。
5. 增加同文本、同 language、只切换 theme 的定向测试，证明无需 `set_text` 或 reparse。

**Errors and lifecycle**

- poisoned cache mutex 继续返回空 styles/plain fallback，不引入 panic 或 retry。
- unknown/no-language 继续完整显示 plain text；主题切换不改变 language registry 行为。
- 旧 background parse task 仍受现有 revision gate 管理；本修复不增加异步 task、订阅或 app 回调。

**Tests**

| Requirement | Proposed test | Assertions |
| --- | --- | --- |
| theme-aware cache | `code_block_recomputes_styles_when_highlight_theme_changes` | 同文本/lang 下，theme Arc 改变后颜色来自新 theme；同 Arc 复用 cache |
| render-time theme | `text_view_code_block_uses_current_theme_without_text_mutation` | 不调用 `set_text`，主题切换后的下一次 render 使用新 syntax |
| source stability | `theme_change_does_not_reparse_or_mutate_markdown_source` | source、revision、block structure 不变 |
| fallback | 保留并扩充 unknown/no-language tests | 文本完整、无 panic、无错误 style |

**Upstream validation**

```bash
cargo fmt --all -- --check
cargo test -p gpui-component text
cargo clippy -p gpui-component --all-targets --all-features -- -D warnings
git diff --check
```

## 5. Workspace 集成门

1. upstream PR 合入后记录完整 commit SHA。
2. 新建以该 SHA 命名的 workspace 迁移批次；更新依赖、lockfile、上游证据和 skill vendoring
   来源，不能覆盖当前 `5b45bcb` 批次。
3. 在新 target 上运行 gpui-component upstream tests、`app-theme` 双 surface invariant tests，
   以及 Jaco `JACO-MARKDOWN-55` 的主题切换 smoke。
4. 只有旧消息代码块在不修改 source 的情况下随 `ActiveTheme` 更新 syntax，才能解除 release blocker。

## 6. No change 与完成条件

- No change：Markdown AST、TextView source/revision、streaming append 合并、selection/copy/actions、
  language registry、unknown-language fallback、code-block background token。
- 禁止方案：Jaco 监听主题后遍历 TextView、用同值 `set_text` 强制 reparse、维护 generation
  counter，或缓存第二份 code palette。
- `UPSTREAM-TEXT-15` 完成条件：上游 PR/API/定向测试已落地，并可取得包含修复的完整 commit SHA。
- workspace handoff 完成条件：由后继 `ROOT-00` 锁定该 SHA、创建新的 hash-specific 迁移批次；
  当前 `5b45bcb` 文档只保留已识别 blocker 的历史证据。
