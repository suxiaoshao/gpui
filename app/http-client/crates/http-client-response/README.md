# http-client-response

ResponseView 独立持有控件订阅、文本/图片/PDF/音频预览及保存任务。接收 core 的 ResponseState，清理意图通过 ResponseEvent 发布；代际标记拒绝过期结果。原始正文只共享 Arc，不复制。

```sh
cargo test -p http-client-response
```

XML 与 SVG 的源码使用 `xml` 语言标识，保留 HTTP 的 tree-sitter 后端；当前上游内置 grammar 不含 XML，因此该配置仍可能显示为普通文本。语言投影不再把这些响应强制降级为 `text`。
