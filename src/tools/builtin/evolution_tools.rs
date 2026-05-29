//! Self-modification tool: read/write/patch source files with build verification.
//! Supports actions: read_file, write_file, create_file, delete_file, patch_file, cargo_check, cargo_test.

use crate::memory::SqliteMemoryStore;
use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

pub struct SelfModifyTool;

#[async_trait::async_trait]
impl Tool for SelfModifyTool {
    fn name(&self) -> &str {
        "self_modify"
    }

    fn description(&self) -> &str {
        "Modify the agent's own source code. Supports reading, writing, creating, deleting, and patching files. Automatically runs cargo check after modifications to verify correctness."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action to perform: read_file, write_file, create_file, delete_file, patch_file, cargo_check, cargo_test".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "path".to_string(),
                description: "Relative path to the source file (e.g. 'src/tools/builtin/my_tool.rs')".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "content".to_string(),
                description: "Full file content for write_file/create_file actions".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "old_text".to_string(),
                description: "Text to find and replace (for patch_file action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "new_text".to_string(),
                description: "Replacement text (for patch_file action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "verify".to_string(),
                description: "Run cargo check after modification (true/false, default: true)".to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "tool_name".to_string(),
                description: "Name for the new tool (for create_tool action), e.g. 'WeatherTool'".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "tool_spec".to_string(),
                description: "Full Rust source code for the new tool implementation (for create_tool action)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        match action {
            "read_file" => self.read_file(params),
            "write_file" | "create_file" => self.write_file(params),
            "delete_file" => self.delete_file(params),
            "patch_file" => self.patch_file(params),
            "cargo_check" => self.run_cargo_check(),
            "cargo_test" => self.run_cargo_test(),
            "create_tool" => self.create_tool(params),
            other => Err(format!("Unknown action: {other}")),
        }
    }
}

impl SelfModifyTool {
    fn project_root(&self) -> Result<PathBuf, String> {
        let mut dir =
            std::env::current_dir().map_err(|e| format!("Cannot get current dir: {e}"))?;
        for _ in 0..10 {
            if dir.join("Cargo.toml").exists() {
                return Ok(dir);
            }
            if !dir.pop() {
                break;
            }
        }
        Err("Could not find project root (Cargo.toml)".to_string())
    }

    fn resolve_path(&self, rel_path: &str) -> Result<PathBuf, String> {
        let root = self.project_root()?;
        Ok(root.join(rel_path))
    }

    fn read_file(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let full_path = self.resolve_path(path)?;
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read {}: {e}", full_path.display()))?;

        let line_count = content.lines().count();
        Ok(serde_json::json!({
            "status": "ok",
            "path": path,
            "lines": line_count,
            "content": content,
        }))
    }

    fn write_file(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: content")?;
        let verify = params
            .get("verify")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let full_path = self.resolve_path(path)?;
        let root = self.project_root()?;
        if !full_path.starts_with(&root) {
            return Err("Path must be within the project directory".to_string());
        }

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {e}"))?;
        }

        std::fs::write(&full_path, content)
            .map_err(|e| format!("Failed to write {}: {e}", full_path.display()))?;

        let verify_result = if verify {
            Some(self.run_cargo_check_inner())
        } else {
            None
        };

