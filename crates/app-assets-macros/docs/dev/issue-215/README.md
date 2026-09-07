# app-assets-macros：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`crates/app-assets-macros`；工作包：WP-02、WP-05。
- 本地编号使用 `app-assets-macros/F-*`、`app-assets-macros/L-*`、`app-assets-macros/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/lib.rs；Cargo.toml。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

保留 define_lucide_icons!/define_svg_icons! 的用户调用形状、元数据和路径注册。Q-01 已确认：所有生成的绝对 GPUI/组件类型路径统一改经 ::app_assets::__private 引用；升级 syn 到候选版本。不得仅修一处 IconNamed 而遗留 AssetSource/SharedString/Result。

## 验证与完成条件

- 候选命令：`cargo test -p app-assets --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：宏不是 standalone 测试完即完成；通过 app-assets 现有生成样例与应用调用方确认路径解析。
