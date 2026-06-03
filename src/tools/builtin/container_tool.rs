//! Container tool: generate Dockerfiles, docker-compose configs, and manage container operations.
//!
//! Actions: generate_dockerfile, generate_compose, docker_run_cmd, analyze, generate_multiarch
//!
//! Supports multi-stage builds, musl static compilation, and optimized Rust container images.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ContainerTool));
}

struct ContainerTool;

#[async_trait::async_trait]
impl Tool for ContainerTool {
    fn name(&self) -> &str {
        "container"
    }

    fn description(&self) -> &str {
        "Container tool for Rust projects. Actions: generate_dockerfile (create optimized Dockerfile), \
         generate_compose (create docker-compose.yml), docker_run_cmd (generate run command), \
         analyze (analyze project for containerization), generate_multiarch (multi-arch build config)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: generate_dockerfile, generate_compose, docker_run_cmd, analyze, generate_multiarch".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the Rust project directory (default: current directory)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path for generated files (default: Dockerfile or docker-compose.yml)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "base_image".to_string(),
                parameter_type: "string".to_string(),
                description: "Base image for runtime (e.g. 'debian:bookworm-slim', 'alpine:3.19', 'distroless/cc')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "builder_image".to_string(),
                parameter_type: "string".to_string(),
                description: "Builder image (e.g. 'rust:1.75-bookworm', 'rust:1.75-alpine')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "target".to_string(),
                parameter_type: "string".to_string(),
                description: "Target triple for cross-compilation (e.g. 'x86_64-unknown-linux-musl')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "features".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated cargo features to enable".to_string(),
                required: false,
            },
            ToolParameter {
                name: "binary_name".to_string(),
                parameter_type: "string".to_string(),
                description: "Binary name (default: from Cargo.toml package name)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "port".to_string(),
                parameter_type: "number".to_string(),
                description: "Exposed port number (default: 8080)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "services".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated services to include in compose (e.g. 'postgres,redis')".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let project_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let output = params.get("output").and_then(|v| v.as_str());

        match action {
            "generate_dockerfile" => self.generate_dockerfile(project_path, output, params),
            "generate_compose" => self.generate_compose(project_path, output, params),
            "docker_run_cmd" => self.docker_run_cmd(project_path, params),
            "analyze" => self.analyze_project(project_path),
            "generate_multiarch" => self.generate_multiarch(project_path, output, params),
            _ => Err(format!(
                "Unknown action: {action}. Valid actions: generate_dockerfile, generate_compose, docker_run_cmd, analyze, generate_multiarch"
            )),
        }
    }
}

impl ContainerTool {
    fn generate_dockerfile(
        &self,
        project_path: &str,
        output: Option<&str>,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let cargo_toml = read_cargo_toml(project_path)?;
        let project_info = parse_cargo_toml(&cargo_toml)?;
        let binary_name = params
            .get("binary_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&project_info.package_name)
            .to_string();

        let target = params.get("target").and_then(|v| v.as_str());
        let is_musl = target.map(|t| t.contains("musl")).unwrap_or(false);

        let base_image = params
            .get("base_image")
            .and_then(|v| v.as_str())
            .unwrap_or("debian:bookworm-slim");

        let builder_image = params
            .get("builder_image")
            .and_then(|v| v.as_str())
            .unwrap_or("rust:1.75-bookworm");

        let features = params.get("features").and_then(|v| v.as_str());
        let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);

        let dockerfile = if is_musl {
            self.generate_musl_dockerfile(
                &binary_name,
                builder_image,
                target.unwrap_or("x86_64-unknown-linux-musl"),
                features,
                port,
                &project_info,
            )
        } else {
            self.generate_multi_stage_dockerfile(
                &binary_name,
                builder_image,
                base_image,
                features,
                port,
                &project_info,
            )
        };

        let output_path = output.unwrap_or("Dockerfile");
        fs::write(output_path, &dockerfile)
            .map_err(|e| format!("Failed to write Dockerfile: {e}"))?;

        Ok(serde_json::json!({
            "action": "generate_dockerfile",
            "output": output_path,
            "binary_name": binary_name,
            "target": target.unwrap_or("default"),
            "base_image": base_image,
            "builder_image": builder_image,
            "size_estimate_kb": self.estimate_image_size(base_image, is_musl),
        }))
    }