        Ok(serde_json::json!({
            "status": "ok",
            "path": path,
            "verify": verify_result,
        }))
    }

    fn delete_file(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;

        let full_path = self.resolve_path(path)?;
        let root = self.project_root()?;

        if !full_path.starts_with(&root) {
            return Err("Path must be within the project directory".to_string());
        }

        if !full_path.exists() {
            return Ok(serde_json::json!({
                "status": "not_found",
                "path": path,
            }));
        }

        std::fs::remove_file(&full_path)
            .map_err(|e| format!("Failed to delete {}: {e}", full_path.display()))?;

        Ok(serde_json::json!({
            "status": "deleted",
            "path": path,
        }))
    }

    fn patch_file(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: path")?;
        let old_text = params
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: old_text")?;
        let new_text = params
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: new_text")?;
        let verify = params
            .get("verify")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let full_path = self.resolve_path(path)?;
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Failed to read {}: {e}", full_path.display()))?;

        if !content.contains(old_text) {
            return Ok(serde_json::json!({
                "status": "error",
                "message": "old_text not found in file",
                "path": path,
            }));
        }

        let new_content = content.replacen(old_text, new_text, 1);
        std::fs::write(&full_path, &new_content)
            .map_err(|e| format!("Failed to write {}: {e}", full_path.display()))?;

        let verify_result = if verify {
            Some(self.run_cargo_check_inner())
        } else {
            None
        };

        Ok(serde_json::json!({
            "status": "patched",
            "path": path,
            "verify": verify_result,
        }))
    }

    fn run_cargo_check(&self) -> Result<Value, String> {
        self.run_cargo_check_inner()
    }

    fn run_cargo_check_inner(&self) -> Result<Value, String> {
        let root = self.project_root()?;
        let output = Command::new("cargo")
            .arg("check")
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run cargo check: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(serde_json::json!({
                "status": "pass",
                "output": format!("{}\n{}", stdout, stderr),
            }))
        } else {
            Ok(serde_json::json!({
                "status": "fail",
                "errors": stderr.lines().filter(|l| l.contains("error")).take(10).collect::<Vec<_>>(),
                "output": format!("{}\n{}", stdout, stderr),
            }))
        }
    }

    fn run_cargo_test(&self) -> Result<Value, String> {
        let root = self.project_root()?;
        let output = Command::new("cargo")
            .arg("test")
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run cargo test: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(serde_json::json!({
                "status": "pass",
                "output": format!("{}\n{}", stdout, stderr),
            }))
        } else {
            Ok(serde_json::json!({
                "status": "fail",
                "output": format!("{}\n{}", stdout, stderr),
            }))
        }
    }

    fn create_tool(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let tool_name = params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: tool_name")?;
        let tool_spec = params
            .get("tool_spec")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: tool_spec")?;

        let file_name = format!("src/tools/builtin/{}.rs", to_snake_case(tool_name));
        let full_path = self.resolve_path(&file_name)?;
        let root = self.project_root()?;

        if !full_path.starts_with(&root) {
            return Err("Path must be within the project directory".to_string());
        }

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {e}"))?;
        }

        std::fs::write(&full_path, tool_spec)
            .map_err(|e| format!("Failed to write tool file: {e}"))?;

        // Update mod.rs to include the new module
        let mod_path = self.resolve_path("src/tools/builtin/mod.rs")?;
        let mod_content = std::fs::read_to_string(&mod_path)
            .map_err(|e| format!("Failed to read mod.rs: {e}"))?;

        let module_decl = format!("pub mod {};", to_snake_case(tool_name));
        if !mod_content.contains(&module_decl) {
            let new_mod_content = format!("{}\n{module_decl}\n", mod_content);
            std::fs::write(&mod_path, new_mod_content)
                .map_err(|e| format!("Failed to update mod.rs: {e}"))?;
        }

        let verify_result = self.run_cargo_check_inner();

        Ok(serde_json::json!({
            "status": "tool_created",
            "tool_name": tool_name,
            "file": file_name,
            "verify": verify_result,
            "next_step": "Register the tool in the tool registry and rebuild",
        }))
    }
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            if prev.is_lowercase()
                || (prev.is_uppercase() && i + 1 < chars.len() && chars[i + 1].is_lowercase())
            {
                result.push('_');
            }
        }
        result.push(ch.to_lowercase().next().unwrap());
    }
    result
}

/// Self-reflection tool: analyzes past evolution events, memories, and system state
/// to identify patterns, lessons learned, and actionable areas for self-improvement.
///
/// Supports multiple focus areas:
/// - `general`: Full overview of system health, memory distribution, and evolution status
/// - `tool_usage`: Analyze how tools are being used and identify unused/dead tools
/// - `knowledge_gaps`: Identify namespaces with sparse memories or areas lacking coverage
/// - `error_patterns`: Look for recurring errors or issues in memories
/// - `response_quality`: Assess completeness of stored knowledge and skill activation
pub struct SelfReflectTool {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for SelfReflectTool {
    fn name(&self) -> &str {
        "self_reflect"
    }

