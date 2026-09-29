# Jaco 退役与仓库边界

归属 [#240](https://github.com/suxiaoshao/gpui/issues/240)。基线为 `8afc8e4d`；状态：Implemented。代码、构建配置与文档清理已完成；验证记录见下文。

## 实施结果

workspace 从 25 个成员收敛为 18 个：四个应用为 Gupi、Feiwen、HTTP Client、Novel Download，另有 14 个共享/工具 crate。现役应用保持原有业务、数据目录和协议。

| 范围 | 结果 |
| --- | --- |
| Jaco 专用 owner | 删除 `app/jaco`、`jaco-agent`、`jaco-core`、`jaco-db`、`jaco-conversation`；专用测试、fixtures、资源、locales、schema、migrations 和应用文档随 owner 删除 |
| 旧图标系统 | 删除 `app-assets`、`app-assets-macros`、Lucide gitlink 和空 `.gitmodules`；注销该子模块本地登记，保留 Git 缓存 |
| MCP 测试工具 | 删除独立 `tools/mcp-auth-test-server` 及其 manifest、锁文件和文档 |
| Quick Look | 删除实现、导出、专用错误、Objective-C 数据源/缓存、framework 链接及专用 imports/features |
| OCR | 删除 `platform-ext::ocr`、`OcrError`、专用测试、Windows AI `build.rs` / `winmd` 生成链、Vision 链接及专用依赖/features |
| Cargo | 删除对应成员、路径声明、Rig/RMCP 声明与 Jaco release profile；Cargo 自动收敛锁图，没有升级保留包 |
| Nix | Linux 目标已无 `get-selected-text` / `enigo` / `libxdo` 路径，删除 `xdotool`；锁图已无 SQLite，移除 devShell 的 SQLite。保留 bindgen、DuckDB 及 GPUI 使用的 X11/Wayland 等依赖，Nix 输入版本保持 |
| CI 与打包 | 删除递归子模块 checkout；xtask 删除 Jaco 目标及专用测试，安装参数改用 Gupi，通用包目录、locales、deep-link 与命令失败 fixture 使用中性名称 |
| 入口与文档 | README、issue/PR 模板、AGENTS、图标 skill 与开发导航改为现役范围；删除 Jaco 专题，保留共享契约和必要的固定历史来源 |

主锁文件从 1586 条 package 记录减少到 1364 条，删除 222 条；按 `(name, version, source)` 比较没有新增记录。剩余传递依赖按可达性保留，不要求曾被 Jaco 使用的包名全部消失。

`cargo shear` 复查进一步移除了 Feiwen 的 `regex`、gpui-form 的两个未使用 dev-dependency、pi-rpc 的 `tracing`，以及根 workspace 中未使用的 `gpui_platform` / `gpui-heatmap` 依赖声明。热力图成员与实现继续保留。

图标依赖通过构建元数据使用，因此在 workspace 的 cargo-shear 配置中保留 `gpui-component-assets`；trybuild 通配符加载的用例和 `include_str!` 嵌入的 Pi 进程 fixture 通过各自 package 的 `ignored-paths` 声明，注释指向真实测试入口。没有删除这些用例或增加无依据的忽略项。

## 保留的能力与接口

- `gpui-lucide` 从官方 `gpui-kit-assets` 元数据指定的目录生成 SVG bytes；Gupi、Feiwen 的图标不依赖已删除的子模块。应用自有 provider SVG、品牌资源和主题 JSON 继续由应用打包。
- `window-ext` 保留主窗口/临时窗口使用的显示、隐藏、层级、定位与平台句柄能力。只删除 Quick Look 接口及对应错误，不提供退役兼容入口。
- `platform-ext::app` / `appearance` 保留前台应用与回填、菜单、图标、通知角标和系统外观能力。`windows-core` 仍服务 `IInspectable`，Windows Foundation/UI_ViewManagement 与 Win32 窗口、显示器和键盘接口保留。
- `gpui-heatmap` 保留独立组件、双语 README 和测试；其组件契约留在 [owner 文档](../../../crates/gpui-heatmap/docs/dev/issue-189/README.md)，原 Jaco 用量/费用专题已删除。
- Form、Store、Operation、Tokio bridge、Pi RPC、app-theme、HTTP 测试服务和 xtask 保留既有职责；本次不重构这些共享能力。
- `bundle jaco` 不再是合法目标；四个现役打包目标与 `--install` 行为保留。Gupi 不增加 OCR、Quick Look 或 Jaco 数据迁移。

本次只删除仓库代码与配置。用户本机 Jaco 数据库、配置、会话、附件、凭据、已安装应用和系统授权未触碰，也未清空 target、Cargo/Git 缓存或卸载本机软件。

## 文档边界

Jaco 专用根专题 `issue-178`、`188`、`189`、`190`、`193`、`195`、`196` 随其实现删除。共享 Form/Store/Operation 和框架迁移文档保留独立契约，移除退役 owner 的专用导航和当前执行要求；必要历史引用固定到删除前源码。

两轮依赖盘点保留原日期的版本和数量，不能当作当前成员列表。Gupi 的现行行为直接描述窗口、布局、资源和状态职责，不依赖 Jaco 源码作为使用说明。验收目标是无失效构建依赖与操作入口，不追求所有历史名称为零。

## 验证范围与结果

本次删除不新增 UI 验收、全量回归或独立应用构建矩阵。保留代码的编译由 workspace 构建检查；xtask 的 CLI 和 fixture 有实际改动，因此运行它的现有测试。删除的实现连同专用测试一起移除。

| 检查 | 结果 |
| --- | --- |
| `cargo metadata --locked --offline --no-deps --format-version 1` | 18 个成员，保留路径均有效 |
| Linux 目标的 `cargo tree --locked --target x86_64-unknown-linux-gnu -e features` | 无取词/xdo 路径；这是依赖图检查，不代表 Linux 实机编译 |
| Cargo.lock 的包身份比较 | 移除 222 条，未新增或升级保留包 |
| `nix develop --command cargo build --workspace --locked` | macOS 通过，包含四个现役应用和保留共享库 |
| `nix develop --command cargo fmt --all -- --check` | 通过 |
| `cargo shear --locked --offline --deny-warnings` | 0 错误、0 警告 |
| `cargo check -p feiwen -p gpui-form -p pi-rpc --all-targets --locked` | macOS 通过，包含受影响包的测试目标 |
| `nix develop --command cargo test -p xtask --locked` | 16 项通过，0 失败 |
| Git 索引、代码与配置残余引用 | 无 Lucide gitlink、`.gitmodules`、已删除 API 或专用构建入口 |
| 文档链接与 `git diff --check HEAD` | 检查 343 个改动文档本地链接，无新增断链或退役路径链接；diff 检查通过 |

Linux/Windows 实际构建未在本次执行，现有三平台 CI 保留。全量回归、Clippy 和界面验收不作为本次退役删除的交付检查。
