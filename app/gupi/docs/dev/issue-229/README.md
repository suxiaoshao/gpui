# 会话目录读取优化

状态：读取优化已实现，受影响构建和目录回归已通过。保留现有搜索，采用顺序字节读取与 sonic-rs 按字段解析；本计划只覆盖这项读取优化。运行时 loading、消息展示的其他反馈另按具体问题确定范围。

## 目标与范围

降低现有全量会话目录扫描的数据准备耗时，保持目录内容、排序、加载与搜索行为一致。

| 项目 | 当前实现 | 确认方案 |
| --- | --- | --- |
| 文件发现 | 默认目录、环境覆盖、全局/项目配置及 header/cwd 继续发现 | 保持一致 |
| 后台执行 | 现有后台任务，最多 8 个 scoped worker | 保持一致 |
| 单文件读取 | 顺序逐行生成 String | `read_until` 顺序读取，复用 `Vec<u8>` |
| JSON 解析 | serde_json::Value 完整解析记录 | 安全的 `sonic_rs::from_slice` 按目录所需字段反序列化 |
| 正文提取 | 随完整记录解析 | 仅在需要首条用户摘要时解析内容 |
| 活动时间与排序 | user/assistant 外层 timestamp 最大值，倒序；相同时间按路径 | 保持一致，不使用 mtime |
| 发布目录 | 全量扫描完成后发布；扫描中报告进度 | 保持一致 |
| 侧栏“显示更多” | 展开已加载条目 | 保持一致 |
| 搜索 | 内存中的标题、项目路径、首条用户摘要 | 保持一致 |
| 取消、刷新与错误 | 现有扫描生命周期和错误处理 | 保持一致 |

本次不实施目录分批读取、增量文件搜索、全文搜索、头尾并发、额外 Rayon 池或 Tokio fs 迁移；不新增搜索 Operation、目录数据库、缓存或文件监听。它们不再作为本计划的待确定项或实施前置条件。其他 serde_json 使用点不随本次优化迁移。

## 数据契约与实现边界

修改集中于 `src/foundation/session_catalog.rs` 的单文件元数据读取；`app/gupi/Cargo.toml` 与锁文件接入 sonic-rs 0.5.8。

现有目录发现、header 读取、SessionInfo 公共结构、后台任务和 CatalogState 保持原有职责。header 发现耗时较小，沿用现有解析即可，不要求统一迁移解析库。

Pi session 是 JSONL 文件，首行是 session header，后续消息与元数据按写入顺序追加；fork/树关系由标识字段描述。物理文件顺序不能直接当作当前分支路径，也不能保证时间戳单调。因此本次仍扫描完整文件，不以尾部第一条相关消息代替活动时间最大值。

需要保留的提取规则：

| 字段 | 现有规则 |
| --- | --- |
| path | 枚举到的文件路径 |
| id、cwd、parent_session | header 字段；保持 BOM 与必需字段处理 |
| name | 每次 session_info 更新该值，最终取最后一条的结果；缺失或非字符串按现有行为处理 |
| first_message | 顺序寻找能生成非空摘要的 user 文本；沿用 text_content 与 summary 的空白处理、120 字符限制，图片等不生成文本时继续寻找 |
| activity | 所有 user/assistant 条目外层 timestamp 的字符串最大值；不改用 message.timestamp、最后追加记录或 mtime |

用安全解析入口读取 JSON，跳过无关字段的对象构造，不关闭 UTF-8 或 JSON 合法性校验。行缓冲复用只影响读取方式；保持空白行、CRLF、末行无换行及字段缺失/类型不匹配的已有处理，不因类型化解析额外收紧原先可接受的记录。继续检查取消标记；读取或解析失败仍走现有整轮失败路径，不发布部分结果。

实现使用 64KiB BufReader 和可复用字节行缓冲。正常记录按字段解析；类型不匹配或重复键等无法走派生结构体的记录，回到原有 serde_json::Value 字段提取规则。私有 JsonObject 包装限制结构体从对象读取，避免 Serde 将数组解释成记录或 message；该差异已由失败回归复现并修复。首条摘要继续复用现有 text_content/summary。

侧栏和快速打开的代码不需要接入新的搜索源。当前扫描器只保存元数据，搜索仍过滤已取得的目录与内存会话；本次不增加消息正文搜索或命中节点跳转。

## 性能依据

### 正式接入后的复测

最终代码由临时 release 对照程序直接引用，以修改前模块作为基线，使用下述同一目录范围轮换执行 5 次。本次快照为 238 个文件、920,781,686 字节；目录集合和全部元数据逐项相等，文件运行前后未变化。数据准备中位数为 **325.99ms → 108.05ms**，同轮减少约 **66.9%**；基线范围 288.66–388.54ms，优化后 101.67–124.37ms。绝对耗时存在波动，不与此前原型的其他轮次拼接计算提升，也不据此承诺固定首屏时间。

材料：[正式模块对照程序](/private/tmp/gupi-reverse-catalog-research/src/bin/sidebar_integrated.rs)、[最终复测结果](/private/tmp/gupi-reverse-catalog-research/sidebar-integrated-final/results.json)。

