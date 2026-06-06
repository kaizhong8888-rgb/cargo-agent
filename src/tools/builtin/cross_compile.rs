//! Cross-compilation and Wasm tool for Rust projects.
//!
//! Supports target toolchain management, cross-build config generation,
//! Wasm size analysis, and multi-architecture build support.
//!
//! Actions: list_targets, install_target, config, build, wasm_analyze, wasm_opt, embedded

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(CrossCompileTool));
}

struct CrossCompileTool;

#[async_trait::async_trait]
impl Tool for CrossCompileTool {
    fn name(&self) -> &str {
        "cross_compile"
    }

    fn description(&self) -> &str {
        "Cross-compilation and Wasm tool for Rust projects. Actions: list_targets (show available \
         compilation targets), install_target (install target toolchain), config (generate \
         .cargo/config.toml for cross-compilation), build (cross-build command), wasm_analyze \
         (analyze Wasm binary size), wasm_opt (optimize Wasm binary), embedded (embedded device config)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: list_targets, install_target, config, build, wasm_analyze, wasm_opt, embedded".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the Rust project directory (default: current directory)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "target".to_string(),
                parameter_type: "string".to_string(),
                description: "Target triple (e.g. 'x86_64-unknown-linux-musl', 'wasm32-unknown-unknown', 'aarch64-apple-darwin')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path for generated config (default: .cargo/config.toml)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "features".to_string(),
                parameter_type: "string".to_string(),
                description: "Comma-separated cargo features to enable".to_string(),
                required: false,
            },
            ToolParameter {
                name: "release".to_string(),
                parameter_type: "boolean".to_string(),
                description: "Build in release mode (default: true)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "wasm_path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to .wasm file for wasm_analyze/wasm_opt actions".to_string(),
                required: false,
            },
            ToolParameter {
                name: "platform".to_string(),
                parameter_type: "string".to_string(),
                description: "Platform category: linux, macos, windows, wasm, embedded, all".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let project_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        match action {
            "list_targets" => self.list_targets(params),
            "install_target" => self.install_target(params),
            "config" => self.generate_config(project_path, params),
            "build" => self.cross_build(project_path, params),
            "wasm_analyze" => self.wasm_analyze(project_path, params),
            "wasm_opt" => self.wasm_opt(params),
            "embedded" => self.embedded_config(project_path, params),
            _ => Err(format!(
                "Unknown action: {action}. Valid: list_targets, install_target, config, build, wasm_analyze, wasm_opt, embedded"
            )),
        }
    }
}

impl CrossCompileTool {
    fn list_targets(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let platform = params
            .get("platform")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let targets = Self::get_targets_for_platform(platform);
        let installed = self.get_installed_targets();

        let mut result: Vec<TargetInfo> = Vec::new();
        for (target, description) in &targets {
            let is_installed = installed.iter().any(|t| t == target);
            let category = Self::categorize_target(target);
            result.push(TargetInfo {
                target: target.to_string(),
                description: description.to_string(),
                is_installed,
                category,
            });
        }

        Ok(serde_json::json!({
            "action": "list_targets",
            "platform": platform,
            "targets": result.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
            "installed_count": result.iter().filter(|t| t.is_installed).count(),
            "total_count": result.len(),
        }))
    }

