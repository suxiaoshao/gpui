# GPUI `0.2.2@1a246efd` / gpui-component `0.5.2@5b45bcb` 迁移总计划

## 1. 迁移身份与状态

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- 文档位置：`docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md`。
- 当前分支：`codex/175-jaco-shortcut-temporary-window`。
- 实现基线：`6351898 refactor: redesign typed form state and bindings`。
- 解析后的 crate versions：GPUI `0.2.2`；gpui-component `0.5.2`。两个 Git package 的
  crate version 在本区间未变化，因此迁移身份必须由 source SHA 区分。
- GPUI source：
  `1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：
  `c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- [已完成] `Cargo.lock` 已锁定目标 SHA，root 与 gpui-component 使用同一个 canonical Zed Git source。
- [已完成] 当前 target 可实施的依赖、运行时、主题生成、JSON 主题、组件复用与应用接入迁移已经完成，
  并通过 workspace build/test/clippy 自动门。
- [未执行] 按本轮约定，不执行实际 UI/Computer Use 验收，也不打包；这些结果不能由自动测试代替。
- [后继阻断] gpui-component `5b45bcb` 的 rendered Markdown 会缓存 parse-time highlight theme。
  修复已在本地上游工作区实现并通过定向测试，但未提交、发布或纳入当前依赖；主题切换验收仍须等待
  新 upstream target SHA，并创建后继迁移批次。

本文件是本次迁移的总协调文档，只定义跨 package 的依赖关系、共享约束、发布门和子计划入口。
各 app/crate 的文件、类型、API 和测试契约以对应子计划为唯一来源。

### 本轮执行证据（2026-07-21）

