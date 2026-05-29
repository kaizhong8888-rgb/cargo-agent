use crate::tools::ToolParameter;
use async_trait::async_trait;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct DatabaseTool;

#[async_trait]
impl crate::tools::Tool for DatabaseTool {
    fn name(&self) -> &str {
        "database"
    }

    fn description(&self) -> &str {
        "Full SQLite database tool: parameterized queries, schema management (CREATE/ALTER/DROP + introspection with indexes/FKs/triggers), versioned migrations, CSV import/export, backup, batch execution"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "Operation: query, execute, execute_batch, list_tables, describe_table, create_table, alter_table, drop_table, import_csv, export_csv, backup, run_sql_file, migrate, migrate_list, migrate_create".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "database_path".into(),
                description: "Path to the SQLite database file".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "sql".into(),
                description: "SQL SELECT query with ? placeholders (for query)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "sql_statement".into(),
                description: "SQL statement (for execute, e.g. INSERT, UPDATE, DELETE, CREATE)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "params".into(),
                description: "JSON array of values to bind to ? placeholders (for query/execute)".into(),
                required: false,
                parameter_type: "array".into(),
            },
            ToolParameter {
                name: "statements".into(),
                description: "Array of SQL statements to run in a transaction (for execute_batch)".into(),
                required: false,
                parameter_type: "array".into(),
            },
            ToolParameter {
                name: "table_name".into(),
                description: "Table name for table operations".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "columns".into(),
                description: "JSON array of column definitions: [{\"name\":\"col\",\"type\":\"TEXT\",\"nullable\":false,\"pk\":false,\"default\":null}] (for create_table)".into(),
                required: false,
                parameter_type: "array".into(),
            },
            ToolParameter {
                name: "alter_action".into(),
                description: "Alter action: add_column, drop_column, rename_column, rename_table (for alter_table)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "column_name".into(),
                description: "Column name for alter_table actions".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "column_type".into(),
                description: "Column type for add_column (e.g. TEXT, INTEGER)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "new_name".into(),
                description: "New name for rename_column/rename_table".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "csv_path".into(),
                description: "Path to CSV file (for import_csv / export_csv)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "has_header".into(),
                description: "Whether CSV has a header row (default: true, for import_csv)".into(),
                required: false,
                parameter_type: "boolean".into(),
            },
            ToolParameter {
                name: "backup_path".into(),
                description: "Path for the backup file (for backup)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "file_path".into(),
                description: "Path to a .sql file to execute (for run_sql_file)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "migrations_dir".into(),
                description: "Directory containing migration SQL files (for migrate/migrate_list)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "description".into(),
                description: "Description for the new migration (for migrate_create)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "new_table_name".into(),
                description: "New table name for rename (for alter_table rename_table)".into(),
                required: false,
                parameter_type: "string".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: action".to_string())?;

        let database_path = params
            .get("database_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: database_path".to_string())?;

        match action {
            "query" => execute_query(database_path, params).await,
            "execute" => execute_statement(database_path, params).await,
            "execute_batch" => execute_batch(database_path, params).await,
            "list_tables" => list_tables(database_path).await,
            "describe_table" => describe_table(database_path, params).await,
            "create_table" => create_table(database_path, params).await,
            "alter_table" => alter_table(database_path, params).await,
            "drop_table" => drop_table(database_path, params).await,
            "import_csv" => import_csv(database_path, params).await,
            "export_csv" => export_csv(database_path, params).await,
            "backup" => backup_database(database_path, params).await,
            "run_sql_file" => run_sql_file(database_path, params).await,
            "migrate" => run_migrations(database_path, params).await,
            "migrate_list" => list_migrations(database_path, params).await,
            "migrate_create" => create_migration(params).await,
            _ => Err(format!(
                "Unknown action: {}. Supported: query, execute, execute_batch, list_tables, \
                 describe_table, create_table, alter_table, drop_table, import_csv, export_csv, \
                 backup, run_sql_file, migrate, migrate_list, migrate_create",
                action
            )),
        }
    }
}

// ============================================================================
// Helper: Open connection
// ============================================================================

fn open_conn(db_path: &str) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|e| format!("Failed to open database '{}': {}", db_path, e))
}

// ============================================================================
// Helper: SQLite value → JSON
// ============================================================================

fn sqlite_value_to_json(val: &rusqlite::types::Value) -> Value {
    match val {
        rusqlite::types::Value::Null => Value::Null,
        rusqlite::types::Value::Integer(i) => json!(i),
        rusqlite::types::Value::Real(f) => json!(f),
        rusqlite::types::Value::Text(t) => json!(t),
        rusqlite::types::Value::Blob(b) => json!(format!("<blob {} bytes>", b.len())),
    }
}

// ============================================================================
// Helper: Quote an identifier safely
// ============================================================================

fn quote_id(id: &str) -> String {
    format!("\"{}\"", id.replace('\"', "\"\""))
}

// ============================================================================
// Helper: Execute a PRAGMA and collect JSON rows
// ============================================================================

fn pragma_to_json(conn: &Connection, sql: &str) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare PRAGMA: {}", e))?;

    let col_names: Vec<String> = (0..stmt.column_count())
        .map(|i| {
            stmt.column_name(i)
                .map(|n| n.to_string())
                .unwrap_or(format!("col_{}", i))
        })
        .collect();

    let col_count = col_names.len();
    let names_clone = col_names.clone();

    let rows_iter = stmt
        .query_map([], move |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in names_clone.iter().enumerate().take(col_count) {
                let val: rusqlite::types::Value = row.get_unwrap(i);
                obj.insert(name.clone(), sqlite_value_to_json(&val));
            }
            Ok(Value::Object(obj))
        })
        .map_err(|e| format!("PRAGMA query failed: {}", e))?;

    let mut results = Vec::new();
    for row in rows_iter {
        let val = row.map_err(|e| format!("Row error: {}", e))?;
        results.push(val);
    }
    Ok(results)
}

// ============================================================================
// 1. QUERY — Parameterized SELECT
// ============================================================================