    fn install_target(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or("target parameter is required for install_target".to_string())?;

        let all_targets = Self::get_targets_for_platform("all");
        if !all_targets.contains_key(target) {
            let suggestions = self.find_similar_targets(target);
            return Err(format!(
                "Unknown target: {target}. Did you mean one of: {}?",
                suggestions.join(", ")
            ));
        }

        let output = std::process::Command::new("rustup")
            .args(["target", "add", target])
            .output()
            .map_err(|e| format!("Failed to run rustup: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "rustup target add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(serde_json::json!({
            "action": "install_target",
            "target": target,
            "status": "installed",
            "output": String::from_utf8_lossy(&output.stdout).to_string().trim().to_string(),
        }))
    }

    fn generate_config(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let _project_path = project_path;
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or("target parameter is required for config generation".to_string())?;

        let output = params
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or(".cargo/config.toml");

        let config_content = self.generate_cargo_config(target)?;

        if let Some(parent) = Path::new(output).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }

        fs::write(output, &config_content)
            .map_err(|e| format!("Failed to write config file: {e}"))?;

        Ok(serde_json::json!({
            "action": "config",
            "target": target,
            "output": output,
            "config_preview": if config_content.len() > 300 {
                format!("{}...", &config_content[..300])
            } else {
                config_content.clone()
            },
        }))
    }

    fn generate_cargo_config(&self, target: &str) -> Result<String, String> {
        let linker = Self::get_linker_for_target(target);
        let rustflags = Self::get_rustflags_for_target(target);

        let mut config = String::new();
        config.push_str(&format!("# Cross-compilation config for {target}\n"));
        config.push_str("# Generated by cargo-agent\n\n");

        config.push_str("[build]\n");
        config.push_str(&format!("target = \"{target}\"\n\n"));

        config.push_str(&format!("[target.{target}]\n"));
        if let Some(l) = linker {
            config.push_str(&format!("linker = \"{l}\"\n"));
        }
        if !rustflags.is_empty() {
            config.push_str(&format!("rustflags = [{}]\n", rustflags.join(", ")));
        }

        if target.contains("musl") {
            config.push_str("\n# MUSL static linking\n");
            config.push_str("[target.x86_64-unknown-linux-musl]\n");
            config.push_str("rustflags = [\"-C\", \"target-feature=+crt-static\"]\n");
        }

        if target.contains("wasm") {
            config.push_str("\n# Wasm optimization\n");
            config.push_str("[profile.release]\n");
            config.push_str("opt-level = \"z\"      # Optimize for size\n");
            config.push_str("lto = true             # Enable link-time optimization\n");
            config.push_str("codegen-units = 1      # Better optimization with single unit\n");
            config.push_str("strip = true           # Strip symbols\n");
        }

        Ok(config)
    }

    fn cross_build(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or("target parameter is required for cross-build".to_string())?;

        let release = params
            .get("release")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let features = params.get("features").and_then(|v| v.as_str());

        let installed = self.get_installed_targets();
        if !installed.iter().any(|t| t == target) {
            return Err(format!(
                "Target '{target}' is not installed. Run `rustup target add {target}` first."
            ));
        }

        let mut args = vec![
            "build".to_string(),
            "--target".to_string(),
            target.to_string(),
        ];
        if release {
            args.push("--release".to_string());
        }
        if let Some(f) = features {
            args.push("--features".to_string());
            args.push(f.to_string());
        }

        let output = std::process::Command::new("cargo")
            .args(&args)
            .current_dir(project_path)
            .output()
            .map_err(|e| format!("Failed to run cargo build: {e}"))?;

        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let binary_path = if success {
            let profile = if release { "release" } else { "debug" };
            let cargo_toml = Path::new(project_path).join("Cargo.toml");
            let binary_name = if cargo_toml.exists() {
                let content = fs::read_to_string(&cargo_toml).unwrap_or_default();
                content
                    .lines()
                    .find(|l| l.trim().starts_with("name") && l.contains('='))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|v| v.trim().trim_matches('"').to_string())
                    .unwrap_or_else(|| "my-app".to_string())
            } else {
                "my-app".to_string()
            };

            let ext = if target.contains("windows") {
                ".exe"
            } else {
                ""
            };
            Some(format!("target/{target}/{profile}/{binary_name}{ext}"))
        } else {
            None
        };

        Ok(serde_json::json!({
            "action": "build",
            "target": target,
            "release": release,
            "success": success,
            "binary_path": binary_path,
            "command": format!("cargo {}", args.join(" ")),
            "output": if success {
                stdout.trim().to_string()
            } else {
                format!("Build failed:\n{stderr}")
            },
        }))
    }

    fn wasm_analyze(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let wasm_path = params.get("wasm_path").and_then(|v| v.as_str());

        let wasm_file = if let Some(p) = wasm_path {
            p.to_string()
        } else {
            self.find_latest_wasm(project_path)?
        };

        if !Path::new(&wasm_file).exists() {
            return Err(format!("Wasm file not found: {wasm_file}"));
        }

        let metadata = fs::metadata(&wasm_file)
            .map_err(|e| format!("Failed to read wasm file metadata: {e}"))?;

        let size_bytes = metadata.len();
        let size_kb = size_bytes as f64 / 1024.0;

        let sections = self.estimate_sections_from_size();
        let optimization_suggestions = self.generate_wasm_opt_suggestions(size_bytes);

        let size_category = if size_bytes < 50_000 {
            "excellent"
        } else if size_bytes < 200_000 {
            "good"
        } else if size_bytes < 500_000 {
            "moderate"
        } else if size_bytes < 1_000_000 {
            "large"
        } else {
            "very_large"
        };

        Ok(serde_json::json!({
            "action": "wasm_analyze",
            "file": wasm_file,
            "size_bytes": size_bytes,
            "size_kb": size_kb,
            "size_category": size_category,
            "sections": sections,
            "optimization_suggestions": optimization_suggestions,
        }))
    }