    fn generate_multi_stage_dockerfile(
        &self,
        binary_name: &str,
        builder_image: &str,
        base_image: &str,
        features: Option<&str>,
        port: u64,
        project_info: &ProjectInfo,
    ) -> String {
        let feature_flag = features.map_or(String::new(), |f| format!(" --features {f}"));
        let workspace_deps = if project_info.is_workspace {
            "# Copy workspace Cargo.toml for dependency resolution\nCOPY Cargo.toml ./\n"
        } else {
            ""
        };

        format!(
            r#"# Multi-stage build for Rust application: {binary_name}
# Stage 1: Builder
FROM {builder_image} AS builder

WORKDIR /app

{workspace_deps}
# Copy only Cargo.toml first for dependency caching
COPY Cargo.toml Cargo.lock* ./

# Create dummy source to compile dependencies
RUN mkdir src && echo "fn main() {{}}" > src/main.rs && \
    cargo build --release{feature_flag} && \
    rm -rf src

# Copy actual source code
COPY . .

# Build the application
RUN cargo build --release{feature_flag}

# Stage 2: Runtime
FROM {base_image}

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r appuser && useradd -r -g appuser -d /home/appuser -s /sbin/nologin appuser && \
    mkdir -p /home/appuser && chown -R appuser:appuser /home/appuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/{binary_name} /app/{binary_name}

# Set ownership
RUN chown appuser:appuser /app/{binary_name}

# Switch to non-root user
USER appuser

# Expose port
EXPOSE {port}

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/{binary_name}", "--health"] || exit 1

# Run the application
ENTRYPOINT ["/app/{binary_name}"]
"#
        )
    }

    fn generate_musl_dockerfile(
        &self,
        binary_name: &str,
        builder_image: &str,
        target: &str,
        features: Option<&str>,
        port: u64,
        project_info: &ProjectInfo,
    ) -> String {
        let feature_flag = features.map_or(String::new(), |f| format!(" --features {f}"));
        let workspace_deps = if project_info.is_workspace {
            "COPY Cargo.toml ./\n"
        } else {
            ""
        };

        format!(
            r#"# Static MUSL build for Rust application: {binary_name}
# This produces a single static binary with no runtime dependencies
# Stage 1: Builder
FROM {builder_image} AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools && \
    rustup target add {target}

WORKDIR /app

{workspace_deps}
# Copy Cargo.toml for dependency caching
COPY Cargo.toml Cargo.lock* ./

# Create dummy source to compile dependencies
RUN mkdir src && echo "fn main() {{}}" > src/main.rs && \
    cargo build --release --target {target}{feature_flag} && \
    rm -rf src

# Copy actual source code
COPY . .

# Build static binary
RUN cargo build --release --target {target}{feature_flag}

# Stage 2: Minimal runtime (distroless)
FROM gcr.io/distroless/static-debian12

# Copy static binary
COPY --from=builder /app/target/{target}/release/{binary_name} /{binary_name}

EXPOSE {port}

ENTRYPOINT ["/{binary_name}"]
"#
        )
    }

    fn generate_compose(
        &self,
        project_path: &str,
        output: Option<&str>,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let cargo_toml = read_cargo_toml(project_path)?;
        let project_info = parse_cargo_toml(&cargo_toml)?;
        let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);
        let services = params
            .get("services")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let compose = self.build_compose(&project_info.package_name, port, services);
        let output_path = output.unwrap_or("docker-compose.yml");

        fs::write(output_path, &compose)
            .map_err(|e| format!("Failed to write docker-compose.yml: {e}"))?;

        let included_services: Vec<&str> = if services.is_empty() {
            vec![]
        } else {
            services.split(',').map(|s| s.trim()).collect()
        };

