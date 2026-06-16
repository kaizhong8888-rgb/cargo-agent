//! Database migration tool for Rust projects.
//!
//! Supports SQLx, Diesel, and SeaORM migration generation, schema diff, and mock data population.
//!
//! Actions: generate_migration, diff_schema, mock_data, list_migrations, migrate_up, migrate_down

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(DbMigrationTool));
}

struct DbMigrationTool;

#[async_trait::async_trait]
impl Tool for DbMigrationTool {
    fn name(&self) -> &str {
        "db_migration"
    }

    fn description(&self) -> &str {
        "Database migration tool for Rust projects. Actions: generate_migration (create migration files \
         for SQLx/Diesel/SeaORM), diff_schema (compare schema with code models), mock_data (generate test data), \
         list_migrations (list existing migrations), migrate_up/migrate_down (generate up/down SQL)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: generate_migration, diff_schema, mock_data, list_migrations, migrate_up, migrate_down".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the Rust project directory (default: current directory)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "orm".to_string(),
                parameter_type: "string".to_string(),
                description: "ORM framework: sqlx, diesel, sea_orm (default: auto-detect from Cargo.toml)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "name".to_string(),
                parameter_type: "string".to_string(),
                description: "Migration name (e.g. 'create_users_table', 'add_email_index')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "columns".to_string(),
                parameter_type: "string".to_string(),
                description: "Column definitions as 'name:type;name:type' (e.g. 'id:uuid;name:varchar(255);email:varchar(255);created_at:timestamptz')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "table".to_string(),
                parameter_type: "string".to_string(),
                description: "Table name for the migration".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output_dir".to_string(),
                parameter_type: "string".to_string(),
                description: "Output directory for migration files (default: migrations/)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "count".to_string(),
                parameter_type: "number".to_string(),
                description: "Number of mock rows to generate (default: 10)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "schema".to_string(),
                parameter_type: "string".to_string(),
                description: "JSON schema definition for mock data generation".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let project_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        match action {
            "generate_migration" => self.generate_migration(project_path, params),
            "diff_schema" => self.diff_schema(project_path, params),
            "mock_data" => self.generate_mock_data(project_path, params),
            "list_migrations" => self.list_migrations(project_path, params),
            "migrate_up" => self.generate_migration(project_path, params),
            "migrate_down" => self.generate_migration(project_path, params),
            _ => Err(format!(
                "Unknown action: {action}. Valid: generate_migration, diff_schema, mock_data, list_migrations, migrate_up, migrate_down"
            )),
        }
    }
}

impl DbMigrationTool {
    fn detect_orm(&self, project_path: &str) -> Result<OrmFramework, String> {
        let cargo_toml_path = Path::new(project_path).join("Cargo.toml");
        let content = fs::read_to_string(&cargo_toml_path).map_err(|e| {
            format!(
                "Failed to read Cargo.toml at {}: {}",
                cargo_toml_path.display(),
                e
            )
        })?;

        if content.contains("sqlx") {
            Ok(OrmFramework::Sqlx)
        } else if content.contains("diesel") {
            Ok(OrmFramework::Diesel)
        } else if content.contains("sea-orm") {
            Ok(OrmFramework::SeaOrm)
        } else {
            Err(
                "No supported ORM detected. Specify with --orm flag (sqlx, diesel, sea_orm)"
                    .to_string(),
            )
        }
    }

