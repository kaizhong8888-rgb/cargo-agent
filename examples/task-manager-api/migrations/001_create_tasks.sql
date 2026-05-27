-- Task Manager Schema
-- SQLite migration

CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'todo'
                CHECK(status IN ('todo', 'in_progress', 'done', 'cancelled')),
    priority    INTEGER NOT NULL DEFAULT 3
                CHECK(priority BETWEEN 1 AND 5),
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_created_at ON tasks(created_at);
