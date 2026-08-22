# Issue #205 GPUI skill 同步计划

## 文档状态

- 状态：`Done`
- 父计划：[Issue #205 全 workspace 依赖升级计划](dependency-upgrade-plan.md)
- 关联 issue：[#205](https://github.com/suxiaoshao/gpui/issues/205)
- 基线证据日期：`2026-08-20`
- 实施状态：skill、vendored documentation、repo-local routing 与验证证据已完成；父依赖升级仍按父计划推进

本文是 Issue #205 依赖升级中 GPUI、gpui-component 和 gpui-base 相关 repo-local skill
与文档产物的唯一实施细则。父计划拥有 target SHA、总顺序和完成状态；本文拥有文件边界、
同步方式和验证门禁。

## 目标

1. 证明 `.agents/skills/gpui/**` 是 `longbridge/gpui-component` 目标提交内 `skills/gpui/**`
   的完整镜像，而不是从 Zed 手工编写的副本。
2. 将 `gpui-component-usage` 中的上游 component docs 快照升级到 target，同时保留本仓自有
   组件选择、状态所有权、组合和无障碍规则。
3. 为 Lestty 的 `gpui-base`-only 边界新建独立 repo-local skill，不误导它使用完整
   `gpui-component`。
4. 使后续 agent 能从 skill 入口路由到 target 实际 API，且每个复制产物都有可审计的
   source path、commit 和 license。

## 非目标

- 不在 `.agents/skills/gpui/**` 中手工追加 Zed 新 API、本仓约定或 Issue #205 迁移结论。
- 不用上游 `skills/gpui-component/**` 直接覆盖本仓的 `gpui-component-usage`。
- 不把 `website/base/**` 声称为上游 `gpui-base` skill；target 中不存在该 skill。
- 不在本工作包进行 Jaco Command、TitleBar、AppMenuBar 或 Lestty UI 的源码迁移。
- 不为每个 app/crate 复制一份 skill 同步计划。

## 权威源与基线

| Artifact family | Source repository / target | Source path | Local destination | 实施前差异 |
| --- | --- | --- | --- | --- |
| GPUI skill | `longbridge/gpui-component@5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3` | `skills/gpui/**` | `.agents/skills/gpui/**` | 23 个文件无增删；5 个文件仅文件末空行差异 |
| Upstream compact component skill | 同上 | `skills/gpui-component/**` | 仅作 `gpui-component-usage` 的对照证据 | 上游从 `57a9903` 到 target 无增删改；本仓不是其镜像 |
| Component documentation | 同上 | `website/docs/components/*.md` | `.agents/skills/gpui-component-usage/references/components/*.md` | target 63 份，本地 61 份；缺 `command.md` 和 `textarea.md` |
| Component provenance/license | 同上 | `LICENSE-APACHE` 及 target metadata | `.agents/skills/gpui-component-usage/references/third-party/**` | 仍记录旧 SHA、旧 `docs/docs/components` 路径和 2024–2025 版权年份 |
| Base guidance | 同上 | `website/base/**`、`crates/base/**`、相关 story/test | `.agents/skills/gpui-base-usage/**` | target 无上游 skill；本地尚未建立 base-only 路由 |

component target 的 lockfile 将 Zed GPUI 固定到
`e0931d5a9dbf4f781b336fdf448739e74a2ac0b5`。验证 API 事实时使用这组 target，不使用
实施当日漂移的 main。

## 已固定决定

### SS-205-01：`gpui` 目录是纯上游镜像

- 同步单位是完整目录，不是已知文件白名单。上游新增或删除必须在本地一致反映。
- 本地仅允许仓库统一 LF 和文件末规范化；此类差异必须在 normalized diff 中归零。
- 若需要补充 GPUI 新 API 或本仓经验，应提交上游或放在另一个 repo-local overlay；不得
  使镜像目录分叉。

### SS-205-02：`gpui-component-usage` 是“快照 + 本地适配”

- `references/components/*.md` 中的上游文档按快照处理，但本地自编 `index.md` 是明示排除项。
- `SKILL.md`、`references/rules/**`、`agents/openai.yaml` 和本地索引由本仓拥有；它们对照上游
  compact skill、docs、source、story 与 tests 手工重放，不做文件覆盖。
- attribution 必须准确枚举 source path、target SHA、license 和本地排除项；未同步的文件不得
  修改标签后冒充 target 快照。

### SS-205-03：`gpui-base-usage` 是独立的 repo-local skill

- target 没有 `skills/gpui-base`，因此新 skill 明确标记为 repo-local adaptation。
- Lestty/base-only 任务由 `gpui-app-development` 路由到 `gpui-base-usage`；Jaco、Feiwen、HTTP Client 和
  Novel Download 继续路由到 `gpui-component-usage`。
- base skill 必须明示记录：无 presentation style、裸 `InputBase` 不等于完整 component `Input`、
  a11y 是逐 primitive/app-owned contract，以及 Lestty 不得通过 theme/assets 间接拉入完整 component。

## 目标文件拓扑

```text
.agents/skills/
├─ gpui/                              # 上游 skills/gpui 完整镜像
├─ gpui-component-usage/
│  ├─ SKILL.md                       # repo-local 消费者路由
│  ├─ references/assets.md          # 未 vendored 上游 assets guide 的本地路由页
│  ├─ references/components/        # upstream docs snapshot，index.md 除外
│  ├─ references/rules/             # repo-local 组合/状态/a11y/errata 规则
│  └─ references/third-party/       # target provenance 和 license
├─ gpui-base-usage/
│  ├─ SKILL.md
│  └─ references/
│     ├─ architecture-and-boundary.md
│     ├─ primitives.md
│     ├─ input-textarea-editor.md
│     ├─ accessibility.md
│     └─ text-selection.md
└─ gpui-app-development/SKILL.md     # base/full component 路由权威
```

## 实施工作包

### WP-SS-205-00：冻结证据

1. 记录 component target、其 lock 中的 GPUI target、上游 skill 文件集和本地 skill 文件集。
2. 生成 source/current/target 的 A/M/D 清单和 normalized hash manifest。
3. 保存旧 provenance、相对链接和 stale-pattern 扫描结果，作为实施前基线。

### WP-SS-205-10：核对 GPUI 镜像

1. 按 target `skills/gpui/**` 完整比较 `.agents/skills/gpui/**`。
2. 本 target 预期 23 个文件、0 add、0 semantic modify、0 delete；5 个文件的文件末差异按
   本仓规范保留并在证据中说明。
3. 若实际 target 与本表不同，停止实施并先更新父计划与本文；不静默跟随 newer main。

### WP-SS-205-20：刷新 component docs 快照

1. 将快照 source 从 `57a9903/docs/docs/components/*.md` 迁到
   `5e5a1a30/website/docs/components/*.md`。
2. 新增 `command.md` 和 `textarea.md`。
3. 刷新 `chart.md`、`dropdown_button.md`、`editor.md`、`input.md`、`list.md`、
   `notification.md`、`scrollable.md`、`tabs.md`、`text-view.md` 和 `title-bar.md`。
4. 对本地 `components/index.md` 执行三方合并：保留本仓选择表和规则路由，吸收 target 新增
   component 与更正事实，不把它计入上游快照 hash。
5. 更新 `gpui-component-docs.md` 的 repository、target SHA、source path、license、排除项和
   normalized-copy 规则；刷新 2024–2026 license 文本。

### WP-SS-205-30：重放 repo-local component 适配

1. 在 `SKILL.md` 和索引中将 Command palette / action search 路由到 `Command`，将多行文本路由到
   `Textarea`，将代码编辑路由到 `Editor`。
2. 记录 Command 没有自定义 predicate/scorer；外部搜索 authority 使用 `.filterable(false)`，
   owner 保留 debounce、cancel、error/retry 和 result identity。
3. 核对 `AppMenuBar::new(cx)`、`TitleBar::window_options()`、system notification 的 handler/identity 边界，
   以及 Command 的可访问名称/active-descendant 缺口。target `title-bar.md` 仍含错误的
   `AppMenuBar::new(window, cx)` 示例；为保持上游快照完整性不就地修改，而是在 repo-local
   `references/rules/upstream-errata.md` 记录 source-backed 更正并从索引路由。
4. 在 rules 中增加 a11y 分层：GPUI AccessKit 基础设施、component 显式语义、app-owned 自绘控件与
   人工辅助技术验收不能互相替代。
5. 将主题规则更新到 target：直接通过 `Theme::global_mut` 改字段后调用 `Theme::sync_base`；
   Markdown `CodeBlock` 的样式缓存已跟随当前 `HighlightTheme`，删除旧 blocker 结论。

### WP-SS-205-40：建立 gpui-base-only skill

1. 新建 `.agents/skills/gpui-base-usage/`，使用目标文件拓扑中的六份文件。
2. 从 target `website/base/**`、`crates/base/**`、story 和 tests 编写精简 repo-local 导航；不全量
   复制 website，每个可执行 API 事实附 target path/symbol 证据。
3. 覆盖 `gpui_base::init`、behavior primitive ownership、Input/Textarea/Editor 分层、TextSelection 边界、
   theme projection、a11y 缺口和 Lestty 禁止依赖。
4. 更新 `gpui-app-development/SKILL.md`：根据 app manifest/target 边界在 `gpui-base-usage` 与
   `gpui-component-usage` 之间分流，不以控件名称猜测。

### WP-SS-205-50：验证与回填

1. 运行目录集合、normalized hash、stale pattern、Markdown 链接、frontmatter、UTF-8、LF 和无行尾空格
   检查。
2. 从每个 skill 入口人工跟随 Command、Textarea、Editor、base init、component init 和 Lestty 路由。
3. 将 source/target hash manifest、实际 A/M/D、规范化例外、验证命令和结果回填本文，再将
   状态改为 `Done`。

## 验证矩阵

### 上游镜像和快照

以处于精确 target 的兄弟 checkout 为例：

```powershell
$componentCheckout = Resolve-Path ../gpui-component
git -C $componentCheckout rev-parse HEAD
git diff --no-index --ignore-blank-lines -- .agents/skills/gpui "$componentCheckout/skills/gpui"
```

第二条命令返回非零时，必须分类为文件集差异、内容差异或已记录的文件末规范化；
不得只因为“看起来相同”忽略。component docs 共 63 份；normalized manifest 在双方排除
upstream/local `index.md` 后比较其余 62 份复制文档。

### Residual gate

```powershell
rg -n "57a9903|docs/docs/components" .agents/skills
rg -n "\.multi_line\(|\.code_editor\(" .agents/skills
rg -n "Command|Textarea|Editor|gpui_base::init|gpui_component::init" .agents/skills
```

前两条为 negative assertion；第三条必须能从各 skill 入口追踪到相应参考。历史计划中保留的旧
SHA 不在此门禁范围；门禁只扫描 `.agents/skills`。`AppMenuBar::new(window, cx)` 会作为已知
upstream target doc defect 保留一处；验证要求 `upstream-errata.md` 明确给出源码签名
`AppMenuBar::new(cx)`。`TitleBar::title_bar_options()` 仍是合法符号，不作 negative assertion；本地路由应说明
构建整个 `WindowOptions` 时优先使用 `TitleBar::window_options()`。

### 文档完整性

- 所有 `SKILL.md` 有有效 frontmatter，名称与目录/路由一致。
- 所有 repo-local Markdown 相对链接可解析，Navigation 不指向删除或未同步文件；精确上游
  快照内无法原地修正的链接缺陷必须在 errata 中登记。
- 文件使用 UTF-8、LF、无 BOM；repo-local 文件无行尾空格。精确镜像继承的 Markdown hard break、
  空白行或无末尾换行必须与 target 一致并作为例外登记。
- attribution 中的 target SHA、source path、排除项和 license 与实际快照一致。

## 实施证据

### 实际文件结果

- skill 范围实际为 `11 added / 20 modified / 0 deleted`：新增 `gpui-base-usage` 六个文件、
  `command.md`、`textarea.md`、`assets.md`、`accessibility.md` 与 `upstream-errata.md`；修改
  component snapshot/rules/provenance/router 及 `gpui/references/event.md` 的 target 空行。
- component source 共 63 份 Markdown，其中 upstream `index.md` 明示不复制；62 份复制文档与
  target 文件集一一对应，本地 `components/index.md` 继续由本仓维护。
- `references/assets.md` 是 repo-local 链接路由页，不冒充上游 snapshot；它让 vendored
  `icon.md` 对上游父级 assets guide 的链接在离线 skill 中仍有可用落点。
- `theme-and-size.md` 已以 target source 更正两项事实：`Theme::global_mut` 后需要
  `Theme::sync_base(cx)`；target `CodeBlock` 会按当前 `HighlightTheme` 重算缓存样式，旧的
  `5b45bcb` lifecycle blocker 不再成立。

### Normalized manifest

规范化算法仅将 CRLF/CR 转为 LF 并移除文件末尾 LF；路径和正文进入同一个 SHA-256 manifest。

| Artifact | Source/local count | Source SHA-256 | Local SHA-256 | Result |
| --- | ---: | --- | --- | --- |
| `skills/gpui/**` | `23 / 23` | `7aba6ca1db227a916bec160e32f683c64818ab0390a1db4d33eed53dde2e2843` | `7aba6ca1db227a916bec160e32f683c64818ab0390a1db4d33eed53dde2e2843` | 文件集与 normalized content 一致 |
| `website/docs/components/*.md`（双方排除 `index.md`） | `62 / 62` | `d6c4ca9d985383a2f5c2c4e360c181ea0a4e4b79e268c9d9913609ba36d03420` | `d6c4ca9d985383a2f5c2c4e360c181ea0a4e4b79e268c9d9913609ba36d03420` | 文件集与 normalized content 一致 |
| `LICENSE-APACHE` | `1 / 1` | — | — | normalized content 一致 |

GPUI 五个文件只保留本仓文件末空行规范化差异：`action.md`、`async.md`、`event.md`、
`focus-handle.md`、`global.md`。`event.md` 的标题前 target 空行已同步，因此不再存在内部差异。

### 验证结果

| Command/check | Actual result |
| --- | --- |
| `git -c safe.directory=... -C ../gpui-component rev-parse HEAD` 与 relevant-path status | HEAD 为 `5e5a1a304b2a5a3d725c03b8759e9ba2b4ad58b3`；target 相关路径无工作区修改 |
| PowerShell normalized file-set/content/hash check | GPUI `23/23`、docs `62/62`、license `1/1`；均 `0` add/delete/mismatch |
| `rg -n "57a9903\|docs/docs/components" ...` | `0` residual |
| `rg -n "\\.multi_line\\(\|\\.code_editor\\(" ...` | `0` residual |
| target source signature assertions | `AppMenuBar::new(cx)`、`Command::filterable`、`TitleBar::window_options`、`Theme::sync_base`、`gpui_base::init` 与 Input/Textarea/Editor state aliases 全部存在 |
| skill entrypoint/router assertions | Command、Textarea、Editor、external `.filterable(false)`、base/full init、base/full route 全部可从入口追踪；Lestty manifest 的 forbidden direct deps 为 `0` |
| UTF-8/BOM/CR scan | 108 个 scoped files，`0` issue |
| repo-local final-LF/trailing-whitespace scan | 22 个 repo-local files，`0` issue |
| Markdown relative-link scan | 105 份 Markdown；仅保留 `select.md -> combobox` 的一个 snapshot-local portability 例外，已在 `upstream-errata.md` 登记 |
| equivalent frontmatter/name/description/placeholder check | 4 个 `SKILL.md`，`0` issue |
| `quick_validate.py` with bundled Python | 环境缺少 `PyYAML`，报 `ModuleNotFoundError: yaml`；依约未安装包，以上等价检查覆盖其 frontmatter、name 与 placeholder 门禁 |
| `git diff --check -- <skill scopes> <plan>` | passed |

精确源文件中保留四类 source-owned 格式例外，并已确认 target 同样存在：
`gpui/references/entity.md` 的 Markdown hard-break 尾随空格、
`gpui/references/test-{examples,reference}.md` 的无末尾 LF，以及 component `plot.md` 的空白行。
这些不是 repo-local 新增格式问题。

Lestty 的完整 transitive `cargo tree` 负断言属于父依赖升级计划的 lock/build 门禁；本文已完成
manifest-level route 与 direct-dependency 断言，不把父计划尚未执行的 lock 验证伪装为 skill 证据。

## 风险与失败处理

| Risk | 预防/检测 | 失败处理 |
| --- | --- | --- |
| 把 `gpui` 镜像改成本地 fork | 完整目录 normalized diff | 恢复 target 镜像；本地补充转移到独立 overlay 或提交上游 |
| 用 compact skill 覆盖本仓 component 规则 | ownership 分层 + 三方合并 | 撤销覆盖，先恢复 repo-local 索引/rules，再逐条重放 target 事实 |
| 只改 provenance 却未刷新全部快照 | target 文件集 + hash manifest | 保留旧 SHA 标签并停止实施，直到快照内容完整同步 |
| Lestty 被路由到完整 component | router walkthrough + Cargo 负断言 | 修正 `gpui-app-development` 路由，不通过为 Lestty 加 component 依赖解决 |
| docs 与 target 导出符号不符 | source/story/test 交叉核对 + stale scan | 以 target 实际符号修正 repo-local 适配，不为旧文档增加 compatibility fiction |

## 完成条件

- [x] `.agents/skills/gpui/**` 与 target `skills/gpui/**` 文件集一致，normalized content 无语义差异。
- [x] component docs target 63 份全部有结论；新增、刷新、本地排除与三方合并均有证据。
- [x] provenance 与 license 指向 target SHA/path/year，不存在“仅改标签”的伪同步。
- [x] `gpui-component-usage` 已覆盖 Command、Textarea、Editor、AppMenuBar、TitleBar、notification 与 a11y
      边界，且保留 repo-local ownership 规则。
- [x] `gpui-base-usage` 与 `gpui-app-development` 分流完成，Lestty 不会被误导依赖完整 component。
- [x] 目录/hash、residual、Markdown 链接、frontmatter、UTF-8/LF 检查完成，结果与 source-owned
      例外已回填。