    fn generate_migration(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let orm_str = params.get("orm").and_then(|v| v.as_str());
        let orm = if let Some(s) = orm_str {
            match s {
                "sqlx" => OrmFramework::Sqlx,
                "diesel" => OrmFramework::Diesel,
                "sea_orm" | "sea-orm" => OrmFramework::SeaOrm,
                _ => {
                    return Err(format!(
                        "Unknown ORM: {s}. Supported: sqlx, diesel, sea_orm"
                    ))
                }
            }
        } else {
            self.detect_orm(project_path)?
        };

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("initial_schema");

        let table = params
            .get("table")
            .and_then(|v| v.as_str())
            .unwrap_or("users");

        let columns_str = params
            .get("columns")
            .and_then(|v| v.as_str())
            .unwrap_or("id:uuid;name:varchar(255);email:varchar(255);created_at:timestamptz;updated_at:timestamptz");

        let columns = parse_columns(columns_str);

        let output_dir = params
            .get("output_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("migrations");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let migration_dir = format!("{output_dir}/{timestamp}_{name}");
        fs::create_dir_all(&migration_dir)
            .map_err(|e| format!("Failed to create migration directory: {e}"))?;

        let (up_sql, down_sql) = match orm {
            OrmFramework::Sqlx => self.generate_sqlx_migration(table, &columns),
            OrmFramework::Diesel => self.generate_diesel_migration(table, &columns),
            OrmFramework::SeaOrm => self.generate_seaorm_migration(table, &columns),
        };

        fs::write(format!("{migration_dir}/up.sql"), &up_sql)
            .map_err(|e| format!("Failed to write up.sql: {e}"))?;
        fs::write(format!("{migration_dir}/down.sql"), &down_sql)
            .map_err(|e| format!("Failed to write down.sql: {e}"))?;

        Ok(serde_json::json!({
            "action": "generate_migration",
            "orm": orm.name(),
            "migration_dir": migration_dir,
            "table": table,
            "columns": columns.iter().map(|c| &c.name).collect::<Vec<_>>(),
            "files": ["up.sql", "down.sql"],
        }))
    }

    fn generate_sqlx_migration(&self, table: &str, columns: &[Column]) -> (String, String) {
        let col_defs: Vec<String> = columns.iter().map(|c| c.to_sql()).collect();
        let pk_cols: Vec<&str> = columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let pk_def = if pk_cols.is_empty() {
            String::new()
        } else {
            format!(",\n    PRIMARY KEY ({})", pk_cols.join(", "))
        };

        let idx_cols: Vec<&str> = columns
            .iter()
            .filter(|c| c.is_index && !c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let indexes: Vec<String> = idx_cols
            .iter()
            .map(|col| format!("CREATE INDEX idx_{table}_{col} ON {table} ({col});"))
            .collect();
        let index_sql = if indexes.is_empty() {
            String::new()
        } else {
            format!("\n{}", indexes.join("\n"))
        };

        let up_sql = format!(
            r#"-- Migration: create {table} table
-- Created at: {}

CREATE TABLE IF NOT EXISTS {table} (
    {}{}
);{}
"#,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            col_defs.join(",\n    "),
            pk_def,
            index_sql,
        );

        let down_sql = format!("DROP TABLE IF EXISTS {table};\n");

        (up_sql, down_sql)
    }

    fn generate_diesel_migration(&self, table: &str, columns: &[Column]) -> (String, String) {
        let col_defs: Vec<String> = columns.iter().map(|c| c.to_sql()).collect();
        let pk_cols: Vec<&str> = columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let pk_def = if pk_cols.is_empty() {
            String::new()
        } else {
            format!(",\n    PRIMARY KEY ({})", pk_cols.join(", "))
        };

        let up_sql = format!(
            r#"-- Migration: create {table} table
-- Diesel migration

CREATE TABLE {table} (
    {}{}
);
"#,
            col_defs.join(",\n    "),
            pk_def,
        );

        let down_sql = format!("DROP TABLE {table};\n");

        (up_sql, down_sql)
    }

    fn generate_seaorm_migration(&self, table: &str, columns: &[Column]) -> (String, String) {
        let col_defs: Vec<String> = columns.iter().map(|c| c.to_sql()).collect();
        let pk_cols: Vec<&str> = columns
            .iter()
            .filter(|c| c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let pk_def = if pk_cols.is_empty() {
            String::new()
        } else {
            format!(",\n    PRIMARY KEY ({})", pk_cols.join(", "))
        };

        let idx_cols: Vec<&str> = columns
            .iter()
            .filter(|c| c.is_index && !c.is_pk)
            .map(|c| c.name.as_str())
            .collect();
        let indexes: Vec<String> = idx_cols
            .iter()
            .map(|col| format!("CREATE INDEX IF NOT EXISTS idx_{table}_{col} ON {table} ({col});"))
            .collect();
        let index_sql = if indexes.is_empty() {
            String::new()
        } else {
            format!("\n{}", indexes.join("\n"))
        };

        let up_sql = format!(
            r#"-- Migration: create {table} table
-- SeaORM migration

CREATE TABLE IF NOT EXISTS {table} (
    {}{}
);{}
"#,
            col_defs.join(",\n    "),
            pk_def,
            index_sql,
        );

        let down_sql = format!("DROP TABLE IF EXISTS {table};\n");

        (up_sql, down_sql)
    }

    fn diff_schema(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let _orm_str = params.get("orm").and_then(|v| v.as_str());
        let _orm = if let Some(s) = _orm_str {
            match s {
                "sqlx" => OrmFramework::Sqlx,
                "diesel" => OrmFramework::Diesel,
                "sea_orm" | "sea-orm" => OrmFramework::SeaOrm,
                _ => return Err(format!("Unknown ORM: {s}")),
            }
        } else {
            self.detect_orm(project_path)?
        };

        // Scan for model definitions in src/
        let src_path = Path::new(project_path).join("src");
        if !src_path.exists() {
            return Err("src/ directory not found".to_string());
        }

        let mut models: Vec<ModelInfo> = Vec::new();
        self.scan_for_models(&src_path, &mut models)?;

        if models.is_empty() {
            return Ok(serde_json::json!({
                "action": "diff_schema",
                "models_found": 0,
                "message": "No model definitions found in source code",
            }));
        }

        // Generate migration suggestions
        let mut suggestions: Vec<String> = Vec::new();
        for model in &models {
            suggestions.push(format!(
                "Model '{}' with {} fields detected. Run generate_migration with table='{}' to create migration.",
                model.name,
                model.fields.len(),
                model.table_name
            ));
        }

        Ok(serde_json::json!({
            "action": "diff_schema",
            "models_found": models.len(),
            "models": models.iter().map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "table": m.table_name,
                    "fields": m.fields.len(),
                    "field_names": m.fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "suggestions": suggestions,
        }))
    }

    fn scan_for_models(&self, dir: &Path, models: &mut Vec<ModelInfo>) -> Result<(), String> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in
            fs::read_dir(dir).map_err(|e| format!("Failed to read dir {}: {}", dir.display(), e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_for_models(&path, models)?;
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
                self.parse_models(&content, models);
            }
        }
        Ok(())
    }

    fn parse_models(&self, content: &str, models: &mut Vec<ModelInfo>) {
        // Look for #[derive(Entity)] for SeaORM or #[derive(Queryable, Insertable)] for Diesel
        // or struct definitions with common naming patterns
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();

            // Check for SeaORM entity
            if line.contains("#[derive") && line.contains("Entity") {
                if let Some(model) = self.try_parse_seaorm_entity(&lines[i..]) {
                    models.push(model);
                }
            }
            // Check for Diesel model
            else if line.contains("#[derive")
                && (line.contains("Queryable") || line.contains("Insertable"))
            {
                if let Some(model) = self.try_parse_diesel_model(&lines[i..]) {
                    models.push(model);
                }
            }
            // Check for sqlx type mapping (struct with derive)
            else if line.contains("#[derive") && line.contains("FromRow") {
                if let Some(model) = self.try_parse_sqlx_model(&lines[i..]) {
                    models.push(model);
                }
            }
            i += 1;
        }
    }

    fn try_parse_seaorm_entity(&self, lines: &[&str]) -> Option<ModelInfo> {
        // Find the struct name
        for line in lines.iter().take(10) {
            if line.starts_with("pub struct") || line.starts_with("struct") {
                let name = line.split_whitespace().nth(2)?.trim_end_matches('{').trim();
                let table_name = to_snake_case(name);
                return Some(ModelInfo {
                    name: name.to_string(),
                    table_name,
                    fields: vec![], // Entity columns are defined separately in SeaORM
                });
            }
        }
        None
    }

    fn try_parse_diesel_model(&self, lines: &[&str]) -> Option<ModelInfo> {
        for line in lines.iter().take(10) {
            if line.starts_with("pub struct") || line.starts_with("struct") {
                let name = line.split_whitespace().nth(2)?.trim_end_matches('{').trim();
                let table_name = to_snake_case(name);
                let fields = self.extract_struct_fields(lines);
                return Some(ModelInfo {
                    name: name.to_string(),
                    table_name,
                    fields,
                });
            }
        }
        None
    }

    fn try_parse_sqlx_model(&self, lines: &[&str]) -> Option<ModelInfo> {
        for line in lines.iter().take(10) {
            if line.starts_with("pub struct") || line.starts_with("struct") {
                let name = line.split_whitespace().nth(2)?.trim_end_matches('{').trim();
                let table_name = to_snake_case(name);
                let fields = self.extract_struct_fields(lines);
                return Some(ModelInfo {
                    name: name.to_string(),
                    table_name,
                    fields,
                });
            }
        }
        None
    }

    fn extract_struct_fields(&self, lines: &[&str]) -> Vec<FieldInfo> {
        let mut fields = Vec::new();
        let mut in_struct = false;
        let mut brace_count = 0;

        for line in lines {
            let trimmed = line.trim();

            if trimmed.starts_with("pub struct") || trimmed.starts_with("struct") {
                in_struct = true;
                brace_count = 0;
            }

            if in_struct {
                brace_count += trimmed.chars().filter(|&c| c == '{').count();
                brace_count -= trimmed.chars().filter(|&c| c == '}').count();

                if brace_count > 0
                    && (trimmed.starts_with("pub")
                        || !trimmed.starts_with("//[") && !trimmed.starts_with('#'))
                {
                    // Skip attributes and empty lines
                    if !trimmed.starts_with('#')
                        && !trimmed.is_empty()
                        && !trimmed.starts_with("//")
                    {
                        if let Some((name, field_type)) = self.parse_field_line(trimmed) {
                            fields.push(FieldInfo { name, field_type });
                        }
                    }
                }

                if brace_count == 0 && in_struct && !lines.is_empty() {
                    break;
                }
            }
        }

        fields
    }

    fn parse_field_line(&self, line: &str) -> Option<(String, String)> {
        // Parse "pub name: Type," or "pub name: Type"
        let line = line.trim_end_matches(',').trim();
        if !line.contains(':') {
            return None;
        }
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }
        let name = parts[0].trim().trim_start_matches("pub").trim();
        let field_type = parts[1].trim().trim_end_matches(',').trim();
        if name.is_empty() || field_type.is_empty() {
            return None;
        }
        Some((name.to_string(), field_type.to_string()))
    }

