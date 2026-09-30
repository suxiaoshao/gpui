# Jaco 退役与仓库边界

归属 [#240](https://github.com/suxiaoshao/gpui/issues/240)。实现基线为 `8afc8e4d`；当前代码与验证范围见下文。

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
| CI 与打包 | 删除递归子模块 checkout；xtask 删除 Jaco 目标及专用测试，安装参数改用 Gupi，通用 CLI 参数与 locales 保留，依赖实际文件和命令的 fixture 随测试清理 |
| 入口与文档 | README、issue/PR 模板、AGENTS、图标 skill 与开发导航改为现役范围；删除 Jaco 专题，保留共享契约和必要的固定历史来源 |

主锁文件从 1586 条 package 记录减少到 1364 条，删除 222 条；按 `(name, version, source)` 比较没有新增记录。剩余传递依赖按可达性保留，不要求曾被 Jaco 使用的包名全部消失。

`cargo shear` 复查进一步移除了 Feiwen 的 `regex`、gpui-form 的两个未使用 dev-dependency、pi-rpc 的 `tracing`，以及根 workspace 中未使用的 `gpui_platform` / `gpui-heatmap` 依赖声明。热力图成员与实现继续保留。

图标依赖通过构建元数据使用，因此在 workspace 的 cargo-shear 配置中保留 `gpui-component-assets`；trybuild 通配符加载的用例通过各自 package 的 `ignored-paths` 声明。Pi 进程 fixture 已随环境依赖测试删除，对应忽略项同步移除。

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

## 环境依赖测试清理

删除 262 个依赖真实文件、进程、TCP、数据库引擎、线程或实际计时的测试：

| 范围 | 删除数量 | 依赖 |
| --- | ---: | --- |
| Gupi | 132 | 文件锁与单实例 TCP、Pi 编译/子进程 fixture、配置和资源落盘、Shell、目录扫描与附件文件 |
| HTTP Client | 39 | HTTP 回环服务、文件请求/保存、响应落盘、媒体文件与 PDF 工作线程 |
| Feiwen | 31 | DuckDB 查询、连接池、备份恢复与文件系统故障注入 |
| Novel Download | 8 | 重定向 TCP 服务与输出文件生命周期 |
| pi-rpc | 26 | 实际 Pi/fixture 子进程、Shell/PATH、管道与真实超时 |
| http-client-test-server | 13 | 服务启动、TCP、CLI 子进程、延迟与连接调度 |
| xtask | 10 | 临时目录、图标打包、文件 manifest 与本机命令发现 |
| gpui-tokio | 3 | 两个实际执行器间的线程调度与退出 |

对应的专用 fixture、测试辅助接口、无消费者的 dev-dependency 和 cargo-shear 忽略项已删除。Feiwen 的无操作故障注入接口和 Novel Download 的测试目录分支同步精简。业务的文件、网络、数据库和子进程能力保留。

纯解析、序列化、状态机、内存队列、受控 GPUI context/headless 交互及 trybuild 编译契约测试保留。后两类有各自的测试层级，不统称为纯单元测试。手动 UI gallery、HTTP 调试服务和 Postman 示例继续保留。

## 验证范围与结果

Jaco 退役后的 macOS workspace 构建与依赖检查已通过。后续测试清理只检查受影响的 8 个包；默认 CI 继续运行三平台 workspace 构建和测试，不增加忽略或屏蔽条件。

| 本轮检查 | 结果 |
| --- | --- |
| 受影响 8 个包 `cargo test --locked --offline` | macOS：398 项测试、1 项文档测试通过，无失败或忽略 |
| 同一组包 `cargo clippy --all-targets --all-features --locked --offline -- -D warnings` | 通过 |
| `cargo shear --locked --offline --deny-warnings` | 无问题 |
| `cargo fmt --all -- --check`、`git diff --check` | 通过 |
| 改动文档的本地链接 | 96 个有效，无断链 |

这组检查不证明真实网络、进程退出、数据库恢复或文件系统行为；Windows/Linux 的本轮结果仍需对应 CI。现有依赖 `block 0.1.6` 的 future-incompat 提示不影响当前检查结果。
