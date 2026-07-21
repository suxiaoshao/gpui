# Jaco 快捷键设置与临时会话

[English](README.md) | [简体中文](README.zh-CN.md)

Issue [#175](https://github.com/suxiaoshao/gpui/issues/175) 统一 Jaco 的聊天输入框、快捷键编辑器和临时会话体验，同时明确应用状态的所有权。

状态：最终设计、实现、自动化测试、旧 API 残留与文档一致性审计已完成；按本轮要求未执行实际 UI 验证。

最终设计包含三个主要部分：

- `ChatForm` 是通过 `ControlSlot` 组合的纯展示 shell；
- generated form store 拥有当前类型化值，owning bound control 让 UI 与之保持同步；
- 快捷键创建的会话与普通临时会话使用同一个 popup 临时窗口运行时。

核心概念、所有权和最终行为见[完整设计](design.zh-CN.md)。

## 文档

### 最终设计

- [Design (English)](design.md)
- [设计（简体中文）](design.zh-CN.md)

### Issue 实施计划

以下文件只描述 issue #175 的产品实现。全应用的 form library 迁移另见
[Jaco gpui-form 类型化双向绑定迁移](../gpui-form-migration.md)。

- [chat-form-refactor.md](chat-form-refactor.md)
- [run-settings.md](run-settings.md)
- [temporary-window-runtime.md](temporary-window-runtime.md)

实施文档不属于稳定的架构契约。
