//! SQLite-backed persistent memory store.
//!
//! Provides CRUD operations for structured memory entries with
//! namespace, tags, importance, and full-text search support.

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Result as SqliteResult};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, serde::Serialize)]
pub struct MemoryEntry {
    pub id: i64,
    pub key: String,
    pub value: String,
    pub namespace: String,
    pub tags: String, // comma-separated
    pub created_at: String,
    pub updated_at: String,
    pub importance: u8,
}

/// SQLite-backed memory store - persists across sessions.
pub struct SqliteMemoryStore {
    conn: Mutex<Connection>,
}

impl SqliteMemoryStore {
    /// Open or create a memory store at the given SQLite path.
    pub fn open(db_path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Create table
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                value TEXT NOT NULL,
                namespace TEXT NOT NULL DEFAULT 'default',
                tags TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                importance INTEGER NOT NULL DEFAULT 5 CHECK(importance BETWEEN 1 AND 10)
            );
            CREATE INDEX IF NOT EXISTS idx_memories_namespace ON memories(namespace);
            CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance);
            CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Store or update a memory entry.
    pub fn store(
        &self,
        key: &str,
        value: &str,
        namespace: &str,
        tags: &[String],
        importance: u8,
    ) -> SqliteResult<MemoryEntry> {
        let now = Utc::now().to_rfc3339();
        let tags_str = tags.join(",");
        let importance = importance.clamp(1, 10);

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Check if key exists to preserve created_at
        let existing_created_at: Option<String> = tx
            .query_row(
                "SELECT created_at FROM memories WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;

        let created_at = existing_created_at.unwrap_or(now.clone());

        tx.execute(
            "INSERT INTO memories (key, value, namespace, tags, created_at, updated_at, importance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(key) DO UPDATE SET
                 value = ?2,
                 namespace = ?3,
                 tags = ?4,
                 updated_at = ?6,
                 importance = ?7",
            (key, value, namespace, tags_str, created_at, now, importance),
        )?;

        let entry = tx.query_row(
            "SELECT id, key, value, namespace, tags, created_at, updated_at, importance
             FROM memories WHERE key = ?1",
            [key],
            |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    namespace: row.get(3)?,
                    tags: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    importance: row.get(7)?,
                })
            },
        )?;

        tx.commit()?;
        Ok(entry)
    }

    /// Recall a memory by key.
    pub fn recall(&self, key: &str) -> SqliteResult<Option<MemoryEntry>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, key, value, namespace, tags, created_at, updated_at, importance
             FROM memories WHERE key = ?1",
            [key],
            |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    namespace: row.get(3)?,
                    tags: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    importance: row.get(7)?,
                })
            },
        )
        .optional()
    }

    /// Search memories with filters.
    pub fn search(
        &self,
        namespace: Option<&str>,
        tag: Option<&str>,
        query: Option<&str>,
        min_importance: Option<u8>,
        limit: usize,
    ) -> SqliteResult<Vec<MemoryEntry>> {
        let mut sql = String::from(
            "SELECT id, key, value, namespace, tags, created_at, updated_at, importance
             FROM memories WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ns) = namespace {
            sql.push_str(" AND namespace = ?");
            params.push(Box::new(ns.to_string()));
        }

        if let Some(t) = tag {
            sql.push_str(" AND (tags LIKE '%' || ? || '%')");
            params.push(Box::new(t.to_string()));
        }

        if let Some(q) = query {
            sql.push_str(" AND (LOWER(key) LIKE '%' || LOWER(?) || '%' OR LOWER(value) LIKE '%' || LOWER(?) || '%')");
            params.push(Box::new(q.to_string()));
            params.push(Box::new(q.to_string()));
        }

        if let Some(min_imp) = min_importance {
            sql.push_str(" AND importance >= ?");
            params.push(Box::new(min_imp as i64));
        }

        sql.push_str(" ORDER BY importance DESC, updated_at DESC LIMIT ?");
        params.push(Box::new(limit as i64));

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(rusqlite::params_from_iter(refs.iter().copied()), |row| {
            Ok(MemoryEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                namespace: row.get(3)?,
                tags: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                importance: row.get(7)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// List all namespaces with memory counts.
    pub fn list_namespaces(&self) -> SqliteResult<Vec<(String, usize)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT namespace, COUNT(*) as count FROM memories GROUP BY namespace ORDER BY count DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Delete a memory by key.
    pub fn delete(&self, key: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM memories WHERE key = ?1", [key])?;
        Ok(rows > 0)
    }

    /// Get memory statistics.
    pub fn stats(&self) -> SqliteResult<MemoryStats> {
        let conn = self.conn.lock().unwrap();

        let total: usize = conn.query_row(
            "SELECT COUNT(*) FROM memories",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT namespace, COUNT(*) as count FROM memories GROUP BY namespace ORDER BY count DESC",
        )?;
        let ns_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        })?;
        let mut by_namespace: Vec<(String, usize)> = Vec::new();
        for row in ns_rows {
            by_namespace.push(row?);
        }

        let mut imp_stmt = conn.prepare(
            "SELECT importance, COUNT(*) as count FROM memories GROUP BY importance ORDER BY importance DESC",
        )?;
        let imp_rows = imp_stmt.query_map([], |row| {
            Ok((row.get::<_, u8>(0)?, row.get::<_, usize>(1)?))
        })?;
        let mut by_importance: Vec<(u8, usize)> = Vec::new();
        for row in imp_rows {
            by_importance.push(row?);
        }

        Ok(MemoryStats {
            total,
            by_namespace,
            by_importance,
        })
    }
}

/// Statistics about the memory system.
pub struct MemoryStats {
    pub total: usize,
    pub by_namespace: Vec<(String, usize)>,
    pub by_importance: Vec<(u8, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_store() -> SqliteMemoryStore {
        let path = env::temp_dir().join(format!("test_mem_{}.db", uuid::Uuid::new_v4()));
        SqliteMemoryStore::open(path).unwrap()
    }

    fn tags(vals: &[&str]) -> Vec<String> {
        vals.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_store_and_recall() {
        let store = temp_store();
        let entry = store
            .store("greeting", "Hello, world!", "default", &tags(&["test"]), 5)
            .unwrap();
        assert_eq!(entry.key, "greeting");
        assert_eq!(entry.value, "Hello, world!");

        let recalled = store.recall("greeting").unwrap().unwrap();
        assert_eq!(recalled.value, "Hello, world!");
    }

    #[test]
    fn test_update_preserves_created_at() {
        let store = temp_store();
        let first = store
            .store("note", "v1", "default", &[], 3)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.store("note", "v2", "default", &[], 7).unwrap();

        let updated = store.recall("note").unwrap().unwrap();
        assert_eq!(updated.value, "v2");
        assert_eq!(updated.importance, 7);
        assert_eq!(updated.created_at, first.created_at);
    }

    #[test]
    fn test_search_by_namespace() {
        let store = temp_store();
        store.store("a", "val_a", "ns1", &tags(&["tag1"]), 5).unwrap();
        store.store("b", "val_b", "ns2", &tags(&["tag1"]), 8).unwrap();
        store.store("c", "val_c", "ns1", &tags(&["tag2"]), 3).unwrap();

        let results = store.search(Some("ns1"), None, None, None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_tag() {
        let store = temp_store();
        store.store("a", "val_a", "ns1", &tags(&["important", "work"]), 9).unwrap();
        store.store("b", "val_b", "ns1", &tags(&["casual"]), 3).unwrap();

        let results = store.search(None, Some("important"), None, None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "a");
    }

    #[test]
    fn test_search_by_query() {
        let store = temp_store();
        store.store("user_name", "Alice", "users", &[], 5).unwrap();
        store.store("user_email", "alice@example.com", "users", &[], 5).unwrap();

        let results = store.search(None, None, Some("alice"), None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_min_importance() {
        let store = temp_store();
        store.store("low", "val", "ns", &[], 2).unwrap();
        store.store("med", "val", "ns", &[], 5).unwrap();
        store.store("high", "val", "ns", &[], 9).unwrap();

        let results = store.search(None, None, None, Some(5), 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_list_namespaces() {
        let store = temp_store();
        store.store("a", "v", "ns1", &[], 5).unwrap();
        store.store("b", "v", "ns1", &[], 5).unwrap();
        store.store("c", "v", "ns2", &[], 5).unwrap();

        let ns = store.list_namespaces().unwrap();
        assert_eq!(ns.len(), 2);
        assert_eq!(ns[0].0, "ns1");
        assert_eq!(ns[0].1, 2);
    }

    #[test]
    fn test_delete() {
        let store = temp_store();
        store.store("to_delete", "val", "ns", &[], 5).unwrap();
        assert!(store.delete("to_delete").unwrap());
        assert!(store.recall("to_delete").unwrap().is_none());
        assert!(!store.delete("nonexistent").unwrap());
    }

    #[test]
    fn test_stats() {
        let store = temp_store();
        store.store("a", "v", "ns1", &[], 3).unwrap();
        store.store("b", "v", "ns1", &[], 7).unwrap();
        store.store("c", "v", "ns2", &[], 7).unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.by_namespace.len(), 2);
    }
}
