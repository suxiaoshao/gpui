# HTTP Client 自动测试边界

自动测试保留请求编译的纯数据部分、校验规则、响应内存收集、协议分类、消息队列和使用受控数据的界面状态。

依赖回环 TCP 服务、真实超时、临时文件和 PDF/音频工作线程的集成测试已删除。HTTP Client 不再通过 dev-dependency 引用 `http-client-test-server`。

[HTTP 测试服务](../../../../../crates/http-client-test-server/docs/README.md) 继续提供手动调试接口和 Postman 重定向示例。它的服务实现与产品 HTTP 请求能力保留，自动测试仅覆盖不需要启动服务的规则。

```sh
cargo test -p http-client --locked
```

通过这些测试不代表真实网络、文件保存、媒体设备或跨平台运行已验证。
