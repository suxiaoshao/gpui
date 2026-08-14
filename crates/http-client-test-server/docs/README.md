# HTTP Client Test Server 文档

- [开发文档](dev/README.md)

## Postman 重定向观察示例

运行：

```shell
cargo run -p http-client-test-server --example postman_redirect
```

示例会启动两个不同端口的 loopback server，并打印跨 origin 的 `302` 与 `307` URL。保持进程运行，
然后在 Postman 中：

1. 创建 `POST` 请求并粘贴其中一个 URL。
2. 在 Authorization 中选择 API Key，Key 使用 `X-API-Key`，Value 使用非敏感测试值，添加到 Header。
3. 添加 raw body，并开启自动跟随重定向。
4. 打开 Postman Console，比较第一跳和第二跳的 method、body 与 headers。

目标使用现有 `/v1/echo`，因此 `307` 可以通过最终 response body 验证请求 body 是否保留；header
转发情况以 Postman Console 的第二跳 raw request 为准。服务不会记录或回显 header value。按 Ctrl-C
会关闭两个服务。

本机 Postman `12.23.1` 的 UI 与打包代码核对结果：

- 主 Response 面板只显示最终响应；Postman Console 会逐跳列出 redirect response 与后续 request。
- 自动跟随默认开启，默认最多 10 跳。未开启 `Follow original HTTP method` 时，`301`/`302`/`303`
  会改为 `GET` 并清理 body；`307`/`308` 保留 method 与 body。
- 跨 origin 时默认清理 `Host`、`Cookie` 与标准 `Authorization`。Authorization 中 API Key 生成的
  自定义 header（例如 `X-API-Key`）不在清理列表中，会继续发送到第二跳；示例只应使用虚构值。

以上 header 结论来自本机 Electron bundle 中的 `postman-runtime ^7.56.0` 与
`postman-request 2.88.1-postman.49` 实现；示例用于在 Postman Console 中动态复核当前客户端行为。
