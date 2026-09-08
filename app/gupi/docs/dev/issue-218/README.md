# Gupi：第一阶段应用设计

- 根计划：[issue-218](../../../../../docs/dev/issue-218/README.md)
- 应用目录：`app/gupi`。
- 消费根契约：D-01 至 D-08、C-01 至 C-03、ERR-01 至 ERR-06。
- 本文拥有 F/L/ST/R/T-100 段与 WP-100 至 WP-103。下列契约指导本阶段实施；实际验证结果见应用 README。

- [运行时契约](runtime.md)：配置 Operation 数据、合法转移、表单处理、Pi 和退出；该专题是这些精确契约的权威定义，本文保留职责概览。

- [Pi 与窗口证据](pi-evidence.md)：官方实现、Rust GUI 参考和第一/二阶段边界。

## 文件与依赖方向

```text
app/gupi/
├── Cargo.toml                       # F-100 新增；应用依赖与 bundle 元数据
├── src/main.rs                      # F-101 新增；只调用 app::run
├── src/app.rs                       # F-102 新增；初始化、窗口和退出编排
├── src/foundation.rs                # F-103 新增；基础模块声明
├── src/foundation/paths.rs          # F-103 新增；应用目录与环境覆盖
├── src/foundation/persistence.rs    # F-103 新增；读取、冲突检测、备份与原子写入
├── src/foundation/i18n.rs           # F-104 新增；Fluent、语言检测与菜单更新
├── src/foundation/assets.rs         # F-105 新增；app-local runtime 资源入口
├── src/state.rs                     # F-106 新增；状态模块声明
├── src/state/config.rs              # F-106 新增；配置 owner、提交和重读
├── src/state/layout.rs              # F-107 新增；布局读取与退出保存
├── src/state/theme.rs               # F-108 新增；系统外观与配置投影
├── src/pi.rs                        # F-109 新增；本阶段仅版本探测
├── src/features.rs                  # F-114 新增；功能模块声明
├── src/features/startup.rs          # F-110 新增；启动界面编排、状态呈现和分类页面
├── src/features/settings.rs         # F-111 新增；唯一设置表单与操作入口
├── src/features/home.rs             # F-112 新增；主窗口业务外壳
├── src/components.rs                # F-115 新增；跨功能 UI 组件声明
├── src/components/recovery.rs       # F-115 新增；共享恢复页面布局
├── src/app/menus.rs                 # F-116 新增；应用菜单与 action 接入
├── assets/                          # F-105 新增；仅实际使用的运行时资源
├── locales/{en-US,zh-CN}/main.ftl    # F-104 新增；运行时文案
├── locales/macos/*/InfoPlist.strings # F-104 新增；按现有 bundle 管线提供中英文
├── build-assets/icon/app-icon.png   # F-105 新增；开发期 bundle 图标
└── README.md                        # F-113 实施时新增；运行、目录与验收边界
```

features/ 按功能组织页面与交互：settings 拥有设置表单，startup 拥有启动界面的编排与分类恢复页面，home 拥有检查通过后的主窗口业务外壳。配置持久化 owner 仍在 state/config.rs，Pi 探测 owner 仍在 pi.rs；不因页面归入 features/ 而移动底层状态职责。components/recovery.rs 仅保存跨功能共用的 RecoveryLayout；菜单与 action 接入归 app/menus.rs，不建立笼统的 src/ui.rs。

features 调用状态 owner，并消费 components；components 不反向依赖 features。状态 owner 调用 foundation/Pi 边界；foundation 不依赖 UI。所有新模块使用 `.rs`。不增加 Gupi 专用共享 crate，不依赖 jaco-core。应用经 gpui_kit 及其 component/assets 接入；版本与 feature 以根 manifest 和锁文件为准。

## L-100：配置与布局数据

以下为应用私有目标类型，文件格式采用对应 snake_case 枚举值：

```rust
pub(crate) enum ThemeMode { System, Light, Dark }
pub(crate) enum AppLanguage { System, English, Chinese }
pub(crate) struct AppConfig {
    pub pi_command: Option<String>,
    pub theme: ThemeMode,
    pub light_theme: Option<String>,
    pub dark_theme: Option<String>,
    pub language: AppLanguage,
}
pub(crate) struct LayoutState {
    pub main_window: Option<WindowPlacement>,
}
```

`pi_command = None` 表示使用 PATH 中的 `pi`；表单空白归一化为 None。覆盖值是单个命令名或绝对可执行路径，不解析 shell 命令行，不接受附带参数。Theme/Language 默认 System。配置字段不保存 Pi 认证、模型或扩展配置。未知字段和不支持的枚举值报校验失败，避免拼写错误被静默忽略。