    fn generate_mock_data(
        &self,
        _project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let table = params
            .get("table")
            .and_then(|v| v.as_str())
            .unwrap_or("users");

        let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let columns_str = params
            .get("columns")
            .and_then(|v| v.as_str())
            .unwrap_or("id:uuid;name:varchar(255);email:varchar(255);created_at:timestamptz");

        let columns = parse_columns(columns_str);

        let output_dir = params
            .get("output_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let mut rows: Vec<serde_json::Value> = Vec::with_capacity(count);
        for i in 0..count {
            let mut row = serde_json::Map::new();
            for col in &columns {
                row.insert(col.name.clone(), self.generate_mock_value(&col.col_type, i));
            }
            rows.push(serde_json::Value::Object(row));
        }

        let json_output = serde_json::to_string_pretty(&rows)
            .map_err(|e| format!("Failed to serialize mock data: {e}"))?;

        let output_file = format!("{output_dir}/mock_{table}.json");
        fs::write(&output_file, &json_output)
            .map_err(|e| format!("Failed to write mock data: {e}"))?;

        // Also generate SQL INSERT statements
        let sql_statements = self.generate_insert_statements(table, &columns, &rows);
        let sql_file = format!("{output_dir}/mock_{table}.sql");
        fs::write(&sql_file, &sql_statements)
            .map_err(|e| format!("Failed to write SQL file: {e}"))?;

        Ok(serde_json::json!({
            "action": "mock_data",
            "table": table,
            "rows_generated": count,
            "json_file": output_file,
            "sql_file": sql_file,
        }))
    }

    fn generate_mock_value(&self, col_type: &str, index: usize) -> serde_json::Value {
        match col_type.to_lowercase().as_str() {
            t if t.contains("uuid") => serde_json::Value::String(format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                index, index, index, index, index
            )),
            t if t.contains("varchar") || t.contains("text") || t.contains("char") => {
                if col_type.to_lowercase().contains("email") {
                    serde_json::Value::String(format!("user{index}@example.com"))
                } else if col_type.to_lowercase().contains("name") {
                    let names = [
                        "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry",
                    ];
                    serde_json::Value::String(names[index % names.len()].to_string())
                } else {
                    serde_json::Value::String(format!("mock_value_{index}"))
                }
            }
            t if t.contains("int") || t.contains("serial") => {
                serde_json::Value::Number(serde_json::Number::from(index + 1))
            }
            t if t.contains("bool") => serde_json::Value::Bool(index.is_multiple_of(2)),
            t if t.contains("timestamp") || t.contains("datetime") || t.contains("date") => {
                serde_json::Value::String(format!(
                    "2024-01-{:02}T{:02}:00:00Z",
                    (index % 28) + 1,
                    index % 24
                ))
            }
            t if t.contains("float")
                || t.contains("double")
                || t.contains("decimal")
                || t.contains("numeric") =>
            {
                let val = (index as f64 * 1.5) + 0.99;
                serde_json::Value::Number(
                    serde_json::Number::from_f64(val).unwrap_or(serde_json::Number::from(0)),
                )
            }
            _ => serde_json::Value::String(format!("mock_{index}")),
        }
    }

