# http-client-response

ResponseView 独立持有控件订阅、文本/图片/PDF/音频预览及保存任务。接收 core 的 ResponseState，清理意图通过 ResponseEvent 发布；代际标记拒绝过期结果。原始正文只共享 Arc，不复制。

```sh
cargo test -p http-client-response
```