        Ok(serde_json::json!({
            "action": "generate_compose",
            "output": output_path,
            "app_port": port,
            "included_services": included_services,
        }))
    }

    fn build_compose(&self, app_name: &str, port: u64, services: &str) -> String {
        let mut compose = format!(
            r#"services:
  {app_name}:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "{port}:{port}"
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "/app/{app_name}", "--health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 5s
"#
        );

        if services.contains("postgres") || services.contains("db") || services.is_empty() {
            compose.push_str(&format!(
                r#"
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: {app_name}
      POSTGRES_PASSWORD: {app_name}_password
      POSTGRES_DB: {app_name}_db
    ports:
      - "5432:5432"
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U {app_name}"]
      interval: 10s
      timeout: 5s
      retries: 5
"#
            ));
        }

        if services.contains("redis") {
            compose.push_str(
                r#"
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
"#,
            );
        }

        if services.contains("mysql") {
            compose.push_str(&format!(
                r#"
  mysql:
    image: mysql:8.0
    environment:
      MYSQL_ROOT_PASSWORD: root_password
      MYSQL_DATABASE: {app_name}_db
      MYSQL_USER: {app_name}
      MYSQL_PASSWORD: {app_name}_password
    ports:
      - "3306:3306"
    volumes:
      - mysql_data:/var/lib/mysql
    healthcheck:
      test: ["CMD", "mysqladmin", "ping", "-h", "localhost"]
      interval: 10s
      timeout: 5s
      retries: 5
"#
            ));
        }

        if services.contains("mongo") {
            compose.push_str(
                r#"
  mongo:
    image: mongo:7
    ports:
      - "27017:27017"
    volumes:
      - mongo_data:/data/db
    environment:
      MONGO_INITDB_ROOT_USERNAME: admin
      MONGO_INITDB_ROOT_PASSWORD: admin_password
"#,
            );
        }

        compose.push_str("\nvolumes:\n");
        if services.contains("postgres") || services.contains("db") || services.is_empty() {
            compose.push_str("  postgres_data:\n");
        }
        if services.contains("redis") {
            compose.push_str("  redis_data:\n");
        }
        if services.contains("mysql") {
            compose.push_str("  mysql_data:\n");
        }
        if services.contains("mongo") {
            compose.push_str("  mongo_data:\n");
        }
        if !services.contains("postgres")
            && !services.contains("db")
            && !services.contains("redis")
            && !services.contains("mysql")
            && !services.contains("mongo")
            && !services.is_empty()
        {
            compose.push_str("  # No persistent volumes needed\n");
        }

        compose
    }

    fn docker_run_cmd(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let cargo_toml = read_cargo_toml(project_path)?;
        let project_info = parse_cargo_toml(&cargo_toml)?;
        let binary_name = params
            .get("binary_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&project_info.package_name);
        let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);
        let env_vars = params.get("env").and_then(|v| v.as_str()).unwrap_or("");
        let volume_mounts = params.get("volumes").and_then(|v| v.as_str()).unwrap_or("");

        let mut cmd = format!("docker run -d --name {binary_name} \\\n  -p {port}:{port}");

        // Add environment variables
        if !env_vars.is_empty() {
            for env in env_vars.split(',').map(|s| s.trim()) {
                cmd.push_str(&format!(" \\\n  -e {env}"));
            }
        }

        // Add volume mounts
        if !volume_mounts.is_empty() {
            for vol in volume_mounts.split(',').map(|s| s.trim()) {
                cmd.push_str(&format!(" \\\n  -v {vol}"));
            }
        }

        cmd.push_str(&format!(" \\\n  {binary_name}:latest"));

        let build_cmd = format!("docker build -t {binary_name}:latest . && \\\n{cmd}");

        Ok(serde_json::json!({
            "action": "docker_run_cmd",
            "build_command": build_cmd,
            "run_command": cmd,
            "binary_name": binary_name,
            "port": port,
        }))
    }

    fn analyze_project(&self, project_path: &str) -> Result<Value, String> {
        let cargo_toml = read_cargo_toml(project_path)?;
        let project_info = parse_cargo_toml(&cargo_toml)?;

        // Check for common web frameworks
        let has_axum = cargo_toml.contains("axum");
        let has_actix = cargo_toml.contains("actix-web");
        let has_rocket = cargo_toml.contains("rocket");
        let has_warp = cargo_toml.contains("warp");
        let has_poem = cargo_toml.contains("poem");

        let web_framework = if has_axum {
            "axum"
        } else if has_actix {
            "actix-web"
        } else if has_rocket {
            "rocket"
        } else if has_warp {
            "warp"
        } else if has_poem {
            "poem"
        } else {
            "none"
        };

        // Check for database drivers
        let has_sqlx = cargo_toml.contains("sqlx");
        let has_diesel = cargo_toml.contains("diesel");
        let has_sea_orm = cargo_toml.contains("sea-orm");
        let has_tokio_postgres = cargo_toml.contains("tokio-postgres");

        // Check for async runtime
        let has_tokio = cargo_toml.contains("tokio");
        let has_async_std = cargo_toml.contains("async-std");

        // Determine if it's a web service
        let is_web_service = has_axum || has_actix || has_rocket || has_warp || has_poem;

        let recommended_port = if is_web_service { 8080 } else { 0 };

        let recommendations = self.generate_recommendations(
            &project_info,
            web_framework,
            has_tokio,
            has_sqlx || has_diesel || has_sea_orm,
        );

        Ok(serde_json::json!({
            "action": "analyze",
            "package_name": project_info.package_name,
            "version": project_info.version,
            "is_workspace": project_info.is_workspace,
            "has_lib": project_info.has_lib,
            "web_framework": web_framework,
            "has_axum": has_axum,
            "has_actix": has_actix,
            "has_rocket": has_rocket,
            "is_web_service": is_web_service,
            "has_database": has_sqlx || has_diesel || has_sea_orm || has_tokio_postgres,
            "database_drivers": {
                "sqlx": has_sqlx,
                "diesel": has_diesel,
                "sea_orm": has_sea_orm,
                "tokio_postgres": has_tokio_postgres,
            },
            "async_runtime": if has_tokio { "tokio" } else if has_async_std { "async-std" } else { "none" },
            "recommended_port": recommended_port,
            "recommendations": recommendations,
        }))
    }

    fn generate_multiarch(
        &self,
        project_path: &str,
        output: Option<&str>,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let cargo_toml = read_cargo_toml(project_path)?;
        let project_info = parse_cargo_toml(&cargo_toml)?;
        let binary_name = params
            .get("binary_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&project_info.package_name);
        let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(8080);

        let platforms = params
            .get("platforms")
            .and_then(|v| v.as_str())
            .unwrap_or("linux/amd64,linux/arm64");

        let buildx_dockerfile = format!(
            r#"# syntax=docker/dockerfile:1

ARG TARGETARCH
ARG TARGETOS

# Builder stage
FROM --platform=$BUILDPLATFORM rust:1.75-bookworm AS builder

ARG TARGETARCH
ARG TARGETOS

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Install cross-compilation targets
RUN case "$TARGETARCH" in \
      amd64) rustup target add x86_64-unknown-linux-gnu ;; \
      arm64) rustup target add aarch64-unknown-linux-gnu ;; \
    esac

WORKDIR /app

COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {{}}" > src/main.rs && \
    case "$TARGETARCH" in \
      amd64) cargo build --release --target x86_64-unknown-linux-gnu ;; \
      arm64) cargo build --release --target aarch64-unknown-linux-gnu ;; \
    esac && \
    rm -rf src

COPY . .

RUN case "$TARGETARCH" in \
      amd64) cargo build --release --target x86_64-unknown-linux-gnu ;; \
      arm64) cargo build --release --target aarch64-unknown-linux-gnu ;; \
    esac

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

