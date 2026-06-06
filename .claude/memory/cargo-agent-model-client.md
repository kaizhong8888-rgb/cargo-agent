---
name: cargo-agent-model-client
description: ModelClient handles LLM API calls with OpenAI-compatible protocol + automatic Anthropic adaptation, retry with exponential backoff, and tool call support.
metadata:
  type: project
---

## ModelClient 架构

**文件**：`src/model/client.rs`

**支持协议**：
- **OpenAI 兼容** — 标准 `/v1/chat/completions` 端点
- **Anthropic Messages** — `/v1/messages` 端点（自动检测 base_url 包含 "anthropic"）

**URL 构建逻辑**：
- `build_openai_url()` — 智能处理 base_url 是否已包含 `/v1` 或 `/v1/chat/completions`
- `build_anthropic_url()` — 同上，适配 Anthropic 路径

**重试机制**：
- 最多 3 次重试
- 可重试错误：429（限流）、500/502/503/504（服务器错误）
- 指数退避：1s → 2s → 4s
- 429 优先使用 Retry-After 头部

**工具调用支持**：
- OpenAI 格式：`tools` 数组，`function` schema
- Anthropic 格式：自动转换消息格式（system message → system 参数，tool_use/tool_result 内容部分）

**DeepSeek 推理模型支持**：
- 解析 `reasoning_content` 字段
- 在后续消息中传回 `reasoning_content`

**流式支持**：
- `chat_stream()` — SSE 流式响应（基础设施已就绪，解析器待实现）

## ModelRouter

**文件**：`src/model/router.rs`

**任务复杂度估算**：
- Low：短消息（<50 词），无复杂关键词
- Medium：中等消息（50-200 词）
- High：长消息（>200 词）、复杂关键词（architect/design/review/security/refactor/optimize/analyze/compare/plan/strategy/implementation/trade-off）、多问号

**模型路由**：
- Low → `low_complexity_model`（如果设置）
- Medium → `default_model`
- High → `high_complexity_model`（如果设置）

当前只设置了 default_model（deepseek-v4-flash），高低复杂度模型为 None。

**为什么**：了解 LLM 客户端如何处理 API 调用、重试、多协议适配和工具调用对调试和扩展至关重要。
