# GPUI 应用工作区

一个基于 **GPUI** 的 Rust 桌面应用工作区，包含多个独立应用，统一管理依赖与构建流程。

## 应用列表

- **ai-chat**：AI 聊天应用，支持会话管理、流式响应与 WebAssembly 扩展
- **feiwen**：小说 / 网页内容阅读器，支持本地数据库存储
- **http-client**：HTTP 请求测试工具（类似 Postman）
- **novel-download**：小说 / 网页内容下载工具

## 目录结构

- `app/ai-chat`：AI 聊天应用
- `app/feiwen`：阅读器
- `app/http-client`：HTTP 客户端
- `app/novel-download`：下载器
- `crates/window-ext`：窗口相关扩展
- `crates/xtask`：工作区任务工具（打包脚本迁移）

## 环境要求

- Rust 1.92+（Edition 2024）
- 推荐：`cargo-watch`（可选，热重载）
- 可选：`diesel_cli`（仅 ai-chat 用于管理迁移）

## 构建与运行

```bash
# 构建整个工作区
cargo build

# 构建指定应用
cargo build -p ai-chat
cargo build -p feiwen
cargo build -p http-client
cargo build -p novel-download

# 运行指定应用
cargo run -p ai-chat
cargo run -p feiwen
cargo run -p http-client
cargo run -p novel-download
```

## 开发与调试

```bash
# 自动重载（需要 cargo-watch）
cargo watch -x 'run -p ai-chat'
```

## 应用打包

当前已为 `ai-chat` 增加 `cargo-bundle` 打包配置，可直接输出系统应用包（如 macOS `.app`）。

```bash
# 首次安装（只需一次）
cargo install cargo-bundle

# 方式 1：在工作区根目录执行
cargo bundle -p ai-chat --release

# 方式 2：使用 xtask（macOS/Linux）
cargo run -p xtask -- bundle-ai-chat

# Windows MSI（xtask 内部使用 tauri-bundler + WiX，支持 --arch/--target/--install）
cargo run -p xtask -- bundle-ai-chat-windows
```

默认产物目录：

```bash
target/release/bundle/

# Windows (xtask bundle-ai-chat-windows)
target/<target-triple>/release/bundle/msi/
```

macOS 下 `bundle-ai-chat` 会在打包完成后自动尝试注入 Liquid Glass 图标（`.icon -> Assets.car`，并写入 `CFBundleIconName=Icon`）。如果系统未安装可用的 `actool`/`xcrun`，会自动降级为普通图标，不影响打包成功。

## 数据与日志位置

- **macOS**
  - 数据：`~/Library/Application Support/top.sushao.{app}/`
  - 日志：`~/Library/Logs/top.sushao.{app}/data.log`

- **Linux**
  - 数据：`$XDG_CONFIG_HOME/top.sushao.{app}/` 或 `~/.config/top.sushao.{app}/`
  - 日志：`$XDG_DATA_HOME/top.sushao.{app}/logs/data.log` 或 `~/.local/share/top.sushao.{app}/logs/data.log`

## ai-chat 专项说明

### 数据库

- 使用 Diesel + SQLite
- 迁移文件已内置，首次运行会自动初始化数据库

```bash
# 进入 ai-chat 目录
cd app/ai-chat

# 生成迁移（需要 diesel_cli）
diesel migration generate migration_name
```

### 扩展（WASM）

ai-chat 支持 WebAssembly 组件扩展：

```bash
# 在扩展目录中构建（示例：app/ai-chat/extensions/url_search）
cargo component build --release
```

扩展目录：`app/ai-chat/extensions/`
- `extension.wasm`：编译后的 WASM 组件
- `config.toml`：扩展元数据

## 技术栈

- GPUI + gpui-component
- Rust 2024 Edition
- tracing / tracing-subscriber（日志）
- Diesel + SQLite（ai-chat、feiwen）

## 许可

未指定。

## Runtime vs Build Assets

- `app/ai-chat/assets/`: runtime assets only (embedded by `rust-embed`).
- `app/ai-chat/build-assets/`: build/package-time assets only (not embedded for runtime).
- Icon assets live in `app/ai-chat/build-assets/icon/`.
- Windows icon default: `app/ai-chat/build-assets/icon/app-icon.ico` (see `app/ai-chat/build.rs`).
- Package icon paths are configured in `app/ai-chat/Cargo.toml` under `[package.metadata.bundle].icon` and use `build-assets/icon/...`.
- Windows MSI bundling in `xtask` uses `tauri-bundler` and reuses the `.ico` path from `[package.metadata.bundle].icon`.
- macOS bundle icon paths are managed by `crates/xtask/src/main.rs` and use `build-assets/icon/...`.
