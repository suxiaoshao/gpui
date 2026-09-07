# app-assets：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/app-assets`；工作包：WP-02。
- 本地编号使用 `app-assets/F-*`、`app-assets/L-*`、`app-assets/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/lib.rs；assets/test-icons/provider.svg。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

保留 SvgIconMetadata、组合资源与现有 fallback；修改组件 assets 包引用。Q-01 已确认：由本 crate 的 #[doc(hidden)] pub mod __private 统一导出宏所需 IconNamed、SharedString、AssetSource、Result 等类型，生成代码经 ::app_assets::__private 引用。不在 feature 模块散落临时 import，不复制类型或增加运行时包装。

## 验证与完成条件

- 候选命令：`cargo test -p app-assets --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：现有 define_svg_icons 测试、资源列表/加载与 Jaco/Feiwen 消费者编译；provider SVG 路径不变。
