//! Project scaffold generator: creates complete Rust project structures.
//!
//! Supports templates: cli, lib, web, game, each with appropriate
//! Cargo.toml, directory layout, and starter code.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ============================================================================
// ScaffoldTool
// ============================================================================

pub struct ScaffoldTool;

#[async_trait::async_trait]
impl Tool for ScaffoldTool {
    fn name(&self) -> &str {
        "project_scaffold"
    }

    fn description(&self) -> &str {
        "Generate a complete Rust project structure from a template. Templates: cli (CLI app with clap), lib (library crate), web (Axum web service), game (simple game). Creates Cargo.toml, src/, tests/, and README.md."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "name".to_string(),
                description: "Project/crate name (e.g. my-crate, my_app)".to_string(),
                required: true,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "template".to_string(),
                description: "Project template: cli, lib, web, game (default: lib)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "target_dir".to_string(),
                description:
                    "Parent directory to create the project in (default: current directory)"
                        .to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "description".to_string(),
                description: "Short project description (default: A new Rust project)".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
            ToolParameter {
                name: "author".to_string(),
                description: "Author name for Cargo.toml".to_string(),
                required: false,
                parameter_type: "string".to_string(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: name")?;

        let template = params
            .get("template")
            .and_then(|v| v.as_str())
            .unwrap_or("lib");

        let target_dir = params
            .get("target_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("A new Rust project");

        let author = params
            .get("author")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("USER").ok());

        match template {
            "cli" => generate_cli_project(name, target_dir, description, author.as_deref()),
            "lib" => generate_lib_project(name, target_dir, description, author.as_deref()),
            "web" => generate_web_project(name, target_dir, description, author.as_deref()),
            "game" => generate_game_project(name, target_dir, description, author.as_deref()),
            other => Err(format!(
                "Unknown template: {other}. Supported: cli, lib, web, game"
            )),
        }
    }
}

fn generate_cli_project(
    name: &str,
    target: &str,
    description: &str,
    author: Option<&str>,
) -> Result<Value, String> {
    let project_dir = Path::new(target).join(name);
    create_dirs(&project_dir, &["src", "tests"])?;

    write_file(
        &project_dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"
{}
[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
clap = {{ version = "4", features = ["derive"] }}
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

[dev-dependencies]
assert_cmd = "2.0"
"#,
            format_author_line(author),
        ),
    )?;

    write_file(
        &project_dir.join("src/main.rs"),
        r#"use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name, version, about)]
struct Args {
    /// Name to greet
    #[arg(short, long, default_value = "World")]
    name: String,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
        tracing::info!("Starting {}", env!("CARGO_PKG_NAME"));
    }

    let greeting = greet(&args.name);
    println!("{greeting}");

    Ok(())
}

fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greet_returns_hello() {
        let result = greet("Rust");
        assert_eq!(result, "Hello, Rust!");
    }
}
"#,
    )?;

    write_file(&project_dir.join("README.md"), &format!(
        "# {name}\n\n{description}\n\n## Usage\n\n```bash\ncargo run -- --name YourName\n```\n\n## Development\n\n```bash\ncargo test\ncargo clippy\n```\n"
    ))?;

    Ok(files_created_value(&project_dir, name, "cli"))
}

fn generate_lib_project(
    name: &str,
    target: &str,
    description: &str,
    author: Option<&str>,
) -> Result<Value, String> {
    let project_dir = Path::new(target).join(name);
    create_dirs(&project_dir, &["src", "tests", "benches", "examples"])?;

    write_file(
        &project_dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"
{}
license = "MIT"

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
thiserror = "1.0"

[dev-dependencies]
rstest = "0.18"
"#,
            format_author_line(author),
        ),
    )?;

    write_file(
        &project_dir.join("src/lib.rs"),
        &format!(
            r#"//! {description}

use thiserror::Error;

/// Errors returned by {name}.
#[derive(Debug, Error)]
pub enum {camel}Error {{
    #[error("invalid input: {{0}}")]
    InvalidInput(String),
}}

/// Result type for {name}.
pub type Result<T> = std::result::Result<T, {camel}Error>;

/// Example public function.
pub fn hello(name: &str) -> Result<String> {{
    if name.is_empty() {{
        return Err({camel}Error::InvalidInput("name cannot be empty".to_string()));
    }}
    Ok(format!("Hello, {{name}}!"))
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn hello_returns_greeting() {{
        let result = hello("Rust").unwrap();
        assert_eq!(result, "Hello, Rust!");
    }}

    #[test]
    fn hello_rejects_empty_name() {{
        let err = hello("").unwrap_err();
        assert!(err.to_string().contains("invalid input"));
    }}
}}
"#,
            camel = to_camel_case(name),
        ),
    )?;

    write_file(
        &project_dir.join("tests/integration_test.rs"),
        &format!(
            r#"// Integration tests for {name}

use {snake}::*;

#[test]
fn library_hello_integration() {{
    let result = hello("Integration").unwrap();
    assert!(result.starts_with("Hello,"));
}}
"#,
            snake = to_snake_case(name),
        ),
    )?;

    write_file(&project_dir.join("README.md"), &format!(
        "# {name}\n\n{description}\n\n## Usage\n\n```rust\nuse {name}::hello;\n\nfn main() {{\n    println!(\"{{}}\", hello(\"World\").unwrap());\n}}\n```\n"
    ))?;

    Ok(files_created_value(&project_dir, name, "lib"))
}