async fn execute_query(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let sql = params
        .get("sql")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: sql".to_string())?;

    let query_params: Vec<Value> = params
        .get("params")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let conn = open_conn(db_path)?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare SQL: {}", e))?;

    // Convert JSON params to rusqlite params
    // Collect column info before query_map (borrow issue)
    let col_names: Vec<String> = (0..stmt.column_count())
        .map(|i| {
            stmt.column_name(i)
                .map(|n| n.to_string())
                .unwrap_or(format!("col_{}", i))
        })
        .collect();

    let names_for_closure = col_names.clone();
    let c_count = col_names.len();

    // Build rusqlite-compatible params and pass them to query_map
    let rusqlite_params: Vec<Box<dyn rusqlite::types::ToSql>> = query_params
        .iter()
        .map(|v| json_to_rusqlite_value(v))
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        rusqlite_params.iter().map(|p| p.as_ref()).collect();

    let rows_iter = stmt
        .query_map(param_refs.as_slice(), move |row| {
            let mut obj = serde_json::Map::new();
            for (i, name) in names_for_closure.iter().enumerate().take(c_count) {
                let val: rusqlite::types::Value = row.get_unwrap(i);
                obj.insert(name.clone(), sqlite_value_to_json(&val));
            }
            Ok(Value::Object(obj))
        })
        .map_err(|e| format!("Query failed: {}", e))?;

    let mut results = Vec::new();
    for row in rows_iter {
        let val = row.map_err(|e| format!("Row error: {}", e))?;
        results.push(val);
    }

    Ok(json!({
        "columns": col_names,
        "rows": results,
        "row_count": results.len(),
        "database": db_path,
        "sql": sql,
    }))
}

fn json_to_rusqlite_value(val: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match val {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Bool(b) => Box::new(if *b { 1i32 } else { 0i32 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(0i32)
            }
        }
        Value::String(s) => Box::new(s.clone()),
        Value::Array(_) | Value::Object(_) => {
            Box::new(serde_json::to_string(val).unwrap_or_default())
        }
    }
}

// ============================================================================
// 2. EXECUTE — Single SQL statement with optional params
// ============================================================================

async fn execute_statement(
    db_path: &str,
    params: &HashMap<String, Value>,
) -> Result<Value, String> {
    let sql = params
        .get("sql_statement")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: sql_statement".to_string())?;

    let exec_params: Vec<Value> = params
        .get("params")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let conn = open_conn(db_path)?;

    if !exec_params.is_empty() {
        let rusqlite_params: Vec<Box<dyn rusqlite::types::ToSql>> = exec_params
            .iter()
            .map(|v| json_to_rusqlite_value(v))
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            rusqlite_params.iter().map(|p| p.as_ref()).collect();
        let affected = conn
            .execute(sql, param_refs.as_slice())
            .map_err(|e| format!("Execute failed: {}", e))?;
        Ok(json!({
            "affected_rows": affected,
            "statement": sql,
            "database": db_path,
        }))
    } else {
        let affected = conn
            .execute(sql, [])
            .map_err(|e| format!("Execute failed: {}", e))?;
        Ok(json!({
            "affected_rows": affected,
            "statement": sql,
            "database": db_path,
        }))
    }
}

// ============================================================================
// 3. EXECUTE BATCH — Multiple statements in a transaction
// ============================================================================

async fn execute_batch(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let statements = params
        .get("statements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing required parameter: statements (JSON array)".to_string())?;

    if statements.is_empty() {
        return Err("statements array is empty".to_string());
    }

    let sql_texts: Vec<&str> = statements.iter().filter_map(|v| v.as_str()).collect();

    if sql_texts.len() != statements.len() {
        return Err("All elements in statements must be strings".to_string());
    }

    let conn = open_conn(db_path)?;

    conn.execute_batch("BEGIN TRANSACTION")
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut results: Vec<Value> = Vec::with_capacity(sql_texts.len());
    let mut total_affected: i64 = 0;

    for (i, sql) in sql_texts.iter().enumerate() {
        match conn.execute(sql, []) {
            Ok(affected) => {
                total_affected += affected as i64;
                results.push(json!({
                    "index": i,
                    "sql": sql,
                    "affected_rows": affected,
                    "status": "ok",
                }));
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(format!(
                    "Batch execution failed at statement {}: {}. All changes rolled back. Error: {}",
                    i, sql, e
                ));
            }
        }
    }

    conn.execute_batch("COMMIT")
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(json!({
        "action": "execute_batch",
        "statement_count": sql_texts.len(),
        "total_affected_rows": total_affected,
        "results": results,
        "database": db_path,
    }))
}

// ============================================================================
// 4. LIST TABLES
// ============================================================================

async fn list_tables(db_path: &str) -> Result<Value, String> {
    let conn = open_conn(db_path)?;

    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' \
             AND name NOT LIKE '_schema_migrations%' ORDER BY name",
        )
        .map_err(|e| format!("Failed to prepare: {}", e))?;

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Query failed: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut table_details: Vec<Value> = Vec::with_capacity(tables.len());
    for table in &tables {
        let count_sql = format!("SELECT COUNT(*) FROM {}", quote_id(table));
        let row_count: i64 = conn
            .query_row(&count_sql, [], |row| row.get(0))
            .unwrap_or(0);

        let ddl: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .ok();

        table_details.push(json!({
            "name": table,
            "row_count": row_count,
            "ddl": ddl,
        }));
    }

    Ok(json!({
        "tables": table_details,
        "table_count": tables.len(),
        "database": db_path,
    }))
}

// ============================================================================
// 5. DESCRIBE TABLE — Full introspection
// ============================================================================

