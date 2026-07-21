# GPUI 应用工作区

一个基于 **GPUI** 的 Rust 桌面应用工作区，包含多个独立应用，统一管理依赖与构建流程。

## 应用列表

- **jaco**：桌面 agent 工作台，支持项目会话、工具、MCP、技能、提示词和快捷键
- **feiwen**：小说 / 网页内容阅读器，支持本地数据库存储
- **http-client**：HTTP 请求测试工具（类似 Postman）
- **novel-download**：小说 / 网页内容下载工具

## 目录结构

- `app/jaco`：桌面 agent 工作台
- `app/feiwen`：阅读器
- `app/http-client`：HTTP 客户端
- `app/novel-download`：下载器
- `crates/window-ext`：窗口相关扩展
- `crates/xtask`：工作区任务工具（打包脚本迁移）

## 环境要求

- Rust 1.95+（Edition 2024）
- 推荐：`cargo-watch`（可选，热重载）
- 可选：`diesel_cli`（仅 jaco 用于管理迁移）

## 构建与运行

```bash
# 构建工作区
cargo build --workspace

# 构建指定应用
cargo build -p jaco
cargo build -p feiwen
cargo build -p http-client
cargo build -p novel-download

# 运行指定应用
cargo run -p jaco
cargo run -p feiwen
cargo run -p http-client
cargo run -p novel-download
```

## 开发与调试

```bash
# 自动重载（需要 cargo-watch）
cargo watch -x 'run -p jaco'
```

## 应用打包

当前已通过 `xtask` 为 workspace app 提供统一打包入口，可直接输出系统应用包（如 macOS `.app`）。

```bash
# macOS/Linux
cargo run -p xtask -- bundle jaco
cargo run -p xtask -- bundle feiwen
cargo run -p xtask -- bundle http-client
cargo run -p xtask -- bundle novel-download

# Windows MSI（xtask 内部使用 tauri-bundler + WiX，支持 --install）
cargo run -p xtask -- bundle jaco --install
```

默认产物目录：

```bash
target/release/bundle/

# Windows
target/<target-triple>/release/bundle/msi/
```

打包前 `xtask` 会从每个 app 的 `build-assets/icon/app-icon.png` 派生 iconset 和 `.ico`。macOS 下如果 app 提供唯一的 `.icon` asset catalog，`xtask` 会在打包完成后自动尝试注入 Liquid Glass 图标（`.icon -> Assets.car`，并写入 `CFBundleIconName`）。如果系统未安装可用的 `actool`/`xcrun`，或 `.icon` 注入失败，会自动降级为普通图标，不影响打包成功。

## 数据与日志位置

- **macOS**
  - 数据：`~/Library/Application Support/top.sushao.{app}/`
  - 日志：`~/Library/Logs/top.sushao.{app}/data.log`

- **Linux**
  - 数据：`$XDG_CONFIG_HOME/top.sushao.{app}/` 或 `~/.config/top.sushao.{app}/`
  - 日志：`$XDG_DATA_HOME/top.sushao.{app}/logs/data.log` 或 `~/.local/share/top.sushao.{app}/logs/data.log`

## 技术栈

- GPUI + gpui-component
- Rust 2024 Edition
- tracing / tracing-subscriber（日志）
- Diesel + SQLite（jaco、feiwen）

## 许可

未指定。

## Runtime vs Build Assets

- `app/jaco/assets/`: runtime assets only (embedded by `rust-embed`).
- `app/{app}/build-assets/`: build/package-time assets only (not embedded for runtime).
- Icon base assets live in `app/{app}/build-assets/icon/app-icon.png`.
- `xtask bundle <app>` derives `app-icon.iconset` and `app-icon.ico` from the base PNG before bundling.
- Windows icon default for `jaco`: `app/jaco/build-assets/icon/app-icon.ico` (see `app/jaco/build.rs`).
- Package icon paths are configured in each app `Cargo.toml` under `[package.metadata.bundle].icon` and use `build-assets/icon/...`.
- Windows MSI bundling in `xtask` uses `tauri-bundler` and reuses the `.ico` path from `[package.metadata.bundle].icon`.
- macOS bundle icon paths are managed by `crates/xtask/src/bundle/` and use `build-assets/icon/...`.
