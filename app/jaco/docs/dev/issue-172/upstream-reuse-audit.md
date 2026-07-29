# 上游复用审计

## 判定规则

每项只能落入 Adopt（直接采用）、Adapt（采用并做本地适配）、Retain（上游不覆盖，保留）
或 Remove（删除本地重复实现）。实现阶段不得保留“以后再看看”的未决项。

GPUI 判定基于
`gpui-component 5b45bcb26b9343d91a123a4d5ed8a654360512e5..57a9903f48160845aabc8b92a1e2f5348c80d439`；
目标 lockfile 的 Zed GPUI 仍为 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。Rig 判定基于
正式 `v0.41.0` 发布包。如果实施目标变化，本页必须先更新。

## 审计矩阵

| 上游能力 | 本地实现/调用点 | 结论 | 实施动作与验收 |
| --- | --- | --- | --- |
| `InputState` text decoration collections | 普通 Jaco Input 与 `crates/gpui-form-gpui-component` 的 `FormInput`/`FormIntegerInput` owning controls；当前无等价本地 range-decoration store | **Adopt** | 直接采用 `TextDecoration`/collection 的 UTF-8 range、edit tracking 和 `set/append/clear/get_ranges`；不在 form core 复制 presentation state，也不为了“用上新 API”增加产品行为 |
| 普通 Input editor | `app/jaco/src/components/chat/input/composer_editor.rs` 及子模块 | **Retain** | chat composer 使用自定义 editor state，承担 skill completion、附件/mention 与提交语义；新 decoration API只属于 `InputState`，不覆盖 composer，不能据此整体替换 |
| PopupMenu async rebuild 与 late submenu parent wiring | `app/jaco/src/app/title_bar_menu.rs`、chat/home/settings menus | **Adopt + Retain construction** | 采用上游 dismiss/priority 修复；现有 submenu 都同步构建，无 app-local async workaround 可删，继续保留构建方式并回归 click-outside/keyboard/dismiss |
| horizontal `ScrollableMask` viewport/capture fix | conversation Markdown/table 与 settings nested scroll | **Adopt** | 不维护本地 scroll mask；回归 horizontal-dominant wheel、vertical bubbling、scroll edge 与 overlay occlusion |
| Select `Caret` 和 Select/Combobox visual/state 变化 | `crates/gpui-form-gpui-component` adapters、Jaco run/settings picker | **Adopt + Adapt** | 不复制 Chevron caret；只在 adapter 消化编译/API差异，保持 domain value、draft、selection projection 和 subscriptions |
| `RadarChart`、radial plot/tooltip | workspace 当前无 radar chart 或自维护等价组件 | **Adopt library only** | 同步 chart 文档并让组件可用；没有产品需求时不新增 Jaco chart、wrapper、icon 或 i18n |
| gpui-component visual `Form`/field layout | `crates/gpui-form`、`crates/gpui-form-macros`、`crates/gpui-form-gpui-component` | **Retain** | 上游 Form 只覆盖呈现/布局，不能替代 typed domain value、validation、submit、array/group、binding subscriptions；保留三个 crate 边界 |
| gpui-component component bindings | `FormControl<T>` owning controls：`FormInput`、`FormIntegerInput`、`FormSelect`、`FormCombobox` | **Adapt** | 保留 `subscriptions + Entity<State>`、`Deref` 和 `ControlAttachment::defer_*` 契约；上游 event/state/setter 变化只在 owning control 内吸收，不恢复已删除的 generic binding/draft API |
| 上游 `gpui` skill | `.agents/skills/gpui` | **Remove local drift + Adopt** | 目录 byte-for-byte 镜像；删除本地空行/定制漂移，`diff -qr` 无输出 |
| 上游 component docs | `.agents/skills/gpui-component-usage/references/components` | **Adapt** | 正文 A/M/D 镜像，本地 index/rules/attribution 保留所有权；当前目标 M=`chart.md`，A/D=无，并更新 RadarChart 导航 |
| `gpui-form` 使用知识 | 现有 `.agents/skills/gpui-form` 已由其他提交覆盖 typed-store/owning-control 架构 | **Retain + targeted correction** | 不重构、不拆 references；只在目标 gpui-component 使 adapter 说明失真时定位修改现有 `SKILL.md`，并可修正 stale `agents/openai.yaml` “form draft” metadata |
| Rig facade/core-agent split | 根 manifest 直接依赖 `rig-core 0.39` | **Remove + Adopt** | 删除 direct `rig-core`，采用 `rig 0.41` facade；不让 Jaco import `rig_agent` 内部 crate |
| Rig `AgentRunner` exact-turn orchestration | `runtime.rs` 使用旧 builder method，tests 依赖旧 max-turn 行为 | **Adapt** | 使用新 `.history/.tool_concurrency/.add_hook`；保留 Jaco run guard 名称，但 `max_steps` 与 model call 1:1 |
| event-specific `AgentHook` | `PersistingPromptHook` | **Remove + Adapt** | 删除旧单体 hook impl，按 completion/tool/stream callbacks 迁移；Jaco persistence/approval policy 保留 |
| `DynamicTool` + `ToolContext` + `ToolOutput` | `RigToolExecutor`、`RegisteredRigTool`、字符串 JSON 反解析 | **Remove + Adopt** | 删除两层旧 compatibility adapter；直接返回 canonical structured output，并由 ToolContext 传 persistence metadata |
| Rig RMCP `rmcp_tools_with_timeout` | Jaco 直接构造旧 public `McpTool` | **Remove + Adapt** | 采用 Rig 的 `CallToolResult` rich-content 转换；Jaco 在 `WithBuilderTools` typestate 用 `vec![tool] + ServerSink + timeout` 注册，只适配 finalized runtime name 与 audit context |
| Rig MCP client/session handler | Jaco `McpSessionManager`、OAuth/config/list-change/UI | **Retain** | Rig 不覆盖产品配置、审批和 app state ownership，不整体替换 |
| Rig GPT-5.6 constants/typed reasoning | `reasoning_additional_params` 手写 OpenAI JSON | **Remove + Adopt** | OpenAI 分支采用 Rig enums/builders；保留 Jaco capability 与 runtime policy |
| Rig Responses WebSocket session | 当前无 WebSocket；只有 HTTP/SSE completion model | **Adopt + Adapt** | 直接使用 connect/send/next_event/close/previous-ID 状态机；Jaco 增加跨 run pool 与 stream decoder |
| Rig private SSE accumulator | 无本地对应实现 | **Retain upstream boundary** | 不复制 private source；基于 public events 实现薄 decoder，并用 parity fixture 防漂移 |
| Rig/OpenAI structured provider errors | 多处只保存 `error.to_string()` | **Remove + Adopt** | 使用 `provider_response_json/status/body` 分类 continuation error，仍写 Jaco `RunErrorPayload` |
| Rig streaming `Unknown(Value)` | `runtime.rs` 当前无分支 | **Adapt** | 保存到 provider response audit，不为未知 hosted output发明用户可见 chat entry |