    fn wasm_opt(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let wasm_path = params
            .get("wasm_path")
            .and_then(|v| v.as_str())
            .ok_or("wasm_path is required for wasm_opt".to_string())?;

        if !Path::new(wasm_path).exists() {
            return Err(format!("Wasm file not found: {wasm_path}"));
        }

        let output_default = format!("{}.optimized.wasm", wasm_path.trim_end_matches(".wasm"));
        let output_path = params
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or(&output_default);

        let has_wasm_opt = std::process::Command::new("wasm-opt")
            .arg("--version")
            .output()
            .is_ok();

        if !has_wasm_opt {
            return Err(
                "wasm-opt not found. Install binaryen: https://github.com/WebAssembly/binaryen"
                    .to_string(),
            );
        }

        let output = std::process::Command::new("wasm-opt")
            .args(["-Oz", "--strip-debug", wasm_path, "-o", output_path])
            .output()
            .map_err(|e| format!("Failed to run wasm-opt: {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "wasm-opt failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let original_size = fs::metadata(wasm_path).map(|m| m.len()).unwrap_or(0);
        let optimized_size = fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

        let reduction = if original_size > 0 {
            (1.0 - optimized_size as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };

        Ok(serde_json::json!({
            "action": "wasm_opt",
            "input": wasm_path,
            "output": output_path,
            "original_size_bytes": original_size,
            "optimized_size_bytes": optimized_size,
            "reduction_percent": format!("{:.1}%", reduction),
            "command": format!("wasm-opt -Oz --strip-debug {wasm_path} -o {output_path}"),
        }))
    }

    fn embedded_config(
        &self,
        _project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("thumbv7em-none-eabihf");

        let output = params
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or(".cargo/config.toml");

        let config_content = Self::generate_embedded_config(target);

        if let Some(parent) = Path::new(output).parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
        }

        fs::write(output, &config_content).map_err(|e| format!("Failed to write config: {e}"))?;

        let memory_x = Self::generate_memory_x(target);

        Ok(serde_json::json!({
            "action": "embedded",
            "target": target,
            "config_output": output,
            "memory_x_content": memory_x,
            "required_features": [
                "cortex-m",
                "cortex-m-rt",
                "embedded-hal",
            ],
            "commands": [
                format!("rustup target add {target}"),
                "cargo install cargo-embed",
                "cargo build --release",
            ],
        }))
    }

    fn generate_embedded_config(target: &str) -> String {
        let mut config = String::new();
        config.push_str(&format!("# Embedded configuration for {target}\n"));
        config.push_str("# Generated by cargo-agent\n\n");

        config.push_str("[build]\n");
        config.push_str(&format!("target = \"{target}\"\n\n"));

        config.push_str(&format!("[target.{target}]\n"));
        config.push_str("runner = \"probe-rs run\"\n");
        config.push_str("rustflags = [\n");
        config.push_str("  \"-C\", \"linker=flip-link\",\n");
        config.push_str("  \"-C\", \"link-arg=-Tlink.x\",\n");
        config.push_str("]\n\n");

        config.push_str("[unstable]\n");
        config.push_str("build-std = [\"core\", \"alloc\"]\n");

        config
    }

    fn generate_memory_x(target: &str) -> String {
        match target {
            t if t.contains("thumbv6") => {
                "MEMORY\n{\n  FLASH : ORIGIN = 0x00000000, LENGTH = 64K\n  RAM   : ORIGIN = 0x20000000, LENGTH = 8K\n}\n"
            }
            t if t.contains("thumbv7em") && t.contains("eabihf") => {
                "MEMORY\n{\n  FLASH : ORIGIN = 0x08000000, LENGTH = 512K\n  RAM   : ORIGIN = 0x20000000, LENGTH = 128K\n}\n"
            }
            t if t.contains("thumbv7m") => {
                "MEMORY\n{\n  FLASH : ORIGIN = 0x08000000, LENGTH = 256K\n  RAM   : ORIGIN = 0x20000000, LENGTH = 64K\n}\n"
            }
            t if t.contains("riscv") => {
                "MEMORY\n{\n  FLASH : ORIGIN = 0x20000000, LENGTH = 256K\n  RAM   : ORIGIN = 0x20040000, LENGTH = 64K\n}\n"
            }
            _ => {
                "MEMORY\n{\n  FLASH : ORIGIN = 0x08000000, LENGTH = 512K\n  RAM   : ORIGIN = 0x20000000, LENGTH = 128K\n}\n"
            }
        }.to_string()
    }

    fn get_targets_for_platform(platform: &str) -> HashMap<String, String> {
        let mut targets = HashMap::new();

        if matches!(platform, "linux" | "all") {
            targets.insert(
                "x86_64-unknown-linux-gnu".to_string(),
                "Linux x86_64 (glibc)".to_string(),
            );
            targets.insert(
                "x86_64-unknown-linux-musl".to_string(),
                "Linux x86_64 (musl, static)".to_string(),
            );
            targets.insert(
                "aarch64-unknown-linux-gnu".to_string(),
                "Linux ARM64 (glibc)".to_string(),
            );
            targets.insert(
                "aarch64-unknown-linux-musl".to_string(),
                "Linux ARM64 (musl, static)".to_string(),
            );
            targets.insert(
                "i686-unknown-linux-gnu".to_string(),
                "Linux x86 (32-bit)".to_string(),
            );
            targets.insert(
                "armv7-unknown-linux-gnueabihf".to_string(),
                "Linux ARMv7 (hard-float)".to_string(),
            );
            targets.insert(
                "arm-unknown-linux-gnueabi".to_string(),
                "Linux ARM (soft-float)".to_string(),
            );
            targets.insert(
                "powerpc64le-unknown-linux-gnu".to_string(),
                "Linux PowerPC64LE".to_string(),
            );
            targets.insert(
                "riscv64gc-unknown-linux-gnu".to_string(),
                "Linux RISC-V 64".to_string(),
            );
        }
        if matches!(platform, "macos" | "all") {
            targets.insert("x86_64-apple-darwin".to_string(), "macOS Intel".to_string());
            targets.insert(
                "aarch64-apple-darwin".to_string(),
                "macOS Apple Silicon (M1/M2)".to_string(),
            );
            targets.insert("aarch64-apple-ios".to_string(), "iOS ARM64".to_string());
            targets.insert(
                "x86_64-apple-ios".to_string(),
                "iOS Simulator x86_64".to_string(),
            );
            targets.insert(
                "aarch64-apple-ios-sim".to_string(),
                "iOS Simulator ARM64".to_string(),
            );
        }
        if matches!(platform, "windows" | "all") {
            targets.insert(
                "x86_64-pc-windows-msvc".to_string(),
                "Windows x86_64 (MSVC)".to_string(),
            );
            targets.insert(
                "x86_64-pc-windows-gnu".to_string(),
                "Windows x86_64 (MinGW)".to_string(),
            );
            targets.insert(
                "i686-pc-windows-msvc".to_string(),
                "Windows x86 (32-bit, MSVC)".to_string(),
            );
            targets.insert(
                "aarch64-pc-windows-msvc".to_string(),
                "Windows ARM64 (MSVC)".to_string(),
            );
        }
        if matches!(platform, "wasm" | "all") {
            targets.insert(
                "wasm32-unknown-unknown".to_string(),
                "WebAssembly (no host bindings)".to_string(),
            );
            targets.insert(
                "wasm32-unknown-emscripten".to_string(),
                "WebAssembly (Emscripten)".to_string(),
            );
            targets.insert("wasm32-wasi".to_string(), "WebAssembly (WASI)".to_string());
        }
        if matches!(platform, "embedded" | "all") {
            targets.insert(
                "thumbv6m-none-eabi".to_string(),
                "ARM Cortex-M0/M0+".to_string(),
            );
            targets.insert(
                "thumbv7m-none-eabi".to_string(),
                "ARM Cortex-M3".to_string(),
            );
            targets.insert(
                "thumbv7em-none-eabi".to_string(),
                "ARM Cortex-M4/M7 (no FPU)".to_string(),
            );
            targets.insert(
                "thumbv7em-none-eabihf".to_string(),
                "ARM Cortex-M4F/M7F (FPU)".to_string(),
            );
            targets.insert(
                "riscv32imc-unknown-none-elf".to_string(),
                "RISC-V 32-bit (compressed)".to_string(),
            );
            targets.insert(
                "riscv32imac-unknown-none-elf".to_string(),
                "RISC-V 32-bit (atomic)".to_string(),
            );
        }

        targets
    }

    fn categorize_target(target: &str) -> String {
        if target.contains("linux") {
            "linux"
        } else if target.contains("apple") {
            "macos/ios"
        } else if target.contains("windows") {
            "windows"
        } else if target.contains("wasm") {
            "wasm"
        } else if target.contains("thumb") || (target.contains("riscv") && target.contains("none"))
        {
            "embedded"
        } else {
            "other"
        }
        .to_string()
    }

    fn get_installed_targets(&self) -> Vec<String> {
        let output = std::process::Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn find_similar_targets(&self, query: &str) -> Vec<String> {
        let all = Self::get_targets_for_platform("all");
        let query_lower = query.to_lowercase();
        let mut similar: Vec<String> = all
            .keys()
            .filter(|t| t.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();
        similar.truncate(5);
        similar
    }

    fn get_linker_for_target(target: &str) -> Option<&'static str> {
        match target {
            t if t.contains("armv7-unknown-linux-gnueabihf") => Some("arm-linux-gnueabihf-gcc"),
            t if t.contains("aarch64-unknown-linux-gnu") => Some("aarch64-linux-gnu-gcc"),
            t if t.contains("x86_64-unknown-linux-musl") => Some("musl-gcc"),
            t if t.contains("x86_64-pc-windows-gnu") => Some("x86_64-w64-mingw32-gcc"),
            _ => None,
        }
    }

    fn get_rustflags_for_target(target: &str) -> Vec<String> {
        let mut flags = Vec::new();

        if target.contains("musl") {
            flags.push("\"-C\"".to_string());
            flags.push("\"target-feature=+crt-static\"".to_string());
        }

        if target.contains("wasm") {
            flags.push("\"-C\"".to_string());
            flags.push("\"opt-level=z\"".to_string());
        }

        flags
    }

    fn find_latest_wasm(&self, project_path: &str) -> Result<String, String> {
        let target_dir = Path::new(project_path).join("target/wasm32-unknown-unknown/release");
        if !target_dir.exists() {
            let debug_dir = Path::new(project_path).join("target/wasm32-unknown-unknown/debug");
            if debug_dir.exists() {
                return self.find_wasm_in_dir(&debug_dir);
            }
            return Err("No wasm binary found. Build with: cargo build --target wasm32-unknown-unknown --release".to_string());
        }
        self.find_wasm_in_dir(&target_dir)
    }

    fn find_wasm_in_dir(&self, dir: &Path) -> Result<String, String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();
            if path.extension().map(|e| e == "wasm").unwrap_or(false) {
                return Ok(path.to_string_lossy().to_string());
            }
        }
        Err("No .wasm file found in target directory".to_string())
    }

    fn estimate_sections_from_size(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({"name": "code", "estimated_percent": 60}),
            serde_json::json!({"name": "data", "estimated_percent": 15}),
            serde_json::json!({"name": "type", "estimated_percent": 10}),
            serde_json::json!({"name": "other", "estimated_percent": 15}),
        ]
    }

