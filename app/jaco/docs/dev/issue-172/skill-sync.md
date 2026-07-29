# GPUI 与 gpui-form skill 同步计划

## 同步基准

skill 同步只能在 `gpui-component` dependency 已更新并固定到新 lockfile commit 后执行。上游源目录必须来自该 commit 的 checkout，不能从另一个随手 clone 的 `main` 复制。

当前快照基线：

- `gpui-component` commit：`5b45bcb26b9343d91a123a4d5ed8a654360512e5`；
- `gpui` skill 源：`skills/gpui/`；
- 组件文档源：`docs/docs/components/*.md`；
- 本地 attribution：`.agents/skills/gpui-component-usage/references/third-party/gpui-component-docs.md`。

本次已审计目标为
`57a9903f48160845aabc8b92a1e2f5348c80d439`。相对当前快照的精确 A/M/D 为：

- `skills/gpui`：Added/Modified/Deleted=无；
- `docs/docs/components`：Modified=`chart.md`；
  Added/Deleted=无。

该清单已按同一目标 commit 执行。上游 `skills/gpui` 在基线到目标之间没有内容变化，
因此未为了 5 个历史 EOF 空行差异改写现有镜像文件；这也避免制造
`git diff --check` 的 `new blank line at EOF`。`gpui-component-usage` 的 `chart.md`
已精确同步，attribution 已更新为目标 SHA；若未来目标 commit 变化，仍须先重新生成
完整 A/M/D。

实施 PR 必须同时记录“当前快照”和“目标快照”，A/M/D 清单以这两个 commit 的实际内容
为准。

## 一、直接镜像 `gpui` skill

目标目录：

```text
.agents/skills/gpui/
├── SKILL.md
└── references/
    ├── action.md
    ├── async.md
    ├── context.md
    ├── element*.md
    ├── entity*.md
    ├── event.md
    ├── focus-handle.md
    ├── global.md
    ├── layout-style.md
    └── test*.md
```

规则：

1. 整个目录以 `<gpui-component-checkout>/skills/gpui/` 为唯一事实源。
2. 上游新增文件必须新增，上游删除文件必须删除，上游修改文件必须完整替换。
3. 不在镜像目录内保留本仓库定制段落、额外 reference 或格式调整。
4. 上游没有 `agents/openai.yaml` 时不自行补；这是直接镜像，不由 `skill-creator` 重写。
5. 完成后 `diff -qr <upstream>/skills/gpui .agents/skills/gpui` 必须无输出。

上游在基线与目标之间虽然没有 skill 文件变化，但当前本地与上游目标仍有 5 个 reference
的结尾空行差异：`action.md`、`async.md`、`event.md`、`focus-handle.md`、`global.md`。
实施时仍按完整镜像消除，不把它们误报为本次上游 A/M/D，也不把它们保留为 fork。

## 二、同步 `gpui-component-usage` 的复制文档

这个 skill 是本仓库创建的，只有组件正文来自上游。目录所有权如下：

| 路径 | 所有权 | 同步方式 |
| --- | --- | --- |
| `SKILL.md` | 本仓库 | 根据更新后 API/组件列表人工维护 |
| `agents/openai.yaml` | 本仓库 | 与 SKILL description/default prompt 保持一致 |
| `references/rules/*.md` | 本仓库 | 保留；仅在上游行为变化导致规则失真时修改 |
| `references/components/index.md` | 本仓库 | 根据目标组件文档重新生成/人工校对 |
| `references/components/*.md`（除 index） | 上游快照 | 按文件名完整同步 A/M/D |
| `references/third-party/gpui-component-docs.md` | 本仓库 attribution | 更新 snapshot commit/path/版权信息 |
| `references/third-party/gpui-component-LICENSE-APACHE` | 上游 license | 与目标 commit 比较，变化时同步 |

同步步骤：