配置路径为 `dirs_next::config_dir()/gupi/config.toml`，布局为同目录 `state.toml`；`GUPI_CONFIG_DIR` 覆盖整个目录，启动时归一化绝对路径。日志目录独立解析，采用 `GUPI_LOG_DIR` 覆盖。系统目录不可确定时给出路径错误，不改用工作目录。日志默认目录沿用 Jaco 平台约定：macOS 为 home_dir()/Library/Logs/gupi，其他平台为 data_local_dir()/gupi/logs；覆盖值忽略空值并归一化绝对路径。窗口布局恢复规则见下文。

实际读取 config 并区分 NotFound、读取失败与解析/校验失败，不用 exists() 推断读取结果。NotFound 是正常的“尚未设置”数据，展示欢迎、语言、外观、Pi 配置四页引导；首次明确保存成功前不自动创建配置文件。引导末页先显式检测草稿 Pi，成功且命令仍匹配时才允许完成并保存；下次从有效配置与 Pi 检查继续。损坏或权限错误进入配置恢复，不能当作首次缺失。

不保存 onboarding_completed，也不判断用户是否曾经使用过应用。用户手动放入有效配置时直接采用；启动时配置被删除则重新进入设置。运行中重读发现文件被删除，仍保留已应用配置和草稿，按配置恢复处理，不清空当前工作。state.toml 不存在只代表没有窗口布局，损坏只触发布局恢复，不决定 welcome 是否显示。

布局只保存位置、尺寸和最大化。默认尺寸为 960×740，最小尺寸为 800×600。恢复时校验有限坐标与正尺寸，将旧的小尺寸扩至新下限，并限制在当前屏幕范围；完全离屏时居中到当前屏幕，部分越界约束到可用区域。类型或格式错误需明确恢复默认，不能与正常的显示器变动混为一类。

## L-101 / ST-100：配置 owner 与发布

配置 owner 统一协调读取、保存、重读和修复，以 gpui-operation 的完整 Operation 为权威状态；Data 持有配置读取结果与磁盘快照，运行态持有任务。持久化状态由界面读取，外观从同一 Form 草稿即时投影，不再复制一份可修改的 Store<AppConfig>。数据明确区分 Missing 与 Configured(AppConfig)：初次读取 NotFound 可成功得到 Missing；它允许呈现初始设置，但不能作为已应用配置启动 Pi。运行中已有 Configured 后重读失败或文件消失保留有效数据。

配置读取/重读与用户选择的修复采用 `repair::Operation`：首次失败对应 Unavailable，保留已应用值的重读失败对应 Degraded。SettingsView 持有单个 Form<AppConfig> 编辑草稿；控件是其绑定投影，不另存一份 settings 值。每次保存由 controller 调用 Form::prepare 生成当前提交快照。配置操作运行期间禁止修改表单及再次保存、重读、写回和重置；UI 禁用与事件入口状态检查同时生效。保存草稿成功后更新表单基线；明确确认的重读成功后替换表单；内存写回成功只更新磁盘快照，保留操作开始前已有的草稿。失败保留草稿，通过原保存按钮再次提交当前内容，不提供独立重试写入入口；冲突覆盖也在确认时重新取值。

目标 controller 入口：`load`、`reload`、`save_draft`、`write_committed`、`reset_with_backup`。持久化返回完整成功结果后才发布已应用配置与磁盘快照，按操作类型处理表单。重读成功与保存成功共用配置发布入口，语言/主题订阅由应用 owner 持有，设置页关闭不会丢失它们。

同一配置 owner 的读写互斥，运行期间不启动另一操作；先检查合法状态，再构造并安装 Task，通过 Complete 发布结果，不依靠忽略非法消息处理正常交互。运行态是任务的唯一所有者。写入期间不提供取消，应用退出等待提交收尾；丢弃 Task 不能回滚文件。正常 abort-on-drop 完成链不额外维护 generation。Operation 的精确 Data/Repair、保存任务接入与退出收尾签名在 Ready 前定稿，不增加第二套同构状态枚举。

### L-102：文件操作不变量

```text
保存表单 -> 完整校验 -> 对照上次磁盘字节快照
  相同/预期不存在 -> 同目录临时文件 -> flush/sync -> 提交替换
  发生变化 -> 返回冲突 -> 用户选择重读或明确覆盖
明确覆盖/重置 -> 重新读取当前文件 -> 独立不覆盖备份
  -> 再核对待覆盖版本 -> 提交；任何前置失败停止替换
提交成功 -> 返回已写入配置与新快照 -> 发布内存 -> 按操作类型处理表单
```

