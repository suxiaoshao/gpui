# HTTP Client

HTTP 请求编辑、执行与响应查看工具。

```sh
cargo run -p http-client
cargo test -p http-client-core -p http-client-request -p http-client-response -p http-client
```

`src/workspace.rs` 组合 [请求视图](crates/http-client-request/README.md) 与 [响应视图](crates/http-client-response/README.md)，转交响应状态、清理意图和保存活动。[执行核心](crates/http-client-core/README.md) 提供传输和原始响应契约。响应预览及保存任务不依赖 RequestView 的 Context。Fluent 资源和 bundle 元数据继续由应用拥有。
