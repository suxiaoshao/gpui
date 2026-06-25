# Model Reasoning Capability 数据来源

本文记录 ai-chat / ai-chat2 中“思考强度 / reasoning effort / thinking”能力应该如何建模。
目标是避免把临时硬编码、provider 默认值、模型族推断误当成准确模型能力。

最后核对时间：2026-06-03。

## 结论

- 不能用统一的 `low / medium / high` 覆盖所有 provider。不同 provider 的控制面并不相同：有的是离散档位，有的是 token budget，有的是 boolean，有的是 always-on。
- OpenAI 的模型能力不能从 `/v1/models` 精确获取；`/v1/models` 只返回 id、owner、availability 这类基础信息。OpenAI 模型页和 reasoning 文档才是 reasoning 档位的事实来源。
- Ollama 的 `/api/show` 能获取每个本地模型的 `capabilities`，可以判断 `thinking`、`vision`、`tools` 等能力；但官方 `capabilities` 不是“档位列表”。Ollama thinking 档位仍要结合官方 docs 和 model family 规则。
- Qwen 没有显示 OpenAI 式思考强度是合理的：Qwen 官方控制面是 `enable_thinking` 开关和部分模型的 `thinking_budget` token 数，不是 `low / medium / high / xhigh` 这类命名档位。通过 Ollama 使用 Qwen 时，Ollama 官方也把 Qwen 归为 boolean `think`，不是 level。
- OpenRouter 是特例：它作为聚合 provider 会标准化 `reasoning` 参数，并且 `/api/v1/models` 返回 `supported_parameters`。但这只表示 OpenRouter 路由层支持哪些参数，不等于原始 provider 的完整能力模型。
- ai-chat2 fresh DB 的 provider model cache 应保存“来源类型”：API-discovered、official-doc-derived、heuristic、manual。UI 只能把 API-discovered 或 official-doc-derived 的档位当成可信档位。

## 当前实现从哪里来

### Legacy `app/ai-chat`

OpenAI provider 已经有模型 id 解析和 hardcoded profile：

- o-series：`low / medium / high`，默认 `medium`。
- GPT-5：`minimal / low / medium / high`，默认 `medium`。
- GPT-5.1：`none / low / medium / high`，默认 `none`。
- GPT-5.2+：`none / low / medium / high / xhigh`，当前代码默认 `none`。
- GPT-5 pro：`high` only。
- GPT-5.2+ pro：`medium / high / xhigh`，默认 `medium`。

这些数据来自 `app/ai-chat/src/llm/provider/openai.rs` 中的 slug 解析和常量表，不是从 OpenAI API 返回的模型能力中读取。它可以作为当前实现事实，但不能视为长期事实来源。

Ollama provider 会调用 `/api/tags` 列模型，再对每个模型调用 `/api/show`。当前映射规则：

- `capabilities` 含 `completion` 才进入模型列表。
- `capabilities` 含 `vision` 映射为 image input。
- `capabilities` 含 `tools` 映射为本地 web tools。
- `capabilities` 含 `thinking` 映射为 reasoning。
- 只有 `family` / `families` 为 `gptoss` 或 `gpt-oss` 时，UI 使用 `low / medium / high` levels。
- 其他 thinking model，包括 Qwen3，当前 UI 使用 boolean `think`。

这个 Ollama 映射与 Ollama 官方 thinking 文档一致：大多数模型接受 boolean，GPT-OSS 接受 `low / medium / high` levels。

### Fresh crates / ai-chat2

`crates/ai-chat-core/src/capabilities.rs` 里的 `conservative_model_capabilities` 现在只保留 provider 级别的基础保守能力，不再给任何 provider 自动填 reasoning 档位。

- `openai / anthropic / gemini / openrouter` 不再因为 provider kind 自动显示 reasoning。
- provider model refresh 通过 provider-specific enrichment 写入 `ReasoningControl` 和 `CapabilitySource`。
- 自动 discovery / docs profile 未命中时，UI 不显示猜测出的 reasoning control。

`conservative_model_capabilities` 仍可作为无模型级数据时的基础 fallback，但它不能作为 reasoning 档位来源。