    fn description(&self) -> &str {
        "Reflect on past evolution events and stored memories to identify patterns, lessons learned, and areas for self-improvement. Supports focus areas: general, tool_usage, knowledge_gaps, error_patterns, response_quality."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "focus".to_string(),
                description: "Area to focus reflection on: tool_usage, response_quality, error_patterns, knowledge_gaps, general".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let focus = params
            .get("focus")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let store = &self.memory;

        match focus {
            "tool_usage" => self.reflect_tool_usage(store),
            "knowledge_gaps" => self.reflect_knowledge_gaps(store),
            "error_patterns" => self.reflect_error_patterns(store),
            "response_quality" => self.reflect_response_quality(store),
            _ => self.reflect_general(store),
        }
    }
}

impl SelfReflectTool {
    /// General reflection: full overview of system health, memory stats, and evolution status.
    fn reflect_general(&self, store: &crate::memory::SqliteMemoryStore) -> Result<Value, String> {
        let stats = store.stats().map_err(|e| format!("Stats error: {e}"))?;

        let evolution_memories = store
            .search(Some("evolution"), None, None, None, 50)
            .map_err(|e| format!("Search error: {e}"))?;

        let high_importance = store
            .search(None, None, None, Some(8), 20)
            .map_err(|e| format!("Search error: {e}"))?;

        // Count event types
        let mut event_type_counts: HashMap<String, u32> = HashMap::new();
        for mem in &evolution_memories {
            for tag in mem.tags.split(',').map(|s| s.trim()) {
                if !tag.is_empty() && tag != "evolution" {
                    *event_type_counts.entry(tag.to_string()).or_insert(0) += 1;
                }
            }
        }

        // Build recommendations based on data
        let mut recommendations: Vec<String> = Vec::new();

        if evolution_memories.is_empty() {
            recommendations
                .push("Start recording evolution events to build self-knowledge".to_string());
        }

        if stats.total < 5 {
            recommendations.push(
                "Store more memories across different namespaces for better context retention"
                    .to_string(),
            );
        }

        if high_importance.len() < 3 {
            recommendations.push(
                "Consider storing more high-importance (8-10) memories for critical knowledge"
                    .to_string(),
            );
        }

        if stats.by_namespace.len() < 3 {
            recommendations.push("Diversify namespaces - use 'security', 'project', 'user_preferences' for better organization".to_string());
        }

        // Check if certain important namespaces exist
        let namespace_names: Vec<&str> =
            stats.by_namespace.iter().map(|(s, _)| s.as_str()).collect();
        if !namespace_names.contains(&"security") {
            recommendations.push(
                "Create a 'security' namespace to store security best practices and audit findings"
                    .to_string(),
            );
        }
        if !namespace_names.contains(&"project") {
            recommendations.push(
                "Create a 'project' namespace to track project-specific context and decisions"
                    .to_string(),
            );
        }

        if recommendations.is_empty() {
            recommendations.push(
                "System is healthy. Continue building knowledge and evolving capabilities."
                    .to_string(),
            );
        }

        let ns_breakdown: Vec<Value> = stats
            .by_namespace
            .iter()
            .map(|(ns, count)| serde_json::json!({"namespace": ns, "count": count}))
            .collect();

        let imp_breakdown: Vec<Value> = stats
            .by_importance
            .iter()
            .map(|(imp, count)| serde_json::json!({"importance": imp, "count": count}))
            .collect();

        let lessons: Vec<&str> = evolution_memories
            .iter()
            .filter(|m| m.tags.contains("lesson_learned") || m.tags.contains("error_learned"))
            .map(|m| m.value.as_str())
            .collect();

        Ok(serde_json::json!({
            "status": "reflected",
            "focus": "general",
            "memory_stats": {
                "total": stats.total,
                "by_namespace": ns_breakdown,
                "by_importance": imp_breakdown,
            },
            "evolution": {
                "total_events": evolution_memories.len(),
                "event_types": event_type_counts,
            },
            "lessons_learned": lessons.len(),
            "recent_lessons": lessons,
            "recommendations": recommendations,
        }))
    }

