//! Loop Manager: manages recurring background tasks (loops).
//!
//! A loop is a recurring task that executes a command/prompt at a fixed interval.
//! Loops run in the background via tokio tasks and can be listed, stopped, or paused.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Unique ID counter for loops.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A single recurring loop task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopTask {
    /// Unique numeric ID.
    pub id: u64,
    /// Human-readable description.
    pub description: String,
    /// The prompt/command to execute each iteration.
    pub command: String,
    /// Interval in seconds between executions.
    pub interval_secs: u64,
    /// Whether the loop is currently running.
    pub enabled: bool,
    /// Number of times this loop has executed.
    pub run_count: u64,
    /// Timestamp of the last run (ISO 8601).
    pub last_run: Option<String>,
}

impl LoopTask {
    fn new(description: String, command: String, interval_secs: u64) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            description,
            command,
            interval_secs,
            enabled: true,
            run_count: 0,
            last_run: None,
        }
    }
}

/// Manages all active loops.
#[derive(Default)]
pub struct LoopManager {
    loops: Arc<RwLock<HashMap<u64, LoopTask>>>,
}

impl LoopManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new loop and start it.
    pub async fn start(
        &self,
        description: String,
        command: String,
        interval_secs: u64,
        handler: impl Fn(String) + Send + Sync + 'static,
    ) -> u64 {
        let task = LoopTask::new(description, command, interval_secs);
        let id = task.id;
        let loops = self.loops.clone();

        // Store the task
        {
            let mut map = loops.write().await;
            map.insert(id, task.clone());
        }

        // Spawn the background loop
        let loops_clone = loops.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            // Skip the first immediate tick
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                interval.tick().await;

                // Check if still enabled
                {
                    let map = loops_clone.read().await;
                    if let Some(t) = map.get(&id) {
                        if !t.enabled {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // Execute the command
                let cmd = {
                    let mut map = loops_clone.write().await;
                    if let Some(t) = map.get_mut(&id) {
                        t.run_count += 1;
                        t.last_run = Some(chrono::Utc::now().to_rfc3339());
                        t.command.clone()
                    } else {
                        break;
                    }
                };

                handler(cmd);
            }

            // Clean up when loop ends
            let mut map = loops_clone.write().await;
            map.remove(&id);
        });

        id
    }

    /// List all active loops.
    pub async fn list(&self) -> Vec<LoopTask> {
        let map = self.loops.read().await;
        let mut loops: Vec<LoopTask> = map.values().cloned().collect();
        loops.sort_by_key(|l| l.id);
        loops
    }

    /// Stop a loop by ID.
    pub async fn stop(&self, id: u64) -> Result<(), String> {
        let mut map = self.loops.write().await;
        if let Some(task) = map.get_mut(&id) {
            task.enabled = false;
            Ok(())
        } else {
            Err(format!("Loop {id} not found"))
        }
    }

    /// Stop all active loops.
    pub async fn stop_all(&self) -> usize {
        let mut map = self.loops.write().await;
        let count = map.len();
        for task in map.values_mut() {
            task.enabled = false;
        }
        map.clear();
        count
    }

    /// Get the count of active loops.
    pub async fn count(&self) -> usize {
        self.loops.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_create_and_list_loop() {
        let manager = LoopManager::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let id = manager
            .start(
                "Test loop".to_string(),
                "echo hello".to_string(),
                3600, // 1 hour, won't trigger during test
                move |_| {
                    counter_clone.fetch_add(1, Ordering::Relaxed);
                },
            )
            .await;

        assert!(id > 0);
        assert_eq!(manager.count().await, 1);

        let loops = manager.list().await;
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].id, id);
        assert_eq!(loops[0].command, "echo hello");
        assert!(loops[0].enabled);
    }

    #[tokio::test]
    async fn test_stop_loop() {
        let manager = LoopManager::new();
        let id = manager
            .start(
                "Test loop".to_string(),
                "echo hello".to_string(),
                3600,
                move |_| {},
            )
            .await;

        assert_eq!(manager.count().await, 1);

        manager.stop(id).await.unwrap();

        // Give the background task time to detect it's disabled and clean up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // After stop(), the loop should be removed from the map
        assert_eq!(manager.count().await, 0);

        // Stopping again should fail
        assert!(manager.stop(id).await.is_err());
    }

    #[tokio::test]
    async fn test_stop_all_loops() {
        let manager = LoopManager::new();

        manager
            .start("Loop 1".to_string(), "cmd1".to_string(), 3600, |_| {})
            .await;
        manager
            .start("Loop 2".to_string(), "cmd2".to_string(), 3600, |_| {})
            .await;
        manager
            .start("Loop 3".to_string(), "cmd3".to_string(), 3600, |_| {})
            .await;

        assert_eq!(manager.count().await, 3);

        let stopped = manager.stop_all().await;
        assert_eq!(stopped, 3);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_loop_execution_count() {
        let manager = LoopManager::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // Use 1 second interval to test execution
        let id = manager
            .start("Fast loop".to_string(), "tick".to_string(), 1, move |_| {
                counter_clone.fetch_add(1, Ordering::Relaxed);
            })
            .await;

        // Wait for 2 executions
        tokio::time::sleep(Duration::from_millis(2500)).await;

        let loops = manager.list().await;
        let loop_task = loops.iter().find(|l| l.id == id).unwrap();

        // Should have run at least once
        assert!(loop_task.run_count >= 1);
        assert!(counter.load(Ordering::Relaxed) >= 1);

        // Stop the loop
        manager.stop(id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Count should not increase after stopping
        let final_count = counter.load(Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(counter.load(Ordering::Relaxed), final_count);
    }
}
