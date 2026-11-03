# Issue #248：Form 校验规则与完整编辑会话

状态：已实施，本地验证完成，待协调者与用户复核。用户已确认本文的五项设计选择；实施基线为 `3ade6e9f`。本文只覆盖 [#248](https://github.com/suxiaoshao/gpui/issues/248) 的 Form 契约和新增能力，不代表整个 Issue 的计划。

## 目标与边界

让一个 `Entity<Form<M>>` 同时拥有类型化模型、基线、尚未解析的编辑草稿及其版本。依赖变化能清除跨字段旧错误；刷新外部数据能原子更新基线并保留本地编辑；控件可显示映射后的值而不把继承值写成覆盖值。

所有权仍为：Form 负责编辑会话、路径身份、校验和同步；原生控件负责光标、选择、焦点与 IME；应用负责继承计算、领域数据、外部持久化和冲突处理。无效文本不再需要塞进领域模型。`Unset`、文件中无法识别但必须保留的 `Unreadable` 仍是应用领域状态。

涉及 `gpui-form`、必要的 `gpui-form-macros` schema 遍历能力、`gpui-form-gpui-component` 及 workspace 内消费者。Gupi 仅作需求与迁移参考，另仓迁移不是本工作验收前置。不修改 gpui-base；不做 Operation/Store 精简、Form Runtime 重写、校验路径族合并、拓扑性能优化或历史文档清理。

## 实施前证据

核对基线：gpui `3ade6e9f829dc96864a05a1c094767efdae6bb56`，与 Gupi 当前锁定版本一致。以下是静态源码结论，不是运行测试结果。

| 位置 | 当前行为与缺口 |
| --- | --- |
| `src/validation.rs:216-259` | `replace_scope` 按 trigger 和输出地址清除，`invalidate_sync` 按错误地址失效；没有独立规则身份和输入依赖。External 空结果无法指认旧 Submit 错误所属规则。 |
| `src/form.rs:123-125,384-428` | dirty 仅比较 model/baseline；控件问题只发 ValidationChanged，文本内容没有进入 Form。 |
| `src/form/transition.rs:75-90` | 只有 ModelApplied 推进 revision。 |
| `src/form.rs:196-258` | replace/reset/rebase 走整表安装，重置动态身份、校验并投影控件；rebase_if_current 只检查 session/revision。 |
| `src/control.rs:217-263`、`src/control/transition.rs` | 已有 editor sequence、lifecycle generation、source suppression；须延用并扩展，不能另造双向同步机制。 |
| `app/feiwen/crates/feiwen-fetch/src/form.rs:66-86`（仓库根路径） | 页码规则读取 start/end，只用 END_PAGE 判断是否运行且错误写 END_PAGE。 |
| Gupi `src/features/settings/pi_config/forms.rs:10-36,395-446` | Slot::Invalid 承载文本，pending 自行比较叶子，merge 手写三方选择。 |
| Gupi `pi_config.rs:529-586`、`pi_config/inputs.rs:264-327` | rebase 后 replace 存在中间状态；手写显示/解析与回到继承值的转换。 |

本目录下源码位置相对 `crates/gpui-form`，标注仓库根路径者除外。Issue 中 transition 行号涉及不同文件，本文以实际 owner 为准。

## 一、校验：规则身份、输入依赖、输出归属分离

引入稳定 `RuleId` 和规则声明：输入依赖（类型化路径/子树）、启动触发时机、执行函数；错误仍可输出到任意当前有效的类型化路径。规则身份不使用错误 code、输出路径或 trigger。动态规则实例的身份为规则 ID 加 occurrence 身份，不能用列表下标。注册重复 ID 返回配置错误，不覆盖已有规则。

每次执行先收集该规则全部结果，再原子替换该规则实例的全部旧结果；空集合也表示成功执行并清空。trigger 只描述本次执行原因，不参与错误所有权。一次变更命中多个输入时，每个规则只执行一次，使用同一个已提交模型和拓扑快照；发布时不能看到旧、新错误并存或暂时清空。

调度契约：

- Mount/Change/Blur 控制首次启动；Submit 执行所有提交规则；External 对当前注册的规则进行全量复核，包括之前由 Submit 启动的规则。
- 已执行过的规则在任一依赖变化后立即复核，不受首次启动 trigger 限制。这样 Submit-only 规则报错后，修正字段可立即清错；尚未启动的规则不会提前干扰输入。
- 不相关路径变化不运行规则、不清其结果。路径结构变化与退休也参与依赖相交判断。
- 依赖存在未解析草稿时，不拿最后一次成功解析值冒充用户输入：移除该规则旧结果并标记等待解析；草稿本身阻止 prepare。解析成功后重新调度。
- Required、garde 等内建规则也有确定的 owner。Control 问题归草稿/控件，Async 结果归异步规则和 generation；同步规则不能清除它们。已有异步取消和版本保护保留，草稿版本变化也令旧任务不能提交。
- 规则移除、validator 更换或动态实例退休时，只清相应 owner 的结果与待执行工作。异步结果必须匹配当前 rule instance、session、version 和 generation。

公开入口收敛为 `ValidationRule::new(id, check).dependency(path).triggers(triggers)` 与 `Form::with_rules(...) -> Result<Self, RuleRegistrationError>`；可选 `.instance(path)` 关联动态 occurrence。执行接收完整快照请求及限定 owner 的 sink，字段级 Required 继续按 schema 生成确定 owner。

旧 `Validator::validate` 无法推断输入依赖：迁移 workspace 中 Fetch、Query、HTTP Request、Download 校验器及相关测试/示例到显式规则；移除旧 ownerless 自定义规则入口，作为明确 breaking change。不保留一套不能清错的平行语义。错误 code 保留用于展示/翻译，不承担规则身份。

## 二、统一编辑状态与版本

Form 增加路径关联的编辑草稿记录，包含原始展示值、编辑者稳定键、映射类型身份及解析问题。泛型核心支持类型化展示值 `D: Clone + PartialEq + 'static`，内部可类型擦除；字符串只是数字输入的常见实例。草稿由 Form 持有，不能依赖控件存活的 Weak 标记。控件卸载不清稿、不使表单突然可保存；重建相同编辑者键的绑定恢复草稿。相同路径的两个编辑器使用不同键，各自草稿都阻止提交；一个编辑器成功写值不隐式清除另一个的未解析文本。

原生编辑统一提交解析结果：已解析值，或展示草稿加解析问题。成功解析时模型写入、该编辑者草稿移除、校验复核和版本推进是一个事务；即使模型数值未变，清除旧草稿也属于有效状态变化。错误文案变化本身不推进编辑 revision；不同文本即使共享同一错误文案也推进 revision。

`is_dirty()` = 模型相对基线变化，或存在未解析草稿。格式等价且可解析的输入按存储值比较，不仅因为显示格式不同而 dirty；若产品要保留这类文本，应用应明确把它建模成领域数据。`prepare` 有任意未解析草稿或尚未接纳的原生编辑时失败，不能仅依赖是否有可见错误。

`FormRevision` 扩展为编辑会话 revision：模型、草稿、基线或拓扑的有效改变推进一次；只读查询、相等写入、单纯重新格式化显示不推进。新增 `FormEvent::DraftChanged`，携带 revision 和路径影响；只有模型改变才发 ModelChanged。联合模型/草稿事务中事件共享一次 revision，最多一次 notify。绑定不得因 DraftChanged 或 ValidationChanged 无条件调用原生 value setter。

### 延迟写入与保存竞态

仅在 defer 闭包中递增 revision 不足以保护“按键已发生、闭包未执行、保存回调先到”的窗口。沿用 editor sequence，并在原生事件入口同步登记 Form 会话共享的待接纳编辑票据，不回调正在更新的控件。FormVersion 额外捕获编辑票据 epoch；prepare 要求待接纳队列为空，rebase_if_current 同时校验 epoch。接纳或丢弃票据后再由 Form 事务推进 revision/通知。票据必须在窗口关闭、绑定退休及显式放弃时结算，不能永久阻塞 prepare。

每个有效原生编辑按队列顺序接纳；生命周期切换使旧 generation 的写入失效，不能仅以 revision 变化丢弃连续输入。即使输入最终回到原值，旧保存凭据也不恢复有效。

## 三、路径基线、生命周期与显式放弃

已实现 API：total path 的 `baseline(&form, cx) -> T`、`is_dirty(&form, cx) -> bool`；dynamic path 的 `try_baseline(...) -> Result<Option<T>, ResolveError>` 与 `try_is_dirty(...) -> Result<bool, ResolveError>`。有效但新建且无基线 occurrence 返回 None；退休路径返回错误。容器 dirty 聚合子树模型差异、结构差异和草稿，不能只查已有子路径，故删除项目后其父容器仍 dirty。

基线比较按存储值，不按显示投影。Form 保留基线结构及当前 occurrence 与基线 occurrence 的对应关系；读取不分配 identity。重排保留 item 身份及其基线，新增项目无基线，删除后重插不是原 occurrence。静态 total path 保持有效。

生命周期参数显式使用 `DraftPolicy::Preserve` 或 `DraftPolicy::Discard`，并返回 `Result<LifecycleReport, LifecycleError>`；replace/reset/rebase 的旧无策略签名迁移为强制选择策略，避免悄悄沿用清稿行为。新增类型字段私有、使用 reader/builders。

| 操作 | 模型/基线语义 | Preserve | Discard |
| --- | --- | --- | --- |
| replace | 换当前模型，基线不变 | 保留仍有效路径的草稿；可能仍 dirty | 显式清除本操作覆盖范围草稿 |
| reset | 当前模型回基线 | 保留草稿，因此不保证 clean | 作为应用“放弃更改”的明确入口，回基线且清稿 |
| rebase | 当前模型及基线设为新值 | 保留草稿，因此不保证 clean | 接受新值并清稿 |
| rebase_preserving_edits | 三方合并当前值，基线设为新值 | 保留本地编辑和草稿，见下节 | 不把 Discard 混入此 API；放弃使用上述显式操作 |
| rebase_if_current | 保存结果安装 | 先做完整版本检查；合法 Prepared 不含草稿，新草稿使检查失败 | 不提供绕过版本检查的强制分支 |

replace/reset/rebase 仍退休动态路径，不能靠匹配 index 或值复活。Preserve 如果会退休带草稿的路径，则整次操作返回 `WouldRetireDrafts`，模型、基线、身份和草稿全部不变；调用方可保留编辑 rebase，或明确 Discard 后重试。不能把“路径无效”当作自动删除用户文本的授权。普通删除/case 切换等结构操作同样在会移除草稿时要求显式放弃；保留现有无草稿路径的正常行为。

程序化 set 与未解析草稿相交时默认保留草稿、更新底层存储值并继续显示草稿；需要覆盖编辑的调用方必须显式清稿。同一个来源的成功解析只清自己的草稿。断开的绑定不改变 owner，只有显式放弃或会话销毁才能清除离线草稿。

## 四、保留编辑的 rebase

输入为旧基线 B、当前存储模型 L、外部新值 R，连同当前草稿 U。在提交前计算完整合并计划；失败不得留下部分新基线。输出模型 L'、基线 R 和冲突报告，一次事务发布，不使用公开 rebase + replace 拼接。

| 每个合并单元的状态 | L' | 冲突 |
| --- | --- | --- |
| L = B，且无草稿 | R | 无 |
| L ≠ B，R = B | L | 无 |
| L = R，且无草稿 | R | 无，归于 clean |
| L ≠ B，R ≠ B，L ≠ R | L | 报告本地/外部同时修改 |
| 有草稿，R = B | L + 原草稿 | 无，仍 dirty |
| 有草稿，R ≠ B | L + 原草稿 | 报告草稿与外部更新冲突，即使 L = B |

默认保留本地值并报告冲突，不替应用选择覆盖磁盘。报告携带版本和类型化可定位的路径；应用检查报告版本后再解决冲突。下次显式接受外部值/放弃草稿才清对应编辑。冲突展示、是否允许保存及文件写入前的并发检测属于应用；Form 报告不是磁盘锁。

通过 schema 的类型化遍历合并普通字段与 `#[form(child)]` 下的叶子；opaque 字段按整体值处理，不走 JSON 序列化。增加 derive 所需遍历能力时同步明确手写 FormSchema 的迁移。

动态集合、Option/case 的结构为保守合并单元：无稳定领域键时不对两份独立模型按下标猜测身份。结构相同且有可证明的 occurrence 对应关系时可递归；外部模型无法提供对应关系的集合按整个集合合并。保留本地子树时保留其活动身份和草稿；选择外部结构则退休旧动态身份。新基线不能对应到保留的本地 occurrence 时，try_baseline 返回 None，父集合负责完整 dirty 判定。不得以外观相等复用已退休 ID。

这提供确定且不丢稿的通用行为，不承诺任意 Vec 的逐项智能合并。Gupi 带业务键的 model maps 如需自动合并不同条目，可继续保留领域合并；本次不增加另一套通用 keyed collection 框架。此限制需要在用户确认时明确接受。

保存成功与刷新必须区分：`rebase_if_current` 是旧保存结果的新鲜度保护，失败后不能直接调用以旧基线为 B 的 rebase_preserving_edits 强行应用。若应用要合并保存期间的新编辑，必须以实际提交快照作为三方共同祖先；本次不提供该额外 API，应用继续处理迟到保存提示/重新加载。

## 五、映射绑定

在现有 ControlBinding/ControlWriter 上增加映射能力，不再要求每个消费者复制订阅、source suppression 和生命周期代码。`ControlMapping<S, D>` 提供：

- `format`：存储值 S 加只读映射上下文到展示值 D。
- `parse`：展示值 D 加当前存储值、存储基线及外部上下文，到已解析 S 或未解析草稿/问题。
- 映射上下文由应用显式提供快照；外部继承值/catalog 改变时显式 refresh。核心不持有 PiConfig，也不主动读取磁盘。

上下文更新重投影无草稿的编辑器；有草稿时保持原文本，不自动重解析或修改存储值。refresh 自身不推进编辑 revision；如果上下文涉及业务有效性，应用同时请求 External 校验。下一次用户编辑按最新上下文解析，解析与入队捕获同一个上下文快照，不能排队后换上下文重新解释已输入文本。

Gupi 的 `Unset -> 继承值`、`Unreadable -> 原始显示`、数字/命令行解析均由应用 mapping 实现。parse 可以在用户改回基线的有效显示值时返回基线 Slot，而非 Set(继承值)。库不硬编码“显示相等意味着存储相等”，也不把继承值更新当用户编辑。

映射绑定仍使用稳定 editor key；重建时 key 的展示类型/映射身份不匹配应返回错误，不能解释旧草稿为新类型。首次挂载有草稿先恢复草稿，无草稿才 format。成功编辑不立即 format 回源控件，避免光标跳动和吞掉输入；其他控件的投影沿用 source suppression 与合并队列。焦点、选择和 IME 不进入 Form model；IME 未提交 composition 不触发解析，最终提交文本才进入上述通路。

## 公开接口与迁移影响

本节记录本分支已实现的公开契约，尚未发布。具体签名和用法见中英文 guide；实现收敛说明见末尾实施记录。

| crate | 公开变化 |
| --- | --- |
| gpui-form | RuleId/ValidationRule 及显式规则注册；替换旧自定义 Validator 入口；路径 baseline/dirty；草稿与编辑者身份、原生编辑接纳入口；FormVersion 内部增加编辑 epoch；FormEvent 增加 DraftChanged；DraftPolicy、生命周期 Result/report/error；rebase_preserving_edits 和冲突报告；映射绑定及上下文刷新。 |
| gpui-form-macros | derive 生成基线/合并遍历及内建规则 owner；保持类型化字段/路径，不合并路径类型族。 |
| gpui-form-gpui-component | 数字等适配器改走统一解析结果，恢复卸载草稿；提供 mapped input 入口，沿用原生控件及订阅生命周期。 |

breaking changes 必须在实现 PR 中列签名及前后用法：Validator、生命周期方法、FormEvent 穷举匹配、手写 FormSchema（若需要扩展 trait）。不为了保持旧错误行为增加长期兼容层。

workspace 消费者逐处迁移 reset 为显式 Discard（用户放弃动作）、加载为明确的 Preserve/Discard，按实际 intent 审核，不能机械全部选 Discard。Gupi 后续可删除 Slot::Invalid、普通字段 pending 和 rebase+replace 的重复状态；保留原始文件、继承计算、磁盘冲突及带业务键结构合并。

## 实施顺序与验证

按以下顺序实施；分支 `feat/248-form-edit-session`，仅本地提交，不 push 或开 PR。

1. 规则 owner/依赖调度及 Fetch 页码回归；迁移其他自定义验证器。验证跨 trigger 空结果清理、同 code 不同规则隔离、一次变更多依赖只执行一次、异步迟到结果拒绝。
2. 草稿所有权、编辑票据、revision/事件和路径基线；适配数字控件。覆盖 `- -> -- -> 有效值 -> 基线`、错误文案相同但文本不同、卸载/重挂、双编辑器、保存回调早于 defer、窗口销毁结算票据。
3. 显式生命周期策略与原子三方 rebase。表驱动覆盖上表；覆盖 Preserve 失败全量回滚、Discard 清稿、删除/重插不复活身份、重排、opaque 集合冲突、基线缺失、模型相等但有草稿、合并后独立字段 clean/dirty。
4. 映射绑定和消费者示例。覆盖 Unset 显示继承值、输入后改回基线恢复 Unset、Unreadable 原值保留、上下文变化不覆稿、旧 queued edit 不被新 projection 覆盖、源控件光标/IME 行为。
5. 更新受影响 crate 的中英文 README/guide、skill contracts 及公开示例；新开发计划依约为中文。进行受影响构建、已有相关回归与必要新增测试、Clippy，按真实消费者做最小启动/编辑/放弃验证；不把源码检查当运行验收。

测试尽量放在已有 validation_snapshot、vnext、binding/adapter 回归体系，测试可见行为和竞态，不镜像私有实现。不因本计划顺带重写 Runtime 或全量跑无关应用流程。

## 已确认的决定

用户已确认：生命周期强制显式草稿策略及破坏性迁移；控件卸载后由 Form 保留草稿；双边修改默认保留本地并报告冲突；缺少领域 identity 的集合整体合并；旧 Validator 改为显式规则声明。

实现与验证记录见下节；仅本地开发和提交，不 push 或开 PR。
## 实施记录

分支：`feat/248-form-edit-session`；基线：`3ade6e9f`。本地实现已完成规则 owner/依赖、草稿/票据、路径基线、生命周期策略、三方合并、映射绑定及 workspace 消费者迁移。

API 收敛：`ValidationRule::new(id, check).dependency(path).triggers(...)`；`with_rules`/`replace_rules` 返回配置 Result；total `set` 改为 `Result<bool, ResolveError>` 以返回退休草稿拒绝，明确弃稿使用 `discard_drafts(typed_subtree, cx)`。动态结构 API 保留既有 Result 边界。新增 `SessionChanged` 供基线/拓扑改变但模型未变时通知，避免误发 ModelChanged。Preserve 与 preserving rebase 在待接纳票据存在时返回 EditsPending；Discard 明确结算票据。

集合、Option 和枚举/case 以整体单元合并；直接 child 的普通结构递归。无通用领域 key 框架。Fetch/Query 的首次业务校验保留 Submit-only，启动后依赖编辑即时复核；HTTP 按 URL/headers/body/auth 四个规则，Download 按 source 规则迁移。加载已保存 query/fetch 任务快照是明确覆盖动作，选 Discard；用户 reset 选 Discard；普通刷新可用 preserving rebase。已有绑定生命周期回归按其目的使用 Preserve/Discard。

本地检查全部通过：

- `cargo check -p gpui-form-gpui-component -p feiwen-fetch -p feiwen-query -p http-client-request -p novel-download-feature`。
- `cargo test -p gpui-form -p gpui-form-gpui-component -p feiwen-fetch -p feiwen-query -p http-client-request -p novel-download-feature --lib --tests --no-fail-fast`：231 项通过；其中 Form 56、适配器 20、Fetch 20、Query 40、HTTP Request 62、Download 33（包含既有回归）。Form 的数量含 trybuild 测试入口。
- `cargo clippy -p gpui-form -p gpui-form-macros -p gpui-form-gpui-component -p feiwen-fetch -p feiwen-query -p http-client-request -p novel-download-feature --all-targets --all-features -- -D warnings`。
- `cargo fmt --all -- --check`、`git diff --check`。
- 原生绑定回归验证已提交 IME 才入 Form、来源文本不立即 format、上下文刷新不覆稿、卸载重挂恢复、显式弃稿。
- 当前 Feiwen 二进制以临时 `.app` 包装及 `/tmp/issue248-feiwen-qa` 数据库启动：9/1 范围不能开始，清空起始页后点击开始未启动任务，跨页保留空草稿，填回 1 可解析；Query 增加条件后重置恢复默认条件。未发起抓取请求；临时数据库与包装已清理。这不是打包/发布验收。

未改依赖版本。既有 `block 0.1.6` future-incompatibility 提示仍在；受影响 crate 的 `-D warnings` Clippy 通过。未执行 Linux/Windows、完整发布打包或 Gupi 另仓迁移；集合/Option/case 仍为保守整体单元，保存期间编辑的迟到保存合并仍需应用持有实际提交祖先。
