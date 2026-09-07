# feiwen：Issue #215 依赖升级

## 根计划与所有权

- Plan ID：`issue-215`。
- 根计划：[状态、目标版本、问题与公共契约](../../../../../docs/dev/issue-215/README.md)。
- Owner：`app/feiwen`；工作包：WP-03、WP-05。
- 本地编号使用 `feiwen/F-*`、`feiwen/L-*`、`feiwen/ST-*`；公共决定/版本不在此重定义。
- 公共实施要求见根计划；实际结果见[实施记录](../../../../../docs/dev/issue-215/implementation.md)。

## 文件与实施范围

- F-01：`Cargo.toml`，只改根计划指定的依赖/feature；root lock 由 WP-01/WP-05 协调。
- F-02：src/main.rs；src/foundation/assets.rs；Cargo.toml。应用 import 统一经 gpui-kit；行为改动与具体映射见下文和根实施记录。
- F-03：本 owner 现有 README/guide 中受影响的 API/依赖示例；不重写无关章节或历史计划。

迁移入口和宏/资源引用，保留 Query/Fetch Form 会话、动态路径身份与操作状态。本 app 使用 DuckDB，保留现有 schema 和数据库访问策略；Diesel/SQLite 配套属于 Jaco。高级查询 trigger 改为 `ctx.selection()`，适应私有字段边界。

## 验证与完成条件

- 候选命令：`cargo test -p feiwen --locked`；实施时按受影响范围执行一次，不与同状态的上层门禁重复。
- 关键场景：使用隔离数据库与本地 mock；Query 动态行重排不丢输入/焦点，Fetch 不用真实 Cookie。
- 标题栏迁移按根 R-10/T-10 与下述 L-01 检查，不要求旧标题栏外观一致。

## WP-03：上游标题栏配置复用

- L-01（根 D-10，F-02，2026-09-06 补充）：`src/main.rs::main_window_options` 以 `TitleBar::window_options()` 为基线；`main_titlebar_options` 只保留应用标题与 `src/app/titlebar.rs::traffic_light_position()` 等实际 override，删除重复的默认配置组合。
- 已核实 [v0.6.0 TitleBar](https://github.com/longbridge/gpui-kit/blob/v0.6.0/crates/component/src/title_bar.rs) helper 同时设置透明标题栏和 `app_owns_titlebar_drag`；采用上游拖动/双击归属，保留窗口尺寸与定位。
- R-10/T-10：更新现有窗口配置断言，运行时验证拖动/双击、标题栏控件点击、窗口标题与尺寸；不新增重复的通用 helper 测试。
- 已按 L-01 实施。高级查询的动态路径、Form 字段和领域重排继续保留；ListItem 外壳相似不足以替代这些必要功能。