async fn describe_table(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let conn = open_conn(db_path)?;

    // Check table exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !exists {
        return Err(format!(
            "Table '{}' does not exist in database '{}'",
            table_name, db_path
        ));
    }

    // Columns
    let col_sql = format!("PRAGMA table_info({})", quote_id(table_name));
    let columns = pragma_to_json(&conn, &col_sql)?;

    // Indexes
    let idx_list_sql = format!("PRAGMA index_list({})", quote_id(table_name));
    let idx_list = pragma_to_json(&conn, &idx_list_sql)?;

    let mut indexes: Vec<Value> = Vec::with_capacity(idx_list.len());
    for idx in &idx_list {
        let idx_name = idx.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let info_sql = format!("PRAGMA index_info({})", quote_id(idx_name));
        let info_rows = pragma_to_json(&conn, &info_sql)?;
        let idx_columns: Vec<Value> = info_rows
            .iter()
            .filter_map(|r| r.get("name").cloned())
            .collect();

        indexes.push(json!({
            "seq": idx.get("seq"),
            "name": idx_name,
            "unique": idx.get("unique").and_then(|v| v.as_i64()).unwrap_or(0) != 0,
            "columns": idx_columns,
        }));
    }

    // Foreign keys
    let fk_sql = format!("PRAGMA foreign_key_list({})", quote_id(table_name));
    let foreign_keys = pragma_to_json(&conn, &fk_sql)?;

    // Triggers
    let mut trig_stmt = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE type='trigger' AND tbl_name=?1 ORDER BY name")
        .map_err(|e| format!("Trigger query failed: {}", e))?;

    let triggers: Vec<Value> = trig_stmt
        .query_map([table_name], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "sql": row.get::<_, Option<String>>(1)?,
            }))
        })
        .map_err(|e| format!("Trigger query failed: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Row count
    let count_sql = format!("SELECT COUNT(*) FROM {}", quote_id(table_name));
    let row_count: i64 = conn
        .query_row(&count_sql, [], |row| row.get(0))
        .map_err(|e| format!("Count query failed: {}", e))?;

    Ok(json!({
        "table": table_name,
        "columns": columns,
        "indexes": indexes,
        "foreign_keys": foreign_keys,
        "triggers": triggers,
        "row_count": row_count,
        "database": db_path,
    }))
}

// ============================================================================
// 6. CREATE TABLE
// ============================================================================

async fn create_table(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let columns = params
        .get("columns")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            "Missing required parameter: columns (JSON array of column definitions)".to_string()
        })?;

    if columns.is_empty() {
        return Err("columns array must not be empty".to_string());
    }

    let mut col_defs: Vec<String> = Vec::with_capacity(columns.len());
    for (i, col) in columns.iter().enumerate() {
        let col_name = col
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("columns[{}] missing 'name' field", i))?;

        let col_type = col
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("columns[{}] missing 'type' field", i))?;

        let mut def = format!("{} {}", quote_id(col_name), col_type.to_uppercase());

        if col.get("pk").and_then(|v| v.as_bool()).unwrap_or(false) {
            def.push_str(" PRIMARY KEY");
        }

        if !col
            .get("nullable")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            def.push_str(" NOT NULL");
        }

        if let Some(def_val) = col.get("default") {
            match def_val {
                Value::Null => def.push_str(" DEFAULT NULL"),
                Value::String(s) => {
                    def.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                }
                Value::Bool(b) => def.push_str(&format!(" DEFAULT {}", if *b { 1 } else { 0 })),
                Value::Number(n) => def.push_str(&format!(" DEFAULT {}", n)),
                Value::Array(_) | Value::Object(_) => {
                    let s =
                        serde_json::to_string(def_val).map_err(|e| format!("JSON error: {}", e))?;
                    def.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                }
            }
        }

        if col.get("unique").and_then(|v| v.as_bool()).unwrap_or(false) {
            def.push_str(" UNIQUE");
        }

        col_defs.push(def);
    }

    let create_sql = format!(
        "CREATE TABLE {} ({})",
        quote_id(table_name),
        col_defs.join(", ")
    );

    let conn = open_conn(db_path)?;
    conn.execute_batch(&create_sql)
        .map_err(|e| format!("CREATE TABLE failed: {}", e))?;

    Ok(json!({
        "action": "create_table",
        "table": table_name,
        "sql": create_sql,
        "database": db_path,
    }))
}

// ============================================================================
// 7. ALTER TABLE
// ============================================================================

async fn alter_table(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let alter_action = params
        .get("alter_action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: alter_action (add_column, drop_column, rename_column, rename_table)".to_string())?;

    let conn = open_conn(db_path)?;

    let sql: String = match alter_action {
        "add_column" => {
            let col_name = params
                .get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter: column_name".to_string())?;
            let col_type = params
                .get("column_type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    "Missing required parameter: column_type (e.g. TEXT, INTEGER)".to_string()
                })?;
            format!(
                "ALTER TABLE {} ADD COLUMN {} {}",
                quote_id(table_name),
                quote_id(col_name),
                col_type.to_uppercase()
            )
        }
        "drop_column" => {
            let col_name = params
                .get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter: column_name".to_string())?;
            format!(
                "ALTER TABLE {} DROP COLUMN {}",
                quote_id(table_name),
                quote_id(col_name)
            )
        }
        "rename_column" => {
            let col_name = params
                .get("column_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter: column_name".to_string())?;
            let new_name = params
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter: new_name".to_string())?;
            format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {}",
                quote_id(table_name),
                quote_id(col_name),
                quote_id(new_name)
            )
        }
        "rename_table" => {
            let new_name = params
                .get("new_name")
                .or_else(|| params.get("new_table_name"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    "Missing required parameter: new_name or new_table_name".to_string()
                })?;
            format!(
                "ALTER TABLE {} RENAME TO {}",
                quote_id(table_name),
                quote_id(new_name)
            )
        }
        _ => {
            return Err(format!(
                "Unknown alter_action: {}. Supported: add_column, drop_column, rename_column, rename_table",
                alter_action
            ));
        }
    };

    conn.execute_batch(&sql)
        .map_err(|e| format!("ALTER TABLE failed: {}", e))?;

    Ok(json!({
        "action": "alter_table",
        "alter_action": alter_action,
        "table": table_name,
        "sql": sql,
        "database": db_path,
    }))
}

// ============================================================================
// 8. DROP TABLE
// ============================================================================

async fn drop_table(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let conn = open_conn(db_path)?;
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", quote_id(table_name)))
        .map_err(|e| format!("DROP TABLE failed: {}", e))?;

    Ok(json!({
        "action": "drop_table",
        "table": table_name,
        "database": db_path,
    }))
}

// ============================================================================
// 9. IMPORT CSV
// ============================================================================