    fn generate_wasm_opt_suggestions(&self, size_bytes: u64) -> Vec<String> {
        let mut suggestions = Vec::new();

        if size_bytes > 500_000 {
            suggestions.push(
                "Binary is large. Enable LTO and opt-level=\"z\" in [profile.release]".to_string(),
            );
            suggestions.push("Use `wasm-opt -Oz` from binaryen to further reduce size".to_string());
            suggestions.push("Consider using `wee_alloc` as global allocator".to_string());
        }

        if size_bytes > 200_000 {
            suggestions.push("Set `codegen-units = 1` for better optimization".to_string());
            suggestions.push("Enable `panic = \"abort\"` to reduce panic handler size".to_string());
        }

        suggestions
            .push("Use `wasm-pack` for optimized builds: wasm-pack build --release".to_string());
        suggestions.push("Consider `trunk` for web apps: trunk build --release".to_string());

        if suggestions.len() <= 1 {
            suggestions.insert(
                0,
                "Binary size looks good! Consider wasm-opt for further optimization.".to_string(),
            );
        }

        suggestions
    }
}

struct TargetInfo {
    target: String,
    description: String,
    is_installed: bool,
    category: String,
}

impl TargetInfo {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "target": self.target,
            "description": self.description,
            "installed": self.is_installed,
            "category": self.category,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = CrossCompileTool;
        assert_eq!(tool.name(), "cross_compile");
        assert!(tool.description().contains("Cross-compilation"));
        assert!(tool.description().contains("wasm"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action"));
        assert!(params.iter().any(|p| p.name == "target"));
        assert!(params.iter().any(|p| p.name == "platform"));
    }

    #[test]
    fn test_categorize_target_linux() {
        assert_eq!(CrossCompileTool::categorize_target("x86_64-unknown-linux-gnu"), "linux");
        assert_eq!(CrossCompileTool::categorize_target("aarch64-unknown-linux-musl"), "linux");
    }

    #[test]
    fn test_categorize_target_macos() {
        assert_eq!(CrossCompileTool::categorize_target("x86_64-apple-darwin"), "macos/ios");
        assert_eq!(CrossCompileTool::categorize_target("aarch64-apple-ios"), "macos/ios");
    }

    #[test]
    fn test_categorize_target_windows() {
        assert_eq!(CrossCompileTool::categorize_target("x86_64-pc-windows-msvc"), "windows");
    }

    #[test]
    fn test_categorize_target_wasm() {
        assert_eq!(CrossCompileTool::categorize_target("wasm32-unknown-unknown"), "wasm");
    }

    #[test]
    fn test_categorize_target_embedded() {
        assert_eq!(CrossCompileTool::categorize_target("thumbv7em-none-eabihf"), "embedded");
        assert_eq!(CrossCompileTool::categorize_target("riscv32imc-unknown-none-elf"), "embedded");
    }

    #[test]
    fn test_get_targets_for_platform_all() {
        let targets = CrossCompileTool::get_targets_for_platform("all");
        // Verify at least one target per category exists
        assert!(targets.contains_key("x86_64-unknown-linux-gnu"));
        assert!(targets.contains_key("x86_64-apple-darwin"));
        assert!(targets.contains_key("x86_64-pc-windows-msvc"));
        assert!(targets.contains_key("wasm32-unknown-unknown"));
        assert!(targets.contains_key("thumbv7em-none-eabihf"));
        assert!(targets.len() > 15);
    }

    #[test]
    fn test_get_targets_for_platform_linux_only() {
        let targets = CrossCompileTool::get_targets_for_platform("linux");
        assert!(targets.contains_key("x86_64-unknown-linux-gnu"));
        assert!(!targets.contains_key("x86_64-apple-darwin"));
        assert!(!targets.contains_key("wasm32-unknown-unknown"));
    }

    #[test]
    fn test_get_targets_for_platform_wasm_only() {
        let targets = CrossCompileTool::get_targets_for_platform("wasm");
        assert!(targets.contains_key("wasm32-unknown-unknown"));
        assert!(targets.contains_key("wasm32-wasi"));
        assert!(!targets.contains_key("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_get_linker_for_target() {
        assert_eq!(
            CrossCompileTool::get_linker_for_target("armv7-unknown-linux-gnueabihf"),
            Some("arm-linux-gnueabihf-gcc")
        );
        assert_eq!(
            CrossCompileTool::get_linker_for_target("aarch64-unknown-linux-gnu"),
            Some("aarch64-linux-gnu-gcc")
        );
        assert_eq!(
            CrossCompileTool::get_linker_for_target("x86_64-unknown-linux-musl"),
            Some("musl-gcc")
        );
        assert_eq!(
            CrossCompileTool::get_linker_for_target("x86_64-pc-windows-gnu"),
            Some("x86_64-w64-mingw32-gcc")
        );
        assert_eq!(CrossCompileTool::get_linker_for_target("wasm32-unknown-unknown"), None);
    }

    #[test]
    fn test_get_rustflags_for_musl() {
        let flags = CrossCompileTool::get_rustflags_for_target("x86_64-unknown-linux-musl");
        assert!(!flags.is_empty());
        assert!(flags.iter().any(|f| f.contains("crt-static")));
    }

    #[test]
    fn test_get_rustflags_for_wasm() {
        let flags = CrossCompileTool::get_rustflags_for_target("wasm32-unknown-unknown");
        assert!(!flags.is_empty());
        assert!(flags.iter().any(|f| f.contains("opt-level")));
    }

    #[test]
    fn test_get_rustflags_for_other() {
        let flags = CrossCompileTool::get_rustflags_for_target("x86_64-unknown-linux-gnu");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_generate_cargo_config_musl() {
        let tool = CrossCompileTool;
        let config = tool.generate_cargo_config("x86_64-unknown-linux-musl").unwrap();
        assert!(config.contains("x86_64-unknown-linux-musl"));
        assert!(config.contains("[build]"));
        assert!(config.contains("MUSL static linking"));
        assert!(config.contains("crt-static"));
    }

    #[test]
    fn test_generate_cargo_config_wasm() {
        let tool = CrossCompileTool;
        let config = tool.generate_cargo_config("wasm32-unknown-unknown").unwrap();
        assert!(config.contains("wasm32-unknown-unknown"));
        assert!(config.contains("Wasm optimization"));
        assert!(config.contains("opt-level = \"z\""));
        assert!(config.contains("lto = true"));
        assert!(config.contains("codegen-units = 1"));
        assert!(config.contains("strip = true"));
    }

    #[test]
    fn test_generate_cargo_config_generic() {
        let tool = CrossCompileTool;
        let config = tool.generate_cargo_config("aarch64-apple-darwin").unwrap();
        assert!(config.contains("aarch64-apple-darwin"));
        assert!(config.contains("[build]"));
        // Generic targets should not have musl/wasm specific sections
        assert!(!config.contains("MUSL static linking"));
        assert!(!config.contains("Wasm optimization"));
    }

    #[test]
    fn test_wasm_opt_suggestions_large() {
        let tool = CrossCompileTool;
        let suggestions = tool.generate_wasm_opt_suggestions(600_000);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.contains("LTO")));
        assert!(suggestions.iter().any(|s| s.contains("wasm-opt")));
        assert!(suggestions.iter().any(|s| s.contains("codegen-units")));
    }

    #[test]
    fn test_wasm_opt_suggestions_medium() {
        let tool = CrossCompileTool;
        let suggestions = tool.generate_wasm_opt_suggestions(300_000);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.contains("codegen-units")));
        assert!(suggestions.iter().any(|s| s.contains("panic")));
    }

