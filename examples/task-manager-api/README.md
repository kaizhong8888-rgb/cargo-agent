# Task Manager API

A production-style REST API built with **Axum**, **SQLite** (via sqlx), and modern Rust patterns.

## Features

- ✅ Full CRUD for tasks (`POST`/`GET`/`PATCH`/`DELETE`)
- ✅ Filtering by status, priority range, and text search
- ✅ Paginated list endpoint
- ✅ Sort by any column (ascending/descending)
- ✅ Structured JSON error responses (`thiserror` + `IntoResponse`)
- ✅ Request logging middleware (method, path, status, duration)
- ✅ CORS enabled (permissive, suitable for local dev)
- ✅ Health-check endpoint (`/health`)
- ✅ In-memory SQLite for integration tests
- ✅ Compile-time checked queries via `sqlx::query`

## Quick Start

```bash
# Clone the repo and change to this directory
cd examples/task-manager-api

# Run (creates task_manager.db in the current directory)
DATABASE_URL="sqlite:task_manager.db?mode=rwc" cargo run

# Server listens on http://0.0.0.0:3000
```

## API Endpoints

### `GET /health`
Liveness probe.

```json
{ "status": "ok", "version": "0.1.0" }
```

### `POST /api/tasks`
Create a new task.

```json
{
  "title": "Buy groceries",
  "description": "Milk, eggs, bread",
  "priority": 2
}
```

### `GET /api/tasks`
List tasks. Supports query parameters:

| Param        | Type   | Description                          |
|-------------|--------|--------------------------------------|
| `status`     | string | `todo`, `in_progress`, `done`, `cancelled` |
| `priority_min` | int | Minimum priority (1-5)              |
| `priority_max` | int | Maximum priority (1-5)              |
| `search`     | string | Full-text search on title & description |
| `sort_by`    | string | `created_at` (default), `updated_at`, `priority`, `status` |
| `sort_order` | string | `desc` (default) or `asc`           |
| `page`       | int    | Page number (default: 1)            |
| `per_page`   | int    | Items per page (default: 20, max: 100) |

### `GET /api/tasks/:id`
Get a single task by ID.

### `PATCH /api/tasks/:id`
Partially update a task.

```json
{
  "status": "in_progress",
  "priority": 5
}
```

### `DELETE /api/tasks/:id`
Delete a task. Returns `204 No Content`.

## Error Responses

All errors return a consistent JSON body:

```json
{
  "error": {
    "code": "not_found",
    "message": "Task abc-123 not found"
  }
}
```

Error codes: `not_found`, `bad_request`, `conflict`, `unprocessable`, `internal_error`.

## Testing

```bash
# Run all tests (uses in-memory SQLite)
cargo test

# Run with logging
RUST_LOG=task_manager_api=debug cargo test

# Run a specific test
cargo test test_create_and_get_task
```

## Environment Variables

| Variable       | Default                     | Description              |
|---------------|-----------------------------|--------------------------|
| `DATABASE_URL` | `sqlite:task_manager.db?mode=rwc` | SQLite database path |
| `BIND_ADDR`    | `0.0.0.0:3000`              | Server listen address    |
| `RUST_LOG`     | `task_manager_api=info,tower_http=info` | Logging filter |

## Project Structure

```
src/
├── main.rs       # Entry point, env vars, tokio runtime
├── lib.rs        # Router setup, server startup
├── models.rs     # Task, NewTask, UpdateTask, TaskQuery
├── handlers.rs   # Route handler functions
├── db.rs         # Database CRUD operations
├── errors.rs     # AppError type → HTTP response mapping
└── middleware.rs # Request logging middleware
migrations/
└── 001_create_tasks.sql
tests/
└── integration_tests.rs
```

## Key Dependencies

- **axum** – Web framework (async, ergonomic, tower-based)
- **sqlx** – Async SQL toolkit with compile-time checked queries
- **thiserror** – Derive macro for `std::error::Error`
- **tower-http** – CORS and tracing middleware
- **tracing** – Structured observability
- **uuid** – V4 UUIDs for task IDs
- **chrono** – UTC timestamps
