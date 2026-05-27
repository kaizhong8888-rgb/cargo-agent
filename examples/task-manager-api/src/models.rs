use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Task status — modelled as an enum for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Done => "done",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(TaskStatus::Todo),
            "in_progress" => Some(TaskStatus::InProgress),
            "done" => Some(TaskStatus::Done),
            "cancelled" => Some(TaskStatus::Cancelled),
            _ => None,
        }
    }
}

/// The core domain model stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl Task {
    /// Convert the DB row into a richer JSON representation.
    pub fn into_json(self) -> TaskJson {
        let status = TaskStatus::from_str(&self.status).unwrap_or(TaskStatus::Todo);
        let created_at: DateTime<Utc> = self.created_at.parse().unwrap_or_default();
        let updated_at: DateTime<Utc> = self.updated_at.parse().unwrap_or_default();
        TaskJson {
            id: self.id,
            title: self.title,
            description: self.description,
            status,
            priority: self.priority,
            created_at,
            updated_at,
        }
    }
}

/// JSON-friendly representation with parsed types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskJson {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payload for creating a new task.
#[derive(Debug, Deserialize)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
}

/// Payload for partially updating a task.
#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<i32>,
}

impl UpdateTask {
    /// Returns true when every field is `None` – a no-op payload.
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.status.is_none()
            && self.priority.is_none()
    }
}

/// Query parameters for listing / searching tasks.
#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    pub status: Option<TaskStatus>,
    pub priority_min: Option<i32>,
    pub priority_max: Option<i32>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl TaskQuery {
    /// Default page size.
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }

    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn offset(&self) -> u32 {
        (self.page() - 1) * self.per_page()
    }
}

// ---------------------------------------------------------------------------
//  Internal helpers used by the DB layer
// ---------------------------------------------------------------------------

/// Snapshot of a task before an update, used for change logging.
#[allow(dead_code)]
pub struct TaskSnapshot {
    pub old: Task,
}

/// Generate a UUID v4 string.
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

/// Current UTC timestamp as an ISO-8601 string.
pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}
