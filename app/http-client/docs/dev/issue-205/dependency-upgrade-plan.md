# HTTP Client 依赖升级计划

- 状态：`In progress`（本地自动化通过；三平台/人工 smoke 待执行）
- Owner：`app/http-client`
- Root hub：[Issue #205](../../../../../docs/dev/issue-205/README.md)
- Root canonical plan：[全 workspace 依赖升级计划](../../../../../docs/dev/issue-205/dependency-upgrade-plan.md)
- Upstream reuse：[上游能力复用审计](../../../../../docs/dev/issue-205/upstream-reuse-audit.md)

## Owner scope

HTTP Client 保留完整 `gpui-component`，不迁移到 `gpui-base`。本 owner 负责启动链、request
表单、response viewer 与 code editor 的具体状态迁移；通用 form text-control adapter 由
`crates/gpui-form-gpui-component` owner 提供。

## 精确依赖与已知命中

`app/http-client/Cargo.toml:26-35` 直接依赖 `gpui`、`gpui_platform`、带
`tree-sitter-languages` feature 的 `gpui-component`、`gpui-component-assets`、`gpui-form`、
`gpui-form-gpui-component`、`gpui-operation/tracing` 与 `gpui-tokio`；95 行的 dev dependency
开启 `gpui/test-support`。这些依赖继续继承 root 统一版本，应用不声明 `gpui-base`。`gpui-tokio` dependency
key 改指同一 Zed target 的上游 `gpui_tokio` package；Rust import 保持不变，本应用继续在自己的 manifest 声明
`fs/io-util/time` 等实际 Tokio features。

本 owner 同时承接 root 已审计的 registry direct 目标：

| Dependency | Current | Target | 重点验证 |
| --- | --- | --- | --- |
| `async-compression` | 0.4.42 | 0.4.43 | gzip/brotli/zlib/zstd streaming decode |
| `base64` | 0.23.0 | 0.23.1 | auth/body encode/decode |
| `bytemuck` | 1.25.0 | 1.25.2 | media/PDF byte conversion |
| `bytes` | 1.12.0 | 1.12.1 | request/response streaming chunks |
| `futures-util` | 0.3.33 | 0.3.34 | stream cancellation/backpressure |
| `http` | 1.4.2 | 1.5.0 | method/header/request type contracts |
| `thiserror` | 2.0.19 | 2.0.20 | transport/decode error chain |

确定会编译失败的输入命中：

- `features/request/response.rs:23,129,317-323,865` 把 code viewer 建模为
  `Entity<InputState>`，并调用 `.code_editor(language)` 后用 `Input::new` 渲染。应改为
  `EditorState::new(...).language(language)`、`Entity<EditorState>` 与 `Editor::new`；
  `line_number/searchable/replaceable/soft_wrap/scroll_beyond_last_line/default_value` 配置保持。
- `features/request/body/http_text.rs:7,12,53,67-74,97,128` 通过 `FormInput` 构造
  `.multi_line(true).code_editor(language)`。应改为公共 `FormEditor`、`EditorState::new`
  `.language(...)` 与 `Editor::new`；format 变化仍通过 `EditorState::set_highlighter` 更新。
- 其余 URL、header、params、auth、form-data 与 x-form 路径是单行 `InputState`，继续使用
  `FormInput`/`Input`，不要批量迁移。
- `features/request/method.rs:81-87` 使用 `gpui_component::IndexPath`；目标组件继续 re-export，
  无需改 import。
- `app/http-client/src/main.rs:26` 的 `gpui_component::init(cx)` 保持唯一初始化入口；79-94 行
  继续使用 component assets、`WindowOptions` 与 `Root`。

## 工作包

### HTTP-DEP-1：依赖与启动链

- 保留完整组件及 tree-sitter feature，消费 root 统一 Zed/longbridge 提交。
- 消费 Zed `gpui_tokio`，验证 request/response workers 的 completion、drop-cancel、文件/网络 I/O 和 timer；
  不依赖 Jaco 或 dev-only target 提供 Tokio feature union。
- 不新增 direct `gpui-base`，不额外调用 `gpui_base::init`。

### HTTP-DEP-2：拆分具体 input state

- 先合入 `gpui-form-gpui-component` 的 `FormEditor`，再迁移 request body editor。
- 单独迁移 response viewer 到 `EditorState`；不得用 type erasure、兼容 shim 或退回无高亮
  textarea 掩盖类型分离。
- 为 request format 切换、response language 选择、readonly/search/soft-wrap/scroll-beyond-last-line
  各保留至少一个可观察断言。

### HTTP-DEP-3：其余组件回归

- 编译 tree/select/dialog/input/number input 等完整组件调用，依赖上游 re-export 保持路径稳定。
- smoke text、JSON、XML/HTML 与未知 response；确认大响应仍能滚动、搜索和复制。

### HTTP-DEP-4：非 GPUI dependency cohort

- 以 `http-client-test-server` fixtures 覆盖四种 content encoding、chunked/large streaming、
  cancellation、invalid body 与 header/method round-trip。
- 对 basic/bearer 等 auth 与 request body 的 base64 path 做 known-vector round-trip。
- 覆盖 PDF/image/audio byte decoding 的 bytemuck/bytes 边界以及 `thiserror` source chain；禁止
  用 lossily mapped string error 隐藏升级后的类型错误。

## Focused verification

```text
cargo check -p http-client --locked
cargo test -p gpui-form-gpui-component --locked
cargo test -p http-client --locked
cargo test -p http-client-test-server --locked
cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings
cargo tree -p http-client --duplicates --locked
cargo tree -p http-client -i gpui_tokio --locked
```

手工 smoke：编辑 raw JSON/text body 并切换 format，发送请求，打开有/无已知 MIME 的 response，
验证 syntax highlight、line number、search、copy、soft wrap 与最后一行滚动空间。

## 完成条件

- 两个 code-editor 路径都使用 `EditorState`/`Editor`，单行路径仍使用 `InputState`/`Input`。
- HTTP Client 没有 direct `gpui-base`，完整组件 init 与 assets 行为保持。
- 上游 `gpui_tokio` 的 worker completion/drop-cancel/network/time 回归通过，不再解析本地 path bridge。
- 七个 registry direct 目标的 compression/auth/stream/http/error focused tests 通过。
- 所有 focused tests、clippy 和 UI smoke 通过。