ARG TARGETARCH
COPY --from=builder /app/target/*/release/{binary_name} /app/{binary_name}

EXPOSE {port}

ENTRYPOINT ["/app/{binary_name}"]
"#
        );

        let build_cmd = format!(
            "docker buildx create --use && \\\ndocker buildx build \\\n  --platform {platforms} \\\n  --push \\\n  -t {binary_name}:latest \\\n  -t {binary_name}:$(date +%Y%m%d) \\\n  ."
        );

        let output_path = output.unwrap_or("Dockerfile.multiarch");
        fs::write(output_path, &buildx_dockerfile)
            .map_err(|e| format!("Failed to write multiarch Dockerfile: {e}"))?;

        Ok(serde_json::json!({
            "action": "generate_multiarch",
            "output": output_path,
            "platforms": platforms,
            "build_command": build_cmd,
            "binary_name": binary_name,
            "note": "Requires docker buildx and QEMU for cross-platform emulation",
        }))
    }

    fn generate_recommendations(
        &self,
        project_info: &ProjectInfo,
        web_framework: &str,
        has_tokio: bool,
        has_database: bool,
    ) -> Vec<String> {
        let mut recs = Vec::new();

        if web_framework == "none" && project_info.has_lib {
            recs.push("This is a library crate. Consider adding a binary target with [[bin]] section for containerization.".to_string());
        }

        if has_tokio {
            recs.push("Use tokio runtime in Docker: add RUSTFLAGS='-C target-feature=+fxsr,+sse,+sse2' for optimized builds.".to_string());
        }

        if has_database {
            recs.push("Consider using docker-compose with database service and health checks for dependency ordering.".to_string());
            recs.push(
                "Use DATABASE_URL environment variable for connection configuration.".to_string(),
            );
        }

        if web_framework == "axum" {
            recs.push(
                "Axum: Consider using tower-http for compression and tracing middleware."
                    .to_string(),
            );
        }

        if web_framework == "actix-web" {
            recs.push(
                "Actix-web: Use workers = num_cpus for optimal performance in production."
                    .to_string(),
            );
        }

        if web_framework == "rocket" {
            recs.push(
                "Rocket: Set ROCKET_ADDRESS=0.0.0.0 and ROCKET_PORT=8080 in container environment."
                    .to_string(),
            );
        }

        if recs.is_empty() {
            recs.push("Consider using musl target for smaller, static binaries (x86_64-unknown-linux-musl).".to_string());
            recs.push(
                "Use distroless or scratch images for minimal runtime footprint.".to_string(),
            );
        }

        recs
    }

    fn estimate_image_size(&self, base_image: &str, is_musl: bool) -> u64 {
        if is_musl {
            // distroless/static is ~3-5MB
            5000
        } else {
            match base_image {
                i if i.contains("distroless") => 20000,
                i if i.contains("alpine") => 30000,
                i if i.contains("slim") => 80000,
                i if i.contains("debian") || i.contains("ubuntu") => 150000,
                _ => 100000,
            }
        }
    }
}

struct ProjectInfo {
    package_name: String,
    version: String,
    is_workspace: bool,
    has_lib: bool,
}

fn read_cargo_toml(project_path: &str) -> Result<String, String> {
    let path = Path::new(project_path).join("Cargo.toml");
    fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read Cargo.toml at {}: {}", path.display(), e))
}

fn parse_cargo_toml(content: &str) -> Result<ProjectInfo, String> {
    let mut package_name = String::from("my-app");
    let mut version = String::from("0.1.0");
    let is_workspace = content.contains("[workspace]");
    let has_lib = content.contains("[lib]");

    // Simple TOML parsing for package info
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name") && line.contains('=') && package_name == "my-app" {
            package_name = line
                .split('=')
                .nth(1)
                .map(|v| v.trim().trim_matches('"').to_string())
                .unwrap_or_else(|| "my-app".to_string());
        }
        if line.starts_with("version") && line.contains('=') && version == "0.1.0" {
            version = line
                .split('=')
                .nth(1)
                .map(|v| v.trim().trim_matches('"').to_string())
                .unwrap_or_else(|| "0.1.0".to_string());
        }
    }

    Ok(ProjectInfo {
        package_name,
        version,
        is_workspace,
        has_lib,
    })
}
