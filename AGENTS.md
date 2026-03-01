# AGENTS.md

本文件定义代码代理（Agent）在本仓库中的默认工作规则。目标是帮助代理快速做出正确决策，而不是重复 README。

## 1. 仓库概览

- 本仓库是基于 `GPUI` 的 Rust workspace，包含多个独立桌面应用和少量共享 crate。
- workspace 成员：
  - `app/ai-chat`
  - `app/feiwen`
  - `app/http-client`
  - `app/novel-download`
  - `app/ai-chat/extensions/url_search`
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
- 新增依赖必须使用完整版本号，例如 `1.2.3`，禁止使用 `^1`、`~1.2`、`1`。
- 涉及行为变更时，至少补充一个对应测试，或明确说明测试缺口。
- 不要修改与当前任务无关的文件格式、导入顺序、目录结构或代码风格。
- 文档与配置文件统一使用 `UTF-8` 和 `LF`。

## 3. UI 与 GPUI 规则

- 编写 UI 时优先使用 `gpui-component` 现成组件；组件库没有合适组件时再自行实现。
- 组件列表与示例优先参考：`https://longbridge.github.io/gpui-component/docs/components/`
- 具体 API 以 `https://docs.rs/gpui-component/latest/gpui_component/` 为准，不要凭记忆猜接口。
- 视图组件实现 `Render` 或项目现有模式。
- 使用 `cx.new(...)` 创建需要上下文管理的实体，不要绕过上下文直接构造。
- Action 使用 `actions!()` 定义并在上下文中注册；键位绑定使用 `KeyBinding::new(...)` 并集中注册。
- 异步任务优先使用 GPUI 提供的 `spawn` / `background_executor` 模式，并保持 UI 更新与后台任务隔离。
- 全局状态通过 `Global`、`cx.global::<T>()`、`cx.update_global(...)` 访问和更新。

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
- `app/ai-chat/extensions/url_search` 是 wasm 扩展，不应按宿主平台直接参与原生 workspace 构建。
- 验证 `url_search` 时应显式指定 wasm target，例如：
  - `cargo build -p url_search --target wasm32-wasip2 --locked`
  - `cargo component build --release`
- `ai-chat` 运行时资源在 `app/ai-chat/assets/`，打包资源在 `app/ai-chat/build-assets/`；不要混用。
