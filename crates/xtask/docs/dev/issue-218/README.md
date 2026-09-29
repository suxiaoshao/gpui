# xtask：Gupi 基础 bundle 入口

- 根计划：[issue-218](../../../../../docs/dev/issue-218/README.md)
- owner：`crates/xtask`；消费根 C-03、D-04、D-05；本地 F/R/T-200 段、WP-200。

## 已核对入口与目标

F-200：修改 `src/cli.rs`，为已有 `BundleApp` 增加 `Gupi`，`package_name()` 返回 `"gupi"`，`app_dir_name()` 沿用同名映射。现有 bundle 从 app manifest 读取元数据，沿用 `src/bundle/settings.rs` 的本地化复制流程；第一阶段不建立第二套打包脚本。

应用自己的 Cargo metadata、图标和 InfoPlist.strings 由 [Gupi 计划](../../../../../app/gupi/docs/dev/issue-218/README.md)拥有。xtask 不携带 Pi 二进制、认证或用户设置，不执行 Pi 安装。

## WP-200：桌面 bundle 可启动

前置：根 WP-01 与应用 WP-103。加入 CLI 枚举和名称映射，沿用现有测试补充 gupi 的解析/包名断言；如既有元数据流程有具体缺口，再在根计划记录实际受影响文件，不预改其他应用。

R-200/T-200：`cargo test -p xtask` 验证参数到包名映射；`cargo run -p xtask -- bundle gupi` 生成 macOS bundle，不带 `--install`。从 Finder 启动生成产物，验证名称、图标、中英文、主窗口和 Pi 定位失败恢复。终端 `cargo run` 成功不能替代此项。

基础验收记录产物路径、系统版本与实际 Pi/Node 环境。Windows/Linux 仅记录本轮确实执行的结果，完整发行矩阵留给 #223。`cargo test -p gupi -p xtask --locked --offline` 已通过，其中 xtask 13 项测试。当前 release bundle 的生成与 Finder 启动尚未验证；开发构建的原生界面结果见应用 README。
