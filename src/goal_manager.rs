//! Goal Manager: tracks the current session goal.
//!
//! A goal is a single-line description of what the user wants to achieve.
//! It persists across the session and can be set, cleared, or marked as done.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// The current goal state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Goal {
    /// The goal description.
    pub description: String,
    /// Whether the goal has been completed.
    pub completed: bool,
    /// Timestamp when the goal was set (ISO 8601).
    pub created_at: String,
    /// Timestamp when the goal was completed (ISO 8601), if applicable.
    pub completed_at: Option<String>,
}

impl Goal {
    fn new(description: String) -> Self {
        Self {
            description,
            completed: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        }
    }

    fn mark_done(&mut self) {
        self.completed = true;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}

/// Manages the current session goal.
#[derive(Default)]
pub struct GoalManager {
    goal: Arc<RwLock<Option<Goal>>>,
}

impl GoalManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a new goal, replacing any existing one.
    pub async fn set(&self, description: String) {
        let mut guard = self.goal.write().await;
        *guard = Some(Goal::new(description));
    }

    /// Get the current goal, if any.
    pub async fn get(&self) -> Option<Goal> {
        self.goal.read().await.clone()
    }

    /// Clear the current goal.
    pub async fn clear(&self) -> bool {
        let mut guard = self.goal.write().await;
        guard.take().is_some()
    }

    /// Mark the current goal as completed.
    pub async fn mark_done(&self) -> Result<(), String> {
        let mut guard = self.goal.write().await;
        if let Some(goal) = guard.as_mut() {
            if goal.completed {
                return Err("Goal is already completed".into());
            }
            goal.mark_done();
            Ok(())
        } else {
            Err("No active goal".into())
        }
    }

    /// Check if there's an active (incomplete) goal.
    pub async fn has_active_goal(&self) -> bool {
        self.goal
            .read()
            .await
            .as_ref()
            .map(|g| !g.completed)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_goal() {
        let manager = GoalManager::new();
        assert!(!manager.has_active_goal().await);

        manager.set("Build a feature".to_string()).await;
        assert!(manager.has_active_goal().await);

        let goal = manager.get().await.unwrap();
        assert_eq!(goal.description, "Build a feature");
        assert!(!goal.completed);
        assert!(goal.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_clear_goal() {
        let manager = GoalManager::new();

        // Clear when no goal exists
        assert!(!manager.clear().await);

        // Set and clear
        manager.set("Test goal".to_string()).await;
        assert!(manager.has_active_goal().await);

        assert!(manager.clear().await);
        assert!(!manager.has_active_goal().await);
        assert!(manager.get().await.is_none());

        // Clear again should return false
        assert!(!manager.clear().await);
    }

    #[tokio::test]
    async fn test_mark_goal_done() {
        let manager = GoalManager::new();

        // Mark done when no goal exists
        assert!(manager.mark_done().await.is_err());

        // Set and mark done
        manager.set("Test goal".to_string()).await;
        assert!(manager.mark_done().await.is_ok());

        let goal = manager.get().await.unwrap();
        assert!(goal.completed);
        assert!(goal.completed_at.is_some());

        // Try to mark again - should fail
        assert!(manager.mark_done().await.is_err());
    }

    #[tokio::test]
    async fn test_set_replaces_existing_goal() {
        let manager = GoalManager::new();

        manager.set("First goal".to_string()).await;
        manager.set("Second goal".to_string()).await;

        let goal = manager.get().await.unwrap();
        assert_eq!(goal.description, "Second goal");
        assert!(!goal.completed);
    }
}