## 明确不删除的核心契约

`gpui-form` 的价值不在于绘制一个表单外壳，而在于以下当前公开契约；上游组件更新后仍需
保留。此处只用于判断 gpui-component 能否替代本地能力，不要求重写现有 skill：

- `FormStore`：`EventEmitter<FormEvent<Self::Field>>`，关联类型
  `Model/Output/Field/ValidationContext/ValidationAdapter/SubmitTransform`，以及
  constructor、validation/prepare、whole-model lifecycle、revision/CAS 和 query 方法。
- `FormField<Form,T>`：typed value/lens、single write transaction、validation/async
  validation、`attach_control`、subscription 和三类 typed error。
- `FormControl<T>`/`ControlAttachment<Form,T>`：owning native entity、subscription
  lifetime 和四个 deferred intent；没有 generic draft/codec/readback。
- `ValidationAdapter<Model>`/`SubmitTransform<Model>`：sync report normalization 与 pure
  prepared-output boundary；persistence task 不进入 form。
- `FormFieldId`/`FormItemId`/`ToFormItemId`：generated schema、稳定数组路径和 error
  routing；groups/arrays 不创建 child form entity。

## 删除门槛

只有同时满足以下条件才把某个本地 workaround 标记为 Remove：

1. 能定位其原始目的和所有调用点；
2. 上游公开 API 或行为测试覆盖同一语义；
3. 删除后 focused test 与交互回归通过；
4. 不把 app-specific domain/state ownership 移入通用组件层。

找不到证据时结论必须是 Retain 并写明原因，不能凭相似命名删除。

## Rig 删除完成后的禁止残留

实施 DR-60/GA-10 后，以下符号或模式应为零：

```text
rig_core::
ToolDyn
RigToolExecutor
RegisteredRigTool
tool_output_to_model_text
rig_core::tool::rmcp::McpTool
.with_history(
.with_tool_concurrency(
.hook(
```

`.hook(` 搜索需要区分非 Rig 同名 API；验收只要求 Rig AgentBuilder 调用点清零。删除本地
adapter 前必须先让 focused tool/MCP/approval tests 覆盖 structured output 和状态顺序，
不能用“新版看起来相同”代替行为证据。
