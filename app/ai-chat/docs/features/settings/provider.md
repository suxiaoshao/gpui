# 设置 - 提供方

## 功能目标

提供方设置用于配置模型服务连接信息，例如 OpenAI API Key、Base URL、HTTP Proxy，以及 Ollama 本地服务配置。

## 入口

- 打开设置窗口后选择“提供方”。
- 从设置搜索中搜索“Provider”“API Key”“Base URL”“Ollama”“OpenAI”。

## 主要状态

- 每个提供方以独立分组展示。
- 普通文本字段可直接编辑保存。
- API Key 等敏感字段默认掩码显示，可由用户主动显示或隐藏。

## 用户动作

- 编辑并保存 Base URL。
- 编辑并保存 HTTP Proxy。
- 输入、显示、隐藏 API Key。
- 切换设置页后返回确认值仍在。

## 边界情况

- 空 API Key 不应被误认为已配置。
- 显示 API Key 后，关闭或重新打开设置页应回到安全默认显示。
- 长 URL 不应溢出或遮挡相邻控件。