Jaco `foundation/persistence.rs` 的 FileLock、atomic_replace 和 copy_new_synced 是参考证据。锁只能协调遵守协议的写入者；普通编辑器可能不遵守锁，检查与 rename 之间不承诺跨进程绝对 CAS。覆盖前保留版本、检测已知冲突，文案不宣称可排除一切并发竞争。不能直接跨 app 引用私有实现；仅移植当前必要的短小基础逻辑及相关不变量。

重读存在 dirty 表单时，先确认成功重读后舍弃草稿；若重读失败，草稿仍保留。内存写回使用已应用值，界面展示该来源；没有有效已应用值时不显示此按钮。备份名称使用不覆盖创建，失败停止覆盖；权限/目录错误给出定位与重试，不推荐反复重置。

rename 已成功但后续 durability 操作失败时，返回“提交结果待核对”，重新读取磁盘以协调状态；不声称磁盘未修改，不盲目重试覆盖。临时文件失败清理，备份保留并展示位置。

## L-103 / ST-101：Pi 环境探测

已应用配置与设置草稿各自的 `Entity<PiProbeController>` 持有 `refresh::Operation`；精确转移和任务归属见运行时 L-112。输入为正常启动的已应用 pi_command 或设置页显式检测的草稿命令；输出为解析后的命令路径与版本字符串。配置路径变化立即使旧结果失效，取消并回收旧探测后重新检测；主题/语言变化不触发探测。

调用契约见根 C-01。目标探测超时 5 秒，stdout/stderr 分别限 16 KiB；超限、非零退出、空版本或不合法版本返回 ERR-05。stdin 置空，不弹终端窗口。UI 保持可交互，重复重试先结束旧任务；超时/取消需要终止并回收持有的子进程，收尾完成后才启动下一次。只报告命令/版本可用，不宣布 RPC 兼容或登录成功。

默认继承应用启动环境，不执行交互式 shell 读取 PATH。桌面无法找到命令时说明与终端环境可能不同，允许选择绝对路径；Node 脚本启动器依赖的 Node 环境缺失同样显示真实错误，不改写用户 shell 配置。Windows .cmd/.bat 的原生调用与进程收尾尚未完成实机验证。

## L-104 / ST-102：启动状态、窗口与界面

启动 owner 持有配置与 Pi Operation 的读取入口、布局结果、主窗口弱句柄和订阅。根组件从数据与 phase 呈现页面，不持久化启动路由或第二套健康状态；页面 Entity 与表单由长寿命 owner 保留，重新渲染不重复创建。

```text
配置读取中 -> 加载页
配置读取失败且无有效数据 -> 配置恢复页
配置读取结果 Missing -> welcome / 初始设置
配置读取结果 Configured -> 检查布局与 Pi
  布局损坏 -> 布局恢复页
  Pi 检查中 -> 检查页
  Pi 不可用 -> Pi 环境设置页
  必要条件全部满足 -> 主窗口外壳
```

Missing 的引导共用一个 Form；主题和语言即时预览，最后检测草稿 Pi，通过后完成保存并变为 Configured。多项问题按配置、布局、Pi 的依赖顺序呈现；布局恢复只处理窗口信息，不创建、删除或重置用户配置。参考 JacoRoot 从资源状态呈现内容的方式，正常启动仍由数据状态决定；Missing 内部以 Stepper 管理设置步骤。

已有主界面运行期间的配置重读失败通过设置页反馈，保留当前内容；Pi 命令变化则暂停进入依赖 Pi 的入口。第一阶段外壳仅有应用标题、环境状态和设置入口，不做可输入却无法提交的假对话框。

分类页面 ConfigRecoveryView、LayoutRecoveryView、PiSetupView 归 features/startup.rs；SettingsView 归 features/settings.rs。共享 RecoveryLayout 归 components/recovery.rs，只负责标题、说明、诊断和按钮排版。恢复按钮由各页提供，不能把所有业务错误转换为一个万能 reset。Config 页使用基础主题/语言渲染，不能依赖加载成功的配置 Store。

组件采用 component 的 Button、Input、Select、Indicator 和现有对话框容器。保存、重读、写回与重置分别呈现；保存进行中显示忙碌并禁用重复提交。焦点进入恢复页时落到标题或首个可操作控件；对话框取消返回触发按钮，表单校验失败聚焦首个无效字段。键盘 Tab 顺序与视觉顺序一致，错误文字不能只靠颜色表达。

ShowSettings、ShowMainWindow、Quit actions 在 app 层注册；macOS/Windows 关闭隐藏，重新唤起恢复窗口并激活，显式 Quit 拒绝新任务、收尾写入、保存布局后退出。Linux 窗口关闭后是否保留进程需要真实平台验证。隐藏不会取消应用级配置任务；销毁设置页释放控件与局部订阅，不释放配置 owner。

## L-105：主题、本地化与诊断

