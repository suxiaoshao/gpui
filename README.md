# GPUI 应用工作区

基于 Rust 2024、GPUI Kit 的桌面应用集合。

| 应用 | 用途 |
| --- | --- |
| [jaco](app/jaco) | Agent 工作台：项目会话、工具、MCP、技能与快捷键 |
| [gupi](app/gupi/README.md) | 本机 Pi 的原生桌面宿主 |
| [feiwen](app/feiwen/README.md) | 小说数据抓取、管理与检索 |
| [http-client](app/http-client) | HTTP 请求测试工具 |
| [novel-download](app/novel-download) | 小说与网页内容下载 |

## 开发与运行

使用支持工作区依赖的 Rust 工具链；依赖版本见 [Cargo.toml](Cargo.toml)。以下以 Jaco 为例，可替换为其他应用包名：

```sh
cargo run -p jaco
cargo test -p jaco
cargo run -p xtask -- bundle jaco
```

打包输出通常位于 `target/release/bundle/`；Windows MSI 位于 `target/<target-triple>/release/bundle/msi/`，支持 `bundle jaco --install`。

## 代码与资源

- `app/`：独立应用；配置、数据位置及业务说明见各应用文档和 `foundation` 模块。
- `crates/`：共享库；`crates/xtask` 提供打包入口。
- `app/{name}/assets/`：运行时资源；`locales/`：运行时及 macOS 本地化。
- `app/{name}/build-assets/`：打包资源。基础图标为 `icon/app-icon.png`，平台派生图标由 xtask 生成；Liquid Glass 构建失败时回退普通图标。

开发约定见 [AGENTS.md](AGENTS.md)，设计与迁移入口见 [开发文档索引](docs/dev/README.md)。
