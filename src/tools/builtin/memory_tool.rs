//! Memory tools: store, recall, search, manage, and analyze memories.
//! Backed by SQLite for persistence across sessions.

use crate::memory::SqliteMemoryStore;
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Store a memory: key-value pairs with namespace, tags, and importance.
pub struct StoreMemory {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for StoreMemory {
    fn name(&self) -> &str {
        "store_memory"
    }

    fn description(&self) -> &str {
        "Store a memory with a key-value pair for later recall. Use namespaces to organize memories. Supports tags and importance level (1-10)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "key".to_string(),
                description: "Unique key identifier for the memory".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "value".to_string(),
                description: "The content/value to remember".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "namespace".to_string(),
                description: "Namespace to organize memories (e.g. 'user_preferences', 'project_context', 'conversation_history')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "tags".to_string(),
                description: "Comma-separated tags for categorization".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "importance".to_string(),
                description: "Importance level 1-10 (10=most important, default: 5)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let key = params
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: key")?
            .to_string();

        let value = params
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: value")?
            .to_string();

        let namespace = params
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let tags_str = params.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let tags: Vec<String> = if tags_str.is_empty() {
            vec![]
        } else {
            tags_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        let importance = params
            .get("importance")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(5)
            .clamp(1, 10);

        let entry = self
            .memory
            .store(&key, &value, &namespace, &tags, importance)
            .map_err(|e| format!("Storage error: {e}"))?;

        Ok(serde_json::json!({
            "status": "stored",
            "key": entry.key,
            "namespace": entry.namespace,
            "importance": entry.importance,
        }))
    }
}

/// Recall/retrieve a memory by key.
pub struct RecallMemory {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for RecallMemory {
    fn name(&self) -> &str {
        "recall_memory"
    }

    fn description(&self) -> &str {
        "Retrieve a stored memory by its key. Returns the full memory entry including value, tags, and metadata."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "key".to_string(),
            description: "The key of the memory to recall".to_string(),
            required: true,
            parameter_type: "string".to_string(),
        }]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let key = params
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: key")?;

        match self
            .memory
            .recall(key)
            .map_err(|e| format!("Recall error: {e}"))?
        {
            Some(entry) => {
                let tags_list: Vec<String> = entry
                    .tags
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                Ok(serde_json::json!({
                    "found": true,
                    "key": entry.key,
                    "value": entry.value,
                    "namespace": entry.namespace,
                    "tags": tags_list,
                    "importance": entry.importance,
                    "created_at": entry.created_at,
                    "updated_at": entry.updated_at,
                }))
            }
            None => Ok(serde_json::json!({
                "found": false,
                "key": key,
                "message": "Memory not found with this key",
            })),
        }
    }
}

/// Search memories by namespace, tags, or text content.
pub struct SearchMemories {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for SearchMemories {
    fn name(&self) -> &str {
        "search_memories"
    }

    fn description(&self) -> &str {
        "Search through stored memories by namespace, tags, or text content. Returns matching memories sorted by importance."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "namespace".to_string(),
                description: "Filter by namespace".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "tag".to_string(),
                description: "Filter by a specific tag".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "query".to_string(),
                description: "Search text in key or value (case-insensitive)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "min_importance".to_string(),
                description: "Minimum importance level (1-10)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
            ToolParameter {
                name: "limit".to_string(),
                description: "Maximum number of results to return (default: 20)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let namespace_filter = params.get("namespace").and_then(|v| v.as_str());
        let tag_filter = params.get("tag").and_then(|v| v.as_str());
        let query = params.get("query").and_then(|v| v.as_str());
        let min_importance = params
            .get("min_importance")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8);
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

        let results = self
            .memory
            .search(namespace_filter, tag_filter, query, min_importance, limit)
            .map_err(|e| format!("Search error: {e}"))?;

        let result_values: Vec<Value> = results
            .iter()
            .map(|entry| {
                let tags_list: Vec<String> = entry
                    .tags
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                serde_json::json!({
                    "key": entry.key,
                    "value": entry.value,
                    "namespace": entry.namespace,
                    "tags": tags_list,
                    "importance": entry.importance,
                    "created_at": entry.created_at,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": result_values.len(),
            "results": result_values,
        }))
    }
}

/// List all namespaces.
pub struct ListNamespaces {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for ListNamespaces {
    fn name(&self) -> &str {
        "list_namespaces"
    }

    fn description(&self) -> &str {
        "List all memory namespaces and the count of memories in each."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![]
    }

    async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
        let namespaces = self
            .memory
            .list_namespaces()
            .map_err(|e| format!("List error: {e}"))?;

        let ns_list: Vec<Value> = namespaces
            .iter()
            .map(|(ns, count)| {
                serde_json::json!({
                    "namespace": ns,
                    "count": count,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "namespaces": ns_list,
        }))
    }
}

/// Delete a memory by key.
pub struct DeleteMemory {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for DeleteMemory {
    fn name(&self) -> &str {
        "delete_memory"
    }

    fn description(&self) -> &str {
        "Delete a stored memory by its key."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "key".to_string(),
            description: "The key of the memory to delete".to_string(),
            required: true,
            parameter_type: "string".to_string(),
        }]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let key = params
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: key")?;

        let deleted = self
            .memory
            .delete(key)
            .map_err(|e| format!("Delete error: {e}"))?;

        if deleted {
            Ok(serde_json::json!({
                "status": "deleted",
                "key": key,
            }))
        } else {
            Ok(serde_json::json!({
                "status": "not_found",
                "key": key,
                "message": "No memory found with this key",
            }))
        }
    }
}

/// Get memory statistics.
pub struct MemoryStats {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for MemoryStats {
    fn name(&self) -> &str {
        "memory_stats"
    }

    fn description(&self) -> &str {
        "Get statistics about the memory system: total memories, per-namespace breakdown, importance distribution."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![]
    }

    async fn execute(&self, _params: &HashMap<String, Value>) -> Result<Value, String> {
        let stats = self
            .memory
            .stats()
            .map_err(|e| format!("Stats error: {e}"))?;

        let ns_breakdown: Vec<Value> = stats
            .by_namespace
            .iter()
            .map(|(ns, count)| {
                serde_json::json!({
                    "namespace": ns,
                    "count": count,
                })
            })
            .collect();

        let imp_breakdown: Vec<Value> = stats
            .by_importance
            .iter()
            .map(|(imp, count)| {
                serde_json::json!({
                    "importance": imp,
                    "count": count,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "total_memories": stats.total,
            "by_namespace": ns_breakdown,
            "by_importance": imp_breakdown,
        }))
    }
}

/// Register all memory tools with the registry, sharing a single memory store.
pub fn register_all(registry: &mut ToolRegistry, memory: Arc<SqliteMemoryStore>) {
    registry.register(Box::new(StoreMemory {
        memory: memory.clone(),
    }));
    registry.register(Box::new(RecallMemory {
        memory: memory.clone(),
    }));
    registry.register(Box::new(SearchMemories {
        memory: memory.clone(),
    }));
    registry.register(Box::new(ListNamespaces {
        memory: memory.clone(),
    }));
    registry.register(Box::new(DeleteMemory {
        memory: memory.clone(),
    }));
    registry.register(Box::new(MemoryStats { memory }));
}