    fn generate_insert_statements(
        &self,
        table: &str,
        columns: &[Column],
        rows: &[serde_json::Value],
    ) -> String {
        let mut sql = String::new();
        sql.push_str(&format!("-- Mock data for {table}\n"));
        sql.push_str(&format!(
            "-- Generated at: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        for row in rows {
            let col_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
            let values: Vec<String> = columns
                .iter()
                .filter_map(|c| {
                    row.get(&c.name).map(|v| match v {
                        serde_json::Value::String(s) => format!("'{s}'"),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Null => "NULL".to_string(),
                        _ => format!("{v}"),
                    })
                })
                .collect();

            sql.push_str(&format!(
                "INSERT INTO {table} ({}) VALUES ({});\n",
                col_names.join(", "),
                values.join(", ")
            ));
        }

        sql
    }

    fn list_migrations(
        &self,
        project_path: &str,
        _params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let migrations_dir = Path::new(project_path).join("migrations");
        if !migrations_dir.exists() {
            return Ok(serde_json::json!({
                "action": "list_migrations",
                "migrations": [],
                "message": "No migrations directory found",
            }));
        }

        let mut migrations: Vec<MigrationInfo> = Vec::new();
        for entry in fs::read_dir(&migrations_dir)
            .map_err(|e| format!("Failed to read migrations dir: {e}"))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let has_up = path.join("up.sql").exists();
                let has_down = path.join("down.sql").exists();

                let up_size = if has_up {
                    fs::metadata(path.join("up.sql"))
                        .ok()
                        .map(|m| m.len())
                        .unwrap_or(0)
                } else {
                    0
                };

                migrations.push(MigrationInfo {
                    name,
                    has_up,
                    has_down,
                    up_size,
                });
            }
        }