## Provider 能力来源矩阵

| Provider | API 能否列模型 | API 能否给 reasoning 能力/档位 | 官方档位 / 控制面 | 当前建议 |
| --- | --- | --- | --- | --- |
| OpenAI | 可以，`/v1/models` 只给基础 model object | 不给完整能力矩阵 | `reasoning.effort` 是模型相关；可包含 `none`、`minimal`、`low`、`medium`、`high`、`xhigh` | 用官方 model docs 建 doc-derived profile；不要只靠 `/v1/models` |
| Ollama | 可以，`/api/tags` | `/api/show.capabilities` 可给 `thinking`，但不返回档位数组 | Native `think` 为 boolean 或 `low / medium / high`；GPT-OSS only levels，大多数 thinking model boolean | 用 `/api/show` 判断支持 thinking；levels 只对官方确认的 family 暴露 |
| Qwen / DashScope | Qwen Cloud / DashScope 有模型列表和模型文档 | thinking 支持由模型文档和请求参数确认，不是 OpenAI 式档位 | Hybrid：`enable_thinking`; depth：`thinking_budget` token 数；thinking-only model always-on | 不显示 low/medium/high；直接建 boolean + optional token budget |
| Anthropic | 可以，Models API 列 available models | Models API 只给基础 model object，不给完整能力矩阵 | 旧/部分模型：`thinking: {type: "enabled", budget_tokens}`；新模型：adaptive thinking + `output_config.effort`，levels 为 `low / medium / high / xhigh / max`，按模型支持 | 用官方 docs 建 profile；不要把 OpenAI 的 `none/minimal` 套给 Claude |
| Gemini | 可以，Models API 返回 `thinking: boolean`、token limits、supported methods | 能确认是否 supports thinking；不返回每个模型的 thinking level 枚举 | Gemini 3+ 使用 `thinkingLevel`；Gemini 2.5 使用 `thinkingBudget`；是否能禁用和 level 集合按模型而定 | API 判断 supports thinking，官方 docs 决定 level/budget UI |
| DeepSeek | 官方 docs 当前列 `deepseek-v4-flash` / `deepseek-v4-pro` | 未发现可列完整能力矩阵的模型能力 API | `thinking.type` enabled/disabled；`reasoning_effort` 真实值为 `high / max`，`low/medium/xhigh` 只是兼容映射 | UI 应显示 disabled/high/max，不把 low/medium 当真实档位 |
| Mistral | API 可列 available models；模型概览解释用途 | Chat API spec 给 `reasoning_effort` 参数，不等于每模型能力矩阵 | Adjustable models：`high / none`；native Magistral 类模型 always reasoning | 对 adjustable model 显示 none/high；native model 不显示 effort select |
| OpenRouter | 可以，`/api/v1/models` 返回 model properties | 返回 `supported_parameters`，可筛选 `reasoning` / `include_reasoning` | OpenRouter 统一 `reasoning`：`effort`、`max_tokens`、`exclude`、`enabled`；effort 支持 OpenAI-style `xhigh/high/medium/low/minimal/none`，并按下游模型映射 | 读取 `supported_parameters`；用 OpenRouter docs 和 model description 解释实际映射 |

## Provider 细节

### OpenAI

官方来源：