    /// Focus on tool usage patterns: analyze available tools and identify coverage.
    fn reflect_tool_usage(
        &self,
        store: &crate::memory::SqliteMemoryStore,
    ) -> Result<Value, String> {
        // Get tool registration info from memories if any
        let tool_mentions = store
            .search(None, None, Some("tool"), None, 30)
            .map_err(|e| format!("Search error: {e}"))?;

        let skills_dir = crate::constants::skills_dir();
        let skills_count = if skills_dir.exists() {
            std::fs::read_dir(&skills_dir)
                .map(|entries| entries.filter_map(|e| e.ok()).count())
                .unwrap_or(0)
        } else {
            0
        };

        let mut recommendations: Vec<String> = Vec::new();

        if skills_count < 5 {
            recommendations.push(format!("Only {skills_count} skills installed. Consider creating more domain-specific skills."));
        } else {
            recommendations.push(format!(
                "Good skill coverage with {skills_count} skills installed."
            ));
        }

        if tool_mentions.is_empty() {
            recommendations.push("No tool usage patterns recorded. Consider storing tool-related memories to track effectiveness.".to_string());
        }

        Ok(serde_json::json!({
            "status": "reflected",
            "focus": "tool_usage",
            "skills_installed": skills_count,
            "tool_mentions_in_memory": tool_mentions.len(),
            "recommendations": recommendations,
        }))
    }

    /// Focus on knowledge gaps: identify namespaces with sparse coverage.
    fn reflect_knowledge_gaps(
        &self,
        store: &crate::memory::SqliteMemoryStore,
    ) -> Result<Value, String> {
        let stats = store.stats().map_err(|e| format!("Stats error: {e}"))?;

        let mut gaps: Vec<Value> = Vec::new();
        let mut recommendations: Vec<String> = Vec::new();

        // Analyze each namespace for depth
        for (ns, count) in &stats.by_namespace {
            let memories = store
                .search(Some(ns), None, None, None, 100)
                .map_err(|e| format!("Search error: {e}"))?;

            let total_importance: u64 = memories.iter().map(|m| m.importance as u64).sum();
            let avg_importance = if *count > 0 {
                total_importance / *count as u64
            } else {
                0
            };

            gaps.push(serde_json::json!({
                "namespace": ns,
                "count": count,
                "avg_importance": avg_importance,
                "depth": if *count < 3 { "shallow" } else if *count < 10 { "moderate" } else { "deep" },
            }));
        }

        if stats.by_namespace.is_empty() {
            recommendations
                .push("No memories stored yet. Start by storing key information.".to_string());
        }

        // Check for overall balance
        let shallow_count = gaps.iter().filter(|g| g["depth"] == "shallow").count();
        if shallow_count > 0 {
            recommendations.push(format!("{shallow_count} namespace(s) have shallow coverage (less than 3 memories). Deepen with more related memories."));
        }

        Ok(serde_json::json!({
            "status": "reflected",
            "focus": "knowledge_gaps",
            "namespaces_analysis": gaps,
            "recommendations": recommendations,
        }))
    }

    /// Focus on error patterns: search for errors and failures in memories.
    fn reflect_error_patterns(
        &self,
        store: &crate::memory::SqliteMemoryStore,
    ) -> Result<Value, String> {
        let error_memories = store
            .search(None, None, Some("error"), None, 30)
            .map_err(|e| format!("Search error: {e}"))?;

        let fail_memories = store
            .search(None, None, Some("fail"), None, 30)
            .map_err(|e| format!("Search error: {e}"))?;

        let mut all_errors: Vec<&str> = Vec::new();
        for mem in &error_memories {
            all_errors.push(&mem.value);
        }
        for mem in &fail_memories {
            let val = &mem.value;
            if !all_errors.contains(&val.as_str()) {
                all_errors.push(val);
            }
        }

        let mut recommendations: Vec<String> = Vec::new();
        if all_errors.is_empty() {
            recommendations.push("No errors recorded. System appears stable.".to_string());
        } else {
            recommendations.push(format!(
                "Found {} error/failure records. Review and address recurring issues.",
                all_errors.len()
            ));
        }

        Ok(serde_json::json!({
            "status": "reflected",
            "focus": "error_patterns",
            "total_error_records": all_errors.len(),
            "errors": all_errors,
            "recommendations": recommendations,
        }))
    }

