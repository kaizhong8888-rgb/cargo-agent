---
name: cargo-agent-memory-and-skills
description: Memory system (SQLite + TF-IDF semantic search) and skills system (YAML-based keyword-triggered context injection) architecture.
metadata:
  type: project
---

## 记忆系统

**存储**：SQLite at `~/.cargo-agent/memories.db`

**MemoryEntry 结构**：
- `key` — 唯一键（字符串）
- `value` — 内容值
- `namespace` — 命名空间（默认 "default"）
- `tags` — 逗号分隔标签
- `importance` — 重要性（1-10）
- `created_at` / `updated_at` — 时间戳

**核心操作**：
- `store(key, value, namespace, tags, importance)` — 存储/更新
- `recall(key)` — 按键回忆
- `search(namespace, tag, query, min_importance, limit)` — 多条件搜索
- `delete(key)` — 删除
- `list_namespaces()` — 列出所有命名空间
- `stats()` — 统计（按命名空间/重要性分组）

**语义搜索**（TF-IDF）：
- `semantic_search(query, limit)` — 组合 TF-IDF + 重要性权重 + 近衰减
- 评分公式：`tf_score * (importance/10) * recency`
- key 匹配权重 3x（比 value 更具体）
- 近衰减：`1.0 / (1.0 + age_hours * 0.01)`

**记忆注入流程**（`inject_memory_context`）：
1. 从用户消息提取关键词
2. 调用 `semantic_search` 获取 top 5
3. 不足 5 条时，用关键词搜索补充高重要性记忆
4. 保留 top 10，作为 system 消息注入对话

## 技能系统

**存储**：`~/.cargo-agent/skills/*.yaml`

**Skill 结构**：
```yaml
name: "rust-helper"
description: "Rust programming helper"
always_active: false
keywords: ["rust", "cargo"]
system_instructions: "Help with Rust code."
reference: "Use `cargo check` to verify."
reference_files: []
```

**激活机制**：
- `always_active: true` — 始终注入
- 关键词匹配 — 用户消息包含任何关键词时注入
- `build_context_for(message)` — 组合所有激活技能的指令

**技能管理**：
- `SkillRegistry::load_from_dir(dir)` — 从目录加载所有 YAML
- `register(skill)` / `remove(name)` — 动态注册/移除
- `matching_skills(message)` — 关键词匹配
- `active_skills()` — 获取始终活跃的技能

**为什么**：记忆和技能系统是 cargo-agent 实现"自进化"和"领域专精"的核心机制。