async fn import_csv(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let csv_path = params
        .get("csv_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: csv_path".to_string())?;

    let has_header = params
        .get("has_header")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let conn = open_conn(db_path)?;

    // Check table exists
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !table_exists {
        return Err(format!(
            "Table '{}' does not exist. Create it first or use create_table.",
            table_name
        ));
    }

    if has_header {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(csv_path)
            .map_err(|e| format!("Failed to read CSV file '{}': {}", csv_path, e))?;

        let headers: Vec<String> = rdr
            .headers()
            .map_err(|e| format!("Failed to read CSV headers: {}", e))?
            .iter()
            .map(|h| h.to_string())
            .collect();

        if headers.is_empty() {
            return Err("CSV has no columns".to_string());
        }

        let quoted_headers: Vec<String> = headers.iter().map(|h| quote_id(h)).collect();
        let placeholders: Vec<String> = (0..headers.len()).map(|_| "?".to_string()).collect();
        let insert_sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_id(table_name),
            quoted_headers.join(", "),
            placeholders.join(", ")
        );

        let mut stmt = conn
            .prepare(&insert_sql)
            .map_err(|e| format!("Failed to prepare INSERT: {}", e))?;

        let mut csv_rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(csv_path)
            .map_err(|e| format!("Failed to re-read CSV: {}", e))?;

        let mut imported: usize = 0;
        let mut errors: Vec<String> = Vec::new();

        for (line_num, result) in csv_rdr.records().enumerate() {
            match result {
                Ok(record) => {
                    let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
                    let params_refs: Vec<&dyn rusqlite::types::ToSql> = values
                        .iter()
                        .map(|v| v as &dyn rusqlite::types::ToSql)
                        .collect();
                    if let Err(e) = stmt.execute(params_refs.as_slice()) {
                        errors.push(format!("Line {}: {}", line_num + 2, e));
                    } else {
                        imported += 1;
                    }
                }
                Err(e) => {
                    errors.push(format!("Line {}: {}", line_num + 2, e));
                }
            }
        }

        Ok(json!({
            "action": "import_csv",
            "table": table_name,
            "csv_file": csv_path,
            "imported_rows": imported,
            "has_header": true,
            "errors": errors,
            "error_count": errors.len(),
        }))
    } else {
        // No header
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(csv_path)
            .map_err(|e| format!("Failed to read CSV: {}", e))?;

        let all_records: Vec<csv::StringRecord> = rdr.records().filter_map(|r| r.ok()).collect();

        if all_records.is_empty() {
            return Ok(json!({
                "action": "import_csv",
                "table": table_name,
                "csv_file": csv_path,
                "imported_rows": 0,
                "note": "CSV file is empty",
            }));
        }

        let col_count = all_records[0].len();
        let placeholders: Vec<String> = (0..col_count).map(|_| "?".to_string()).collect();
        let insert_sql = format!(
            "INSERT INTO {} VALUES ({})",
            quote_id(table_name),
            placeholders.join(", ")
        );

        let mut stmt = conn
            .prepare(&insert_sql)
            .map_err(|e| format!("Failed to prepare INSERT: {}", e))?;

        let mut imported: usize = 0;
        let mut errors: Vec<String> = Vec::new();

        for (line_num, record) in all_records.iter().enumerate() {
            let values: Vec<String> = record.iter().map(|v| v.to_string()).collect();
            if values.len() != col_count {
                errors.push(format!(
                    "Line {}: expected {} columns, got {}",
                    line_num + 1,
                    col_count,
                    values.len()
                ));
                continue;
            }
            let params_refs: Vec<&dyn rusqlite::types::ToSql> = values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            if let Err(e) = stmt.execute(params_refs.as_slice()) {
                errors.push(format!("Line {}: {}", line_num + 1, e));
            } else {
                imported += 1;
            }
        }

        Ok(json!({
            "action": "import_csv",
            "table": table_name,
            "csv_file": csv_path,
            "imported_rows": imported,
            "has_header": false,
            "errors": errors,
            "error_count": errors.len(),
        }))
    }
}

// ============================================================================
// 10. EXPORT CSV
// ============================================================================

async fn export_csv(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let table_name = params
        .get("table_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: table_name".to_string())?;

    let csv_path = params
        .get("csv_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: csv_path".to_string())?;

    let conn = open_conn(db_path)?;

    let select_sql = format!("SELECT * FROM {}", quote_id(table_name));
    let mut stmt = conn
        .prepare(&select_sql)
        .map_err(|e| format!("Failed to prepare SELECT: {}", e))?;

    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| {
            stmt.column_name(i)
                .map(|n| n.to_string())
                .unwrap_or(format!("col_{}", i))
        })
        .collect();

    let mut wtr = csv::Writer::from_path(csv_path)
        .map_err(|e| format!("Failed to create CSV writer: {}", e))?;

    wtr.write_record(&col_names)
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    let rows_iter = stmt
        .query_map([], {
            let names_for_closure = col_names.clone();
            let c_count = col_names.len();
            move |row| {
                let mut obj = serde_json::Map::new();
                for (i, name) in names_for_closure.iter().enumerate().take(c_count) {
                    let val: rusqlite::types::Value = row.get_unwrap(i);
                    obj.insert(name.clone(), sqlite_value_to_json(&val));
                }
                Ok(Value::Object(obj))
            }
        })
        .map_err(|e| format!("Query failed: {}", e))?;

    let mut exported: usize = 0;
    for row_result in rows_iter {
        let row_val: Value = row_result.map_err(|e| format!("Row error: {}", e))?;
        let obj = match &row_val {
            Value::Object(m) => m,
            _ => continue,
        };
        let mut csv_row: Vec<String> = Vec::with_capacity(col_count);
        for col_name in &col_names {
            let s = match obj.get(col_name) {
                Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Array(a)) => serde_json::to_string(&a).unwrap_or_default(),
                Some(Value::Object(o)) => serde_json::to_string(&o).unwrap_or_default(),
                None => String::new(),
            };
            csv_row.push(s);
        }
        wtr.write_record(&csv_row)
            .map_err(|e| format!("Failed to write CSV row: {}", e))?;
        exported += 1;
    }

    wtr.flush()
        .map_err(|e| format!("Failed to flush CSV: {}", e))?;

    Ok(json!({
        "action": "export_csv",
        "table": table_name,
        "csv_file": csv_path,
        "exported_rows": exported,
        "columns": col_names,
    }))
}

// ============================================================================
// 11. BACKUP
// ============================================================================

async fn backup_database(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let backup_path = params
        .get("backup_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: backup_path".to_string())?;

    let source = Connection::open(db_path)
        .map_err(|e| format!("Failed to open source database '{}': {}", db_path, e))?;

    source
        .backup(rusqlite::DatabaseName::Main, backup_path, None::<fn(_)>)
        .map_err(|e| format!("Backup failed: {}", e))?;

    let src_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);
    let dst_size = std::fs::metadata(backup_path).map(|m| m.len()).unwrap_or(0);

    Ok(json!({
        "action": "backup",
        "source": db_path,
        "backup": backup_path,
        "source_size_bytes": src_size,
        "backup_size_bytes": dst_size,
    }))
}

// ============================================================================
// 12. RUN SQL FILE
// ============================================================================

