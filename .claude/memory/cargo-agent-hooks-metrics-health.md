---
name: cargo-agent-hooks-metrics-health
description: Hook system (before/after chat/tool), session metrics (atomic counters), and HTTP health endpoint architecture.
metadata:
  type: project
---

## 钩子系统

**文件**：`src/hooks.rs`

**事件类型**：
- `BeforeChat` — 聊天请求前（user_message, model, message_count）
- `AfterChat` — 聊天响应后（response, duration_ms, tool_calls, tokens, error）
- `BeforeTool` — 工具调用前（tool_name, arguments）
- `AfterTool` — 工具调用后（result, duration_ms, success）

**HookManager**：
- `register(name, hook_fn)` — 注册命名钩子
- `dispatch(event)` — 分发事件到所有钩子
- `dispatch_and_log(event)` — 分发并记录非空响应

**内置钩子**：
1. **audit_log_hook** — 记录所有工具调用（审计日志）
2. **metrics_hook** — 将工具调用时序数据馈送到 SessionMetrics

## 指标系统

**文件**：`src/metrics.rs`

**SessionMetrics**（原子计数器）：
- LLM 指标：api_calls, prompt_tokens, completion_tokens, total_tokens
- 工具指标：tool_calls, tool_success, tool_errors
- 对话指标：user_messages, assistant_responses, chat_errors
- 延迟指标：total_chat_latency_ms, total_tool_latency_ms

**计算指标**：
- `tool_success_rate()` — 成功率百分比
- `avg_chat_latency_ms()` — 平均聊天延迟
- `avg_tool_latency_ms()` — 平均工具延迟
- `session_duration_secs()` — 会话持续时间

**ToolMetric**（每个工具的详细统计）：
- calls, total_duration_ms, errors, min_duration_ms, max_duration_ms
- `average_duration_ms()` — 平均延迟

## 健康端点

**文件**：`src/health.rs`

- 端口 8787，纯 `std::net::TcpListener`（无框架依赖）
- 端点：`/health`
- 响应：status, version, uptime_seconds, total_requests, total_errors, memory_bytes

**为什么**：钩子和指标系统是可观测性的基础，对调试性能问题和理解代理行为很重要。