    /// Focus on response quality: assess stored knowledge completeness.
    fn reflect_response_quality(
        &self,
        store: &crate::memory::SqliteMemoryStore,
    ) -> Result<Value, String> {
        let stats = store.stats().map_err(|e| format!("Stats error: {e}"))?;

        // Count high-quality (importance >= 7) vs low-quality memories
        let high_quality: u64 = stats
            .by_importance
            .iter()
            .filter(|(imp, _)| *imp >= 7)
            .map(|(_, count)| *count as u64)
            .sum();

        let low_quality: u64 = stats
            .by_importance
            .iter()
            .filter(|(imp, _)| *imp <= 3)
            .map(|(_, count)| *count as u64)
            .sum();

        let mut recommendations: Vec<String> = Vec::new();

        if stats.total == 0 {
            recommendations.push("No memories to evaluate. Start building knowledge.".to_string());
        } else {
            let high_pct = (high_quality as f64 / stats.total as f64) * 100.0;
            if high_pct < 30.0 {
                recommendations.push(format!("Only {high_pct:.0}% of memories are high-importance (>=7). Focus on capturing more critical knowledge."));
            } else {
                recommendations.push(format!(
                    "Good knowledge quality: {high_pct:.0}% of memories are high-importance."
                ));
            }
        }

        Ok(serde_json::json!({
            "status": "reflected",
            "focus": "response_quality",
            "total_memories": stats.total,
            "high_importance_count": high_quality,
            "low_importance_count": low_quality,
            "recommendations": recommendations,
        }))
    }
}

/// Record an evolution event for tracking agent growth over time.
pub struct RecordEvolutionTool {
    memory: Arc<SqliteMemoryStore>,
}

#[async_trait::async_trait]
impl Tool for RecordEvolutionTool {
    fn name(&self) -> &str {
        "record_evolution"
    }

    fn description(&self) -> &str {
        "Record a self-evolution event: tool_created, tool_modified, code_changed, lesson_learned, config_updated, or personality_created. Builds a history of agent growth."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "event_type".to_string(),
                description: "Type of evolution event: tool_created, tool_modified, code_changed, lesson_learned, config_updated, personality_created, error_learned".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Detailed description of what changed and why".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "files".to_string(),
                description: "Comma-separated list of affected files".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "importance".to_string(),
                description: "Importance of this evolution (1-10, default: 5)".to_string(),
                required: false,
                parameter_type: "number".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let event_type = params
            .get("event_type")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: event_type")?;

        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: description")?;

        let files = params
            .get("files")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let importance = params
            .get("importance")
            .and_then(|v| v.as_u64())
            .map(|v| v as u8)
            .unwrap_or(5)
            .clamp(1, 10);

        let key = format!("evolution_{}", event_type);

        let store = &self.memory;

        let tags = vec![event_type.to_string(), "evolution".to_string()];

        let value = if files.is_empty() {
            description.to_string()
        } else {
            format!("{description} [files: {files}]")
        };

        let entry = store
            .store(&key, &value, "evolution", &tags, importance)
            .map_err(|e| format!("Store error: {e}"))?;

        Ok(serde_json::json!({
            "status": "recorded",
            "event_type": event_type,
            "key": entry.key,
            "importance": entry.importance,
        }))
    }
}

/// Register all evolution tools.
/// Manage skills: list, create, update, delete skill definitions.
pub struct ManageSkillsTool;

#[async_trait::async_trait]
impl Tool for ManageSkillsTool {
    fn name(&self) -> &str {
        "manage_skills"
    }

