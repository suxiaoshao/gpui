# 原生体验与应用图标

状态：图标主题当前为七套；彩虹十格按行使用红橙黄、绿蓝、紫红橙、绿蓝，复用红、橙、黄、绿、蓝、紫六种纯色，不提供彩虹渐变。其余本机与跨平台验证边界见文末。关联 [#223](https://github.com/suxiaoshao/gpui/issues/223)。

## 目标与范围

- 让菜单栏标记在小尺寸、不同明暗背景下可辨认，继续展示原有未读数字。
- 保持 Pi 标记的轮廓，提供经典、经典渐变、Pi 彩色、Pi 渐变、彩虹、乌克兰及乌克兰渐变七套图标主题。
- 为 macOS 提供分层 `.icon` 源文件；验证用户选择主题时，Dock 能应用什么资源、恢复默认后是什么行为。
- 当前七个主题为经典、经典渐变、Pi 彩色、Pi 渐变、彩虹、乌克兰、乌克兰渐变；默认保留经典。
- 彩虹使用六种纯色复用填满十格，不提供彩虹渐变或贯穿式横条；乌克兰保持上下蓝黄双色。

## 当前实现

### Tray

`app/tray.rs` 在 macOS 使用 `assets/brand/tray-template.png`，不再缩放完整应用图标。源 SVG 收紧留白；PNG 为 36×36，tray-icon 按 18pt 显示并启用 template，由系统处理菜单栏前景色。

菜单保持两个窗口入口，动态会话区域插在设置/退出之前；原有数字、通知目标与动作路由不变。Windows 继续使用现有 32px 应用图标。暂不添加彩色 Tray 偏好。

### 图标主题设置

外观页提供带图片预览的单选项，使用既有设置/Radio 组件。`AppConfig.icon_theme` 保存主题，沿用设置的即时保存与按字段合并，不提交 Pi 路径编辑草稿。

`state/icons.rs` 只记录已应用值以避免重复原生调用，配置仍是持久化来源；Welcome 等使用统一 `app_logo` 的应用内品牌标记同步更新。功能性 Lucide 图标、模型提供商 Logo 不随品牌配色改变。

macOS 非默认主题优先加载 Assets.car 中具名图标；未打包的开发运行使用同源 Icon Composer 导出的 PNG。经典主题恢复 bundle 默认。Windows/Linux 目前只更新应用内标记，不声称改变系统任务栏或启动器图标。设置说明已明确范围。

`script/generate-icon-themes.py` 从官方 SVG 路径与配色定义生成变体、各 `.icon` 源目录和应用运行所需的 PNG；源目录/运行资源提交，编译的 Assets.car、icns/iconset 等仍在打包阶段生成。xtask 读取 `build-assets/icon/default-icon` 选择默认图标并一次编译所有 `.icon`；其他应用只有单个图标时保持既有行为。

经典保留原有系统明暗外观；其余六款使用 Default / Dark 相同的有色基底（淡蓝紫、蓝灰、淡紫或明紫）。Pi 渐变遵循官方三色的空间关系，采用上方珊瑚、左下蓝、右下金的三向融合；乌克兰渐变在蓝黄两端之间加入亮青过渡，减少暗淡的橄榄色中段。乌克兰两款背景使用明紫 `#A67AD8`，Default / Dark 导出预览均已检查；此次仅更换背景，蓝黄 SVG 标记及其渐变保持原样。具体颜色与生成方法见[图标资源说明](../../../build-assets/icon/README.md)。

### 独立图标程序

[构建脚本](icon-probe/build.py) 与 [AppKit 程序](icon-probe/main.swift) 仅用于开发验证，不加入 Gupi 的运行路径，不读写其用户配置，不连接 Pi，也不修改 Finder 或已安装应用。

构建时在独立临时目录生成：

- `Classic.icon`：复用当前 `Gupi.icon` 的浅色/深色配置。
- `Color.icon`：复用同一结构，使用官网三色 SVG，并去掉会覆盖三色的统一前景填充。
- 两份资源由 `actool` 一起编译为 `Assets.car`；默认使用 Classic，保留编译器输出的图标 plist 字段与 `.icns`。
- `ictool` 从 Color.icon 导出 PNG，用于比较位图与编译资源的动态切换。

程序提供恢复默认、切换 PNG、切换编译资源三项操作，以及独立的模板 Tray。切换 PNG 会请求测试数字 1，恢复默认会清除数字；Dock 数字仍受系统 Badge 授权影响，不把数字出现作为图标验证的前提。

复现命令（从仓库根目录执行，需要 Xcode 的 actool、Swift、Icon Composer/ictool）：

```sh
python3 app/gupi/docs/dev/issue-223/icon-probe/build.py
# 使用输出的确切路径打开 Gupi Icon Probe.app。
# 验证结束退出该程序，临时目录中的产物可删除。
```

脚本不下载资源，不安装应用。生成的 `.icon`、`.car`、`.icns`、PNG 和 app bundle 不提交 Git；可从同一脚本重建。经典主源继续归属 `build-assets/icon/Gupi.icon`。

## 已核实的边界

截至 2026-09-24：

| 能力 | 结论 |
| --- | --- |
| 官网新色 | `#F09082`、`#4D9ABF`、`#F1BE58` 三块纯色；当前 SVG 无渐变 |
| 打包 `.icon` | 项目已有 actool 接入；小程序同时编译两份成功 |
| 动态 PNG | AppKit `applicationIconImage` 可设置，传 nil 恢复打包图标 |
| 编译资源 | 本机 `NSImage(named: "Color")` 能加载 Assets.car 中的 Color，不能沿用“只能 PNG”的假设 |
| Liquid Glass | 已验证资源加载与赋值路径，尚未确认动态替换后 Dock 保留完整动态材质/外观响应 |
| Finder／退出后的图标 | 本轮不修改；不能由运行中的 Dock setter 推导出永久替换能力 |
| iOS alternate icon API | UIApplication 接口，不作为 macOS 实现依据 |

资料：[Pi logo](https://pi.dev/logo-auto.svg)、[Pi Press Kit](https://pi.dev/press-kit)、[Icon Composer](https://developer.apple.com/documentation/xcode/creating-your-app-icon-using-icon-composer)、[applicationIconImage](https://developer.apple.com/documentation/appkit/nsapplication/applicationiconimage)、[alternate icons](https://developer.apple.com/documentation/xcode/configuring-your-app-to-use-alternate-app-icons)。

## 图标后续验证

- 已实现统一主题选择、应用内同步、macOS 切换与多图标打包；继续验证真实 Dock 的材质与系统外观响应。
- 不通过修改已签名 bundle 的 Info.plist/Assets.car 来切换图标，不写用户全局 Finder 或系统主题设置。
- 菜单栏使用单色适配系统。本阶段不增加彩色 Tray 跟随，也不改变 Finder／退出后的图标，保持已确认的范围。

## 图标迭代验证（当时的八套主题版本）

本机 macOS 27.0 (26A428)、Xcode 27.0 (27A266a)。当前七套主题的 release `.app` 已重新构建，七份 `.icon` 同时编译，ad-hoc 签名校验通过；以下保留当时八套主题版本的交互验证范围。

- [x] Gupi 构建、gupi/platform-ext/xtask 的 Clippy 与格式检查通过；文档链接、图标 JSON、10 个新增中英文翻译键及 `git diff --check` 通过。
- [x] 配置回归 11 项通过（包含图标主题保存不提交 Pi 草稿）；macOS 打包测试 5 项通过（包含多主题默认选择）。
- [x] 八份 `.icon` 同时编译为 Assets.car；隔离的真实 Gupi 包显示全部预览，鼠标选择、Tab/空格选择、配置保存与重启后恢复“Pi 渐变”均已验证，选中项同时显示勾选和边框。
- [x] 验证使用独立 GUPI_CONFIG_DIR/GUPI_DATA_DIR/GUPI_LOG_DIR/PI_CODING_AGENT_DIR，Pi 命令为 `/usr/bin/false`，未连接真实 Pi 或读取用户历史。
- [x] 两份 `.icon` 经 actool 编译，ictool 导出彩色 PNG，Swift 验证程序构建与启动成功。
- [x] 原生界面点击编译资源、PNG、恢复默认三项动作；界面状态分别确认执行完成，无异常。`NSImage(named: Color)` 返回可用对象。
- [x] 验证程序已退出；没有修改已安装 Gupi、系统外观或 Finder 图标。
- [ ] 实际 Dock 中的 Liquid Glass 材质和外观切换仍需目视确认。自动化可以读小程序窗口，但读取 Dock 超时，因此不把 setter 成功写成视觉验收通过。
- [ ] 正式 Gupi 的 Tray 小尺寸/动态列表和 Windows 仍未完成目视/平台验收；不以 macOS 设置页验证替代。

当前七套图标主题见上文“图标主题设置”；彩虹为六色纯色复用填入十格。

## 桌面功能实现

### 菜单与 Tray

- 应用菜单分为 Gupi、会话、编辑、显示、窗口、帮助。macOS 提供 Services、隐藏/显示其他应用和本地化系统窗口列表；Windows/Linux 使用同一份菜单定义的 AppMenuBar。
- 会话命令复用现有 `Run` Action，并根据当前窗口的能力启停；编辑命令使用输入组件 Action 和平台 OsAction，作用于当前焦点。命令面板从当前主/临时窗口进入原有面板，Dialog 与图片预览的动作拦截继续生效。
- 帮助入口包括 Gupi 使用指南、Pi 文档、报告问题、日志位置与复制基础诊断；关于弹窗和设置的关于页也可复制诊断。只包含版本、平台/架构、Pi 命令和配置/日志位置，不复制会话正文、环境变量或凭据，不自动上传。
- 菜单定义只在语言、键位或前台窗口的命令可用性改变时重建；组件菜单订阅专用菜单变更信号，避免随组件全局状态和流式正文重复刷新。
- Tray 保留主窗口、临时窗口、设置和退出；活动会话按待答、未读、运行中排序。显示实际应用键位和已注册临时窗口快捷键；系统快捷键只展示，不通过 Tray 重复注册。
- 通知偏好仍在现有通知设置中管理；不引入第二套 DND 或会话状态。

### Startup

欢迎页之后是三个任务页，共享同一个 `Form<AppConfig>`：

| 页面 | 配置 | 默认/继续方式 |
| --- | --- | --- |
| 语言与外观 | 九种语言与跟随系统、亮暗模式；展开可选浅色/深色主题和应用图标 | 默认跟随系统及经典图标，即时预览，保留默认可继续 |
| 连接 Pi | 自动发现或指定 Pi 路径、版本检测和失败反馈、Pi 使用文档 | 可“稍后配置 Pi”；未检测成功也能继续 |
| 快捷键与通知 | macOS/Windows 临时窗口全局快捷键、回答完成通知、等待用户操作通知；Linux 不显示全局键位控件 | 快捷键默认为空；完成默认后台通知，待答默认开启；可沿用默认完成 |

欢迎页也可直接使用默认设置。所有选项完成时才提交；已有配置不重新进入引导。图标选择复用外观页的控件；临时窗口键位复用现有录制/清除/冲突校验，不手动输入键位，不引入多段快捷键。

Pi 检测只属于设置检查；主窗口与临时窗口不等待应用级检查。缺少 Pi 或无法连接在实际创建的会话中反馈，用户仍可使用本地界面。模型和登录由 Pi 管理，不创建第二份默认模型配置，不自动创建会话探测模型。

### 九种语言与平台资源

本轮一次覆盖英语、简中、繁中、日语、韩语、德语、法语、西语、巴西葡语。应用 Fluent 资源使用统一注册表；系统语言解析保留繁简脚本/地区区别，缺失译文使用英语回退。组件语言映射以锁定的 gpui-kit 资源为准，键和参数保持一致。

macOS 的明确语言选择在创建 GPUI 应用之前写入进程的 volatile `NSArgumentDomain/AppleLanguages`，保留该 domain 的其他键；不写系统全局或持久 defaults。“跟随系统”不创建覆盖。应用启动时保留覆盖前的系统语言，保证从明确语言切回“系统”时应用文案可以即时恢复。应用文案热切换，原生对话框在用户重启后应用新语言，不自动停止任务。

打包使用应用自己的 `package.metadata.bundle.localizations` 声明，生成对应的 `.lproj`、InfoPlist.strings、CFBundleLocalizations 和 WiX 语言配置；其他应用不被迫补齐九种语言。Windows 继续 MSI，为每种语言生成对应安装包，内置 WiX UI 加自有 `.wxl` 文案覆盖安装、升级、卸载流程；`--install` 默认选择英文包，用户可直接使用其他语言产物。Windows 公共对话框跟随系统显示语言，应用提供的文字由应用翻译。不引入 NSIS/Tauri 运行时。

### 权限恢复、单实例与日志

- 设置提供 macOS 辅助功能说明和系统设置入口；通知页提供通知/Badge 说明及 macOS/Windows 系统设置入口。不在 Startup 同时索取全部权限，不重复弹授权框；系统授权后重新使用相应功能，沿用现有实时预检与剪贴板保留行为。
- 同一配置目录由标准文件锁持有一个进程。锁由操作系统随进程退出释放；私有 loopback 端点只接收唤醒主窗口请求，不传会话内容、不创建 Pi。再次启动收到已有进程确认后退出；不同 `GUPI_CONFIG_DIR` 保持独立。
- 配置目录保留 `instance.lock`（不删除以避免多个锁 inode）及当前端点 `instance.json`；退出后端点记录可存在，下次取得锁时原子替换，记录本身不判定进程存活。
- 日志沿用 `gupi.log`，按约 5 MiB 轮转，保留三个归档；不自动上传。日志位置和可复制诊断入口便于定位启动、权限及连接问题。
- 关闭窗口、临时窗口回收和受管退出沿用当前所有权：隐藏不停止 Pi，退出等待配置和 Pi 关闭。不增加退出确认、自动恢复上次选中会话或临时会话跨重启保存。

## 依据与已确认选择

对照 Jaco 的原生菜单/跨平台菜单组件、Zed 的应用/编辑/窗口/帮助分组，复用 Gupi 现有动作；本地 Jaco 未发现独立 Tray 实现，不把窗口菜单描述为 Tray。锁定的 tauri-bundler 2.9.4 提供 MSI 多语言配置；原生 AppKit 资源通过现有 platform-ext 接入。

用户已确认：九种语言一次完成；Pi 可稍后配置；原生语言允许手动重启生效；保持 MSI；图标只扩展应用内与运行时 macOS Dock、Tray 单色；同一配置目录单实例。没有剩余产品待确定项。

## 本轮验证

- [x] 六款派生图标的有色背景、Pi 三向渐变与乌克兰亮青中间色已同步 SVG、`.icon` 和 PNG；Default / Dark 原生导出均成功。隔离 release 包的设置页已在浅色、深色模式实点检查，背景与新渐变正常显示；七套图标重新编译打包，ad-hoc 签名校验通过。经典 PNG 与官方纯色 SVG 未改变。
- [x] 改动前在隔离 Gupi 测试包直接启动两次，观察到两个独立进程，确认单实例接入有实际依据；测试进程已关闭。
- [x] 当前改动的 Gupi 245 项、platform-ext 1 项、xtask 17 项测试通过。`cargo clippy -p gupi -p platform-ext -p xtask --all-targets --all-features --locked -- -D warnings`、workspace 格式检查与 `git diff --check` 通过。单实例回归需要本机 loopback socket；在沙箱中因权限失败，在正常本机权限下复测通过。
- [x] 新引导使用英文、Pi 命令 `/usr/bin/false`，实点“稍后配置 Pi”和完成；偏好正确保存，能够进入主界面，Pi 错误只在会话内反馈。已有配置重启跳过引导。
- [x] 原生六组菜单可用；运行验证发现 About 的全局动作与派发窗口借用冲突，改为保留目标后 defer 执行，并补回归。最终 release 包实点已确认弹窗正确出现；Edit → Select All 实际选中当前草稿，删除后 Edit → Undo 正确恢复。原生菜单退出成功。
- [x] 同一配置目录再次启动在约 0.03 秒退出并唤醒已有主窗口；独立配置与锁释放由回归覆盖。
- [x] 九种 Fluent 资源在移除彩虹渐变选项后均为 530 个消息键，解析、键/参数和系统语言映射校验通过；九份 WiX XML 可解析，语言及安装包选择回归通过。
- [x] 当时八套主题版本的正式 macOS release `.app` 构建完成；Info.plist 与资源目录都包含九种本地化，Assets.car 包含八个具名图标。`codesign --verify --deep --strict` 通过（ad-hoc 签名）。英文配置重启后，系统文件选择框的 Open、Recents、Cancel 等原生文字实测为英文。

- [x] 会话删除后不再自动创建替代草稿：当前项删除后清除选择，显示“开始一段对话”和“新建会话”入口；后台删除保持当前选择。未落盘会话按 key 匹配，不误删其他空路径草稿，不执行文件删除。删除相关 7 项回归通过，覆盖已落盘会话、启动失败、其他草稿与附件保留、等待自身 Pi 退出、删除失败，以及删除后由用户显式新建。`cargo clippy -p gupi --all-targets --locked -- -D warnings` 通过。
- [x] 删除后的 release 原生界面验证通过：独立测试配置使用 `/usr/bin/false` 复现 `/` 下启动失败的“未命名会话”；实点“移到废纸篓”后会话与空项目分组均消失，侧栏显示“暂无会话”，正文显示新建入口。点击正文新建按钮后仅出现一条会话，再次删除仍正确清空；测试应用已退出。正式 macOS 包已重建，Liquid Glass 图标资源编译成功。

界面验证使用独立配置、数据、日志与 Pi 目录。Pi 失败路径使用 `/usr/bin/false`；流式路径使用 `script/gupi-runtime-gallery --prepare-only` 的离线本地 faux provider。测试不发送真实模型请求；自动发现 Pi 仅检测版本。

平台边界：本机为 macOS 27/Apple Silicon。Windows MSI 安装、升级、卸载、缩放及原生菜单/热键，Linux .deb 和 Intel/旧版 macOS 没有本机实测条件，需对应平台验收。已有 CI 编译不等于实机可用性验收；本地 ad-hoc 签名不等于 Developer ID 公证。Dock 动态 Liquid Glass 的材质/系统外观变化仍需目视确认。

### 2026-09-25 本机验收

使用当前 release 包的独立副本、独立配置/数据/日志/Pi 目录及 Pi 0.87.1，本地 faux provider 复用 `script/gupi-runtime-gallery` 与 `tests/fixtures/runtime-gallery.mjs`。不读取真实会话、不请求外部模型、不改变用户设置。应用副本、测试进程和测试工作区在记录结果后清理。

- 长会话：生成并由 Pi 读取 1000 组 user/assistant 消息（2000 条、约 2.07 MiB），包含段落、代码和表格。打包应用加载到第 1000 轮，连续向前滚动到第 987 轮、跳回末尾及折叠/展开侧栏成功；观察范围内没有空白内容或崩溃。没有逐条遍历全部消息，也没有测量帧率。
- 并发：从界面依次发送三个普通会话，再发送一个临时会话，四个任务使用各自的 Pi 实例并重叠运行。A/B/C 的每份 session 均有 34 条 entry（2 user、14 assistant、13 toolResult 等，包含预置的一轮），本次运行各产生 13 条 assistant 和 13 条工具结果；最终 stopReason 为 stop，无 error/aborted，报告为“全部完成”。临时窗口运行中关闭，重新创建窗口后原会话及完整最终回答仍在，运行状态已结束，界面显示 3m 5s；临时会话按既定行为不写会话 JSONL。
- 打包命令发现：设置中将 Pi 路径草稿留空并点击检测，正确发现 `/Users/sushao/Library/pnpm/bin/pi`，显示“Pi 已就绪 0.87.1”。没有保存该草稿；实际离线任务继续使用测试 wrapper。绝对路径启动由四个任务验证。
- 应用内键位和菜单：Cmd+, 打开设置、Cmd+P 搜索并切换会话、Cmd+B 切换侧栏、Cmd+Q 退出可用。运行中通过原生“窗口”菜单打开临时窗口成功。全局快捷键录制、确认和落盘成功，但自动化按键未能确认系统级派发，仍需实体键盘验收。
- 设置返回：用户在正常应用中确认左上角返回按钮可用。自动化观察到的延迟未在人工操作中复现；原实现已有刷新通知，不保留重复通知改动。
- 隐藏/恢复：日志确认主窗口持续隐藏约 81 秒后显示，隐藏期间没有界面自动化操作；恢复后会话仍在。之前一次隐藏后截图会被自动化立即唤醒，已排除该片段，不把工具造成的唤醒作为应用故障，也不将其标为隐藏采样。
- 退出：应用级退出日志从开始到完成约 77 ms；一秒采样确认 GUI 退出时所追踪的六个 Pi 子进程均已退出，无残留。六个实例包括四个任务、长会话与启动空会话，不将全部进程误计为六个并发任务。

资源采样为 11:37:09–11:52:50，共 942 次、一秒间隔，按精确可执行路径找到 GUI 后按父子进程关系跟踪 Pi。下表是指定片段的观测值；CPU 使用 macOS `ps` 的百分比，非严格的一秒增量或帧率。RSS 不等于堆分配或物理 footprint，受系统内存压缩等影响。

| 片段 | 样本数 | GUI RSS | Pi 合计 RSS | GUI CPU 中位数 / 最大值 |
| --- | ---: | ---: | ---: | ---: |
| 长会话加载与滚动 | 97 | 56.4–87.9 MiB | 15.4–148.1 MiB | 1.8% / 40.8% |
| 并发流式及临时窗口操作 | 156 | 63.8–88.6 MiB | 215.5–335.0 MiB | 25.7% / 62.0% |
| 完成后空闲 | 129 | 37.2–83.6 MiB | 85.0–114.9 MiB | 0.0% / 6.3% |
| 实际隐藏区间 | 80 | 37.2–54.8 MiB | 85.0–97.9 MiB | 0.0% / 1.0% |

并发片段 GUI 与 Pi 的同一时刻 RSS 合计最高为 420.9 MiB。这里只证明上述本地负载能够完成，不据一次运行断言没有泄漏或覆盖长时间高负载。原始采样临时保留在 `/tmp/gupi-223-acceptance.csv`，汇总为 `/tmp/gupi-223-acceptance-summary.json` 和 `/tmp/gupi-223-acceptance-metrics.json`。

本机仍需人工确认：

- Dock 材质和 Tray 小图标/动态菜单：原生自动化读取 Dock、SystemUIServer 超时；独立测试包没有系统通知/Badge 授权，不修改系统授权以替代正式包验收。
- 实体键盘触发全局快捷键及再次按下关闭临时窗口。
- 精确冷启动耗时：独立包能启动并显示界面，但工具调用耗时明显波动，不能充当应用启动基准；本次不报告精确冷启动成绩。Windows/Linux 与其他 macOS 版本的边界保持如上。

## 范围外

正文查找/插件持久消息/会话信息归 #242，原子输入标签/问卷归 #243，逐条队列归 #222；Jaco 清理归 #240，Pi/项目设置归 #244，市场归 #245。已有通知状态和投递归 #241，此处仅补桌面入口与权限指引。

不新增自动更新、登录时启动、深链接、系统分享服务、自动崩溃上传、永久 Finder 图标、Windows 动态任务栏图标、NSIS 或 Linux Tray/全局热键后端。
