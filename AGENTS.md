# AGENTS.md

本文件定义代码代理（Agent）在本仓库中的默认工作规则。目标是帮助代理快速做出正确决策，而不是重复 README。

## 1. 仓库概览

- 本仓库是基于 `GPUI` 的 Rust workspace，包含多个独立桌面应用和共享 crate。
- workspace 成员：
  - `app/ai-chat`
  - `app/ai-chat2`
  - `app/feiwen`
  - `app/http-client`
  - `app/novel-download`
  - `crates/ai-chat-agent`
  - `crates/ai-chat-core`
  - `crates/ai-chat-db`
  - `crates/app-assets`
  - `crates/app-assets-macros`
  - `crates/app-theme`
  - `crates/gpui-store`
  - `crates/gpui-tokio`
  - `crates/platform-ext`
  - `crates/window-ext`
  - `crates/xtask`
- 技术基线：
  - Rust Edition: `2024`
  - 推荐 Rust: `1.92+`
  - 关键依赖通过 workspace 统一声明；当前 `gpui` / `gpui_platform` 来自 `zed-industries/zed` git，`gpui-component` / `gpui-component-assets` 来自 `longbridge/gpui-component` git。

## 2. 代码修改原则

- 架构设计应以最优解为目标，不以最小变更为优先；允许在必要时进行大规模重构，但应以良好的整体设计为最终导向。
- 保持现有模块边界、命名风格和错误处理方式。
- 新增代码优先复用现有类型、Result 别名、日志模式和公共能力。
- 不要为了“先通过测试”“先修复现象”“先完成功能”而引入 hack、临时绕过或层层兜底来掩盖问题；发现设计、建模、状态流转或架构本身有缺陷时，优先定位并修正根因。
- 不要用额外兜底逻辑把问题隐藏起来；新增兼容或保护分支前，先判断是否是在掩盖上游不正确的数据流、抽象或生命周期设计，并优先修复原有代码。
- 禁止新增或编写 `mod.rs`；Rust 模块入口统一使用与模块同名的 `{module}.rs` 文件。
- 新增依赖必须使用完整版本号，例如 `1.2.3`，禁止使用 `^1`、`~1.2`、`1`。
- 涉及行为变更时，至少补充一个对应测试，或明确说明测试缺口。
- 不要修改与当前任务无关的文件格式、导入顺序、目录结构或代码风格。
- 提交代码前必须运行 `cargo fmt`，并保留其格式化结果。
- 文档与配置文件统一使用 `UTF-8` 和 `LF`。

## 3. UI 与 GPUI 规则

- GPUI app 结构、本仓库模块边界、资源位置和验证默认规则优先参考 repo-local skill：`gpui-app-development`。
- 编写 UI 时默认优先使用 `gpui-component` 现成组件；组件选择、文档/story/API 检查和 Web/shadcn 风格转译规则参考：`gpui-component-usage`。
- 涉及 UI 图标、Lucide 图标声明、运行时资源或打包 app icon 时参考：`gpui-app-icon-usage`。
- 涉及用户可见文案、Fluent `.ftl`、语言设置或 macOS bundle 本地化时参考：`gpui-i18n`。
- 涉及 `crates/gpui-store` 或明确要把 app 状态接入 `gpui-store` 时参考：`gpui-store`；不要在无关任务中顺手迁移 app 状态。
- 具体 GPUI API 统一参考 repo-local skill：`gpui`；其中 action/keybinding、async、context、entity、event、focus、global、layout/style、element、test 等按该 skill 的 Navigation 加载对应 reference 文件。
- 不要重复实现 `gpui-component` 已提供的表格、按钮、输入、选择器、对话框、滚动条等通用能力；只补齐组件库没有覆盖且当前 app 确实需要的局部缺口。
- 涉及 Web / React / CSS / Tailwind / shadcn/ui 风格参考时，只吸收其设计意图与交互模式；最终必须以仓库现有的 GPUI / `gpui-component` 模式实现，不能照搬 DOM、CSS 或 React 组件习惯。
- 常规产品界面避免无意义卡片堆砌、过度装饰渐变和多重强调色；优先用版式、对齐、间距、字号、对比度和少量有目的的动效建立层级。
- runtime 资源、本地化资源和打包资源不要混放：运行时资源走 app-local assets / `with_assets(...)`，文案走 `locales/{en-US,zh-CN}/main.ftl`，macOS bundle 文案走 `locales/macos/*/InfoPlist.strings`，app icon 走 `build-assets/icon/app-icon.png`。