    fn description(&self) -> &str {
        "Manage skill definitions: list all skills, create a new skill, update an existing skill, or delete a skill. Skills are domain-specific knowledge bundles that enhance agent capabilities."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                description: "Action: list, create, update, delete, show".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "name".to_string(),
                description: "Skill name (for create/update/delete/show)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Skill description (for create/update)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "system_instructions".to_string(),
                description: "The skill's system instructions (for create/update)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "keywords".to_string(),
                description: "Comma-separated trigger keywords (for create/update)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "always_active".to_string(),
                description: "Whether the skill is always active (true/false, default: false)"
                    .to_string(),
                required: false,
                parameter_type: "boolean".to_string(),
            },
            ToolParameter {
                name: "reference".to_string(),
                description: "Reference content for the skill (for create/update)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        use crate::skills::{Skill, SkillRegistry};

        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: action")?;

        let skills_dir = crate::constants::skills_dir();

        match action {
            "list" => {
                let registry = SkillRegistry::load_from_dir(&skills_dir)
                    .map_err(|e| format!("Failed to load skills: {e}"))?;
                let skills: Vec<Value> = registry
                    .list()
                    .iter()
                    .map(|(name, desc, active)| {
                        serde_json::json!({
                            "name": name,
                            "description": desc,
                            "always_active": active,
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "status": "ok",
                    "count": skills.len(),
                    "skills": skills,
                }))
            }
            "show" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: name for show action")?;
                let skill_path = skills_dir.join(format!("{name}.yaml"));
                let skill = Skill::from_file(&skill_path)
                    .map_err(|e| format!("Failed to load skill '{name}': {e}"))?;
                Ok(serde_json::json!({
                    "status": "ok",
                    "skill": {
                        "name": skill.name,
                        "description": skill.description,
                        "always_active": skill.always_active,
                        "keywords": skill.keywords,
                        "system_instructions": skill.system_instructions,
                        "reference": skill.reference,
                    },
                }))
            }
            "create" | "update" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: name")?;
                let description = params
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let system_instructions = params
                    .get("system_instructions")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let keywords_str = params
                    .get("keywords")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let keywords: Vec<String> = if keywords_str.is_empty() {
                    vec![]
                } else {
                    keywords_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect()
                };
                let always_active = params
                    .get("always_active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let reference = params
                    .get("reference")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let skill = Skill {
                    name: name.to_string(),
                    description,
                    always_active,
                    keywords,
                    system_instructions,
                    reference,
                    reference_files: vec![],
                };

                let file_path = skill
                    .save_to(&skills_dir)
                    .map_err(|e| format!("Failed to save skill: {e}"))?;

                Ok(serde_json::json!({
                    "status": if action == "create" { "created" } else { "updated" },
                    "name": name,
                    "file": file_path.display().to_string(),
                }))
            }
            "delete" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing required parameter: name for delete action")?;
                let file_path = skills_dir.join(format!("{name}.yaml"));
                if !file_path.exists() {
                    return Ok(serde_json::json!({
                        "status": "not_found",
                        "name": name,
                    }));
                }
                std::fs::remove_file(&file_path)
                    .map_err(|e| format!("Failed to delete skill: {e}"))?;
                Ok(serde_json::json!({
                    "status": "deleted",
                    "name": name,
                }))
            }
            other => Err(format!("Unknown action: {other}")),
        }
    }
}