async fn run_sql_file(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let file_path = params
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: file_path".to_string())?;

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read SQL file '{}': {}", file_path, e))?;

    let conn = open_conn(db_path)?;
    conn.execute_batch(&content)
        .map_err(|e| format!("SQL execution failed: {}", e))?;

    let stmt_count = content
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .count();

    Ok(json!({
        "action": "run_sql_file",
        "file": file_path,
        "statement_count": stmt_count,
        "database": db_path,
    }))
}

// ============================================================================
// 13. MIGRATE — Apply pending migrations
// ============================================================================

const MIGRATIONS_TABLE: &str = "_schema_migrations";

fn ensure_migrations_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {} (
            version TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            applied_at TEXT NOT NULL DEFAULT (datetime('now')),
            checksum TEXT NOT NULL DEFAULT '',
            execution_time_ms INTEGER NOT NULL DEFAULT 0
        )",
        quote_id(MIGRATIONS_TABLE)
    ))
    .map_err(|e| format!("Failed to create migrations table: {}", e))
}

async fn run_migrations(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let migrations_dir = params
        .get("migrations_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: migrations_dir".to_string())?;

    let conn = open_conn(db_path)?;
    ensure_migrations_table(&conn)?;

    let dir = std::path::Path::new(migrations_dir);
    if !dir.exists() {
        return Err(format!(
            "Migrations directory '{}' does not exist",
            migrations_dir
        ));
    }

    let mut migration_files: Vec<(String, String)> = Vec::with_capacity(64);
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read migrations directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("sql") {
            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                migration_files.push((file_name.to_string(), path.to_string_lossy().to_string()));
            }
        }
    }

    migration_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Get already applied versions
    let mut stmt = conn
        .prepare(&format!(
            "SELECT version FROM {} ORDER BY version",
            quote_id(MIGRATIONS_TABLE)
        ))
        .map_err(|e| format!("Failed to query migrations: {}", e))?;

    let applied: std::collections::HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to query migrations: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut applied_list: Vec<Value> = Vec::with_capacity(migration_files.len());
    let mut errors: Vec<String> = Vec::with_capacity(1);

    for (filename, full_path) in &migration_files {
        let version = filename.split('_').next().unwrap_or(filename).to_string();

        let desc = filename
            .strip_prefix(&format!("{}_", version))
            .or_else(|| filename.strip_prefix(&version))
            .unwrap_or("")
            .strip_suffix(".sql")
            .unwrap_or("")
            .to_string();

        if applied.contains(&version) {
            continue;
        }

        let content = std::fs::read_to_string(full_path)
            .map_err(|e| format!("Failed to read migration '{}': {}", filename, e))?;

        let checksum = content_hash(&content);

        let start = std::time::Instant::now();

        conn.execute_batch("BEGIN TRANSACTION")
            .map_err(|e| format!("Failed to begin transaction for '{}': {}", filename, e))?;

        match conn.execute_batch(&content) {
            Ok(_) => {
                let elapsed = start.elapsed().as_millis() as i64;

                conn.execute(
                    &format!(
                        "INSERT INTO {} (version, filename, description, checksum, execution_time_ms) \
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        quote_id(MIGRATIONS_TABLE)
                    ),
                    rusqlite::params![version, *filename, desc, checksum, elapsed],
                )
                .map_err(|e| format!("Failed to record migration '{}': {}", filename, e))?;

                conn.execute_batch("COMMIT")
                    .map_err(|e| format!("Failed to commit migration '{}': {}", filename, e))?;

                applied_list.push(json!({
                    "version": version,
                    "filename": filename,
                    "description": desc,
                    "execution_time_ms": elapsed,
                    "status": "applied",
                }));
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                errors.push(format!(
                    "Migration '{}' failed: {}, rolled back.",
                    filename, e
                ));
            }
        }
    }

    Ok(json!({
        "action": "migrate",
        "migrations_dir": migrations_dir,
        "applied": applied_list,
        "applied_count": applied_list.len(),
        "pending_count": migration_files.len() - applied.len(),
        "errors": errors,
        "error_count": errors.len(),
        "database": db_path,
    }))
}

// ============================================================================
// 14. MIGRATE LIST — Show migration status
// ============================================================================

async fn list_migrations(db_path: &str, params: &HashMap<String, Value>) -> Result<Value, String> {
    let migrations_dir = params
        .get("migrations_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: migrations_dir".to_string())?;

    let conn = open_conn(db_path)?;
    ensure_migrations_table(&conn)?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT version, filename, description, applied_at, checksum, execution_time_ms \
             FROM {} ORDER BY version",
            quote_id(MIGRATIONS_TABLE)
        ))
        .map_err(|e| format!("Failed to query migrations: {}", e))?;

    let applied_rows: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "version": row.get::<_, String>(0)?,
                "filename": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "applied_at": row.get::<_, String>(3)?,
                "checksum": row.get::<_, String>(4)?,
                "execution_time_ms": row.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| format!("Failed to query migrations: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let applied_versions: std::collections::HashSet<String> = applied_rows
        .iter()
        .filter_map(|v| {
            v.get("version")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let dir = std::path::Path::new(migrations_dir);
    let mut available: Vec<Value> = Vec::with_capacity(64);

    if dir.exists() {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read migrations directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    let version = file_name.split('_').next().unwrap_or(file_name).to_string();
                    let is_applied = applied_versions.contains(&version);

                    let desc = file_name
                        .strip_prefix(&format!("{}_", version))
                        .or_else(|| file_name.strip_prefix(&version))
                        .unwrap_or("")
                        .strip_suffix(".sql")
                        .unwrap_or("")
                        .to_string();

                    available.push(json!({
                        "version": version,
                        "filename": file_name,
                        "description": desc,
                        "status": if is_applied { "applied" } else { "pending" },
                    }));
                }
            }
        }
    }

    available.sort_by(|a, b| {
        let va = a["version"].as_str().unwrap_or("");
        let vb = b["version"].as_str().unwrap_or("");
        va.cmp(vb)
    });

    Ok(json!({
        "action": "migrate_list",
        "migrations_dir": migrations_dir,
        "applied": applied_rows,
        "applied_count": applied_rows.len(),
        "available": available,
        "available_count": available.len(),
        "database": db_path,
    }))
}

// ============================================================================
// 15. MIGRATE CREATE — Generate a new migration template
// ============================================================================

