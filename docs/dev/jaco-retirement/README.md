# Jaco 退役与关联清理

归属 [#240](https://github.com/suxiaoshao/gpui/issues/240)，独立于 Gupi 主 Issue 的后续清理任务。

## 范围与结论

- 核对日期：整体清单为 2026-09-20；Quick Look、OCR 调用与删除范围于 2026-09-26 按当前源码复核。
- 状态：清理清单已整理，尚未执行。按用户已有决定，在 Gupi 合入后处理；本轮只交付文档，不删除代码、子模块或本机数据。
- 目标：删除 Jaco 及失去用途的专用内容，修正其余项目的构建、打包、导航和说明。保留仍维护的应用与独立共享能力，不为退役应用继续升级或迁移。
- Quick Look、OCR 已确定随 Jaco 退役删除，不再作为独立通用能力暂留，也不迁移到 Gupi。
- 入口：[Gupi 统一待处理文档](../issue-217/follow-ups.md)、[依赖更新记录](../dependency-refresh-2026-09/README.md)。本文件是清理范围的统一来源，其他入口只引用。

## 一并删除的内容

| 位置 | 删除范围与依据 |
| --- | --- |
| `app/jaco/` | 应用源码、专用测试/fixtures、assets、provider 图标、主题、locales、build-assets、`build.rs`、manifest 和应用文档一起删除；不迁移到 Gupi。应用的 Windows 资源编译、截屏/热键/模型配置等随入口消失 |
| `crates/jaco-agent/` | Jaco 模型/provider、工具、MCP、资源加载等运行时及对应测试、文档 |
| `crates/jaco-core/` | Jaco 专用领域类型、测试与文档 |
| `crates/jaco-db/` | Diesel/SQLite 数据层，包括仓库内 migration、schema、数据库开发配置与测试；不删除用户数据库 |
| `crates/jaco-conversation/` | Jaco 会话编排、测试与文档 |
| `crates/app-assets/`、`crates/app-assets-macros/` | 当前唯一应用使用方是 Jaco。Gupi、Feiwen 已迁到 `gpui-lucide`；旧路径注册宏、元数据、组合资源及其专用测试/文档一起删除 |
| `window-ext` 的 Quick Look | 当前唯一应用调用在 Jaco 普通文件附件预览；Gupi 无调用。删除 `src/quick_look.rs`、模块声明、`preview_file_with_quick_look` 导出、Quick Look 专用错误类型、`QuickLookUI` framework 链接及专用 Objective-C 数据源和缓存；清掉只供该功能使用的 imports、依赖 features 与说明。保留整个 `window-ext` crate 及现役窗口控制 API |
| `platform-ext::ocr` | 当前仅 Jaco 截屏功能调用，Gupi 无调用。删除 `src/ocr.rs`、`src/ocr/`、模块及 `OcrError` 导出/定义、OCR 专用测试与说明；删除 `winmd/`、生成 `windows_ai_bindings.rs` 的 `build.rs`、manifest 中的 build 声明与 `windows-bindgen` 构建依赖、Windows AI/OCR 专用 features 和 macOS `Vision` framework 链接。按剩余引用清理 `windows-core`、`windows-future` 等直接依赖，保留 `platform-ext::app`、`appearance` 和对应原生依赖 |
| `tools/mcp-auth-test-server/` | README 明确服务于 Jaco bearer/OAuth 测试；连同独立 `Cargo.lock`、空 `[workspace]` 和工具文档删除，不先搬进主 workspace。当前 `tools/` 没有其他工具，删后无需保留空目录 |
| `third_party/lucide` | 旧 `define_lucide_icons!` 读取此目录；新 `gpui-lucide/build.rs` 读取 `gpui-kit-assets` 的 `DEP_GPUI_KIT_DEFAULT_ICONS_ICONS_DIR`。Jaco 与旧宏删除后，子模块不再参与任何保留应用的图标构建 |

对应证据：[根 manifest](../../../Cargo.toml)、[旧宏路径](../../../crates/app-assets-macros/src/lib.rs)、[新图标生成器](../../../crates/gpui-lucide/build.rs)、[MCP 工具说明](../../../tools/mcp-auth-test-server/README.md)。这些链接在清理完成时需同步移除或改写，避免留下失效导航。

Quick Look 的现有调用链为 Jaco `open_attachment` → `open_file_preview` → `window_ext::preview_file_with_quick_look`，失败后退回系统打开；见 [Jaco 附件流程](../../../app/jaco/src/components/chat/input/attachment_flow.rs)和 [Quick Look 实现](../../../crates/window-ext/src/quick_look.rs)。删除依据是唯一消费者随 Jaco 退役，而非“从未接入”。

## Cargo 与依赖图调整

1. 从根 `workspace.members` 删除 Jaco、四个 jaco-* crate、两个旧 assets crate，共 7 个成员。按当前清单，25 个成员会剩 18 个（4 个应用、14 个共享/工具 crate），其中包含保留的 `gpui-heatmap`。
2. 删除 `workspace.dependencies` 中 `jaco-agent`、`jaco-conversation`、`jaco-core`、`jaco-db`、`app-assets` 的路径声明，以及无使用方的 `rig`、`rmcp` 声明。删除 `[profile.release.package]` 下 Jaco 专项；如果表已空，一并移除空表。
3. 更新主 `Cargo.lock`，让 Cargo 按剩余成员重新求解并移除不可达包。保留现有允许范围，避免借清理重新全量升级依赖；不按名字手工批量删锁文件条目。
4. 工具自己的锁文件随目录删除。后续新增且仍维护的 Rust 工具放入 `crates/` 并加入主 workspace，共用主锁文件，不延续当前独立工具的组织方式。
5. 收敛 `platform-ext` 的 OCR 专用构建链和依赖：移除 `build.rs`、`winmd/`、`build = "build.rs"` 及 `windows-bindgen`，核对 Windows AI/OCR features 和直接依赖在其余模块中的使用后删除无用声明。当前 `windows-future` 只用于 OCR；`windows-core` 仍由 `appearance.rs` 使用，继续保留。保留 `app` / `appearance` 使用的系统交互与外观 API；不整包删除 `platform-ext`。

以下为整包退役对象独占的直接依赖清单；OCR 模块删除带来的额外依赖收敛按上一步处理：

`anyhow`、`async-trait`、`axum`、`diesel`、`dirs`、`grep-matcher`、`grep-regex`、`grep-searcher`、`hex`、`libsqlite3-sys`、`notify-debouncer-full`、`rig`、`rmcp`、`rust-embed`、`schemars`、`sha2`、`similar`、`unicode-segmentation`、`winresource`、`xcap`。

这里列出的是会消失的直接依赖边，不保证这些包从锁文件全部消失。例如 GPUI 或其他上游仍可能传递依赖 `rust-embed`、`anyhow`、`sha2`；以新依赖图可达性为准。

以下声明明确继续保留：

- `gpui-lucide`、`gpui-component-assets`（实际包 `gpui-kit-assets`）：提供新 SVG bytes 图标和组件默认图标。删除 Git 子模块不等于停止使用 Lucide。
- `proc-macro2`、`quote`、`syn`：`gpui-form-macros` 仍使用。
- `garde`：`gpui-form` 仍使用。
- `image`、`reqwest`、`tokio`、`serde` 等多应用依赖：随 Jaco 删除其局部声明，不删除其他消费者的依赖。
- 根 GPUI 配套版本/alias 保持统一，检查删除 Jaco 后的 feature 合并结果，避免其他应用偶然依赖 Jaco 开启的 feature。重点看语法高亮语言与平台功能，各保留应用应能单独构建。

## 子模块、CI 与脚本

| 文件/位置 | 清理时的调整 |
| --- | --- |
| [`.gitmodules`](../../../.gitmodules) | 当前只有 Lucide 一项；正确移除 Git 索引中的 `160000` gitlink 和对应节，之后删除空 `.gitmodules`。不能只删磁盘目录，否则克隆仍会拉取或报失效路径 |
| [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) | 删除 checkout 的 `submodules: recursive`。保留正常 checkout、Rust 缓存、三平台矩阵和 workspace build/test/clippy；当前没有独立 Jaco CI job，无需凭空拆改矩阵 |
| 本地 Git 子模块登记 | 操作前检查子模块自身工作区，保留未提交内容；通过 Git 的子模块移除流程处理本地登记。`.git/modules/third_party/lucide` 属于本地缓存，不是需要提交的源码删除；不为仓库清理顺便清空其他 worktree 或 Git 历史 |
| [flake.nix](../../../flake.nix)、[flake.lock](../../../flake.lock) | macOS/Linux 开发环境及原生依赖统一由 Nix 维护，旧安装脚本已移除。退役后依据保留应用的 native 依赖和 Linux 构建证据逐项判断无用项，不增加本机反向卸载操作 |
| `script/gupi-runtime-gallery`、`script/gupi-ui-gallery` | 服务于 Gupi，保留；当前没有旧子模块初始化或 Jaco 启动命令 |
| [`.github/dependabot.yml`](../../../.github/dependabot.yml) | 当前只管理 GitHub Actions，没有 MCP 工具目录或子模块的更新任务，无需调整 |
| [`.zed/tasks.json`](../../../.zed/tasks.json)、[`.gitignore`](../../../.gitignore) | 当前无 Jaco 专用任务；图标派生产物忽略规则为 `app/*` 通用规则，保留。没有发现需要删除的子模块初始化脚本 |

清理的直接收益是：普通 checkout 不再要求递归子模块；CI 不再下载旧 Lucide 仓库；workspace 构建不再编译 Jaco 及其专用运行时。

## 打包、模板与有效导航

| 位置 | 删除或调整 |
| --- | --- |
| [crates/xtask/src/cli.rs](../../../crates/xtask/src/cli.rs) | 删除 `BundleApp::Jaco`、名称映射及仅验证 `bundle jaco` 的测试。将 `--install` 通用解析测试改为 Gupi，保留 Windows 安装参数覆盖；`bundle gupi/feiwen/http-client/novel-download` 保持可用 |
| [crates/xtask/src/bundle/settings.rs](../../../crates/xtask/src/bundle/settings.rs) | Jaco 名称、`top.sushao.jaco`、`jaco-screenclip` 位于通用打包测试 fixture。改为中性的测试应用和 `fixture-*` scheme；保留相对路径、locales、deep-link 元数据转换测试，不把 fictitious scheme 加入 Gupi 真实 manifest |
| [crates/xtask/src/bundle/macos.rs](../../../crates/xtask/src/bundle/macos.rs)、[cmd.rs](../../../crates/xtask/src/cmd.rs) | `Jaco.app` 目录样本及 `jaco-xtask-command-that-must-not-exist-*` 改为中性测试名字。通用包目录查找、命令失败测试继续保留 |
| [README.md](../../../README.md) | 删除应用表中的 Jaco，运行/测试/打包示例改为 Gupi，Windows `--install` 示例同步改名 |
| 三份 `.github/ISSUE_TEMPLATE/{bug_report,feature_request,tech_request}.yml`、`.github/pull_request_template.md` | 当前 scope 列表含 Jaco、缺 Gupi；移除 Jaco，补上 Gupi，不删除通用 workspace/CI/packaging 入口 |
| [AGENTS.md](../../../AGENTS.md) | 删除 Jaco Diesel/migration/schema/provider adapter 专用约定，保留其他项目约定 |
| `.agents/skills/gpui-app-icon-usage/SKILL.md` | 删除“旧 assets 与子模块暂留给 Jaco”的过渡说明；保留新 `gpui-lucide`、默认组件资源和应用自有 SVG 的说明 |
| Gupi 源码注释 | `features/settings/preferences.rs` 与 `features/home/navigation.rs` 中 “Match Jaco” 是设计来源，不是运行依赖。改成直接描述当前网格/导航行为即可，无需改实现 |

没有发现额外的发布 workflow 或 Jaco 专用 CI 打包任务；打包分派由上述 xtask 和应用 manifest 控制。

## 文档整理

### 随专用代码删除

- 被删除应用/crate/tool 目录内的 README、开发计划、测试指南、fixture 说明随 owner 一起删除。
- 根 `docs/dev/issue-178/`、`issue-188/`、`issue-190/`、`issue-193/`、`issue-195/`、`issue-196/` 是 Jaco 专题；删除专题并从根开发索引移除入口。失效的旧验收项不转为 Gupi 待办。
- 根 `docs/dev/issue-189/` 是 Jaco 用量/费用/热力图接入专题；若保留 `gpui-heatmap`，先把它仍需的独立组件说明留在该 crate 自己的 README/计划中，再删除 Jaco 应用统计专题。修正 `crates/gpui-heatmap/docs/dev/issue-189/README.md` 对根专题的链接。

### 保留共享结论，移除失效引用

- `docs/dev/issue-175/`、`issue-182/`、`issue-199/`、`issue-215/`、`docs/dev/migrations/` 和共享 crate 内的计划包含跨应用/API 结论，不能因出现 Jaco 就整目录删除。去掉 Jaco 专用工作包、失效当前任务和已删除源码链接；必要历史证据保留固定提交来源，不继续导航到不存在的工作区文件。
- Gupi 的输入框、临时窗口、设置等文档曾以 Jaco 为参考。保留已经确认的 Gupi 行为；对仍有比较价值的来源引用固定历史提交，对无用的逐文件比较删去。不要为了维持参考链接保留整套 Jaco 源码。
- Form/Store/Operation 指南中的 Jaco 示例，能说明通用契约的改为当前消费者或中性例子；不删除这些共享能力的设计说明与回归要求。
- [依赖更新记录](../dependency-refresh-2026-09/README.md) 的版本盘点保留其基线语义；“暂留给 Jaco”“未来删除”等当前状态改为实际结果。更新[统一待处理文档](../issue-217/follow-ups.md)与[根开发索引](../README.md)，不复制第二份清单。

## 共享能力的保留边界

Quick Look、OCR 已列入上方一并删除范围。其他共享能力继续按其实际消费者和已确认用途保留。

| 能力 | 当前证据与保留/删除边界 |
| --- | --- |
| `gpui-heatmap` | 唯一应用消费者是 Jaco，但有独立通用组件契约、中英文 README 和自己的测试。按已有依赖更新结论保留；不能删除 `time` 或 GPUI 依赖来间接破坏它 |

**明确有现役消费者的共享内容：**`platform-ext::app` 被 Gupi 临时窗口用于前台应用/回填/鼠标显示器；`platform-ext::appearance` 被 `app-theme` 使用；`window-ext` 被 Gupi 主/临时窗口使用；`app-theme` 被 Gupi、Feiwen 使用；Form、Store、Operation、Tokio bridge、Pi RPC、HTTP 测试服务和 xtask 各自保留。应用自己的图标、provider SVG 和官方 `gpui-kit-assets` 资源也继续保留。

## 实施顺序与足够的验证

1. 开始清理时重扫当前引用和子模块状态，确认没有新增消费者或用户未提交的子模块改动；只处理已确认范围。
2. 删除专用 owner、workspace 声明、Jaco 打包枚举及 profile；移除子模块 gitlink / `.gitmodules` 与 CI 递归拉取配置，更新主锁图。
3. 删除 Quick Look 与 OCR 的实现、导出和专用支持内容，连同 OCR 的 Windows 元数据/绑定生成链及平台专用依赖一起清理，核对已无应用调用。保留现役窗口控制、前台应用交互、系统外观能力。改写通用测试 fixture、模板、项目说明、skills 和文档导航，按上面的边界保留其他共享能力。
4. 核对 `cargo metadata --no-deps --format-version 1`：无已删除成员/路径；检查新依赖图，不将仍有传递使用方的包误报为遗漏。
5. 对剩余 workspace 执行格式、构建、现有测试和 Clippy；这时无需再携带 `--exclude jaco*`。额外关注 xtask CLI、Form 宏、图标 bytes、设置和消息显示的现有回归，不重写已经覆盖的测试。
6. 用不初始化子模块的干净 checkout 验证构建；确认根 manifest、CI、scripts 没有旧目录引用，macOS/Windows/Linux 的实际结果分别记录。Windows 安装参数解析由测试覆盖，不自动在用户机器安装或卸载软件。
7. 检查改动文档链接与有效导航。清理后允许必要的历史名称/出处存在，验收目标是无失效构建依赖和当前操作指令，不追求全仓库 `jaco` 字符串为零。

不删除用户本机 Jaco 配置、会话、附件、SQLite 数据库、凭据、已安装应用或系统授权；也不自动清空 `target/`、Cargo/Git 缓存。它们不属于此次仓库退役范围。

## 本轮检查结果

整体清单已核对根与各 owner manifest、源码引用、`.gitmodules` / gitlink、`.github/`、`script/`、`.zed/tasks.json` 及文档导航。Quick Look、OCR 的删除范围已复核应用调用与库实现，OCR 同时核对了 Windows 绑定生成链。尚未执行代码清理；文档变更验证为链接、事实对照和 diff 检查，不代表删除后的构建已通过。