    #[test]
    fn test_wasm_opt_suggestions_small() {
        let tool = CrossCompileTool;
        let suggestions = tool.generate_wasm_opt_suggestions(50_000);
        assert!(!suggestions.is_empty());
        // Small binaries get default suggestions (wasm-pack, trunk) plus the "looks good" hint
        assert!(suggestions.iter().any(|s| s.contains("wasm-pack") || s.contains("trunk") || s.contains("looks good")));
    }

    #[test]
    fn test_target_info_to_json() {
        let info = TargetInfo {
            target: "wasm32-unknown-unknown".to_string(),
            description: "Wasm".to_string(),
            is_installed: true,
            category: "wasm".to_string(),
        };
        let json = info.to_json();
        assert_eq!(json["target"], "wasm32-unknown-unknown");
        assert_eq!(json["description"], "Wasm");
        assert_eq!(json["installed"], true);
        assert_eq!(json["category"], "wasm");
    }

    #[test]
    fn test_estimate_sections() {
        let tool = CrossCompileTool;
        let sections = tool.estimate_sections_from_size();
        assert_eq!(sections.len(), 4);
        let total_percent: u32 = sections.iter().map(|s| s["estimated_percent"].as_u64().unwrap() as u32).sum();
        assert_eq!(total_percent, 100);
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = CrossCompileTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("unknown")),
        ]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_list_targets_action() {
        let tool = CrossCompileTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("list_targets")),
            ("platform".to_string(), serde_json::json!("wasm")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "list_targets");
        assert_eq!(result["platform"], "wasm");
        assert!(result["total_count"].as_u64().unwrap() >= 2);
    }

    #[tokio::test]
    async fn test_config_missing_target() {
        let tool = CrossCompileTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("config")),
        ]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("target parameter is required"));
    }
}
