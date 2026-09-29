# Issue #215：保留的调查结论

原调查服务于 GPUI Kit 0.6.0 的发布包迁移，后续版本见[依赖更新记录](../dependency-refresh-0.7.0/README.md)。
本页不再维护已退役应用、旧资源宏或 OCR 的候选工作。

## 依赖配套

- GPUI runtime、macros、platform、Kit、Component 和 Assets 必须使用同一配套发布系列；版本以根 manifest 和 Cargo.lock 为准。
- 依赖更新不能整分支覆盖应用后续修复。类型来源、平台 features、测试支持和应用语言 features 应分别核对。
- 既有 Tokio bridge 的 runtime 与任务生命周期由本地 crate 维护，不因依赖入口变化引入第二套 bridge。
- Windows crate 家族按调用方 API 和版本约束配套更新。原 OCR 的 bindgen / winmd 生成链已随退役删除，不再保留这项版本限制。

## SVG 与资源职责

已采用 `gpui-lucide`：从官方 assets 提供的目录生成独立 SVG 字节常量，经上游 `Icon::data` 显示，具体说明见[crate README](../../../crates/gpui-lucide/README.md)。
应用自有 provider SVG、主题 JSON 和品牌图像继续由应用打包；组件默认资源使用官方 Assets。
不再维护旧路径选择宏、整组运行时图标查找表或 Lucide Git 子模块。

## 通用组件与业务职责

- Message、Bubble、Attachment、Marker 与 MessageScroller 承担展示和滚动；消息身份、权限、状态与用量由应用决定。
- Shimmer 的可见性来自真实处理状态，不能把所有未终止任务都视为动画开启条件。
- 选择器替换必须保留领域值与行索引映射、查询与选择语义；选项刷新不能写回用户选择。
- 应用直接消费官方主题和编辑器行为；局部适配仅保留上游组件无法表达的业务约束。

历史调查快照保留在 Git 中；本页仅记录仍有使用价值的共享结论。