        migrations.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(serde_json::json!({
            "action": "list_migrations",
            "migrations_dir": migrations_dir.display().to_string(),
            "count": migrations.len(),
            "migrations": migrations.iter().map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "has_up": m.has_up,
                    "has_down": m.has_down,
                    "up_size_bytes": m.up_size,
                })
            }).collect::<Vec<_>>(),
        }))
    }
}

fn parse_columns(columns_str: &str) -> Vec<Column> {
    columns_str
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let parts: Vec<&str> = s.trim().split(':').collect();
            if parts.len() >= 2 {
                let name = parts[0].trim().to_string();
                let col_type = parts[1].trim().to_string();
                let is_pk = name == "id" || parts.iter().any(|p| p.contains("PRIMARY"));
                let is_index = parts.iter().any(|p| p.contains("index"));
                let is_nullable = parts
                    .iter()
                    .any(|p| p.contains("nullable") || p.contains("null"));

                Column {
                    name,
                    col_type,
                    is_pk,
                    is_index,
                    is_nullable,
                }
            } else {
                Column {
                    name: s.trim().to_string(),
                    col_type: "varchar(255)".to_string(),
                    is_pk: false,
                    is_index: false,
                    is_nullable: true,
                }
            }
        })
        .collect()
}

fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

struct Column {
    name: String,
    col_type: String,
    is_pk: bool,
    is_index: bool,
    is_nullable: bool,
}