## 4. 提权与 GitHub 规则

- 任何需要联网、安装依赖、修改 `Cargo.lock`、执行 `cargo add` / `cargo install`、运行 `gh`、或其他超出沙盒权限的命令，都必须先申请权限。不要先在沙盒内试跑 `gh` 或联网命令。
- `gh` 的认证、查询和写操作都必须提权运行，包括 `gh auth status`、`gh pr view`、`gh pr create`、`gh pr ready`、`gh workflow run` 等。沙盒内 `gh` 失败、token 无效或无法访问 keyring，不代表真实环境不可用，禁止据此切换到其他方案。
- 如果命令因网络、权限或沙盒限制失败，应直接对原命令申请权限重跑，不要改用绕过限制的替代方案。
- 删除文件、覆盖生成产物、批量改写文件或改写 lockfile 前，也要先说明并申请权限。
- GitHub 相关操作必须优先使用提权后的 `gh`，不要手写推测性结果。只有在提权后的 `gh` 明确不可用时，才允许使用 GitHub App / MCP 作为降级方案，并必须在汇报中说明原因。
- 编写 issue、PR、release note、评论前，先检查 `.github/` 下的模板和 workflow。
- issue 默认使用：
  - `.github/ISSUE_TEMPLATE/bug_report.yml`
  - `.github/ISSUE_TEMPLATE/feature_request.yml`
  - `.github/ISSUE_TEMPLATE/tech_request.yml`
- PR 默认使用 `.github/pull_request_template.md`。
- 编写 PR 内容时，必须基于“当前分支相对远程最新 `main`”的整体差异进行总结，不要只根据最后一次提交或本次会话中的改动来写。
- 用户要求“提交 PR / 开 PR / 提 PR”时，默认创建可直接 review 的普通 PR；只有用户明确说“draft PR / 草稿 PR”时才创建 draft。
- issue / PR 标题和描述要明确对应应用或 crate，例如 `ai-chat`、`feiwen`、`http-client`、`novel-download`、`window-ext`、`xtask`。

## 5. 验证与 CI

- 代码修改后，至少执行与改动直接相关的验证命令。
- 合入 `main` 的变更默认要通过 `.github/workflows/ci.yml`。
- 当前 CI 默认覆盖 `macOS`、`Linux`、`Windows` 三个平台。
- Linux 系统依赖统一维护在 `script/bootstrap` 和 `script/install-linux.sh`，不要在 workflow 中散落重复的安装命令。
- 默认验证基线：
  - `cargo build`
  - `cargo test`
  - `cargo clippy --all-targets --all-features -- -D warnings`
- 汇报结果时要写明实际执行过的命令；如果没执行，必须说明原因。

## 6. 仓库特例

- 应用入口通常位于 `app/{name}/src/main.rs`。
- 公共能力优先放在 `crates/window-ext` 等共享 crate，避免在多个 app 中复制实现。
- `ai-chat` 使用 Diesel + SQLite；涉及数据层变更时要同步检查 migration、schema 和 service 映射。
- `ai-chat-agent` 的 provider 运行时优先以 `rig-core` 和当前 adapter 代码为准；不要为 OpenAI/Ollama 等 provider 原生 API 维护 repo-local skill，除非当前实现明确绕过 Rig 且需要专门的本地流程。
- `ai-chat` 运行时资源在 `app/ai-chat/assets/`，打包资源在 `app/ai-chat/build-assets/`；不要混用。