1. 分别列出目标上游和本地 `components` 文件名。
2. 生成三类清单：Added、Modified、Deleted；清单必须包含文件名，不能只写数量。
3. 对除 `index.md` 外的上游组件文档做 byte-for-byte 更新。
4. 删除上游已删除的 component reference，避免 skill 继续推荐不存在组件。
5. 重写本地 `index.md` 的分类与 shadcn mental mapping：
   - 新组件放入准确的 action/input/overlay/data/navigation 类别；本目标没有新增文档，
     但 `chart.md` 正文新增 `RadarChart` 后要确认 index 的 Chart 路由仍准确；
   - 删除消失组件及所有链接；
   - 组件 rename 同步修改链接文本和文件名；
   - 保留本地 rules 链接。
6. 检查 `SKILL.md` 的 Component Selection 表是否包含新增关键组件、是否引用已删除组件。
7. 更新 attribution 中的完整目标 commit SHA。
8. 对每个 Markdown 相对链接做存在性检查。

不能直接用上游 `docs/docs/components/index.md` 覆盖本地 index：上游 index 是网站导航，本地 index 还承担 app component selection 和 shadcn-to-GPUI 路由。

## 三、现有 repo-local `gpui-form` skill 的窄范围校正

`.agents/skills/gpui-form/{SKILL.md,agents/openai.yaml}` 已由其他提交完成
typed-store/owning-control skill。Issue #172 不再重构该 skill，不拆 references，不重写
`SKILL.md`，也不重复审计三个 form crate 的整体架构。

在 `gpui-component` 更新完成后，只执行与本次依赖更新直接相关的漂移检查：

1. 读取现有 `SKILL.md`、`crates/gpui-form-gpui-component/docs/guide.md` 和四个 adapter
   的实际实现。
2. 对比目标上游 `InputState`、Select、Combobox 的公开 event/state/setter API。
3. 只有当上游变化使现有 skill 中关于 `FormInput`、`FormIntegerInput<N>`、
   `FormSelect<D>`、`FormCombobox<D>` 的说明失真时，才对原 `SKILL.md` 做定位明确的
   小修改；没有漂移则保持文件不变。
4. 不因 text decoration、Select Caret 或 chart 新能力改动 form core；这些能力没有当前
   产品需求时只记录复用结论。

已发现一个与 crate API 无关的小优化：`agents/openai.yaml` 仍使用旧的 “form draft”
表述，与当前 typed-store/owning-control skill 不一致。实施 WP-70 时可把 metadata
定位修正为：

```yaml
interface:
  display_name: "GPUI Form"
  short_description: "Build and integrate typed GPUI forms"
  default_prompt: "Use $gpui-form to implement or review this typed GPUI form flow."
```

这是现有 skill 的 metadata 校正，不新增 reference、icon、brand color、MCP dependency，
也不引入新的 skill 设计工作。只有实际修改了 `SKILL.md` 或 `agents/openai.yaml` 时，才运行：

```sh
python3 /Users/sushao/.codex/skills/.system/skill-creator/scripts/quick_validate.py \
  .agents/skills/gpui-form
```

## 四、skill 同步验收

- `gpui` 与目标上游内容一致；允许上述 5 个仅由上游 EOF 空行造成的既有 byte diff，
  本次上游 A/M/D 仍为无。
- `gpui-component-usage` 有完整 Added/Modified/Deleted 清单；本目标
  M=`chart.md`，A/D=无。
- 所有本地 index 链接存在，已删除组件没有残留引用。
- attribution commit 等于 Cargo.lock 使用的 `gpui-component` commit。
- `gpui-form` 不新增 references；只有存在可证明的 gpui-component adapter API 漂移时
  才修改现有 `SKILL.md`。
- 若执行 metadata 小修，`agents/openai.yaml` 不再包含旧的 “form draft” 表述，并通过
  `quick_validate.py`；若两个 skill 文件都未修改，则不重复运行该验证。
- 各 `agents/openai.yaml` 与对应 SKILL.md 一致。
- `rg` 不发现旧组件名、旧 API 或已删除 reference。
- skill 变更完成后再实施 Jaco reasoning UI，以更新后的组件 API 和文档为准。
