# AGENTS.md

## 项目与实现约束

- 基于 GPUI 的 Rust 多应用 workspace；成员、工具链与依赖版本以 `Cargo.toml` 和工具链配置为准。应用入口通常为 `app/{name}/src/main.rs`，共享能力放入职责对应的 `crates/`。
- 在授权目标内选择架构与维护成本整体最优的方案；修复建模、状态流或生命周期的根因，避免用临时绕过和层层兜底掩盖缺陷。复用现有类型、Result 别名、日志和公共能力，保持约定一致。
- Rust 模块使用 `{module}.rs`，禁止新增 `mod.rs`；新增依赖使用完整版本号。Rust 格式遵循项目配置，文档与配置使用 UTF-8、LF。
- `jaco` 数据层使用 Diesel + SQLite，相关变更须覆盖 migration、schema 和 service 映射。
- `jaco-agent` provider 以 `rig-core` 和当前 adapter 实现为准；仅在明确绕过 Rig 且需要专门本地流程时，才维护 provider 原生 API skill。

## UI 与技能入口

以下技能位于 `.agents/skills/`，按任务读取对应入口及必要参考：

| 涉及内容 | skill |
| --- | --- |
| app 结构、模块边界、资源与开发入口 | `gpui-app-development` |
| 组件选择、API 与 Web/shadcn 风格转译 | `gpui-component-usage` |
| GPUI API | `gpui`，按 Navigation 定位 reference |
| UI 图标、运行时资源与 app icon | `gpui-app-icon-usage` |
| 用户文案、Fluent 与 bundle 本地化 | `gpui-i18n` |
| `crates/gpui-store` 或 app 状态接入 | `gpui-store` |

- 应用统一通过 `gpui_kit`、`gpui_kit::component` 与 `gpui_kit::assets` 接入；共享 crate 可保留指向同一发布包的 `gpui`/`gpui_component` 别名，具体版本以根 manifest 为准。
- 优先使用 `gpui-component` 已有组件，仅补齐当前 app 所需缺口。Web 参考只用于设计与交互意图，落地遵循 GPUI 模式。
- 用版式、对齐、间距、字号、对比度和有目的的动效建立层级，避免无意义卡片、装饰渐变和多重强调色。
- 运行时资源使用 app-local `assets/` 与 `with_assets(...)`；文案位于 `locales/{en-US,zh-CN}/main.ftl`，bundle 文案位于 `locales/macos/*/InfoPlist.strings`，app icon 位于 `build-assets/icon/app-icon.png`。运行时与打包资源分开存放。

## GitHub 与验证

- Issue、PR 等内容遵循 `.github/` 中适用的模板与 workflow；PR 模板为 `.github/pull_request_template.md`，标题和描述须标明应用或 crate。
- PR 描述覆盖当前分支相对远程最新 `main` 的整体差异；默认创建普通 PR，用户明确要求草稿时才创建 draft。
- 验证以生产行为和关键契约为准，优先运行或调整已有测试；仅为具体回归风险及未覆盖的不变量新增测试，不按文件、方法、字段或分层数量配测试。
- 应用测试覆盖自身业务接入，避免重复验证上游实现或机械复述代码、固定文案。删除无生产用途的实现时同步清理专用测试，不为测试保留无用接口。
- 构建、测试和严格 Clippy 按受影响 crate、契约、平台或发布范围选择；适用 CI 与 hooks 仍须通过，合入 `main` 以 `.github/workflows/ci.yml` 为准。
- Linux 系统依赖统一维护在 `script/bootstrap` 和 `script/install-linux.sh`，避免在 workflow 重复安装逻辑。
- 文档与指令改动仅检查相关结构、链接和差异；汇报实际验证命令，以及未完成的适用检查和原因。