async fn create_migration(params: &HashMap<String, Value>) -> Result<Value, String> {
    let migrations_dir = params
        .get("migrations_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: migrations_dir".to_string())?;

    let description = params
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("new_migration");

    let dir = std::path::Path::new(migrations_dir);
    std::fs::create_dir_all(dir).map_err(|e| {
        format!(
            "Failed to create migrations directory '{}': {}",
            migrations_dir, e
        )
    })?;

    let now = chrono::Utc::now();
    let version = now.format("%Y%m%d%H%M%S").to_string();

    let safe_desc: String = description
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe_desc = safe_desc.trim_matches('_').to_string();
    let safe_desc = if safe_desc.is_empty() {
        "migration".to_string()
    } else {
        safe_desc
    };

    let filename = format!("{}_{}.sql", version, safe_desc);
    let filepath = dir.join(&filename);

    let content = format!(
        "-- Migration: {}\n-- Created: {}\n-- Description: {}\n\n-- Write your SQL here\n\n",
        filename,
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        description,
    );

    std::fs::write(&filepath, &content).map_err(|e| {
        format!(
            "Failed to write migration file '{}': {}",
            filepath.display(),
            e
        )
    })?;

    Ok(json!({
        "action": "migrate_create",
        "migrations_dir": migrations_dir,
        "filename": filename,
        "filepath": filepath.to_string_lossy().to_string(),
        "version": version,
        "description": description,
    }))
}

// ============================================================================
// Helper: Simple hash for checksum
// ============================================================================