- `cargo build --workspace`：通过。
- `cargo test --workspace`：通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`：通过。
- dependency tree：仅解析到 GPUI `1a246efd` 与 gpui-component `5b45bcb2`。
- 实际 UI/Computer Use、打包和三平台 CI：未执行。

## 2. 子计划索引

| Owner | 工作包 | 范围 | 子计划 |
| --- | --- | --- | --- |
| workspace（当前 target） | ROOT-00 | 单一 dependency source 与 Rust 支持基线 | [workspace.md](workspace.md) |
| workspace（后继 target） | ROOT-80 | 跨包验证与三平台发布门 | 新 SHA 冻结后创建独立 workspace 计划；继承 [ROOT-80 contract](workspace.md) |
| shared evidence | EVIDENCE | 完整依赖区间、上游变更、features/MSRV/platform、非目标 | [dependency-evidence.md](dependency-evidence.md) |
| gpui-component upstream | UPSTREAM-TEXT-15 | TextView CodeBlock 改为 render-time current theme 与 theme-aware styles cache | [upstream-text-theme.md](upstream-text-theme.md) |
| `crates/app-theme` | THEME-10 | M3 颜色 role/button state layer、既有颜色兼容，以及 editor/Markdown 共用的代码内容 palette | [app-theme 子计划](../../../../crates/app-theme/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `crates/gpui-form-gpui-component` | FORM-20 | `IntegerInput<N>: View` 与新版 Combobox value API 验证 | [表单组件适配子计划](../../../../crates/gpui-form-gpui-component/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/jaco`（当前 target） | JACO-WINDOW-10..JACO-COMPONENT-50；JACO-MARKDOWN-55 非主题证据 | window/timer/layout、JSON preset、ThemeToken、List/picker/scroll/input，以及流式 Markdown/language/plain fallback | [Jaco 子计划](../../../../app/jaco/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/jaco`（后继 target） | JACO-MARKDOWN-55 current-theme 子门；JACO-VERIFY-60 | 既有 TextView 随当前主题刷新与 Jaco 最终发布门 | 新 SHA 冻结后创建独立 Jaco 计划；当前入口见 [UPSTREAM-TEXT-15](upstream-text-theme.md) |
| `app/feiwen` | FEIWEN-10..40 | owned titlebar、官方 TitleBar/Progress、ThemeToken、URL/scroll/platform | [Feiwen 子计划](../../../../app/feiwen/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/http-client` | HTTP-10..20 | URL content type 与升级后的 request UI 回归 | [HTTP Client 子计划](../../../../app/http-client/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/novel-download` | NOVEL-10..20 | workspace ThemeToken；明确保留 crawler timer | [Novel Download 子计划](../../../../app/novel-download/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| repo-local skills | SKILL-70 | GPUI strict mirror 与 gpui-component consumer docs/rules | [skill-sync.md](skill-sync.md) |

## 3. 唯一执行顺序

```text
ROOT-00
├── THEME-10 ─────────────┬── JACO-WINDOW-10..JACO-COMPONENT-50
│                         │    └── JACO-MARKDOWN-55 (non-theme evidence)
│                         └── FEIWEN-10..40
├── UPSTREAM-TEXT-15 ─────────> new gpui-component SHA / successor migration
│                                └──> successor JACO-MARKDOWN / SKILL / ROOT release gates
├── FORM-20 ─────────────────> JACO-WINDOW-10..JACO-COMPONENT-50
├── HTTP-10..20
└── NOVEL-10..20

THEME-10 + FORM-20 + JACO-WINDOW-10..JACO-COMPONENT-50
  + JACO-MARKDOWN-55 (non-theme evidence) + FEIWEN + HTTP + NOVEL
  -> SKILL-70 (5b45bcb snapshot + known-blocker rule)

ROOT-80 is blocked for this target and is carried into the successor migration.
```

- `ROOT-00` 只确认依赖 source、锁文件和工具链基线，不再次无目标升级依赖。
- `UPSTREAM-TEXT-15` 是当前 `5b45bcb` target 的 release blocker。修复落地后 source SHA
  会变化，必须新建 hash-specific 后继迁移，不能直接把当前文档改写成已包含修复。
- `THEME-10` 必须先固定全部 generated theme 语义，包括 colors、component tokens、
  `HighlightThemeStyle` 以及 editor/Markdown 共用的代码内容 palette；使用 `app-theme` 的
  Jaco/Feiwen 随后只做消费和视觉验收。Novel Download 只消费 gpui-component token，不依赖该 crate。
- `FORM-20` 先验证新 `View` 与 Combobox 契约，Jaco 再进行完整 picker/form 回归。
- `SKILL-70` 只同步当前 target 的真实 API，并记录 TextView blocker；后继 target 必须再次同步，
  不能让当前 skill 文档预告尚不存在的 API。
- `ROOT-80` 是唯一发布门；它在 `5b45bcb` 上不可达，必须由包含修复的新 hash-specific
  迁移批次继承并完成。当前批次只能保持 blocked，不能标为完成。

## 4. 跨 package 冻结契约

1. `gpui`、`gpui_macros`、`gpui_platform` 只能解析为同一个 Zed source SHA；禁止通过本地类型转换掩盖两套 GPUI 类型宇宙。
2. 本仓支持与验证基线调整为 Rust `1.95+`，但不宣称这是 GPUI 上游正式 MSRV；CI 继续使用 stable。
3. 所有调用 `Window::start_window_move` 的自绘标题栏窗口必须设置
   `WindowOptions::app_owns_titlebar_drag = true`。
4. 可渲染背景使用 `Theme.tokens`；文字、边框、caret、图标和颜色计算继续使用 `Hsla`。
   token 透明度必须写在 `.background.opacity(...)`，禁止经 `ThemeToken::Deref` 丢失 gradient。
5. Jaco 的 `ListState` confirm/cancel 回调继续使用 `window.defer`；上游焦点或 popover 修复不替代本地重入边界。
6. JSON themes 必须完整同步上游目标 SHA `themes/` 目录的 22 个文件，不能只补 Aurora；同步后
   按 theme variant 比较 active/inactive tab：上游已区分时采用上游五个 tab 键，仍未区分时
   才重放该 variant 已确认的 Jaco tab overlay，持久化 theme ID 不迁移。
7. `.agents/skills/gpui` 是 strict upstream mirror，不能混入 repo-local 说明；消费规则写入 `gpui-component-usage`。
8. `crates/app-theme` 是 generated Material theme 的唯一 owner：editor 与 rendered Markdown
   共用 plain/muted/syntax 内容 palette，但各自保留 editor chrome 与 code-block surface；
   应用不得生成、覆盖或同步第二套代码配色。
9. Material 3 参考范围只包含语义颜色与 state layer；组件 border、radius、padding、size、
   typography、shadow/elevation、动效和 focus ring 由 gpui-component 负责，应用不得按 Android
   组件参数二次覆盖。
10. [当前缺口 / 后继发布契约] TextView code-block syntax 必须在 render 时使用当前 active
   highlight theme，并按 theme identity 失效 styles cache；`5b45bcb` 尚不满足。禁止由 Jaco
   监听主题、遍历 TextView 或用同值 `set_text` 伪造更新。

## 5. 验收责任汇总

| Surface | Owner | 自动证据 | 人工/Computer Use 证据 |
| --- | --- | --- | --- |
| dependency/source/features | [ROOT-00](workspace.md) | cargo tree、locked build | N/A |
| Material semantics | THEME-10 | app-theme role/state、共享代码 palette 与双 surface invariant tests | light/dark editor 与 Markdown 代码块 |
| TextView theme lifecycle | UPSTREAM-TEXT-15 | upstream current-theme/cache tests | 既有消息只切换主题即可更新 syntax |
| form component adapter | FORM-20 | adapter tests | integer/combobox interaction |
| Jaco current-target runtime/theme/list/focus | JACO-WINDOW-10..JACO-COMPONENT-50 + JACO-MARKDOWN-55 非主题证据 | Jaco focused tests | main/settings/about/temporary/screenshot、Aurora、picker、streaming/fallback |
| Jaco successor release | JACO-MARKDOWN-55 current-theme 子门 + JACO-VERIFY-60 | successor dependency gate + Jaco/upstream tests | 既有消息主题切换与完整 Jaco matrix |
| Feiwen titlebar/progress/theme | FEIWEN-10..40 | Feiwen tests + CI | macOS/Linux/Windows titlebar、progress |
| HTTP Client request UI | HTTP-10..20 | package tests | URL/request/scroll smoke |
| Novel workspace | NOVEL-10..20 | package tests + source gate | light/dark workspace |
| skills/docs | SKILL-70 | recursive diff、link/residual gates | rendered Markdown review |
| platform | [ROOT-80](workspace.md) | macOS/Linux/Windows CI | Linux/Windows startup smoke |

### Release blockers

以下任一项失败都阻止完成本迁移：

- Cargo graph 出现两份 Zed source/SHA；
- 任一自绘标题栏窗口缺失 owned-drag flag；
- Aurora 在解析、设置预览或应用背景中退化为代表色；
- picker/temporary/completion confirm/cancel 再次发生 `ListState` 重入 panic；
- 临时窗口搜索焦点或 Up/Down/Enter/Escape 导航回归；
- generated Material theme 的 editor 与 Markdown 代码内容色分叉、任一场景对比度不达标，
  或应用重新维护局部 code palette；
- 当前 gpui-component target 仍把 highlight theme 固定在 Markdown parse 结果中，导致主题切换后
  background 与 syntax 来自不同主题；
- Taffy/root-fill/Scrollable 变化破坏关键窗口、dialog、列表或 composer；
- macOS、Linux、Windows CI 任一失败，或 Feiwen 平台标题栏 smoke 未通过。

详细自动命令、平台 smoke 和完成证据由 [workspace.md](workspace.md) 唯一维护；包计划只维护各自的定向测试。

## 6. 执行交接审计

- [x] 每个 package 有独立、带目标 hash 的实施计划。
- [x] 根文档只保留总顺序、共享契约、发布门和引用。
- [x] 共享依赖证据和 repo-level skill 同步已从 package 实现中分离。
- [x] 新迁移将创建新的 target ID，不覆盖本批次。
- [x] 每个子计划负责自己的 exact files、API contract、测试和 No change surfaces。