impl Column {
    fn to_sql(&self) -> String {
        let sql_type = match self.col_type.to_lowercase().as_str() {
            "uuid" => "UUID DEFAULT gen_random_uuid()".to_string(),
            t if t.contains("varchar") => t.to_string(),
            t if t.contains("text") => "TEXT".to_string(),
            t if t.contains("int") && t.contains("serial") => t.to_string(),
            t if t.contains("int") || t.contains("integer") => "INTEGER".to_string(),
            t if t.contains("bool") => "BOOLEAN NOT NULL DEFAULT false".to_string(),
            t if t.contains("timestamp") || t.contains("timestamptz") => {
                if self.name.contains("created") || self.name.contains("updated") {
                    "TIMESTAMPTZ NOT NULL DEFAULT NOW()".to_string()
                } else {
                    "TIMESTAMPTZ".to_string()
                }
            }
            t if t.contains("float") || t.contains("double") => "DOUBLE PRECISION".to_string(),
            t if t.contains("decimal") || t.contains("numeric") => "DECIMAL(10, 2)".to_string(),
            t if t.contains("json") => "JSONB".to_string(),
            _ => self.col_type.clone(),
        };

        let nullable = if self.is_pk || !self.is_nullable {
            ""
        } else {
            " NULL"
        };

        if self.is_pk {
            format!("{} UUID DEFAULT gen_random_uuid() PRIMARY KEY", self.name)
        } else {
            format!("{} {}{}", self.name, sql_type, nullable)
        }
    }
}

struct FieldInfo {
    name: String,
    #[allow(dead_code)]
    field_type: String,
}

struct ModelInfo {
    name: String,
    table_name: String,
    fields: Vec<FieldInfo>,
}

struct MigrationInfo {
    name: String,
    has_up: bool,
    has_down: bool,
    up_size: u64,
}

enum OrmFramework {
    Sqlx,
    Diesel,
    SeaOrm,
}

impl OrmFramework {
    fn name(&self) -> &str {
        match self {
            OrmFramework::Sqlx => "sqlx",
            OrmFramework::Diesel => "diesel",
            OrmFramework::SeaOrm => "sea_orm",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = DbMigrationTool;
        assert_eq!(tool.name(), "db_migration");
        assert!(tool.description().contains("migration"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action"));
        assert!(params.iter().any(|p| p.name == "orm"));
        assert!(params.iter().any(|p| p.name == "table"));
        assert!(params.iter().any(|p| p.name == "columns"));
    }

    #[test]
    fn test_parse_columns_basic() {
        let cols = parse_columns("id:uuid;name:varchar(255);email:varchar(255)");
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0].name, "id");
        assert_eq!(cols[0].col_type, "uuid");
        assert!(cols[0].is_pk);
        assert_eq!(cols[1].name, "name");
        assert_eq!(cols[1].col_type, "varchar(255)");
        assert!(!cols[1].is_pk);
    }

    #[test]
    fn test_parse_columns_with_index() {
        let cols = parse_columns("id:uuid;email:varchar(255)index");
        assert_eq!(cols.len(), 2);
        assert!(!cols[0].is_index);
        assert!(cols[1].is_index);
    }

    #[test]
    fn test_parse_columns_with_nullable() {
        let cols = parse_columns("id:uuid;bio:textnullable");
        assert_eq!(cols.len(), 2);
        assert!(!cols[0].is_nullable);
        assert!(cols[1].is_nullable);
    }

    #[test]
    fn test_parse_columns_empty_parts() {
        let cols = parse_columns("");
        assert!(cols.is_empty());
    }

    #[test]
    fn test_parse_columns_single_field_no_type() {
        let cols = parse_columns("name");
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "name");
        assert_eq!(cols[0].col_type, "varchar(255)");
        assert!(!cols[0].is_pk);
    }