- [Models](https://developers.openai.com/api/docs/models)
- [All models](https://developers.openai.com/api/docs/models/all)
- [Reasoning models](https://developers.openai.com/api/docs/guides/reasoning)
- [List models API reference](https://platform.openai.com/docs/api-reference/models/list)
- [Latest model guide](https://developers.openai.com/api/docs/guides/latest-model)

事实：

- `/v1/models` 只能作为“当前账号可用模型 id 列表”和基础 model object 来源。它不包含完整 modality、tool、reasoning effort、structured output、web search 等能力矩阵。
- OpenAI reasoning docs 说明 `reasoning.effort` 的 supported values 是 model-dependent，可包含 `none`、`minimal`、`low`、`medium`、`high`、`xhigh`。
- OpenAI model pages当前会直接展示模型的 reasoning 档位。例如 GPT-5.5 / GPT-5.4 系列页面展示 `none / low / medium / high / xhigh`，GPT-5 页面展示 `minimal / low / medium / high`，GPT-5.2-Codex 页面展示 `low / medium / high / xhigh`。
- 因此 OpenAI 能力建模应该是 doc-derived profile，而不是只按 `gpt-5*` 前缀推断。前缀推断可作为 fallback，但必须带 `source = heuristic`。

当前风险：

- 当前 `app/ai-chat` 把 GPT-5.2+ 普通模型默认 effort 设为 `none`。最新 OpenAI latest-model guide 中 GPT-5.5 默认是 `medium`。如果继续支持最新模型，默认值需要按具体模型版本从官方文档刷新，而不是用 `minor >= 2` 一刀切。

### Ollama

官方来源：

- [Thinking](https://docs.ollama.com/capabilities/thinking)
- [Show model details](https://docs.ollama.com/api-reference/show-model-details)
- [Ollama API docs](https://docs.ollama.com/api)

事实：

- `/api/show` 返回模型详情，其中 `capabilities` 是 string array，示例包括 `completion`、`vision`，thinking 文档和当前实现也使用 `thinking`、`tools`。
- Native `/api/chat` / `/api/generate` 的 `think` 字段接受 `boolean | "high" | "medium" | "low"`。
- Ollama thinking 文档明确：大多数 thinking models 接受 boolean；GPT-OSS 需要 `low / medium / high` levels，且不能完全 disable trace。
- Qwen3、DeepSeek R1、DeepSeek-v3.1 在 Ollama thinking 文档中是 supported thinking models，但不是 levels model。

实现建议：

- 保留当前 `/api/show` discovery。
- `capabilities` 只决定有无 thinking，不决定档位。
- `OllamaThinkingCapability::Levels` 只给官方确认的 level family，例如 `gpt-oss` / `gptoss`。
- Qwen 经 Ollama 时显示 boolean `think`，不要显示 `low / medium / high`。

### Qwen / DashScope

官方来源：

- [Qwen Cloud Thinking](https://docs.qwencloud.com/developer-guides/text-generation/thinking)
- [Qwen open-source quickstart: Thinking Budget](https://qwen.readthedocs.io/en/stable/getting_started/quickstart.html#thinking-budget)
- [Qwen Cloud OpenAI Chat](https://docs.qwencloud.com/api-reference/chat/openai-chat)

事实：

- Qwen thinking model 分为 hybrid 和 thinking-only。
- Hybrid model 通过 `enable_thinking` 开关按请求启用或关闭 thinking。Qwen3.5 默认 enabled；Qwen3、Qwen3-VL、Qwen3-Omni 默认 disabled；具体默认值应看 model list / model docs。
- Thinking-only model，例如 QwQ 和 `-thinking` variants，不能 disable thinking。
- Qwen3 之后支持 `thinking_budget` token cap，但 Qwen Cloud 文档注明 Chat Completions / DashScope 支持，Responses API 不支持。
- open-source Qwen 文档说明 thinking budget 是模型特定控制，开源框架中不一定原生可用；Alibaba Cloud Model Studio API 有实现。

实现建议：

- Direct Qwen/DashScope provider 应建 provider-specific settings：
  - `enable_thinking: bool | unavailable`
  - `thinking_budget: Option<u32>`，按模型支持和最大值校验
  - `thinking_only: bool`
- 不把 Qwen 映射成 OpenAI-style `ReasoningEffort`，除非 provider 明确返回或官方 docs 明确给命名档位。

### Anthropic

官方来源：

- [List Models API](https://anthropic.mintlify.app/en/api/models-list)
- [Extended thinking](https://platform.claude.com/docs/en/build-with-claude/extended-thinking)
- [Effort](https://platform.claude.com/docs/en/build-with-claude/effort)

事实：

- Models API 返回 available models，字段包括 `id`、`display_name`、`created_at`、`type`，不能作为完整 capability matrix。
- Claude extended thinking 有两套控制：
  - Manual：`thinking: {type: "enabled", budget_tokens: N}`。
  - Adaptive：`thinking: {type: "adaptive"}`，用 `output_config.effort` 控制深度。
- `effort` API levels 是 `low / medium / high / xhigh / max`，但模型支持不同：例如 `xhigh` 只在 Opus 4.7 / 4.8 等模型上可用，`max` 也不是所有模型都有。
- Anthropic `effort` 不是 OpenAI `reasoning.effort`。它影响整体 token spend，也可以不启用 thinking 时使用。

实现建议：

- Anthropic provider 应保存 `thinking_mode = manual_budget | adaptive | unsupported`。
- UI 不应显示 OpenAI `none/minimal`。
- 对 adaptive model 显示 `low / medium / high`，并按具体模型补 `xhigh / max`。
- 对 manual-only model 显示 token budget，而不是命名档位。

### Gemini

官方来源：

- [Gemini thinking](https://ai.google.dev/gemini-api/docs/thinking)
- [Gemini Models API](https://ai.google.dev/api/models)
- [Gemini models overview](https://ai.google.dev/gemini-api/docs/models)

事实：

- Gemini Models API 的 `Model` resource 包含 `thinking: boolean`，以及 `inputTokenLimit`、`outputTokenLimit`、`supportedGenerationMethods` 等字段。
- `thinking: boolean` 只能表示模型是否支持 thinking；它不列出 `thinkingLevel` 可选值。
- Gemini 3+ 推荐 `thinkingLevel`。
- Gemini 2.5 系列不支持 `thinkingLevel`，应使用 `thinkingBudget`。
- `thinkingBudget = 0` 可关闭部分 2.5 thinking；`thinkingBudget = -1` 表示 dynamic thinking；具体范围和能否关闭按模型文档。
- Gemini 3 Pro / Flash / Flash-Lite 对 `minimal`、`low`、`medium`、`high` 的支持不同；不能用一张全局档位表。

实现建议：

- API discovery 读取 `Model.thinking`。
- level/budget controls 用 official-doc-derived per-model profile。
- 对 2.5 model 使用 budget UI；对 3+ model 使用 level UI；若 docs 未确认具体 levels，只显示 enabled/dynamic 状态。

### DeepSeek

官方来源：

- [Thinking Mode](https://api-docs.deepseek.com/guides/thinking_mode)
- [Create Chat Completion](https://api-docs.deepseek.com/api/create-chat-completion)
- [Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing)

事实：

- 当前官方模型页列 `deepseek-v4-flash` 和 `deepseek-v4-pro`，都支持 non-thinking 和 thinking，默认 thinking enabled。
- Thinking toggle：`thinking: {"type": "enabled" | "disabled"}`。
- Thinking effort：真实支持 `high / max`。
- 兼容映射：`low`、`medium` 映射到 `high`，`xhigh` 映射到 `max`。
- thinking mode 下不支持 temperature、top_p、presence_penalty、frequency_penalty；兼容原因下设置这些参数不报错但无效。

实现建议：

- UI 应暴露 `disabled / high / max`。
- 不显示 `low / medium`，除非 UI 明确标注它们只是 aliases。
- DeepSeek tool-use thinking 必须正确保存并回传 `reasoning_content`，这属于 runtime/persistence 能力，不只是 picker 档位。

### Mistral

官方来源：

- [Adjustable reasoning](https://docs.mistral.ai/studio-api/conversations/reasoning/adjustable)
- [Chat API spec](https://docs.mistral.ai/api)
- [Models overview](https://docs.mistral.ai/models)

事实：

- Adjustable reasoning 当前用于 `mistral-small-latest` 和 `mistral-medium-3-5`。
- Chat API `reasoning_effort` 枚举是 `high | none`。
- `high` 返回 thinking chunk；`none` 表示 minimal thinking 且不返回 thinking chunk。
- Native Magistral reasoning model always emits reasoning traces，不需要也不应该显示 adjustable effort。

实现建议：

- 对 adjustable model 显示 `none / high`。
- 对 native reasoning model 显示“always reasoning”，不显示 effort select。
- 模型列表 API 可用于可用模型；reasoning 类型仍以 docs-derived profile 为准。

### OpenRouter

官方来源：

- [List all models and properties](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [Models overview](https://openrouter.ai/docs/guides/overview/models)
- [Reasoning tokens](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)

事实：

- `/api/v1/models` 返回 `architecture`、`context_length`、`pricing`、`top_provider`、`supported_parameters`、`default_parameters` 等字段。
- `supported_parameters` 可包含 `reasoning`、`include_reasoning`、`tools`、`structured_outputs` 等，并可用 query filter。
- OpenRouter `reasoning` 参数统一了多个 provider：
  - `effort`: `xhigh / high / medium / low / minimal / none`
  - `max_tokens`
  - `exclude`
  - `enabled`
- `reasoning.max_tokens` 当前支持 Gemini thinking models、Anthropic reasoning models，以及部分 Alibaba Qwen thinking models，并映射到各自 provider 参数。
- `reasoning.effort` 当前支持 OpenAI reasoning models 和 Grok models。对只支持 token budget 的模型，OpenRouter 会按比例映射。

实现建议：

- OpenRouter provider 应优先读取 `/api/v1/models.supported_parameters`。
- 若有 `reasoning`，UI 可以显示 OpenRouter unified reasoning control，但需要把“OpenRouter normalized”与“origin provider native”区别开。
- 对下游模型的精确 levels 仍需参考 OpenRouter model description 或官方 docs，不要从 provider slug 盲推。

## 建模建议

### Capability source

Provider model cache 应增加来源标记，至少能表达：

```text
CapabilitySource =
  ApiDiscovered(provider, endpoint, checked_at)
  OfficialDocs(provider, url, checked_at)
  Heuristic(reason)
  Manual(user_or_config)
```

UI 展示原则：

- `ApiDiscovered` 和 `OfficialDocs` 可以直接显示为可选控制。
- `Heuristic` 只能作为 fallback；应避免显示过细档位，或在内部标记为低可信。
- `Manual` 由用户显式配置时可以使用，但不能推广到同 provider 的其他模型。

### Reasoning control shape

不要把所有 provider 压成一个 `Vec<ReasoningEffort>`。建议拆成 provider-neutral union：

```text
ReasoningControl =
  None
  Boolean { default_enabled }
  Levels { values, default_value }
  TokenBudget { min, max, default, dynamic_supported, off_supported }
  AdaptiveLevels { values, default_value }
  AlwaysOn { visible_summary_supported }
```

映射示例：

- OpenAI GPT-5.5：`Levels { none, low, medium, high, xhigh; default = medium }`
- OpenAI GPT-5：`Levels { minimal, low, medium, high; default = medium }`
- Ollama Qwen3：`Boolean { default_enabled = provider/model default }`
- Ollama GPT-OSS：`Levels { low, medium, high; default = medium }`
- Qwen Cloud Qwen3.5：`Boolean + TokenBudget`，而不是 levels。
- Anthropic Opus 4.8：`AdaptiveLevels { low, medium, high, xhigh, max; default = high }`
- Anthropic manual model：`TokenBudget`
- Gemini 2.5 Flash：`TokenBudget { off_supported = true, dynamic_supported = true }`
- Gemini 3 Flash：`Levels`，具体 values 按 model docs。
- DeepSeek V4：`Levels { disabled, high, max; default = high }`
- Mistral adjustable：`Levels { none, high }`
- Mistral native Magistral：`AlwaysOn`
- OpenRouter：`Composite(Boolean + Levels + TokenBudget)` under `source = OpenRouterNormalized`

## 下一步

- 已移除 fresh `conservative_model_capabilities` 的 provider-wide reasoning 默认。
- 已为 Ollama、Gemini、OpenRouter 实现 API discovery enrichment，并保存 raw provider model payload。
- 已为 OpenAI、Anthropic、DeepSeek、Mistral 建 doc-derived profile registry，并记录 source URL 和 checked_at。
- Composer reasoning selector 已改为按 `ReasoningControl` 派生 selection；TokenBudget 已提供 Off / Dynamic / Custom 选择和 numeric input。
- `ai-chat-agent` 已能把 `ReasoningSelectionSnapshot` 转成 provider-specific additional params；真实 agent run 接线仍依赖后续 conversation/runtime UI。
