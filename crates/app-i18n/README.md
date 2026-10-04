# app-i18n

共享 Fluent 运行时，提供 Locale、I18n 的资源构造、系统 locale 检测和翻译。应用拥有资源并注册 GPUI global；业务校验消息映射保留在 feature。缺失 key 返回 key，正式 UI 应保持 locale 契约测试。

```sh
cargo test -p app-i18n
```