fn generate_web_project(
    name: &str,
    target: &str,
    description: &str,
    author: Option<&str>,
) -> Result<Value, String> {
    let project_dir = Path::new(target).join(name);
    create_dirs(&project_dir, &["src", "tests", "migrations"])?;

    write_file(
        &project_dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"
{}

[dependencies]
axum = "0.7"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tokio = {{ version = "1.0", features = ["full"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
anyhow = "1.0"
"#,
            format_author_line(author),
        ),
    )?;

    write_file(
        &project_dir.join("src/main.rs"),
        r#"use axum::{routing::get, Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(root));

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "Hello from cargo-agent web service!"
}
"#,
    )?;

    write_file(&project_dir.join("README.md"), &format!(
        "# {name}\n\n{description}\n\n## Running\n\n```bash\ncargo run\n# Server starts on http://localhost:3000\ncurl http://localhost:3000/health\n```\n"
    ))?;

    Ok(files_created_value(&project_dir, name, "web"))
}

fn generate_game_project(
    name: &str,
    target: &str,
    description: &str,
    author: Option<&str>,
) -> Result<Value, String> {
    let project_dir = Path::new(target).join(name);
    create_dirs(&project_dir, &["src", "assets"])?;

    write_file(
        &project_dir.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"
{}

[dependencies]
macroquad = "0.4"
rand = "0.8"
"#,
            format_author_line(author),
        ),
    )?;

    write_file(
        &project_dir.join("src/main.rs"),
        r#"use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "My Game".to_owned(),
        window_width: 800,
        window_height: 600,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut player_x = screen_width() / 2.0;
    let mut player_y = screen_height() / 2.0;
    let speed = 200.0;

    loop {
        clear_background(LIGHTGRAY);

        // Player movement
        if is_key_down(KeyCode::Right) { player_x += speed * get_frame_time(); }
        if is_key_down(KeyCode::Left) { player_x -= speed * get_frame_time(); }
        if is_key_down(KeyCode::Down) { player_y += speed * get_frame_time(); }
        if is_key_down(KeyCode::Up) { player_y -= speed * get_frame_time(); }

        // Draw player
        draw_circle(player_x, player_y, 20.0, BLUE);
        draw_text("Use arrow keys to move", 10.0, 20.0, 20.0, DARKGRAY);

        next_frame().await
    }
}
"#,
    )?;

    write_file(&project_dir.join("README.md"), &format!(
        "# {name}\n\n{description}\n\n## Controls\n\n- Arrow keys to move\n\n## Build\n\n```bash\ncargo run --release\n```\n"
    ))?;

    Ok(files_created_value(&project_dir, name, "game"))
}

// ============================================================================
// Helpers
// ============================================================================

fn create_dirs(base: &Path, subdirs: &[&str]) -> Result<(), String> {
    fs::create_dir_all(base).map_err(|e| format!("Failed to create {base:?}: {e}"))?;
    for dir in subdirs {
        fs::create_dir_all(base.join(dir)).map_err(|e| format!("Failed to create {dir}: {e}"))?;
    }
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("Failed to write {path:?}: {e}"))
}

fn files_created_value(project_dir: &Path, name: &str, template: &str) -> Value {
    let mut files = Vec::new();
    let _ = walk_dir(project_dir, 0, &mut files);
    serde_json::json!({
        "status": "ok",
        "project": name,
        "template": template,
        "path": project_dir.to_string_lossy(),
        "files": files,
    })
}

fn walk_dir(dir: &Path, depth: usize, out: &mut Vec<Value>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let _indent = "  ".repeat(depth);
        let kind = if path.is_dir() { "dir" } else { "file" };
        out.push(serde_json::json!({
            "path": path.to_string_lossy(),
            "kind": kind,
        }));
        if path.is_dir() {
            walk_dir(&path, depth + 1, out)?;
        }
    }
    Ok(())
}

fn to_camel_case(name: &str) -> String {
    name.split(&['_', '-', '.'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn to_snake_case(name: &str) -> String {
    name.replace(['-', ' '], "_").to_lowercase()
}

fn format_author_line(author: Option<&str>) -> String {
    match author {
        Some(a) => format!(r#"authors = ["{a}"]"#),
        None => String::new(),
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ScaffoldTool));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_camel_case_converts() {
        assert_eq!(to_camel_case("my-crate"), "MyCrate");
        assert_eq!(to_camel_case("hello_world"), "HelloWorld");
    }

    #[test]
    fn to_snake_case_converts() {
        assert_eq!(to_snake_case("my-crate"), "my_crate");
        assert_eq!(to_snake_case("Hello World"), "hello_world");
    }

    #[test]
    fn scaffold_tool_metadata() {
        let tool = ScaffoldTool;
        assert_eq!(tool.name(), "project_scaffold");
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "name" && p.required));
        assert!(params.iter().any(|p| p.name == "template" && !p.required));
    }

    #[test]
    fn generate_lib_project_creates_files() {
        let temp = std::env::temp_dir().join(format!("scaffold-test-{}", uuid::Uuid::new_v4()));
        let result =
            generate_lib_project("test-lib", temp.to_str().unwrap(), "A test", Some("Test"))
                .unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["template"], "lib");

        let project_dir = temp.join("test-lib");
        assert!(project_dir.join("Cargo.toml").exists());
        assert!(project_dir.join("src/lib.rs").exists());
        assert!(project_dir.join("tests/integration_test.rs").exists());
        assert!(project_dir.join("README.md").exists());

        let _ = fs::remove_dir_all(&temp);
    }
}