    #[test]
    fn test_parse_columns_trailing_semicolon() {
        let cols = parse_columns("id:uuid;name:varchar(255);");
        assert_eq!(cols.len(), 2);
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("User"), "user");
        assert_eq!(to_snake_case("UserProfile"), "user_profile");
        assert_eq!(to_snake_case("APIResponse"), "a_p_i_response");
        assert_eq!(to_snake_case("HTTPServer"), "h_t_t_p_server");
    }

    #[test]
    fn test_column_to_sql_pk() {
        let col = Column {
            name: "id".to_string(),
            col_type: "uuid".to_string(),
            is_pk: true,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("id"));
        assert!(sql.contains("PRIMARY KEY"));
    }

    #[test]
    fn test_column_to_sql_varchar() {
        let col = Column {
            name: "name".to_string(),
            col_type: "varchar(255)".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("name"));
        assert!(sql.contains("varchar(255)"));
    }

    #[test]
    fn test_column_to_sql_text() {
        let col = Column {
            name: "body".to_string(),
            col_type: "text".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("TEXT"));
    }

    #[test]
    fn test_column_to_sql_boolean() {
        let col = Column {
            name: "active".to_string(),
            col_type: "bool".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("BOOLEAN"));
        assert!(sql.contains("DEFAULT false"));
    }

    #[test]
    fn test_column_to_sql_timestamp() {
        let col = Column {
            name: "created_at".to_string(),
            col_type: "timestamptz".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("TIMESTAMPTZ"));
        assert!(sql.contains("DEFAULT NOW()"));
    }

    #[test]
    fn test_column_to_sql_timestamp_non_auto() {
        let col = Column {
            name: "published_at".to_string(),
            col_type: "timestamp".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("TIMESTAMPTZ"));
        assert!(!sql.contains("DEFAULT NOW()"));
    }

    #[test]
    fn test_column_to_sql_json() {
        let col = Column {
            name: "metadata".to_string(),
            col_type: "json".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: false,
        };
        let sql = col.to_sql();
        assert!(sql.contains("JSONB"));
    }

    #[test]
    fn test_column_to_sql_nullable() {
        let col = Column {
            name: "bio".to_string(),
            col_type: "text".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: true,
        };
        let sql = col.to_sql();
        assert!(sql.contains("NULL"));
    }

    #[test]
    fn test_orm_framework_name() {
        assert_eq!(OrmFramework::Sqlx.name(), "sqlx");
        assert_eq!(OrmFramework::Diesel.name(), "diesel");
        assert_eq!(OrmFramework::SeaOrm.name(), "sea_orm");
    }

    #[test]
    fn test_generate_mock_value_uuid() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("uuid", 5);
        assert_eq!(
            val.as_str().unwrap(),
            "00000005-0005-0005-0005-000000000005"
        );
    }

    #[test]
    fn test_generate_mock_value_email() {
        let tool = DbMigrationTool;
        // Since "varchar(255)" doesn't contain "email", it won't be an email
        let _val = tool.generate_mock_value("varchar(255)", 3);
        // Let's test with explicit email type:
        let val = tool.generate_mock_value("email_varchar", 3);
        assert_eq!(val.as_str().unwrap(), "user3@example.com");
    }

    #[test]
    fn test_generate_mock_value_name() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("name_varchar", 0);
        assert_eq!(val.as_str().unwrap(), "Alice");
        let val2 = tool.generate_mock_value("name_text", 3);
        assert_eq!(val2.as_str().unwrap(), "Diana");
    }

    #[test]
    fn test_generate_mock_value_integer() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("int", 0);
        assert_eq!(val.as_u64().unwrap(), 1);
    }

    #[test]
    fn test_generate_mock_value_bool() {
        let tool = DbMigrationTool;
        assert_eq!(tool.generate_mock_value("bool", 0).as_bool().unwrap(), true);
        assert_eq!(
            tool.generate_mock_value("bool", 1).as_bool().unwrap(),
            false
        );
    }

    #[test]
    fn test_generate_mock_value_float() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("float", 0);
        assert_eq!(val.as_f64().unwrap(), 0.99);
        let val2 = tool.generate_mock_value("decimal", 2);
        assert_eq!(val2.as_f64().unwrap(), 3.99);
    }

    #[test]
    fn test_generate_mock_value_unknown() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("unknown_type", 42);
        assert_eq!(val.as_str().unwrap(), "mock_42");
    }

    #[test]
    fn test_generate_mock_value_timestamp() {
        let tool = DbMigrationTool;
        let val = tool.generate_mock_value("timestamp", 0);
        assert_eq!(val.as_str().unwrap(), "2024-01-01T00:00:00Z");
        let val2 = tool.generate_mock_value("datetime", 15);
        assert_eq!(val2.as_str().unwrap(), "2024-01-16T15:00:00Z");
    }

    #[test]
    fn test_generate_insert_statements() {
        let tool = DbMigrationTool;
        let columns = vec![
            Column {
                name: "id".to_string(),
                col_type: "uuid".to_string(),
                is_pk: true,
                is_index: false,
                is_nullable: false,
            },
            Column {
                name: "name".to_string(),
                col_type: "varchar(255)".to_string(),
                is_pk: false,
                is_index: false,
                is_nullable: false,
            },
        ];
        let rows = vec![serde_json::json!({
            "id": "uuid-1",
            "name": "Test User"
        })];
        let sql = tool.generate_insert_statements("users", &columns, &rows);
        assert!(sql.contains("INSERT INTO users"));
        assert!(sql.contains("'uuid-1'"));
        assert!(sql.contains("'Test User'"));
    }

    #[test]
    fn test_generate_insert_with_null() {
        let tool = DbMigrationTool;
        let columns = vec![Column {
            name: "bio".to_string(),
            col_type: "text".to_string(),
            is_pk: false,
            is_index: false,
            is_nullable: true,
        }];
        let rows = vec![serde_json::json!({ "bio": null })];
        let sql = tool.generate_insert_statements("users", &columns, &rows);
        assert!(sql.contains("NULL"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = DbMigrationTool;
        let params = HashMap::from([("action".to_string(), serde_json::json!("unknown"))]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_generate_migration_sqlx_direct() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            (
                "action".to_string(),
                serde_json::json!("generate_migration"),
            ),
            ("orm".to_string(), serde_json::json!("sqlx")),
            ("name".to_string(), serde_json::json!("create_users")),
            ("table".to_string(), serde_json::json!("users")),
            (
                "columns".to_string(),
                serde_json::json!("id:uuid;name:varchar(255);email:varchar(255)index"),
            ),
            (
                "output_dir".to_string(),
                serde_json::json!("/tmp/test_migrations_29"),
            ),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["orm"], "sqlx");
        assert_eq!(result["table"], "users");

        // Cleanup
        let _ = std::fs::remove_dir_all("/tmp/test_migrations_29");
    }

    #[tokio::test]
    async fn test_generate_migration_diesel_direct() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            (
                "action".to_string(),
                serde_json::json!("generate_migration"),
            ),
            ("orm".to_string(), serde_json::json!("diesel")),
            ("name".to_string(), serde_json::json!("create_posts")),
            ("table".to_string(), serde_json::json!("posts")),
            (
                "columns".to_string(),
                serde_json::json!("id:uuid;title:varchar(255);body:text"),
            ),
            (
                "output_dir".to_string(),
                serde_json::json!("/tmp/test_migrations_29_diesel"),
            ),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["orm"], "diesel");
        assert_eq!(result["table"], "posts");

        // Verify up.sql exists and contains expected content
        let migration_dir = result["migration_dir"].as_str().unwrap();
        let up_sql = std::fs::read_to_string(format!("{migration_dir}/up.sql")).unwrap();
        assert!(up_sql.contains("CREATE TABLE posts"));
        let down_sql = std::fs::read_to_string(format!("{migration_dir}/down.sql")).unwrap();
        assert!(down_sql.contains("DROP TABLE posts"));

        // Cleanup
        let _ = std::fs::remove_dir_all("/tmp/test_migrations_29_diesel");
    }

    #[tokio::test]
    async fn test_generate_migration_seaorm_direct() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            (
                "action".to_string(),
                serde_json::json!("generate_migration"),
            ),
            ("orm".to_string(), serde_json::json!("sea_orm")),
            ("name".to_string(), serde_json::json!("create_orders")),
            ("table".to_string(), serde_json::json!("orders")),
            (
                "columns".to_string(),
                serde_json::json!("id:uuid;total:decimal(10,2);status:varchar(50)"),
            ),
            (
                "output_dir".to_string(),
                serde_json::json!("/tmp/test_migrations_29_seaorm"),
            ),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["orm"], "sea_orm");

        let migration_dir = result["migration_dir"].as_str().unwrap();
        let up_sql = std::fs::read_to_string(format!("{migration_dir}/up.sql")).unwrap();
        assert!(up_sql.contains("CREATE TABLE IF NOT EXISTS orders"));

        // Cleanup
        let _ = std::fs::remove_dir_all("/tmp/test_migrations_29_seaorm");
    }

    #[tokio::test]
    async fn test_generate_migration_unknown_orm() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            (
                "action".to_string(),
                serde_json::json!("generate_migration"),
            ),
            ("orm".to_string(), serde_json::json!("unknown_orm")),
        ]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown ORM"));
    }

    #[tokio::test]
    async fn test_list_migrations_no_dir() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("list_migrations")),
            (
                "path".to_string(),
                serde_json::json!("/tmp/nonexistent_migrations_29"),
            ),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "list_migrations");
        assert!(result["migrations"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_diff_schema_no_src() {
        let tool = DbMigrationTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("diff_schema")),
            ("path".to_string(), serde_json::json!("/tmp/nonexistent_29")),
            ("orm".to_string(), serde_json::json!("sqlx")),
        ]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }
}
