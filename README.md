# GPUI 应用工作区

基于 Rust 2024、GPUI Kit 的桌面应用集合。

| 应用 | 用途 |
| --- | --- |
| [feiwen](app/feiwen/README.md) | 小说数据抓取、管理与检索 |
| [http-client](app/http-client) | HTTP 请求测试工具 |
| [novel-download](app/novel-download) | 小说与网页内容下载 |

Gupi 已迁至独立项目 [suxiaoshao/gupi](https://github.com/suxiaoshao/gupi)，在新仓库开发、打包和发布。

## 开发与运行

macOS / Linux 使用 `nix develop` 进入由 `flake.lock` 固定的 Rust、C/C++ 和系统库环境。Rust 版本与组件统一声明在 `rust-toolchain.toml`；Windows 原生开发使用 Rustup / MSVC，WSL 使用 Linux 环境。

macOS 仍需完整 Xcode 与 Metal 工具（缺少组件时执行 `xcodebuild -downloadComponent MetalToolchain`）；Linux 运行 GUI 需要宿主图形会话和驱动。Feiwen 的 DuckDB 继续通过 Cargo 的 bundled feature 编译。

```sh
nix develop
# 使用当前 Fish：nix develop --command fish --no-config
```

CI 的 macOS / Linux 检查进入同一 devShell，Windows 使用同一 Rust 版本的原生工具链。以下以 Feiwen 为例，可替换为其他应用包名：

```sh
cargo run -p feiwen
cargo test -p feiwen
cargo run -p xtask -- bundle feiwen
```

打包输出通常位于 `target/release/bundle/`；Windows MSI 位于 `target/<target-triple>/release/bundle/msi/`，支持 `bundle feiwen --install`。

## 代码与资源

- `app/{name}/src/`：应用启动、窗口、导航和功能组合；`app/{name}/crates/`：应用专属业务能力。配置、数据位置及业务说明见各应用 README。
- `crates/`：共享库；`crates/xtask` 提供打包入口。
- `app/{name}/assets/`：运行时资源；`locales/`：运行时及 macOS 本地化。
- `app/{name}/build-assets/`：打包资源。基础图标为 `icon/app-icon.png`，平台派生图标由 xtask 生成；Liquid Glass 构建失败时回退普通图标。

开发约定见 [AGENTS.md](AGENTS.md)，设计与迁移入口见 [开发文档索引](docs/dev/README.md)。

官方 GPUI skills 使用 npx skills 管理，来源由 [skills-lock.json](skills-lock.json) 记录；项目自有架构和接入规则位于 `.agents/skills/`。
