# AGENTS.md

本文件定义代码代理（Agent）在本仓库中的默认工作规则。目标是帮助代理快速做出正确决策，而不是重复 README。

## 1. 仓库概览

- 本仓库是基于 `GPUI` 的 Rust workspace，包含多个独立桌面应用和少量共享 crate。
- workspace 成员：
  - `app/ai-chat`
  - `app/feiwen`
  - `app/http-client`
  - `app/novel-download`
  - `crates/window-ext`
  - `crates/xtask`
- 技术基线：
  - Rust Edition: `2024`
  - 推荐 Rust: `1.92+`
  - 关键依赖：`gpui = 0.2.2`、`gpui-component = 0.5.1`、`gpui-component-assets = 0.5.1`

## 2. 代码修改原则

- 先做最小变更，再逐步扩展，避免大面积无关重构。
- 保持现有模块边界、命名风格和错误处理方式。
- 新增代码优先复用现有类型、Result 别名、日志模式和公共能力。
- 不要为了“先通过测试”“先修复现象”“先完成功能”而引入 hack、临时绕过或层层兜底来掩盖问题；发现设计、建模、状态流转或架构本身有缺陷时，优先定位并修正根因。
- 不要用额外兜底逻辑把问题隐藏起来；新增兼容或保护分支前，先判断是否是在掩盖上游不正确的数据流、抽象或生命周期设计，并优先修复原有代码。
- 禁止新增或编写 `mod.rs`；Rust 模块入口统一使用与模块同名的 `{module}.rs` 文件。
- 新增依赖必须使用完整版本号，例如 `1.2.3`，禁止使用 `^1`、`~1.2`、`1`。
- 涉及行为变更时，至少补充一个对应测试，或明确说明测试缺口。
- 不要修改与当前任务无关的文件格式、导入顺序、目录结构或代码风格。
- 文档与配置文件统一使用 `UTF-8` 和 `LF`。

## 3. UI 与 GPUI 规则

- 涉及落地页、官网页、品牌页、视觉稿、高表现力界面或需要明显视觉设计判断的 UI 任务时，默认遵从 `frontend-skill` 的设计原则；重点吸收其关于构图、层级、留白、图像叙事、文案克制与动效节制的要求，不要产出模板化 SaaS 卡片拼盘。
- 编写 UI 时优先使用 `gpui-component` 现成组件；组件库没有合适组件时再自行实现。
- 在本仓库中落实 `frontend-skill` 时，要映射到 GPUI 语境：`GPUI` 的布局与样式 API 可类比 `tailwindcss` 的组合思路，`gpui-component` 的组件复用方式可类比 `shadcn/ui`，但必须以仓库现有的 GPUI / `gpui-component` 模式实现，不能照搬 Web DOM、CSS 或 React 组件习惯。
- 组件列表与示例优先参考：`https://longbridge.github.io/gpui-component/docs/components/`
- 具体 API 以 `https://docs.rs/gpui-component/latest/gpui_component/` 为准，不要凭记忆猜接口。
- 视图组件实现 `Render` 或项目现有模式。
- 使用 `cx.new(...)` 创建需要上下文管理的实体，不要绕过上下文直接构造。
- Action 使用 `actions!()` 定义并在上下文中注册；键位绑定使用 `KeyBinding::new(...)` 并集中注册。
- 异步任务优先使用 GPUI 提供的 `spawn` / `background_executor` 模式，并保持 UI 更新与后台任务隔离。
- 全局状态通过 `Global`、`cx.global::<T>()`、`cx.update_global(...)` 访问和更新。
- 常规产品界面同样避免无意义卡片堆砌、过度装饰渐变和多重强调色；优先用版式、对齐、间距、字号、对比度和少量有目的的动效建立层级。

## 4. 提权与 GitHub 规则

- 任何需要联网、安装依赖、修改 `Cargo.lock`、执行 `cargo add` / `cargo install`、运行 `gh` 写操作，或其他超出沙盒权限的命令，都必须先申请权限。
- 如果命令因网络、权限或沙盒限制失败，应直接对原命令申请权限，不要改用绕过限制的替代方案。
- 删除文件、覆盖生成产物、批量改写文件或改写 lockfile 前，也要先说明并申请权限。
- GitHub 相关操作优先使用 `gh`，不要手写推测性结果。
- 编写 issue、PR、release note、评论前，先检查 `.github/` 下的模板和 workflow。
- issue 默认使用：
  - `.github/ISSUE_TEMPLATE/bug_report.yml`
  - `.github/ISSUE_TEMPLATE/feature_request.yml`
  - `.github/ISSUE_TEMPLATE/tech_request.yml`
- PR 默认使用 `.github/pull_request_template.md`。
- 编写 PR 内容时，必须基于“当前分支相对远程最新 `main`”的整体差异进行总结，不要只根据最后一次提交或本次会话中的改动来写。
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
- `ai-chat` 运行时资源在 `app/ai-chat/assets/`，打包资源在 `app/ai-chat/build-assets/`；不要混用。