复用 app-theme 的系统外观能力；语言检测与菜单刷新参考 Jaco 的 foundation/i18n.rs。仅应用本地配置类型，所有用户文字从 Fluent 获取；zh 系统语言选中文，其余英文，手动选择覆盖系统检测。语言切换同时更新页面、菜单、校验与后续错误消息，错误值保存语义和参数，不保存翻译后的字符串。

Fluent 两份文件同步维护以下键组：`app-title`；`menu-{settings,show-main,quit}`；`startup-{checking,welcome,continue}`；`settings-{pi-command,theme,language,save,reload,write-current}`；`recovery-{config,layout,pi}-title`；`action-{retry,locate,reset,cancel,confirm}`；`error-{config-read,config-parse,config-validation,config-conflict,config-write,layout,pi-probe}`。错误参数只包含已筛选的路径、状态或版本，不嵌入整份配置和 stderr。界面采用 Form 布局、Stepper、可搜索语言 Combobox 与真实主题预览网格；Input 和语言控件通过 Form 的 typed control binding 连接。主题和语言草稿立即投影到整个窗口和菜单，亮暗主题独立保存，系统外观与强调色变化重新应用当前草稿。Fluent 完整键位于应用 locales。

日志记录操作种类、耗时、结果分类和操作系统错误码；不记录配置原文、环境变量全表或 Pi 认证。用户可主动展开有限诊断，控制字符清理后显示。日志初始化失败保留 stderr 诊断与界面提示，不连带使设置不可访问。

开发期图标按根 D-04；bundle 图标与运行时资源分别放置。具体官方源文件、摘要及生成命令在引入资产前记录，源文件、摘要及生成命令见 build-assets/icon/README.md。

## 工作包与最小充分验证

| 工作包 | 前置 | 实施内容 | 完成条件 |
| --- | --- | --- | --- |
| WP-100 应用可启动 | 根 WP-01 | F-100 至 F-105、F-112、F-114、F-116：薄入口、基础资源、菜单、启动窗口 | `cargo check -p gupi`；隔离目录运行显示本地化引导，无 Jaco/Pi 配置写入 |
| WP-101 配置与恢复 | WP-100 | F-106、F-107、F-110、F-111、F-115：L-100 至 L-102 与分类恢复 | T-100 至 T-103；读写失败不丢失内存、草稿或原文件 |
| WP-102 探测与状态呈现 | WP-101 | F-109、F-110：L-103、L-104 | T-104、T-105；过期结果不放行，失败有对应可操作页面 |
| WP-103 桌面一致性 | WP-102 | F-104、F-108、F-112、F-113、F-116：L-104、L-105 | T-106、T-107；主题、语言、隐藏、重开与明确退出闭环 |

| 不变量 | 验证 | 场景与断言 |
| --- | --- | --- |
| R-100 失败不丢数据 | T-100，config/persistence 单元测试 | 缺失、损坏、权限失败、冲突、备份失败；原文件与草稿保留，不发布假成功 |
| R-101 提交版本一致 | T-101，config controller 测试 | 保存期间编辑与其他配置操作均被拒绝；重读失败保留草稿；内存写回不 rebase 已有草稿 |
| R-102 内存写回来源明确 | T-102，设置交互 | 草稿与已应用值不同；写回写入已应用值；无有效内存时按钮不存在 |
| R-103 布局恢复可控 | T-103，布局测试与手工 | 损坏不自动覆盖；显示器移除后可见；布局恢复不改变配置、不重新触发 welcome |
| R-104 探测有界 | T-104，临时可执行 fixture | 成功、非零、空输出、超限、超时、取消；进程回收，迟到结果无效 |
| R-105 数据决定界面 | T-105，启动 controller 测试 | config 缺失进入 welcome 且保存前不创建文件；手动提供有效配置跳过 welcome；保存后退出重启继续 Pi 检查；损坏/权限失败进入恢复；启动时删除配置重新设置；多项失败按依赖顺序处理，Pi 失败能打开设置但不能进入主界面 |
| R-106 主题语言一致 | T-106，macOS 手工 | 保存与重读均更新页面及菜单；系统外观变化生效，重启恢复设置 |
| R-107 窗口任务寿命正确 | T-107，macOS 手工 | 关闭隐藏、重新唤起、写入中 Quit；不重复创建窗口，不遗失正在提交任务 |

新增测试按这些不变量组织，可由一个测试覆盖多个入口，不机械地按文件建测试。测试使用临时 GUPI_CONFIG_DIR/GUPI_LOG_DIR 与本地探测 fixture，不调用模型、不读取真实 Pi 凭据。实施验证命令为 `cargo test -p gupi`、`cargo clippy -p gupi --all-targets -- -D warnings`；仅在对应代码落地后执行，跨 workspace 验证按当时适用 CI 范围安排。
