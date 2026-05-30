//! Process Tool: manage system processes.
//!
//! Actions: list (list running processes), info (get process details),
//! kill (terminate a process), tree (show process tree), search (find processes by name).

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use sysinfo::{Pid, System};

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ProcessTool));
}

struct ProcessTool;

#[async_trait::async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &str {
        "process"
    }

    fn description(&self) -> &str {
        "Manage system processes. Actions: list (list running processes), \
         info (get process details), kill (terminate a process), \
         tree (show process tree), search (find processes by name)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: list, info, kill, tree, search".to_string(),
                required: true,
            },
            ToolParameter {
                name: "pid".to_string(),
                parameter_type: "number".to_string(),
                description: "Process ID (for info/kill)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "name".to_string(),
                parameter_type: "string".to_string(),
                description: "Process name filter (for list/search)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "limit".to_string(),
                parameter_type: "number".to_string(),
                description: "Maximum number of processes to return (default: 50)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "sort_by".to_string(),
                parameter_type: "string".to_string(),
                description: "Sort field: cpu, memory, name, pid (default: cpu)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "signal".to_string(),
                parameter_type: "string".to_string(),
                description: "Signal to send: term (default), kill, int (for kill action)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "list" => list_processes(params),
            "info" => process_info(params),
            "kill" => kill_process(params),
            "tree" => process_tree(params),
            "search" => search_processes(params),
            _ => Err(format!("Unknown action: {action}. Valid: list, info, kill, tree, search")),
        }
    }
}

fn list_processes(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let name_filter = params.get("name").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let sort_by = params.get("sort_by").and_then(|v| v.as_str()).unwrap_or("cpu");

    let mut processes: Vec<Value> = Vec::with_capacity(sys.processes().len().min(limit));

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();

        if let Some(filter) = name_filter {
            if !name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }

        processes.push(serde_json::json!({
            "pid": pid.as_u32(),
            "name": name,
            "cpu_usage": process.cpu_usage(),
            "memory_bytes": process.memory(),
            "memory_display": format_memory(process.memory()),
            "status": format!("{:?}", process.status()),
            "parent_pid": process.parent().map(|p| p.as_u32()),
        }));

        if processes.len() >= limit {
            break;
        }
    }

    processes.sort_by(|a, b| {
        match sort_by {
            "cpu" => b["cpu_usage"].as_f64().unwrap_or(0.0)
                .partial_cmp(&a["cpu_usage"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal),
            "memory" => b["memory_bytes"].as_u64().unwrap_or(0)
                .cmp(&a["memory_bytes"].as_u64().unwrap_or(0)),
            "name" => a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")),
            "pid" => a["pid"].as_u64().unwrap_or(0).cmp(&b["pid"].as_u64().unwrap_or(0)),
            _ => std::cmp::Ordering::Equal,
        }
    });

    Ok(serde_json::json!({
        "total_processes": sys.processes().len(),
        "returned": processes.len(),
        "sort_by": sort_by,
        "processes": processes,
    }))
}

fn process_info(params: &HashMap<String, Value>) -> Result<Value, String> {
    let pid_val = params
        .get("pid")
        .and_then(|v| v.as_u64())
        .ok_or("'pid' is required for info action")?;

    let mut sys = System::new_all();
    sys.refresh_all();

    let pid = Pid::from_u32(pid_val as u32);
    let process = sys
        .process(pid)
        .ok_or_else(|| format!("Process with PID {pid_val} not found"))?;

    let cwd = process.cwd().map(|p| p.to_string_lossy().to_string());
    let root = process.root().map(|p| p.to_string_lossy().to_string());
    let cmd: Vec<String> = process
        .cmd()
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    Ok(serde_json::json!({
        "pid": pid.as_u32(),
        "name": process.name().to_string_lossy().to_string(),
        "cmd": cmd,
        "exe": process.exe().map(|p| p.to_string_lossy().to_string()),
        "cwd": cwd,
        "root": root,
        "cpu_usage": process.cpu_usage(),
        "memory_bytes": process.memory(),
        "memory_display": format_memory(process.memory()),
        "virtual_memory": process.virtual_memory(),
        "status": format!("{:?}", process.status()),
        "parent_pid": process.parent().map(|p| p.as_u32()),
        "start_time": process.start_time(),
        "run_time_seconds": process.run_time(),
        "user_id": process.user_id().map(|u| format!("{u:?}")),
        "group_id": process.group_id().map(|g| format!("{g:?}")),
    }))
}

