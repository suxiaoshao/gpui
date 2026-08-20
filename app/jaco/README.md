# Jaco

Jaco 是一个基于 GPUI 的桌面 Agent 工作台。它围绕项目会话、模型供应商、工具调用、MCP、Skills、提示词和全局快捷键，提供统一的本地工作空间。

## 名称来源

`Jaco` 在法语中是非洲灰鹦鹉的通称之一，也写作 `jacot` 或 `jacquot`，读音接近 `/ʒa.ko/`。这里采用短写 `Jaco` 作为产品名，并以灰鹦鹉作为品牌意象。可参阅 [Larousse](https://www.larousse.fr/dictionnaires/francais/jacquot/44651) 和 [CNRTL](https://www.cnrtl.fr/definition/jacquot) 的词条。

## 当前功能

- 项目化会话、无项目临时会话，以及项目和会话的搜索、置顶、重命名与软删除。
- OpenAI、Anthropic、Gemini、Ollama、OpenRouter、DeepSeek、Moonshot/Kimi、Azure OpenAI 等模型供应商，以及自定义 OpenAI-compatible endpoint。
- 本地文件读取、目录浏览、文件查找、内容搜索、写入和编辑工具；会话可选择 Ask、Auto-approve 或 Full Access 权限模式。
- stdio 与 Streamable HTTP MCP server，支持工具筛选、审批覆盖和 OAuth 2.1 授权。
- built-in、user、project 与 plugin 来源的 Skills。
- 提示词、模型、推理强度和全局快捷键管理；快捷键可携带当前选择、剪贴板或截图启动临时会话。
- 图片、PDF 和文本类附件，Markdown 渲染与 Tree-sitter 代码高亮。
- 系统、亮色和暗色外观，内置主题、Material You 自定义色，以及英语和简体中文界面。

## 数据与凭据

- 应用标识为 `top.sushao.jaco`。配置包含 `config.toml` 与 `state.toml`，本地数据包含 `jaco.sqlite3`、附件和临时项目。
- `JACO_CONFIG_DIR` 会同时覆盖配置根和数据根；开发或测试时应指向隔离目录。`JACO_LOG_DIR` 可单独覆盖日志目录。
- 同一个数据目录由 `jaco.sqlite3.lock` 排斥第二个实例，不要让两个进程共享同一测试目录。
- Provider API key 与 MCP OAuth 凭据使用系统凭据存储。MCP 的静态环境变量或 Header 仍可能写入 `config.toml`，不要提交真实配置。
- 附件会复制到本地数据目录，应与会话数据库一起视为敏感数据并纳入备份或清理。

## 目录结构

```text
app/jaco/
├─ src/main.rs       # 进程入口
├─ src/app.rs        # 初始化、窗口、菜单与退出生命周期
├─ src/features/     # home、conversation、settings、skills、temporary 等功能
├─ src/components/   # 应用内复用 UI 与聊天组件
├─ src/state/        # 配置、供应商、项目、MCP、提示词、快捷键和主题状态
├─ src/foundation/   # 资源、i18n、路径、持久化与搜索基础能力
├─ src/database.rs   # jaco-db 会话、锁、刷新与修复入口
├─ src/platform/     # 截图与显示器平台能力
├─ assets/           # 运行时嵌入的主题与供应商图标
├─ locales/          # Fluent 文案与 macOS bundle 本地化
├─ build-assets/     # 打包图标
└─ docs/dev/         # Issue 对应的实现计划与迁移记录
```

## 开发

环境基线与系统依赖见[工作区 README](../../README.md)。以下命令均从 workspace 根目录执行：

```bash
cargo run -p jaco
cargo build -p jaco --locked
cargo test -p jaco --locked
cargo fmt --all -- --check
cargo clippy -p jaco --all-targets --all-features --locked -- -D warnings
cargo run -p xtask -- bundle jaco
```

开发计划与迁移记录见 [docs/dev/README.md](docs/dev/README.md)。