pub fn register_all(registry: &mut ToolRegistry, memory: Arc<SqliteMemoryStore>) {
    registry.register(Box::new(SelfModifyTool));
    registry.register(Box::new(SelfReflectTool {
        memory: memory.clone(),
    }));
    registry.register(Box::new(RecordEvolutionTool { memory }));
    registry.register(Box::new(ManageSkillsTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_root_finds_cargo_toml() {
        let tool = SelfModifyTool;
        let root = tool.project_root();
        assert!(root.is_ok());
        assert!(root.unwrap().join("Cargo.toml").exists());
    }

    #[test]
    fn test_read_file_finds_source() {
        let tool = SelfModifyTool;
        let result = tool.read_file(&HashMap::from([
            ("action".to_string(), Value::String("read_file".to_string())),
            ("path".to_string(), Value::String("Cargo.toml".to_string())),
        ]));
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["status"], "ok");
        assert!(value["lines"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn test_cargo_check_passes() {
        let tool = SelfModifyTool;
        let result = tool.run_cargo_check();
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["status"], "pass");
    }

    #[test]
    fn test_write_and_delete_file() {
        let tool = SelfModifyTool;

        // Create a temp file
        let result = tool.write_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("write_file".to_string()),
            ),
            (
                "path".to_string(),
                Value::String("src/tools/builtin/test_temp.rs".to_string()),
            ),
            (
                "content".to_string(),
                Value::String("// temp file\n".to_string()),
            ),
            ("verify".to_string(), Value::Bool(false)),
        ]));
        assert!(result.is_ok());

        // Verify it was created
        let read_result = tool.read_file(&HashMap::from([
            ("action".to_string(), Value::String("read_file".to_string())),
            (
                "path".to_string(),
                Value::String("src/tools/builtin/test_temp.rs".to_string()),
            ),
        ]));
        assert!(read_result.is_ok());

        // Delete it
        let delete_result = tool.delete_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("delete_file".to_string()),
            ),
            (
                "path".to_string(),
                Value::String("src/tools/builtin/test_temp.rs".to_string()),
            ),
        ]));
        assert!(delete_result.is_ok());
        assert_eq!(delete_result.unwrap()["status"], "deleted");
    }

    #[test]
    fn test_patch_file_replaces_text() {
        let tool = SelfModifyTool;
        let test_path = "src/tools/builtin/test_patch.rs";

        // Create a file
        tool.write_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("write_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
            (
                "content".to_string(),
                Value::String("fn hello() { println!(\"world\"); }".to_string()),
            ),
            ("verify".to_string(), Value::Bool(false)),
        ]))
        .unwrap();

        // Patch it
        let patch_result = tool.patch_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("patch_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
            ("old_text".to_string(), Value::String("world".to_string())),
            (
                "new_text".to_string(),
                Value::String("universe".to_string()),
            ),
            ("verify".to_string(), Value::Bool(false)),
        ]));
        assert!(patch_result.is_ok());
        assert_eq!(patch_result.unwrap()["status"], "patched");

        // Verify the change
        let read_result = tool
            .read_file(&HashMap::from([
                ("action".to_string(), Value::String("read_file".to_string())),
                ("path".to_string(), Value::String(test_path.to_string())),
            ]))
            .unwrap();
        assert!(read_result["content"]
            .as_str()
            .unwrap()
            .contains("universe"));

        // Cleanup
        tool.delete_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("delete_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
        ]))
        .unwrap();
    }

    #[test]
    fn test_patch_file_missing_old_text() {
        let tool = SelfModifyTool;
        let test_path = "src/tools/builtin/test_patch2.rs";

        tool.write_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("write_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
            (
                "content".to_string(),
                Value::String("fn hello() {}".to_string()),
            ),
            ("verify".to_string(), Value::Bool(false)),
        ]))
        .unwrap();

        let patch_result = tool.patch_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("patch_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
            (
                "old_text".to_string(),
                Value::String("NONEXISTENT".to_string()),
            ),
            (
                "new_text".to_string(),
                Value::String("something".to_string()),
            ),
            ("verify".to_string(), Value::Bool(false)),
        ]));
        assert!(patch_result.is_ok());
        assert_eq!(patch_result.unwrap()["status"], "error");

        // Cleanup
        tool.delete_file(&HashMap::from([
            (
                "action".to_string(),
                Value::String("delete_file".to_string()),
            ),
            ("path".to_string(), Value::String(test_path.to_string())),
        ]))
        .unwrap();
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("WeatherTool"), "weather_tool");
        assert_eq!(to_snake_case("SimpleTool"), "simple_tool");
        assert_eq!(to_snake_case("MyNewTool"), "my_new_tool");
    }
}
