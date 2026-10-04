# http-client-request

RequestView 持有编辑 Form、校验、准备和发送任务。编辑字符串保持可变 String；准备失败保留当前结果。通过 RequestEvent 投影响应状态，保存活动由 shell 传入用于禁用发送。

```sh
cargo test -p http-client-request
```
