# HTTP Client

HTTP Client 是一个使用 Rust、GPUI 与 gpui-component 构建的桌面 HTTP 请求调试工具。目前聚焦单请求工作流：编辑请求、发送或取消，以及查看和保存响应。

## 当前功能

- HTTP/HTTPS 请求与 GET、POST、PUT、DELETE、PATCH、HEAD、OPTIONS、TRACE、CONNECT 方法。
- Params、Authorization、Headers、Body 和 Settings 请求页签；参数与请求头支持启用、禁用和重排。
- None、Basic、Bearer 与 API Key 认证，其中 API Key 可写入 Header 或 Query。
- multipart/form-data、x-www-form-urlencoded、文本和二进制文件请求体。
- 重定向、保留方法与超时设置；最多跟随 10 次重定向，跨 origin 时不转发 Authorization 与 Cookie。
- 发送、取消与接收进度；展示状态、最终 URL、HTTP 版本、耗时和响应大小，并可保存完整响应。
- Auto、Text、JSON、XML、Hex、Base64、Image、Audio 与 PDF 响应查看器，以及独立的 Headers 页签。
- gzip、Brotli、deflate 与 zstd 响应解压，英语和简体中文界面。

## 当前边界

- 当前只有单请求页面，尚未实现 History、Favorites、Environment、多请求标签页、Cookie Jar 和持久化。
- 不支持视频预览；音频预览不包含 Opus；加密 PDF 不支持预览。
- 单次响应最多捕获 50 MiB。超过 8 MiB 时内容会转存到会话临时文件，普通内联预览上限为 2 MiB，完整内容仍可保存。
- 诊断输出会尽量隐去 URL、请求体、Header 值和临时路径，但这不是通用脱敏保证；不要在请求中使用不必要的真实凭据。
- Linux 构建音频能力需要 ALSA 开发库；apt 系发行版可使用仓库的 `script/bootstrap` 安装依赖。

## 目录结构

```text
app/http-client/
├─ src/main.rs                 # GPUI 初始化、日志、i18n 与窗口
├─ src/foundation/i18n.rs      # Fluent locale 与验证消息
├─ src/features/request.rs     # 单请求页面、发送、取消与响应协调
├─ src/features/request/       # draft、表单、参数、认证、请求体与设置
│  ├─ prepared.rs              # 校验表单快照并生成 PreparedRequest
│  ├─ runtime.rs               # 单请求 operation 状态机
│  ├─ transport.rs             # reqwest transport 入口
│  ├─ transport/               # request body、redirect 与 worker
│  ├─ response.rs              # 响应状态与视图协调
│  └─ response/                # 收集、解压、viewer、保存和媒体预览
├─ locales/                    # en-US、zh-CN 与 macOS bundle 文案
├─ build-assets/               # 打包图标
└─ docs/dev/                   # Issue 对应的实现与迁移记录
```

## 开发

环境基线与系统依赖见[工作区 README](../../README.md)。以下命令均从 workspace 根目录执行：

```bash
cargo run -p http-client
cargo build -p http-client --locked
cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast
cargo fmt --all -- --check
cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings
cargo run -p xtask -- bundle http-client
```

开发计划与迁移记录见 [docs/dev/README.md](docs/dev/README.md)。