fn content_hash(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut crate::tools::ToolRegistry) {
    registry.register(Box::new(DatabaseTool));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use serde_json::json;

    fn create_test_db(name: &str) -> String {
        let path = format!("/tmp/test_db_{}_{}.sqlite", std::process::id(), name);
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                age INTEGER,
                email TEXT
             );
             INSERT INTO users VALUES (1, 'Alice', 30, 'alice@example.com');
             INSERT INTO users VALUES (2, 'Bob', 25, 'bob@example.com');
             INSERT INTO users VALUES (3, 'Charlie', 35, 'charlie@example.com');
             CREATE INDEX idx_users_name ON users(name);
             CREATE INDEX idx_users_age ON users(age);",
        )
        .unwrap();
        path
    }

    fn create_empty_db(name: &str) -> String {
        let path = format!("/tmp/test_db_{}_{}.sqlite", std::process::id(), name);
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE test_simple (id INTEGER PRIMARY KEY, value TEXT)")
            .unwrap();
        path
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path));
        let _ = std::fs::remove_file(format!("{}-shm", path));
    }

    // ---- Action routing / error handling ----

    #[tokio::test]
    async fn test_missing_params() {
        let params = HashMap::new();
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Missing required parameter"), "Got: {}", err);
    }

    #[tokio::test]
    async fn test_invalid_action() {
        let params = HashMap::from([
            ("action".to_string(), json!("nonexistent_action")),
            ("database_path".to_string(), json!(":memory:")),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unknown action"), "Got: {}", err);
    }

    // ---- 1. Query ----

    #[tokio::test]
    async fn test_query_basic() {
        let path = create_test_db("query_basic");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
            ("sql".to_string(), json!("SELECT * FROM users ORDER BY id")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["row_count"], 3);
        let rows = result["rows"].as_array().unwrap();
        assert_eq!(rows[0]["name"], "Alice");
        assert_eq!(rows[1]["name"], "Bob");
        assert_eq!(rows[2]["name"], "Charlie");
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_query_with_params() {
        let path = create_test_db("query_params");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
            (
                "sql".to_string(),
                json!("SELECT name, age FROM users WHERE age > ?"),
            ),
            ("params".to_string(), json!([28])),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["row_count"], 2);
        let rows = result["rows"].as_array().unwrap();
        let names: Vec<&str> = rows.iter().filter_map(|r| r["name"].as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Charlie"));
        assert!(!names.contains(&"Bob"));
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_query_with_multi_params() {
        let path = create_test_db("query_multi");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
            (
                "sql".to_string(),
                json!("SELECT name FROM users WHERE age > ? AND age < ?"),
            ),
            ("params".to_string(), json!([20, 32])),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["row_count"], 2);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_query_no_results() {
        let path = create_test_db("query_empty");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
            (
                "sql".to_string(),
                json!("SELECT * FROM users WHERE age > ?"),
            ),
            ("params".to_string(), json!([100])),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["row_count"], 0);
        assert!(result["rows"].as_array().unwrap().is_empty());
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_query_bad_sql() {
        let path = create_test_db("query_bad");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
            ("sql".to_string(), json!("SELECT * FROM nonexistent_table")),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_query_missing_sql() {
        let path = create_test_db("query_no_sql");
        let params = HashMap::from([
            ("action".to_string(), json!("query")),
            ("database_path".to_string(), json!(path)),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Missing required parameter: sql"));
        cleanup(&path);
    }

    // ---- 2. Execute ----

    #[tokio::test]
    async fn test_execute_insert() {
        let path = create_test_db("exec_insert");
        let params = HashMap::from([
            ("action".to_string(), json!("execute")),
            ("database_path".to_string(), json!(path)),
            (
                "sql_statement".to_string(),
                json!(
                    "INSERT INTO users (name, age, email) VALUES ('Diana', 28, 'diana@test.com')"
                ),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["affected_rows"], 1);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 4);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_update() {
        let path = create_test_db("exec_update");
        let params = HashMap::from([
            ("action".to_string(), json!("execute")),
            ("database_path".to_string(), json!(path)),
            (
                "sql_statement".to_string(),
                json!("UPDATE users SET age = 31 WHERE name = 'Alice'"),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["affected_rows"], 1);

        let conn = Connection::open(&path).unwrap();
        let age: i64 = conn
            .query_row("SELECT age FROM users WHERE name = 'Alice'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(age, 31);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_delete() {
        let path = create_test_db("exec_delete");
        let params = HashMap::from([
            ("action".to_string(), json!("execute")),
            ("database_path".to_string(), json!(path)),
            (
                "sql_statement".to_string(),
                json!("DELETE FROM users WHERE name = 'Bob'"),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["affected_rows"], 1);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_with_params() {
        let path = create_test_db("exec_params");
        let params = HashMap::from([
            ("action".to_string(), json!("execute")),
            ("database_path".to_string(), json!(path)),
            (
                "sql_statement".to_string(),
                json!("INSERT INTO users (name, age, email) VALUES (?1, ?2, ?3)"),
            ),
            ("params".to_string(), json!(["Eve", 22, "eve@test.com"])),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["affected_rows"], 1);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 4);
        cleanup(&path);
    }

    // ---- 3. Execute Batch ----

    #[tokio::test]
    async fn test_execute_batch() {
        let path = create_test_db("batch");
        let params = HashMap::from([
            ("action".to_string(), json!("execute_batch")),
            ("database_path".to_string(), json!(path)),
            (
                "statements".to_string(),
                json!([
                    "INSERT INTO users (name, age, email) VALUES ('Frank', 40, 'frank@test.com')",
                    "INSERT INTO users (name, age, email) VALUES ('Grace', 32, 'grace@test.com')",
                    "UPDATE users SET age = 26 WHERE name = 'Bob'",
                ]),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["statement_count"], 3);
        assert_eq!(result["total_affected_rows"], 3);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_batch_rollback_on_error() {
        let path = create_test_db("batch_rollback");
        let params = HashMap::from([
            ("action".to_string(), json!("execute_batch")),
            ("database_path".to_string(), json!(path)),
            (
                "statements".to_string(),
                json!([
                    "INSERT INTO users (name, age, email) VALUES ('Grace', 32, 'grace@test.com')",
                    "INSERT INTO nonexistent VALUES (1)",
                ]),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("rolled back") || err.contains("failed"),
            "Got: {}",
            err
        );

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_batch_empty() {
        let path = create_test_db("batch_empty");
        let params = HashMap::from([
            ("action".to_string(), json!("execute_batch")),
            ("database_path".to_string(), json!(path)),
            ("statements".to_string(), json!([])),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_execute_batch_missing_param() {
        let path = create_test_db("batch_noparam");
        let params = HashMap::from([
            ("action".to_string(), json!("execute_batch")),
            ("database_path".to_string(), json!(path)),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required parameter"));
        cleanup(&path);
    }

    // ---- 4. List Tables ----

    #[tokio::test]
    async fn test_list_tables() {
        let path = create_test_db("list_tables");
        let params = HashMap::from([
            ("action".to_string(), json!("list_tables")),
            ("database_path".to_string(), json!(path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table_count"], 1);
        let tables = result["tables"].as_array().unwrap();
        assert!(tables.iter().any(|t| t["name"] == "users"));
        let users = tables.iter().find(|t| t["name"] == "users").unwrap();
        assert_eq!(users["row_count"], 3);
        assert!(users["ddl"].as_str().unwrap_or("").contains("CREATE TABLE"));
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_list_tables_empty_db() {
        let path = create_empty_db("list_empty");
        let params = HashMap::from([
            ("action".to_string(), json!("list_tables")),
            ("database_path".to_string(), json!(path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table_count"], 1);
        cleanup(&path);
    }

    // ---- 5. Describe Table ----

    #[tokio::test]
    async fn test_describe_table() {
        let path = create_test_db("describe");
        let params = HashMap::from([
            ("action".to_string(), json!("describe_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table"], "users");
        let cols = result["columns"].as_array().unwrap();
        assert!(cols.iter().any(|c| c["name"] == "id"));
        assert!(cols.iter().any(|c| c["name"] == "name"));
        assert!(cols.iter().any(|c| c["name"] == "age"));
        assert_eq!(result["row_count"], 3);

        let indexes = result["indexes"].as_array().unwrap();
        assert!(indexes.iter().any(|idx| idx["name"] == "idx_users_name"));
        assert!(indexes.iter().any(|idx| idx["name"] == "idx_users_age"));

        assert!(result["foreign_keys"].is_array());
        assert!(result["triggers"].is_array());
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_describe_nonexistent_table() {
        let path = create_test_db("describe_nonexist");
        let params = HashMap::from([
            ("action".to_string(), json!("describe_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("nonexistent")),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_describe_table_missing_name() {
        let path = create_test_db("describe_noname");
        let params = HashMap::from([
            ("action".to_string(), json!("describe_table")),
            ("database_path".to_string(), json!(path)),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    // ---- 6. Create Table ----

    #[tokio::test]
    async fn test_create_table() {
        let path = create_test_db("create_table");
        let params = HashMap::from([
            ("action".to_string(), json!("create_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("products")),
            (
                "columns".to_string(),
                json!([
                    {"name": "id", "type": "INTEGER", "pk": true, "nullable": false},
                    {"name": "name", "type": "TEXT", "nullable": false},
                    {"name": "price", "type": "REAL", "nullable": true, "default": 0.0},
                    {"name": "active", "type": "INTEGER", "nullable": false, "default": true}
                ]),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table"], "products");
        assert!(result["sql"].as_str().unwrap().contains("CREATE TABLE"));

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='products'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_create_table_missing_columns() {
        let path = create_test_db("create_missing_cols");
        let params = HashMap::from([
            ("action".to_string(), json!("create_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("fail")),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_create_table_empty_columns() {
        let path = create_test_db("create_empty_cols");
        let params = HashMap::from([
            ("action".to_string(), json!("create_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("fail")),
            ("columns".to_string(), json!([])),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    // ---- 7. Alter Table ----

    #[tokio::test]
    async fn test_alter_table_add_column() {
        let path = create_test_db("alter_add");
        let params = HashMap::from([
            ("action".to_string(), json!("alter_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
            ("alter_action".to_string(), json!("add_column")),
            ("column_name".to_string(), json!("score")),
            ("column_type".to_string(), json!("REAL")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["alter_action"], "add_column");

        let conn = Connection::open(&path).unwrap();
        let sql = "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name='score'";
        let count: i64 = conn.query_row(sql, [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_alter_table_rename_table() {
        let path = create_test_db("alter_rename_table");
        let params = HashMap::from([
            ("action".to_string(), json!("alter_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
            ("alter_action".to_string(), json!("rename_table")),
            ("new_name".to_string(), json!("employees")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["alter_action"], "rename_table");

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='employees'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_alter_table_invalid_action() {
        let path = create_test_db("alter_bad");
        let params = HashMap::from([
            ("action".to_string(), json!("alter_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
            ("alter_action".to_string(), json!("invalid_action")),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown alter_action"));
        cleanup(&path);
    }

    // ---- 8. Drop Table ----

    #[tokio::test]
    async fn test_drop_table() {
        let path = create_test_db("drop");
        let params = HashMap::from([
            ("action".to_string(), json!("drop_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table"], "users");

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='users'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_drop_nonexistent_table() {
        let path = create_test_db("drop_nonexist");
        let params = HashMap::from([
            ("action".to_string(), json!("drop_table")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("nonexistent")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["table"], "nonexistent");
        cleanup(&path);
    }

    // ---- 9. CSV Import/Export ----

    #[tokio::test]
    async fn test_import_export_csv_roundtrip() {
        let path = create_test_db("csv_rt");
        let csv_path = format!("/tmp/test_csv_rt_{}.csv", std::process::id());

        let params = HashMap::from([
            ("action".to_string(), json!("export_csv")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
            ("csv_path".to_string(), json!(csv_path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["exported_rows"], 3);
        assert_eq!(result["columns"].as_array().unwrap().len(), 4);

        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE users_copy (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, email TEXT)")
            .unwrap();

        let params = HashMap::from([
            ("action".to_string(), json!("import_csv")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users_copy")),
            ("csv_path".to_string(), json!(csv_path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["imported_rows"], 3);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users_copy", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);

        let _ = std::fs::remove_file(&csv_path);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_import_csv_no_header() {
        let path = create_test_db("csv_noheader");
        let csv_path = format!("/tmp/test_csv_noheader_{}.csv", std::process::id());

        let mut wtr = csv::WriterBuilder::new()
            .has_headers(false)
            .from_path(&csv_path)
            .unwrap();
        wtr.write_record(["10", "Xavier", "45", "x@test.com"])
            .unwrap();
        wtr.write_record(["11", "Yvonne", "29", "y@test.com"])
            .unwrap();
        wtr.flush().unwrap();

        let params = HashMap::from([
            ("action".to_string(), json!("import_csv")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("users")),
            ("csv_path".to_string(), json!(csv_path)),
            ("has_header".to_string(), json!(false)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["imported_rows"], 2);
        assert_eq!(result["has_header"], false);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5);

        let _ = std::fs::remove_file(&csv_path);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_export_csv_empty_table() {
        let path = create_empty_db("csv_empty");
        let csv_path = format!("/tmp/test_csv_empty_{}.csv", std::process::id());

        let params = HashMap::from([
            ("action".to_string(), json!("export_csv")),
            ("database_path".to_string(), json!(path)),
            ("table_name".to_string(), json!("test_simple")),
            ("csv_path".to_string(), json!(csv_path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["exported_rows"], 0);

        let _ = std::fs::remove_file(&csv_path);
        cleanup(&path);
    }

    // ---- 10. Backup ----

    #[tokio::test]
    async fn test_backup() {
        let path = create_test_db("backup");
        let backup_path = format!("/tmp/test_backup_{}.sqlite", std::process::id());

        let params = HashMap::from([
            ("action".to_string(), json!("backup")),
            ("database_path".to_string(), json!(path)),
            ("backup_path".to_string(), json!(backup_path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["source"], path);
        assert_eq!(result["backup"], backup_path);
        assert!(result["source_size_bytes"].as_u64().unwrap_or(0) > 0);

        let conn = Connection::open(&backup_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);

        let _ = std::fs::remove_file(&backup_path);
        cleanup(&path);
    }

    // ---- 11. Run SQL File ----

    #[tokio::test]
    async fn test_run_sql_file() {
        let path = create_empty_db("sql_file");
        let sql_path = format!("/tmp/test_sql_{}.sql", std::process::id());

        let sql_content = "INSERT INTO test_simple (id, value) VALUES (1, 'hello');\nINSERT INTO test_simple (id, value) VALUES (2, 'world');\n";
        std::fs::write(&sql_path, sql_content).unwrap();

        let params = HashMap::from([
            ("action".to_string(), json!("run_sql_file")),
            ("database_path".to_string(), json!(path)),
            ("file_path".to_string(), json!(sql_path)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["statement_count"], 2);

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM test_simple", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let _ = std::fs::remove_file(&sql_path);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_run_sql_file_nonexistent() {
        let path = create_test_db("sql_noexist");
        let params = HashMap::from([
            ("action".to_string(), json!("run_sql_file")),
            ("database_path".to_string(), json!(path)),
            (
                "file_path".to_string(),
                json!("/tmp/nonexistent_file_12345.sql"),
            ),
        ]);
        let result = DatabaseTool.execute(&params).await;
        assert!(result.is_err());
        cleanup(&path);
    }

    // ---- 12. Migrations ----

    #[tokio::test]
    async fn test_migrate_create_and_apply() {
        let path = create_empty_db("migrate");
        let migrations_dir = format!("/tmp/test_migrations_{}", std::process::id());
        let _ = std::fs::remove_dir_all(&migrations_dir);

        // Create a migration
        let params = HashMap::from([
            ("action".to_string(), json!("migrate_create")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
            ("description".to_string(), json!("create_posts_table")),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "migrate_create");
        assert!(result["filepath"]
            .as_str()
            .unwrap()
            .contains("create_posts_table"));
        let filepath = result["filepath"].as_str().unwrap().to_string();

        // Write actual migration content
        let migration_sql = "CREATE TABLE posts (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            body TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );\n";
        std::fs::write(&filepath, migration_sql).unwrap();

        // Apply migration
        let params = HashMap::from([
            ("action".to_string(), json!("migrate")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["applied_count"], 1);
        assert_eq!(result["error_count"], 0);

        // Verify table exists
        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='posts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Re-apply (should be no-op)
        let params = HashMap::from([
            ("action".to_string(), json!("migrate")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["applied_count"], 0);

        // List migrations
        let params = HashMap::from([
            ("action".to_string(), json!("migrate_list")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["applied_count"], 1);
        assert!(result["available"].as_array().unwrap().len() >= 1);

        let _ = std::fs::remove_dir_all(&migrations_dir);
        cleanup(&path);
    }

    #[tokio::test]
    async fn test_migrate_list_no_migrations_dir() {
        let path = create_empty_db("migrate_list_bad");
        let migrations_dir = format!("/tmp/test_migrations_nonexist_{}", std::process::id());

        let params = HashMap::from([
            ("action".to_string(), json!("migrate_list")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["applied_count"], 0);
        assert_eq!(result["available_count"], 0);

        cleanup(&path);
    }

    #[tokio::test]
    async fn test_migrate_create_no_description() {
        let path = create_empty_db("migrate_create_node");
        let migrations_dir = format!("/tmp/test_migrate_node_{}", std::process::id());

        let params = HashMap::from([
            ("action".to_string(), json!("migrate_create")),
            ("database_path".to_string(), json!(path)),
            ("migrations_dir".to_string(), json!(migrations_dir)),
        ]);
        let result = DatabaseTool.execute(&params).await.unwrap();
        assert_eq!(result["description"], "new_migration");

        let _ = std::fs::remove_dir_all(&migrations_dir);
        cleanup(&path);
    }
}
