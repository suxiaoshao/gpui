# GPUI `0.2.2@1a246efd` / gpui-component `0.5.2@5b45bcb` 迁移总计划

此页保留该固定版本迁移的共享结论与历史证据；当前版本以根 manifest 为准。Jaco 专用工作包与发布验收已退役，不延续到现役应用。

## 1. 迁移身份与状态

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- 文档位置：`docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md`。
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
| `app/feiwen` | FEIWEN-10..40 | owned titlebar、官方 TitleBar/Progress、ThemeToken、URL/scroll/platform | [Feiwen 子计划](../../../../app/feiwen/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/http-client` | HTTP-10..20 | URL content type 与升级后的 request UI 回归 | [HTTP Client 子计划](../../../../app/http-client/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| `app/novel-download` | NOVEL-10..20 | workspace ThemeToken；明确保留 crawler timer | [Novel Download 子计划](../../../../app/novel-download/docs/dev/migrations/gpui-1a246efd-component-5b45bcb.md) |
| repo-local skills | SKILL-70 | GPUI strict mirror 与 gpui-component consumer docs/rules | [skill-sync.md](skill-sync.md) |

## 3. 共享迁移顺序

workspace 类型来源先统一，再验证 app-theme 和 Form，最后由现役应用消费；共享 Markdown 主题缓存问题由组件层负责。原 Jaco 工作包与专用发布门已退役。

## 4. 跨 package 冻结契约

1. `gpui`、`gpui_macros`、`gpui_platform` 只能解析为同一个 Zed source SHA；禁止通过本地类型转换掩盖两套 GPUI 类型宇宙。
2. 本仓支持与验证基线调整为 Rust `1.95+`，但不宣称这是 GPUI 上游正式 MSRV；CI 继续使用 stable。
3. 所有调用 `Window::start_window_move` 的自绘标题栏窗口必须设置
   `WindowOptions::app_owns_titlebar_drag = true`。
4. 可渲染背景使用 `Theme.tokens`；文字、边框、caret、图标和颜色计算继续使用 `Hsla`。
   token 透明度必须写在 `.background.opacity(...)`，禁止经 `ThemeToken::Deref` 丢失 gradient。
5. 应用的 `ListState` confirm/cancel 回调继续使用 `window.defer`；上游焦点或 popover 修复不替代本地重入边界。
6. 共享主题生成与应用 JSON 预设保持原所有权，持久化 theme ID 不迁移。
7. `.agents/skills/gpui` 是 strict upstream mirror，不能混入 repo-local 说明；消费规则写入 `gpui-component-usage`。
8. `crates/app-theme` 是 generated Material theme 的唯一 owner：editor 与 rendered Markdown
   共用 plain/muted/syntax 内容 palette，但各自保留 editor chrome 与 code-block surface；
   应用不得生成、覆盖或同步第二套代码配色。
9. Material 3 参考范围只包含语义颜色与 state layer；组件 border、radius、padding、size、
   typography、shadow/elevation、动效和 focus ring 由 gpui-component 负责，应用不得按 Android
   组件参数二次覆盖。
10. [当前缺口 / 后继发布契约] TextView code-block syntax 必须在 render 时使用当前 active
   highlight theme，并按 theme identity 失效 styles cache；`5b45bcb` 尚不满足。禁止由应用
   监听主题、遍历 TextView 或用同值 `set_text` 伪造更新。

## 5. 验收责任汇总

| Surface | Owner | 自动证据 | 人工/Computer Use 证据 |
| --- | --- | --- | --- |
| dependency/source/features | [ROOT-00](workspace.md) | cargo tree、locked build | N/A |
| Material semantics | THEME-10 | app-theme role/state、共享代码 palette 与双 surface invariant tests | light/dark editor 与 Markdown 代码块 |
| TextView theme lifecycle | UPSTREAM-TEXT-15 | upstream current-theme/cache tests | 既有消息只切换主题即可更新 syntax |
| form component adapter | FORM-20 | adapter tests | integer/combobox interaction |
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
