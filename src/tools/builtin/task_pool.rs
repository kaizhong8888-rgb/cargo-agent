use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

pub struct ConcurrentTaskPool;

#[async_trait::async_trait]
impl Tool for ConcurrentTaskPool {
    fn name(&self) -> &str {
        "concurrent_task_pool"
    }

    fn description(&self) -> &str {
        "A concurrent task pool that limits the number of simultaneously running tasks (max 10). \
         Each task is a shell command or a pause with a specified duration. \
         Returns detailed execution results including timing and exit status."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "tasks".to_string(),
                parameter_type: "array".to_string(),
                description: "Array of task objects. Each task must have an 'id' (string) and either \
                              a 'command' (string, shell command) or 'sleep_ms' (number, sleep duration). \
                              Example: [{\"id\":\"task1\",\"command\":\"echo hello\"}, \
                              {\"id\":\"task2\",\"sleep_ms\":500}]".to_string(),
                required: true,
            },
            ToolParameter {
                name: "max_concurrent".to_string(),
                parameter_type: "number".to_string(),
                description: "Maximum number of concurrent tasks (default: 10, max: 10)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "timeout_secs".to_string(),
                parameter_type: "number".to_string(),
                description: "Timeout per task in seconds (default: 30)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let tasks = params
            .get("tasks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing or invalid 'tasks' parameter".to_string())?;

        let max_concurrent = params
            .get("max_concurrent")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .min(10) as usize;

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        if tasks.is_empty() {
            return Err("Tasks array must not be empty".to_string());
        }

        // Create a semaphore to limit concurrency
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let start_time = std::time::Instant::now();

        // Spawn all tasks, each acquiring a permit from the semaphore first
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let id = task
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            let command = task
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let sleep_ms = task.get("sleep_ms").and_then(|v| v.as_u64());

            let sem_clone = Arc::clone(&semaphore);
            let timeout = Duration::from_secs(timeout_secs);

            let handle = tokio::spawn(async move {
                let task_start = std::time::Instant::now();

                // Acquire semaphore permit - this is where concurrency limiting happens
                let _permit = match sem_clone.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        return serde_json::json!({
                            "id": id,
                            "status": "error",
                            "error": "Failed to acquire semaphore permit",
                            "duration_ms": task_start.elapsed().as_millis(),
                        });
                    }
                };

                let result = if let Some(cmd_str) = &command {
                    // Execute shell command with timeout
                    let cmd_result = tokio::time::timeout(timeout, async {
                        let output = tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(cmd_str)
                            .output()
                            .await;

                        match output {
                            Ok(out) => {
                                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                serde_json::json!({
                                    "status": if out.status.success() { "success" } else { "failed" },
                                    "exit_code": out.status.code().unwrap_or(-1),
                                    "stdout": stdout,
                                    "stderr": stderr,
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "status": "error",
                                    "error": format!("Failed to execute command: {}", e),
                                })
                            }
                        }
                    })
                    .await;

                    match cmd_result {
                        Ok(val) => val,
                        Err(_) => {
                            serde_json::json!({
                                "status": "timeout",
                                "error": format!("Task timed out after {}s", timeout.as_secs()),
                            })
                        }
                    }
                } else if let Some(ms) = sleep_ms {
                    // Simulate work by sleeping
                    let sleep_duration = Duration::from_millis(ms);
                    let sleep_result = tokio::time::timeout(timeout, sleep(sleep_duration)).await;

                    match sleep_result {
                        Ok(_) => {
                            serde_json::json!({
                                "status": "success",
                                "slept_ms": ms,
                            })
                        }
                        Err(_) => {
                            serde_json::json!({
                                "status": "timeout",
                                "error": format!("Task timed out after {}s", timeout.as_secs()),
                            })
                        }
                    }
                } else {
                    serde_json::json!({
                        "status": "error",
                        "error": "Task must have either 'command' or 'sleep_ms' field",
                    })
                };

                let duration_ms = task_start.elapsed().as_millis();

                serde_json::json!({
                    "id": id,
                    "duration_ms": duration_ms,
                    "result": result,
                })
            });

            handles.push(handle);
        }

        // Collect all results
        let mut results = Vec::with_capacity(handles.len());
        let mut total_success = 0u64;
        let mut total_failed = 0u64;
        let mut total_timeout = 0u64;

        for handle in handles {
            match handle.await {
                Ok(result) => {
                    let status = result["result"]["status"].as_str().unwrap_or("unknown");
                    match status {
                        "success" => total_success += 1,
                        "failed" => total_failed += 1,
                        "timeout" => total_timeout += 1,
                        _ => {}
                    }
                    results.push(result);
                }
                Err(e) => {
                    total_failed += 1;
                    results.push(serde_json::json!({
                        "id": "unknown",
                        "status": "error",
                        "error": format!("Task panicked: {}", e),
                    }));
                }
            }
        }

        let total_duration_ms = start_time.elapsed().as_millis();

        Ok(serde_json::json!({
            "summary": {
                "total_tasks": tasks.len(),
                "success": total_success,
                "failed": total_failed,
                "timeout": total_timeout,
                "max_concurrent": max_concurrent,
                "total_duration_ms": total_duration_ms,
            },
            "results": results,
        }))
    }
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ConcurrentTaskPool));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_basic_concurrent_execution() {
        let tool = ConcurrentTaskPool;
        let mut params = HashMap::new();

        // Create 20 tasks that sleep for 200ms each.
        // With max_concurrent=10, total time should be ~400ms (2 batches), not ~4000ms.
        let tasks: Vec<Value> = (0..20)
            .map(|i| {
                json!({
                    "id": format!("task_{}", i),
                    "sleep_ms": 200,
                })
            })
            .collect();

        params.insert("tasks".to_string(), json!(tasks));
        params.insert("max_concurrent".to_string(), json!(10));

        let result = tool.execute(&params).await.unwrap();

        let summary = &result["summary"];
        assert_eq!(summary["total_tasks"], 20);
        assert_eq!(summary["success"], 20);
        assert_eq!(summary["failed"], 0);

        // Total time should be significantly less than 20*200ms = 4000ms
        // With 10 concurrent, 2 batches * ~200ms = ~400ms, plus some overhead
        let total_ms = summary["total_duration_ms"].as_u64().unwrap();
        assert!(
            total_ms < 2000,
            "Expected total time < 2000ms for 2 batches of 200ms sleeps, got {}ms",
            total_ms
        );
    }

    #[tokio::test]
    async fn test_semaphore_limits_concurrency() {
        let tool = ConcurrentTaskPool;
        let mut params = HashMap::new();

        let tasks: Vec<Value> = (0..5)
            .map(|i| {
                json!({
                    "id": format!("task_{}", i),
                    "sleep_ms": 100,
                })
            })
            .collect();

        params.insert("tasks".to_string(), json!(tasks));
        params.insert("max_concurrent".to_string(), json!(2));

        let result = tool.execute(&params).await.unwrap();
        let summary = &result["summary"];
        assert_eq!(summary["total_tasks"], 5);
        assert_eq!(summary["success"], 5);
        assert_eq!(summary["max_concurrent"], 2);
    }

    #[tokio::test]
    async fn test_shell_command_execution() {
        let tool = ConcurrentTaskPool;
        let mut params = HashMap::new();

        let tasks: Vec<Value> = vec![
            json!({"id": "echo_test", "command": "echo 'hello world'"}),
            json!({"id": "pwd_test", "command": "pwd"}),
        ];

        params.insert("tasks".to_string(), json!(tasks));
        params.insert("max_concurrent".to_string(), json!(2));

        let result = tool.execute(&params).await.unwrap();
        let summary = &result["summary"];
        assert_eq!(summary["total_tasks"], 2);
        assert_eq!(summary["success"], 2);

        // Verify stdout capture
        let results = result["results"].as_array().unwrap();
        let echo_result = &results[0];
        assert_eq!(
            echo_result["result"]["stdout"].as_str().unwrap().trim(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn test_empty_tasks_returns_error() {
        let tool = ConcurrentTaskPool;
        let mut params = HashMap::new();
        params.insert("tasks".to_string(), json!([]));
        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_failure_propagation() {
        let tool = ConcurrentTaskPool;
        let mut params = HashMap::new();

        let tasks: Vec<Value> = vec![json!({"id": "fail_task", "command": "exit 42"})];

        params.insert("tasks".to_string(), json!(tasks));
        params.insert("max_concurrent".to_string(), json!(1));

        let result = tool.execute(&params).await.unwrap();
        let summary = &result["summary"];
        assert_eq!(summary["total_tasks"], 1);
        assert_eq!(summary["failed"], 1);

        let task_result = &result["results"][0]["result"];
        assert_eq!(task_result["status"], "failed");
        assert_eq!(task_result["exit_code"], 42);
    }
}