### 方案确认时的原型测量

2026-09-14，临时 Rust release 原型使用 sonic-rs 0.5.8。对当前默认目录发现流程进行 5 次暖缓存、轮换顺序测量：238 个 session、14 个记录的目录路径、10 个 cwd 项目，共 920,640,884 字节。当前 cwd 为 `/Users/sushao/Documents/code/gpui`，无 session 目录环境覆盖；未额外传入 UI 内存草稿中的 known projects。

原型保留现有发现、最多 8 个 scoped worker、路径队列、取消检查和最终排序；目录与全部元数据逐项相等，运行前后文件未变化。

| 阶段中位数 | 当前实现 | 顺序字节缓冲 + sonic-rs |
| --- | --- | --- |
| 目录/配置发现、header 读取 | 5.28ms | 5.45ms |
| 元数据读取 | 235.78ms | 86.97ms |
| 完整 scan 返回 | 241.33ms | 92.42ms |
| 侧栏纯数据准备 | 0.18ms | 0.18ms |
| 从开始扫描到数据准备好 | **241.50ms** | **92.60ms** |

同轮总耗时减少约 **61.7%**，约 **2.61 倍**。各阶段中位数独立计算，不要求恰好相加。纯数据准备重现路径映射、元数据 clone/sort、cwd 分组、同名项目消歧和标题准备；不包含活跃会话/草稿合并、真实 GPUI 主线程排队、元素构建、布局、绘制及应用启动。因此这不是窗口首屏耗时，也不保证冷缓存或其他机器得到相同比例。

### 为什么保留字节缓冲

此前约 82ms 的微基准使用字节缓冲，而约 149ms 的完整流程原型使用 String；不能仅用“测量范围不同”解释差距。固定同一文件集、路径顺序与 8 个 scoped worker 后，5 次中位数为：

| 元数据读取路径 | 耗时 |
| --- | --- |
| 早期微基准字节缓冲 | 86.87ms |
| 完整流程原型 String 缓冲 | 136.47ms |
| 完整流程原型改回字节缓冲 | 86.76ms |
| 同一字节版额外调用标准库 UTF-8 校验 | 144.81ms |

主要差异来自 UTF-8 校验路径。`read_line` 需要构造合法 String；`read_until` 读取字节后由安全的 `sonic_rs::from_slice` 校验。`from_str` 接收已合法字符串，不再进行该检查。两种正常路径均有校验；没有使用 unchecked 接口，也不能说 String 版本重复校验。线程池和候选顺序的交叉对照没有解释掉这段差距。

### 为什么不采用头尾并发

在相同 Tokio fs 头尾读取函数中，仅将串行调用改为同文件两端同时读取，完整耗时由 315.52ms 变为 319.39ms，没有全量收益；238 个文件都发生了两端读取区间重叠。头部摘要通常较早取得，尾部为确定不存在标题等情况仍可能读到头部。该证据不代表所有异步 I/O 都慢，但不足以支持为本次优化增加复杂度。

保留全文件顺序扫描同时保留了时间最大值等既有语义，当前已测得足够明确的收益，无需变更排序或加载交互。

临时研究材料（本机临时目录可能被清理，上述数据已记录于本文）：

- [字节版完整流程原型](/private/tmp/gupi-reverse-catalog-research/src/bin/sidebar_bytes.rs)、[原始结果](/private/tmp/gupi-reverse-catalog-research/sidebar-bytes/results.json)。
- [读取路径对照](/private/tmp/gupi-reverse-catalog-research/src/bin/explain_gap.rs)、[原始结果](/private/tmp/gupi-reverse-catalog-research/gap-reading/results.json)。
- [头尾串行/并发对照](/private/tmp/gupi-reverse-catalog-research/src/bin/concurrent_ends.rs)、[原始结果](/private/tmp/gupi-reverse-catalog-research/concurrent-ends/results.json)。

## 实施顺序与必要验证

- [x] 确认读取优化范围，保持现有加载、排序和搜索。
- [x] 临时 Rust 原型验证性能与真实样本元数据一致性。
- [x] 在 Gupi 接入 sonic-rs，替换单文件记录读取与按字段解析；保留现有 header 和目录流程。
- [x] 运行现有目录相关回归，并补充此次解析替换的具体风险用例：标题更新/清空、首条非空文本、乱序活动时间，以及行边界、UTF-8/JSON 错误与宽松字段提取兼容性。
- [x] 完成受影响构建、格式与相关检查，复测同一目录的元数据一致性和数据准备耗时；不设固定毫秒验收线，也不运行无关全量测试或创建临时 .app。

验证命令：`cargo build -p gupi --locked --offline`、`cargo test -p gupi session_catalog --locked --offline`（7 个通过）、`cargo clippy -p gupi --all-targets --locked --offline -- -D warnings`、`cargo fmt --all -- --check`。没有启动或注册测试 .app，真实窗口呈现不属于本次计时。

前期目录与 Codex 时间来源证据见[扫描调研记录](../session-catalog-scan-draft.md)。