fn kill_process(params: &HashMap<String, Value>) -> Result<Value, String> {
    let pid_val = params
        .get("pid")
        .and_then(|v| v.as_u64())
        .ok_or("'pid' is required for kill action")?;

    let signal = params.get("signal").and_then(|v| v.as_str()).unwrap_or("term");

    let mut sys = System::new_all();
    sys.refresh_all();

    let pid = Pid::from_u32(pid_val as u32);
    let process = sys
        .process(pid)
        .ok_or_else(|| format!("Process with PID {pid_val} not found"))?;

    let name = process.name().to_string_lossy().to_string();
    let killed = match signal {
        "kill" | "9" => process.kill(),
        "int" | "2" => {
            #[cfg(unix)]
            {
                unsafe { libc::kill(pid_val as i32, libc::SIGINT) == 0 }
            }
            #[cfg(not(unix))]
            {
                process.kill()
            }
        }
        _ => process.kill(),
    };

    Ok(serde_json::json!({
        "success": killed,
        "pid": pid.as_u32(),
        "name": name,
        "signal": signal,
        "message": if killed {
            format!("Process '{name}' (PID: {}) terminated", pid.as_u32())
        } else {
            format!("Failed to terminate process '{name}' (PID: {})", pid.as_u32())
        },
    }))
}

fn process_tree(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, process) in sys.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent.as_u32()).or_default().push(pid.as_u32());
        }
    }

    for child_list in children.values_mut() {
        child_list.sort();
    }

    let mut roots: Vec<u32> = sys
        .processes()
        .keys()
        .map(|p| p.as_u32())
        .filter(|pid| {
            sys.process(Pid::from_u32(*pid)).is_none()
        })
        .collect();
    roots.sort();

    let mut tree = Vec::new();
    let mut count = 0;

    for root_pid in &roots {
        if count >= limit {
            break;
        }
        if let Some(node) = build_tree_node(*root_pid, &sys, &children, &mut count, limit, 0) {
            tree.push(Value::Object(node));
        }
    }

    Ok(serde_json::json!({
        "total_processes": sys.processes().len(),
        "root_count": roots.len(),
        "tree": tree,
        "limit": limit,
    }))
}

fn build_tree_node(
    pid: u32,
    sys: &System,
    children: &HashMap<u32, Vec<u32>>,
    count: &mut usize,
    limit: usize,
    depth: usize,
) -> Option<serde_json::Map<String, Value>> {
    if *count >= limit {
        return None;
    }
    *count += 1;

    let pid_obj = Pid::from_u32(pid);
    let process = sys.process(pid_obj)?;

    let mut node = serde_json::Map::new();
    node.insert("pid".to_string(), Value::Number(pid.into()));
    node.insert("name".to_string(), Value::String(process.name().to_string_lossy().to_string()));
    node.insert("depth".to_string(), Value::Number(depth.into()));
    node.insert("cpu_usage".to_string(), Value::Number(serde_json::Number::from_f64(process.cpu_usage() as f64).unwrap_or(0.into())));
    node.insert("memory_display".to_string(), Value::String(format_memory(process.memory())));

    if let Some(child_pids) = children.get(&pid) {
        let mut child_nodes = Vec::new();
        for child_pid in child_pids {
            if *count >= limit {
                break;
            }
            if let Some(child_node) = build_tree_node(*child_pid, sys, children, count, limit, depth + 1) {
                child_nodes.push(Value::Object(child_node));
            }
        }
        if !child_nodes.is_empty() {
            node.insert("children".to_string(), Value::Array(child_nodes));
        }
    }

    Some(node)
}

fn search_processes(params: &HashMap<String, Value>) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("'name' is required for search action")?;

    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let mut sys = System::new_all();
    sys.refresh_all();

    let mut results = Vec::with_capacity(limit);

    for (pid, process) in sys.processes() {
        let proc_name = process.name().to_string_lossy().to_string();
        if proc_name.to_lowercase().contains(&name.to_lowercase()) {
            results.push(serde_json::json!({
                "pid": pid.as_u32(),
                "name": proc_name,
                "cpu_usage": process.cpu_usage(),
                "memory_bytes": process.memory(),
                "memory_display": format_memory(process.memory()),
                "status": format!("{:?}", process.status()),
                "parent_pid": process.parent().map(|p| p.as_u32()),
            }));

            if results.len() >= limit {
                break;
            }
        }
    }

    Ok(serde_json::json!({
        "query": name,
        "found": results.len(),
        "processes": results,
    }))
}

fn format_memory(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}
