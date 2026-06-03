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
                serde_json::Value::Number(serde_json::Number::from_f64(val).unwrap())
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
        result.push(c.to_lowercase().next().unwrap());
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

#[allow(dead_code)]
struct FieldInfo {
    name: String,
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
