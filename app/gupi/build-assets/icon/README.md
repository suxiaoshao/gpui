# 开发期图标

来源：Pi 官方 README 引用的 https://pi.dev/logo-auto.svg ，获取于 2026-09-07。
源 SVG SHA-256：`03d509c104b9570063fa268fd3235ed7e0e41dafd93124ca94cae3726f58f117`。

按本轮用户决定临时用于开发；官方许可邮件尚待回复，发布前重新确认。
SVG 内容原样保留；PNG 由 `rsvg-convert -w 1024 -h 1024 pi-logo.svg -o app-icon.png` 生成。
Gupi.icon 使用 Assets/logo.svg 矢量图层：默认浅灰背景、黑色标记，Dark 深灰背景、白色标记；切换由 macOS 图标外观设置控制。已通过 actool 编译，并在 Icon Composer 查看 Default / Dark 预览。普通平台图标仍由 app-icon.png 派生。

应用内使用 assets/brand/logo-black.svg 与 logo-white.svg，按当前 GPUI Theme 的亮暗模式选择。两份仅将原始 SVG 的 CSS 填充改为明确的黑/白属性，路径和 viewBox 保持一致。当前 GPUI 的 usvg/simplecss 渲染链不支持原始 @media (prefers-color-scheme: dark) 规则。
