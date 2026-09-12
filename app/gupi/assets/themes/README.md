# 主题资源

直接打包 `longbridge/gpui-kit` 的 21 份主题 JSON，与 Jaco 使用同一上游快照，逐字节保持一致。

- 上游提交：`382fc28feb2de118c7a523c1b3a22d43aacd2f04`
- 来源目录：[themes/](https://github.com/longbridge/gpui-kit/tree/382fc28feb2de118c7a523c1b3a22d43aacd2f04/themes)
- Gupi 独立打包这些文件，运行时不依赖 Jaco 应用。
- 不保留旧 Jaco tab 配色覆盖；上游已移除的 Matrix 不再打包。保存的预设不可用时沿用既有默认主题回退。

主题解析、选择、预览与 Material You/系统强调色生成由 `app-theme` 承担。应用主题统一使用
`app_theme::apply_theme_config`，同时同步组件主题和 Base 层的分隔条、滚动条及 Markdown 默认颜色。
