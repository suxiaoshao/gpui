# Agent.md

本文件为代码代理（Agent）在本仓库工作时的执行指南。

## 1. 项目概览

- 这是一个基于 GPUI 的 Rust 工作区，包含多个独立桌面应用。
- 工作区成员：
  - `app/ai-chat`
  - `app/feiwen`
  - `app/http-client`
  - `app/novel-download`
  - `app/ai-chat/extensions/url_search`
  - `crates/window-ext`
  - `crates/xtask`

## 2. 技术栈与版本

- Rust Edition: 2024
- 推荐 Rust: 1.92+
- 关键依赖：
  - `gpui = 0.2.2`
  - `gpui-component = 0.5.1`
  - `gpui-component-assets = 0.5.1`

## 3. 常用命令

```bash
# 构建全部
cargo build

# 测试全部
cargo test

# 静态检查（默认验证项）
cargo clippy --all-targets --all-features -- -D warnings

# 运行单个应用
cargo run -p ai-chat
cargo run -p feiwen
cargo run -p http-client
cargo run -p novel-download

# 构建单个应用
cargo build -p ai-chat
cargo build -p feiwen
cargo build -p http-client
cargo build -p novel-download

# 打包（xtask）
cargo run -p xtask -- bundle-ai-chat
cargo run -p xtask -- bundle-ai-chat-windows
```

说明：`bundle-ai-chat`（macOS/Linux）仍走 `cargo-bundle`；`bundle-ai-chat-windows` 使用 `tauri-bundler` + WiX。

可选开发工具：

```bash
# 自动重载（示例）
cargo watch -x 'run -p ai-chat'
```

## 4. 目录约定

- 每个应用入口：`app/{name}/src/main.rs`
- 公共能力：`crates/window-ext`
- `ai-chat` 扩展目录：`app/ai-chat/extensions/`
- `ai-chat` 数据库相关：`app/ai-chat/src/database/`

## 5. GPUI 开发约定

- 视图组件实现 `Render`（或项目既有模式）。
- 使用 `cx.new(...)` 创建实体，避免绕过上下文直接构造需要上下文管理的对象。
- Action 使用 `actions!()` 定义并在上下文中注册处理器。
- 键位绑定使用 `KeyBinding::new(...)` 并集中注册。
- 异步任务优先使用 GPUI 提供的 `spawn` / `background_executor` 模式，与 UI 更新隔离。
- 全局状态通过 `Global` + `cx.global::<T>()` / `cx.update_global(...)` 访问和更新。

## 6. ai-chat 特殊说明

- 使用 Diesel + SQLite，迁移为内置模式，首次运行自动初始化。
- 若需新增迁移：

```bash
cd app/ai-chat
diesel migration generate <migration_name>
```

- 扩展基于 WebAssembly Component Model：

```bash
cd app/ai-chat/extensions/url_search
cargo component build --release
```

## 7. 日志与数据路径

macOS:
- 数据：`~/Library/Application Support/top.sushao.{app}/`
- 日志：`~/Library/Logs/top.sushao.{app}/data.log`

Linux:
- 数据：`$XDG_CONFIG_HOME/top.sushao.{app}/` 或 `~/.config/top.sushao.{app}/`
- 日志：`$XDG_DATA_HOME/top.sushao.{app}/logs/data.log` 或 `~/.local/share/top.sushao.{app}/logs/data.log`

## 8. 代码修改原则（给 Agent）

- 先做最小变更，再逐步扩展，避免大面积无关重构。
- 保持现有模块边界与命名风格。
- 新增代码优先与现有错误类型、Result 别名、日志模式保持一致。
- 涉及行为变更时，至少补充一个对应测试或说明测试缺口。
- 不要修改与当前任务无关的文件格式、导入顺序或代码风格。

## 9. 提交前检查清单

- `cargo build` 通过
- `cargo test` 通过（或明确说明失败项）
- `cargo clippy --all-targets --all-features -- -D warnings` 通过（至少覆盖改动范围）
- 关键路径手动验证（对应 app 的核心流程）
- 若改动 `ai-chat` 数据层，确认迁移与 schema 一致

## 10. 快速决策建议

- UI 问题：优先检查 GPUI 渲染生命周期与状态订阅。
- 状态不同步：优先检查 entity/global 更新是否在正确上下文。
- 异步问题：确认任务生命周期、取消时机和 UI 更新线程边界。
- 数据问题：先验证 service 层映射，再查 SQL 模型与 schema。

## Runtime vs Build Assets

- `app/ai-chat/assets/`: runtime assets only (embedded by `rust-embed`).
- `app/ai-chat/build-assets/`: build/package-time assets only (not embedded for runtime).
- Icon assets live in `app/ai-chat/build-assets/icon/`.
- Windows icon default: `app/ai-chat/build-assets/icon/app-icon.ico` (see `app/ai-chat/build.rs`).
- Package icon paths are configured in `app/ai-chat/Cargo.toml` under `[package.metadata.bundle].icon` and use `build-assets/icon/...`.
- Windows MSI bundling in `xtask` uses `tauri-bundler` and reuses the `.ico` path from `[package.metadata.bundle].icon`.
- macOS bundle icon paths are managed by `crates/xtask/src/main.rs` and use `build-assets/icon/...`.

## 11. 文件编码

- 文档与配置文件必须使用 UTF-8 编码读写，禁止使用本地默认编码导致乱码。
