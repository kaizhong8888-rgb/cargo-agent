//! Fuzzing driver generator and manager for Rust projects.
//!
//! Generates cargo-fuzz targets, manages corpus, parses crash reports,
//! and recommends fuzzing strategies.
//!
//! Actions: generate_target, list_targets, run, corpus, parse_crash, strategies, init

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(FuzzDriverTool));
}

struct FuzzDriverTool;

#[async_trait::async_trait]
impl Tool for FuzzDriverTool {
    fn name(&self) -> &str {
        "fuzz_driver"
    }

    fn description(&self) -> &str {
        "Fuzzing driver tool for Rust projects. Actions: generate_target (create cargo-fuzz target), \
         list_targets (list existing fuzz targets), run (generate fuzz run command), corpus (manage corpus), \
         parse_crash (parse crash report), strategies (recommend fuzzing strategies), init (initialize \
         cargo-fuzz in project)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: generate_target, list_targets, run, corpus, parse_crash, strategies, init".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the Rust project directory (default: current directory)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "target_name".to_string(),
                parameter_type: "string".to_string(),
                description: "Fuzz target name (e.g. 'fuzz_parse', 'fuzz_deserialize')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "function".to_string(),
                parameter_type: "string".to_string(),
                description: "Function to fuzz (e.g. 'my_crate::parse', 'my_crate::deserialize')".to_string(),
                required: false,
            },
            ToolParameter {
                name: "input_type".to_string(),
                parameter_type: "string".to_string(),
                description: "Input data type: bytes (default), string, json, protobuf, structured".to_string(),
                required: false,
            },
            ToolParameter {
                name: "max_len".to_string(),
                parameter_type: "number".to_string(),
                description: "Maximum input length for fuzzer (default: 4096)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "jobs".to_string(),
                parameter_type: "number".to_string(),
                description: "Number of parallel fuzzing jobs (default: 1)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "timeout".to_string(),
                parameter_type: "number".to_string(),
                description: "Timeout in seconds for fuzzing session (default: 3600)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "corpus_dir".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to corpus directory (default: fuzz/corpus/<target>)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "crash_input".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to crash input file for parse_crash action".to_string(),
                required: false,
            },
            ToolParameter {
                name: "crate_name".to_string(),
                parameter_type: "string".to_string(),
                description: "Crate name for fuzz target (default: auto-detect from Cargo.toml)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let project_path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        match action {
            "generate_target" => self.generate_target(project_path, params),
            "list_targets" => self.list_targets(project_path),
            "run" => Self::run_fuzz(params),
            "corpus" => self.manage_corpus(project_path, params),
            "parse_crash" => Self::parse_crash(params),
            "strategies" => self.recommend_strategies(project_path, params),
            "init" => self.init_fuzz(project_path),
            _ => Err(format!(
                "Unknown action: {action}. Valid: generate_target, list_targets, run, corpus, parse_crash, strategies, init"
            )),
        }
    }
}

impl FuzzDriverTool {
    fn generate_target(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let crate_name_opt: Option<String> = params
            .get("crate_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Self::detect_crate_name(project_path));
        let crate_name = crate_name_opt
            .ok_or("Could not detect crate name. Specify with --crate_name".to_string())?;

        let target_name = params
            .get("target_name")
            .and_then(|v| v.as_str())
            .unwrap_or("fuzz_target_1");

        let function = params.get("function").and_then(|v| v.as_str());
        let input_type = params
            .get("input_type")
            .and_then(|v| v.as_str())
            .unwrap_or("bytes");

        let max_len = params
            .get("max_len")
            .and_then(|v| v.as_u64())
            .unwrap_or(4096);

        let fuzz_dir = Path::new(project_path).join("fuzz");
        let fuzz_targets_dir = fuzz_dir.join("fuzz_targets");

        fs::create_dir_all(&fuzz_targets_dir)
            .map_err(|e| format!("Failed to create fuzz directory: {e}"))?;

        let fuzz_cargo = fuzz_dir.join("Cargo.toml");
        if !fuzz_cargo.exists() {
            let cargo_content = Self::generate_fuzz_cargo(&crate_name);
            fs::write(&fuzz_cargo, cargo_content)
                .map_err(|e| format!("Failed to write fuzz Cargo.toml: {e}"))?;
        } else {
            Self::add_fuzz_dependency(&fuzz_cargo, &crate_name)?;
        }

        let target_file = fuzz_targets_dir.join(format!("{target_name}.rs"));
        let target_source = Self::generate_fuzz_source(&crate_name, function, input_type, max_len);
        fs::write(&target_file, target_source)
            .map_err(|e| format!("Failed to write fuzz target: {e}"))?;

        let corpus_dir = fuzz_dir.join("corpus").join(target_name);
        fs::create_dir_all(&corpus_dir)
            .map_err(|e| format!("Failed to create corpus directory: {e}"))?;

        Ok(serde_json::json!({
            "action": "generate_target",
            "target_name": target_name,
            "target_file": target_file.display().to_string(),
            "fuzz_dir": fuzz_dir.display().to_string(),
            "corpus_dir": corpus_dir.display().to_string(),
            "input_type": input_type,
            "max_len": max_len,
            "run_command": format!("cargo +nightly fuzz run {target_name}"),
        }))
    }

    fn generate_fuzz_cargo(crate_name: &str) -> String {
        format!(
            r#"[package]
name = "{crate_name}-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"

[dependencies.{crate_name}]
path = ".."

[[bin]]
name = "fuzz_target_1"
path = "fuzz_targets/fuzz_target_1.rs"
test = false
doc = false
"#
        )
    }

    fn add_fuzz_dependency(fuzz_cargo: &Path, crate_name: &str) -> Result<(), String> {
        let content = fs::read_to_string(fuzz_cargo)
            .map_err(|e| format!("Failed to read fuzz Cargo.toml: {e}"))?;

        if content.contains(&format!("[dependencies.{crate_name}]")) {
            return Ok(());
        }

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut insert_idx = lines.len();

        for (i, line) in lines.iter().enumerate().rev() {
            if line.trim().starts_with("[[bin]]") {
                insert_idx = i;
                break;
            }
        }

        let dep_section = format!("\n[dependencies.{crate_name}]\npath = \"..\"\n");

        if !content.contains("libfuzzer-sys") {
            let deps_idx = lines.iter().position(|l| l.trim() == "[dependencies]");
            if let Some(idx) = deps_idx {
                lines.insert(idx + 1, "libfuzzer-sys = \"0.4\"".to_string());
            }
        }

        lines.insert(insert_idx, dep_section);

        fs::write(fuzz_cargo, lines.join("\n"))
            .map_err(|e| format!("Failed to update fuzz Cargo.toml: {e}"))?;

        Ok(())
    }

    fn generate_fuzz_source(
        crate_name: &str,
        function: Option<&str>,
        input_type: &str,
        max_len: u64,
    ) -> String {
        let mut source = String::new();
        source.push_str("#![no_main]\n");
        source.push_str("use libfuzzer_sys::fuzz_target;\n");
        source.push_str(&format!("use {crate_name};\n\n"));

        match input_type {
            "string" => {
                source.push_str(&format!(
                    r#"fuzz_target!(|data: &[u8]| {{
    // Fuzz with UTF-8 string input
    if let Ok(s) = std::str::from_utf8(data) {{
        if s.len() <= {max_len} {{
"#
                ));
                if let Some(func) = function {
                    source.push_str(&format!("            let _ = {func}(s);\n"));
                } else {
                    source.push_str("            // TODO: Call your function here\n");
                    source.push_str("            // Example: my_crate::parse(s);\n");
                }
                source.push_str("        }\n    }\n});\n");
            }
            "json" => {
                source.push_str(
                    r#"fuzz_target!(|data: &[u8]| {
    // Fuzz with JSON input
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(s) {
"#,
                );
                if let Some(func) = function {
                    source.push_str(&format!("            let _ = {func}(json);\n"));
                } else {
                    source.push_str("            // TODO: Call your function here\n");
                    source.push_str("            // Example: my_crate::handle_json(json);\n");
                }
                source.push_str("        }\n    }\n});\n");
            }
            "structured" => {
                source.push_str("fuzz_target!(|data: &[u8]| {\n");
                source.push_str("    // Fuzz with structured input using arbitrary crate\n");
                source.push_str("    // Add `arbitrary = { version = \"1\", features = [\"derive\"] }` to dependencies\n");
                source.push_str("    use arbitrary::{Arbitrary, Unstructured};\n\n");
                source.push_str("    if let Ok(mut u) = Unstructured::new(data) {\n");
                source.push_str("        // #[derive(Arbitrary, Debug)]\n");
                source.push_str("        // struct MyInput { ... }\n");
                if let Some(func) = function {
                    source.push_str(&format!("        // let _ = {func}(input);\n"));
                } else {
                    source.push_str("        // TODO: Define structured input and call function\n");
                }
                source.push_str("    }\n});\n");
            }
            _ => {
                source.push_str(&format!(
                    r#"fuzz_target!(|data: &[u8]| {{
    // Fuzz with raw bytes input
    if data.len() <= {max_len} {{
"#
                ));
                if let Some(func) = function {
                    source.push_str(&format!("        let _ = {func}(data);\n"));
                } else {
                    source.push_str("        // TODO: Call your function here\n");
                    source.push_str("        // Example: my_crate::parse(data);\n");
                }
                source.push_str("    }\n});\n");
            }
        }

        source
    }

    fn list_targets(&self, project_path: &str) -> Result<Value, String> {
        let fuzz_targets_dir = Path::new(project_path).join("fuzz/fuzz_targets");

        if !fuzz_targets_dir.exists() {
            return Ok(serde_json::json!({
                "action": "list_targets",
                "targets": [],
                "message": "No fuzz directory found. Use 'init' or 'generate_target' first.",
            }));
        }

        let mut targets: Vec<FuzzTargetInfo> = Vec::new();

        for entry in fs::read_dir(&fuzz_targets_dir)
            .map_err(|e| format!("Failed to read fuzz_targets dir: {e}"))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();

            if path.extension().map(|e| e == "rs").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let corpus_dir = Path::new(project_path).join(format!("fuzz/corpus/{name}"));
                let corpus_count = if corpus_dir.exists() {
                    fs::read_dir(&corpus_dir)
                        .map(|entries| entries.count())
                        .unwrap_or(0)
                } else {
                    0
                };

                let crashes_dir = Path::new(project_path).join(format!("fuzz/crashes/{name}"));
                let crash_count = if crashes_dir.exists() {
                    fs::read_dir(&crashes_dir)
                        .map(|entries| entries.count())
                        .unwrap_or(0)
                } else {
                    0
                };

                targets.push(FuzzTargetInfo {
                    name,
                    file: path.to_string_lossy().to_string(),
                    corpus_count,
                    crash_count,
                });
            }
        }

        targets.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(serde_json::json!({
            "action": "list_targets",
            "targets": targets.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
            "total_targets": targets.len(),
        }))
    }

    fn run_fuzz(params: &HashMap<String, Value>) -> Result<Value, String> {
        let target_name = params
            .get("target_name")
            .and_then(|v| v.as_str())
            .ok_or("target_name is required for run action".to_string())?;

        let jobs = params.get("jobs").and_then(|v| v.as_u64()).unwrap_or(1);
        let timeout = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);
        let max_len = params
            .get("max_len")
            .and_then(|v| v.as_u64())
            .unwrap_or(4096);

        let mut args = vec![
            "+nightly".to_string(),
            "fuzz".to_string(),
            "run".to_string(),
            target_name.to_string(),
        ];

        if jobs > 1 {
            args.push("-j".to_string());
            args.push(jobs.to_string());
        }

        args.push("--".to_string());
        args.push(format!("-max_len={max_len}"));
        args.push(format!("-timeout={}", timeout / jobs.max(1)));

        let command_str = format!("cargo {}", args.join(" "));

        Ok(serde_json::json!({
            "action": "run",
            "target": target_name,
            "command": command_str,
            "jobs": jobs,
            "timeout_secs": timeout,
            "max_len": max_len,
            "corpus_dir": format!("fuzz/corpus/{target_name}"),
            "note": "Requires nightly toolchain and cargo-fuzz installed: cargo install cargo-fuzz",
        }))
    }

    fn manage_corpus(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let target_name = params
            .get("target_name")
            .and_then(|v| v.as_str())
            .ok_or("target_name is required for corpus action".to_string())?;

        let default_corpus_dir = format!("{project_path}/fuzz/corpus/{target_name}");
        let corpus_dir = params
            .get("corpus_dir")
            .and_then(|v| v.as_str())
            .unwrap_or(&default_corpus_dir);

        let corpus_path = Path::new(corpus_dir);
        let mut files: Vec<CorpusFile> = Vec::new();
        let mut total_size = 0u64;

        if corpus_path.exists() {
            for entry in
                fs::read_dir(corpus_path).map_err(|e| format!("Failed to read corpus dir: {e}"))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
                let path = entry.path();
                if path.is_file() {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    total_size += size;
                    files.push(CorpusFile {
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size,
                    });
                }
            }
        }

        files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(serde_json::json!({
            "action": "corpus",
            "target": target_name,
            "corpus_dir": corpus_dir,
            "file_count": files.len(),
            "total_size_bytes": total_size,
            "files": files.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
        }))
    }

    fn parse_crash(params: &HashMap<String, Value>) -> Result<Value, String> {
        let crash_input = params
            .get("crash_input")
            .and_then(|v| v.as_str())
            .ok_or("crash_input is required for parse_crash action".to_string())?;

        if !Path::new(crash_input).exists() {
            return Err(format!("Crash input file not found: {crash_input}"));
        }

        let crash_data =
            fs::read(crash_input).map_err(|e| format!("Failed to read crash file: {e}"))?;

        let file_size = crash_data.len();
        let as_string = String::from_utf8_lossy(&crash_data);
        let is_valid_utf8 = String::from_utf8(crash_data.clone()).is_ok();

        let analysis = Self::analyze_crash_input(&crash_data);
        let target_name = Self::detect_target_from_crash_path(crash_input);

        Ok(serde_json::json!({
            "action": "parse_crash",
            "crash_file": crash_input,
            "file_size": file_size,
            "is_valid_utf8": is_valid_utf8,
            "preview": if file_size <= 100 {
                format!("{as_string:?}")
            } else {
                format!("{as_string:?}... ({} bytes total)", file_size)
            },
            "analysis": analysis,
            "reproduction_command": format!("cargo +nightly fuzz run {target_name} {crash_input}"),
            "minimize_command": format!("cargo +nightly fuzz minimize {target_name} {crash_input}"),
        }))
    }

    fn analyze_crash_input(data: &[u8]) -> serde_json::Value {
        let mut analysis = serde_json::Map::new();

        analysis.insert("length".to_string(), Value::Number(data.len().into()));

        let null_count = data.iter().filter(|&&b| b == 0).count();
        analysis.insert("null_bytes".to_string(), Value::Number(null_count.into()));

        let printable = data.iter().filter(|&&b| (32..=126).contains(&b)).count();
        analysis.insert(
            "printable_ascii_percent".to_string(),
            Value::Number(((printable * 100 / data.len().max(1)) as u64).into()),
        );

        if data.len() > 4 {
            let mut repeated = 0;
            for i in 0..data.len() - 1 {
                if data[i] == data[i + 1] {
                    repeated += 1;
                }
            }
            analysis.insert(
                "repeated_adjacent_bytes".to_string(),
                Value::Number(repeated.into()),
            );
        }

        if data.is_empty() {
            analysis.insert(
                "suggested_type".to_string(),
                Value::String("empty_input".to_string()),
            );
        } else if String::from_utf8(data.to_vec()).is_ok() {
            analysis.insert(
                "suggested_type".to_string(),
                Value::String("utf8_string".to_string()),
            );
        } else {
            analysis.insert(
                "suggested_type".to_string(),
                Value::String("binary".to_string()),
            );
        }

        Value::Object(analysis)
    }

    fn detect_target_from_crash_path(path: &str) -> String {
        if let Some(pos) = path.find("crashes/") {
            let after = &path[pos + 8..];
            if let Some(end) = after.find('/') {
                return after[..end].to_string();
            }
        }
        "fuzz_target_1".to_string()
    }

    fn recommend_strategies(
        &self,
        project_path: &str,
        params: &HashMap<String, Value>,
    ) -> Result<Value, String> {
        let crate_name_opt: Option<String> = params
            .get("crate_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Self::detect_crate_name(project_path));
        let crate_name = crate_name_opt;

        let src_path = Path::new(project_path).join("src");
        let mut fuzzable_functions: Vec<FuzzableFunc> = Vec::new();

        if src_path.exists() {
            self.scan_for_fuzzable_functions(&src_path, &mut fuzzable_functions)?;
        }

        let strategies = Self::generate_strategy_recommendations(&fuzzable_functions);

        Ok(serde_json::json!({
            "action": "strategies",
            "crate_name": crate_name,
            "fuzzable_functions_found": fuzzable_functions.len(),
            "functions": fuzzable_functions.iter().map(|f| f.to_json()).collect::<Vec<_>>(),
            "recommended_strategies": strategies,
            "setup_commands": [
                "rustup default nightly",
                "cargo install cargo-fuzz",
                "cargo fuzz init (or use generate_target action)",
            ],
        }))
    }

    fn init_fuzz(&self, project_path: &str) -> Result<Value, String> {
        let crate_name = Self::detect_crate_name(project_path)
            .ok_or("Could not detect crate name from Cargo.toml".to_string())?;

        let fuzz_dir = Path::new(project_path).join("fuzz");
        if fuzz_dir.exists() {
            return Err(
                "Fuzz directory already exists. Use generate_target to add new targets."
                    .to_string(),
            );
        }

        fs::create_dir_all(fuzz_dir.join("fuzz_targets"))
            .map_err(|e| format!("Failed to create fuzz directory: {e}"))?;
        fs::create_dir_all(fuzz_dir.join("corpus"))
            .map_err(|e| format!("Failed to create corpus directory: {e}"))?;

        let cargo_content = Self::generate_fuzz_cargo(&crate_name);
        fs::write(fuzz_dir.join("Cargo.toml"), cargo_content)
            .map_err(|e| format!("Failed to write fuzz Cargo.toml: {e}"))?;

        let default_target = Self::generate_fuzz_source(&crate_name, None, "bytes", 4096);
        fs::write(
            fuzz_dir.join("fuzz_targets/fuzz_target_1.rs"),
            default_target,
        )
        .map_err(|e| format!("Failed to write default fuzz target: {e}"))?;

        let workspace_toml = Path::new(project_path).join("Cargo.toml");
        if workspace_toml.exists() {
            let content = fs::read_to_string(&workspace_toml).unwrap_or_default();
            if content.contains("[workspace]") {
                let _ = Self::add_fuzz_to_workspace(&workspace_toml);
            }
        }

        Ok(serde_json::json!({
            "action": "init",
            "crate_name": crate_name,
            "fuzz_dir": fuzz_dir.display().to_string(),
            "default_target": "fuzz_target_1",
            "next_steps": [
                "Edit fuzz/fuzz_targets/fuzz_target_1.rs to call your function",
                "cargo +nightly fuzz run fuzz_target_1",
                "Add more targets with generate_target action",
            ],
        }))
    }

    fn add_fuzz_to_workspace(workspace_toml: &Path) -> Result<(), String> {
        let content = fs::read_to_string(workspace_toml)
            .map_err(|e| format!("Failed to read Cargo.toml: {e}"))?;

        if content.contains("\"fuzz\"") {
            return Ok(());
        }

        if let Some(pos) = content.find("members") {
            let after = &content[pos..];
            if let Some(bracket_pos) = after.find('[') {
                let insert_pos = pos + bracket_pos + 1;
                let mut new_content = content.clone();
                new_content.insert_str(insert_pos, "\n    \"fuzz\",");
                fs::write(workspace_toml, new_content)
                    .map_err(|e| format!("Failed to update workspace Cargo.toml: {e}"))?;
            }
        }

        Ok(())
    }

    fn detect_crate_name(project_path: &str) -> Option<String> {
        let cargo_path = Path::new(project_path).join("Cargo.toml");
        if !cargo_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&cargo_path).ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("name") && trimmed.contains('=') {
                return Some(
                    trimmed
                        .split('=')
                        .nth(1)?
                        .trim()
                        .trim_matches('"')
                        .to_string(),
                );
            }
        }
        None
    }

    fn scan_for_fuzzable_functions(
        &self,
        dir: &Path,
        functions: &mut Vec<FuzzableFunc>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read dir: {e}"))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_for_fuzzable_functions(&path, functions)?;
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                let content = fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
                self.extract_fuzzable_functions(&content, &path, functions);
            }
        }
        Ok(())
    }

    fn extract_fuzzable_functions(
        &self,
        content: &str,
        file: &Path,
        functions: &mut Vec<FuzzableFunc>,
    ) {
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if (trimmed.starts_with("pub fn") || trimmed.starts_with("pub async fn"))
                && !trimmed.contains("fn main")
                && (trimmed.contains("(&str")
                    || trimmed.contains("&[u8]")
                    || trimmed.contains("String")
                    || trimmed.contains("Vec<u8>"))
            {
                let func_name = trimmed
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or("unknown")
                    .split('(')
                    .next()
                    .unwrap_or("unknown")
                    .to_string();

                let param_info = if let Some(start) = trimmed.find('(') {
                    if let Some(end) = trimmed.find(')') {
                        trimmed[start..=end].to_string()
                    } else {
                        "(...)".to_string()
                    }
                } else {
                    "(...)".to_string()
                };

                functions.push(FuzzableFunc {
                    name: func_name,
                    file: file.to_string_lossy().to_string(),
                    line: i + 1,
                    param_signature: param_info,
                    fuzz_priority: Self::assess_fuzz_priority(trimmed),
                });
            }

            if trimmed.contains("fn parse")
                || trimmed.contains("fn decode")
                || trimmed.contains("fn deserialize")
                || trimmed.contains("fn from_str")
                || trimmed.contains("fn from_bytes")
                || trimmed.contains("fn from_slice")
            {
                let func_name = trimmed
                    .split_whitespace()
                    .find(|w| w.starts_with("fn"))
                    .and_then(|w| w.split('(').next())
                    .map(|s| s.trim_start_matches("fn ").to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                if !functions.iter().any(|f| f.name == func_name) {
                    functions.push(FuzzableFunc {
                        name: func_name,
                        file: file.to_string_lossy().to_string(),
                        line: i + 1,
                        param_signature: "(...)".to_string(),
                        fuzz_priority: "high".to_string(),
                    });
                }
            }
        }
    }

    fn assess_fuzz_priority(line: &str) -> String {
        let lower = line.to_lowercase();
        if lower.contains("parse")
            || lower.contains("decode")
            || lower.contains("deserialize")
            || lower.contains("validate")
        {
            "high".to_string()
        } else if lower.contains("process") || lower.contains("handle") || lower.contains("convert")
        {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }

    fn generate_strategy_recommendations(functions: &[FuzzableFunc]) -> Vec<String> {
        let mut strategies = Vec::new();

        let high_priority: Vec<&FuzzableFunc> = functions
            .iter()
            .filter(|f| f.fuzz_priority == "high")
            .collect();

        if !high_priority.is_empty() {
            strategies.push(format!(
                "Found {} high-priority fuzz targets (parse/decode/deserialize functions). Start with these.",
                high_priority.len()
            ));
        }

        if functions
            .iter()
            .any(|f| f.param_signature.contains("&str") || f.param_signature.contains("String"))
        {
            strategies.push(
                "String/UTF-8 input: Use libfuzzer's built-in string mutation strategies."
                    .to_string(),
            );
        }

        if functions
            .iter()
            .any(|f| f.param_signature.contains("&[u8]") || f.param_signature.contains("Vec<u8>"))
        {
            strategies.push(
                "Binary input: Consider using honggfuzz for better binary mutation coverage."
                    .to_string(),
            );
        }

        if functions.len() > 5 {
            strategies.push("Multiple targets: Run fuzzing in parallel with different seeds for better coverage.".to_string());
        }

        strategies
            .push("Use `-dict` with a custom dictionary for domain-specific inputs.".to_string());
        strategies
            .push("Seed corpus with known-good inputs for faster coverage discovery.".to_string());
        strategies.push(
            "Run with ASAN: RUSTFLAGS=\"-Zsanitizer=address\" cargo +nightly fuzz run <target>"
                .to_string(),
        );
        strategies.push(
            "Minimize crash inputs: cargo +nightly fuzz minimize <target> <crash-file>".to_string(),
        );

        if strategies.len() <= 2 {
            strategies.push(
                "Add more fuzzable functions by implementing parsers, decoders, or validators."
                    .to_string(),
            );
        }

        strategies
    }
}

struct FuzzTargetInfo {
    name: String,
    file: String,
    corpus_count: usize,
    crash_count: usize,
}

impl FuzzTargetInfo {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "file": self.file,
            "corpus_entries": self.corpus_count,
            "crash_count": self.crash_count,
        })
    }
}

struct CorpusFile {
    name: String,
    size: u64,
}

impl CorpusFile {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "size_bytes": self.size,
        })
    }
}

struct FuzzableFunc {
    name: String,
    file: String,
    line: usize,
    param_signature: String,
    fuzz_priority: String,
}

impl FuzzableFunc {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "file": self.file,
            "line": self.line,
            "signature": self.param_signature,
            "priority": self.fuzz_priority,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_metadata() {
        let tool = FuzzDriverTool;
        assert_eq!(tool.name(), "fuzz_driver");
        assert!(tool.description().contains("Fuzzing"));
        let params = tool.parameters();
        assert!(params.iter().any(|p| p.name == "action"));
        assert!(params.iter().any(|p| p.name == "target_name"));
        assert!(params.iter().any(|p| p.name == "input_type"));
        assert!(params.iter().any(|p| p.name == "crate_name"));
    }

    #[test]
    fn test_generate_fuzz_cargo() {
        let cargo = FuzzDriverTool::generate_fuzz_cargo("my_crate");
        assert!(cargo.contains("my_crate-fuzz"));
        assert!(cargo.contains("libfuzzer-sys"));
        assert!(cargo.contains("[dependencies.my_crate]"));
        assert!(cargo.contains("path = \"..\""));
        assert!(cargo.contains("edition = \"2021\""));
    }

    #[test]
    fn test_generate_fuzz_source_bytes() {
        let source = FuzzDriverTool::generate_fuzz_source("my_crate", None, "bytes", 4096);
        assert!(source.contains("#![no_main]"));
        assert!(source.contains("use libfuzzer_sys::fuzz_target"));
        assert!(source.contains("use my_crate"));
        assert!(source.contains("raw bytes input"));
        assert!(source.contains("data.len() <= 4096"));
    }

    #[test]
    fn test_generate_fuzz_source_bytes_with_function() {
        let source = FuzzDriverTool::generate_fuzz_source(
            "my_crate",
            Some("my_crate::parse"),
            "bytes",
            1024,
        );
        assert!(source.contains("let _ = my_crate::parse(data)"));
        assert!(source.contains("data.len() <= 1024"));
    }

    #[test]
    fn test_generate_fuzz_source_string() {
        let source = FuzzDriverTool::generate_fuzz_source("my_crate", None, "string", 2048);
        assert!(source.contains("UTF-8 string input"));
        assert!(source.contains("std::str::from_utf8"));
        assert!(source.contains("s.len() <= 2048"));
    }

    #[test]
    fn test_generate_fuzz_source_json() {
        let source = FuzzDriverTool::generate_fuzz_source("my_crate", None, "json", 4096);
        assert!(source.contains("JSON input"));
        assert!(source.contains("serde_json::from_str"));
    }

    #[test]
    fn test_generate_fuzz_source_structured() {
        let source = FuzzDriverTool::generate_fuzz_source("my_crate", None, "structured", 4096);
        assert!(source.contains("structured input"));
        assert!(source.contains("arbitrary"));
        assert!(source.contains("Unstructured"));
    }

    #[test]
    fn test_assess_fuzz_priority_high() {
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn parse(data: &[u8])"),
            "high"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn decode(bytes: Vec<u8>)"),
            "high"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn deserialize(s: &str)"),
            "high"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn validate(input: String)"),
            "high"
        );
    }

    #[test]
    fn test_assess_fuzz_priority_medium() {
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn process(data: &[u8])"),
            "medium"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn handle(input: String)"),
            "medium"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn convert(s: &str)"),
            "medium"
        );
    }

    #[test]
    fn test_assess_fuzz_priority_low() {
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn hello_world()"),
            "low"
        );
        assert_eq!(
            FuzzDriverTool::assess_fuzz_priority("pub fn add(a: i32, b: i32)"),
            "low"
        );
    }

    #[test]
    fn test_generate_strategy_recommendations_empty() {
        let strategies = FuzzDriverTool::generate_strategy_recommendations(&[]);
        assert!(!strategies.is_empty());
        assert!(strategies.iter().any(|s| s.contains("dictionary")));
        assert!(strategies.iter().any(|s| s.contains("ASAN")));
    }

    #[test]
    fn test_generate_strategy_recommendations_with_functions() {
        let funcs = vec![
            FuzzableFunc {
                name: "parse".to_string(),
                file: "src/lib.rs".to_string(),
                line: 10,
                param_signature: "(data: &[u8])".to_string(),
                fuzz_priority: "high".to_string(),
            },
            FuzzableFunc {
                name: "decode_string".to_string(),
                file: "src/lib.rs".to_string(),
                line: 20,
                param_signature: "(s: &str)".to_string(),
                fuzz_priority: "high".to_string(),
            },
        ];
        let strategies = FuzzDriverTool::generate_strategy_recommendations(&funcs);
        assert!(strategies.iter().any(|s| s.contains("high-priority")));
        assert!(strategies.iter().any(|s| s.contains("String/UTF-8")));
        assert!(strategies.iter().any(|s| s.contains("Binary input")));
    }

    #[test]
    fn test_detect_target_from_crash_path() {
        assert_eq!(
            FuzzDriverTool::detect_target_from_crash_path(
                "/path/to/fuzz/crashes/fuzz_parse/crash-123"
            ),
            "fuzz_parse"
        );
        assert_eq!(
            FuzzDriverTool::detect_target_from_crash_path("/some/crashes/my_target/other"),
            "my_target"
        );
        // Without crashes/ in path, should return default
        assert_eq!(
            FuzzDriverTool::detect_target_from_crash_path("/some/other/path"),
            "fuzz_target_1"
        );
    }

    #[test]
    fn test_analyze_crash_input_empty() {
        let analysis = FuzzDriverTool::analyze_crash_input(&[]);
        assert_eq!(analysis["length"].as_u64().unwrap(), 0);
        assert_eq!(analysis["suggested_type"].as_str().unwrap(), "empty_input");
    }

    #[test]
    fn test_analyze_crash_input_utf8() {
        let data = b"hello world";
        let analysis = FuzzDriverTool::analyze_crash_input(data);
        assert_eq!(analysis["length"].as_u64().unwrap(), 11);
        assert_eq!(analysis["suggested_type"].as_str().unwrap(), "utf8_string");
        assert_eq!(analysis["null_bytes"].as_u64().unwrap(), 0);
        assert_eq!(analysis["printable_ascii_percent"].as_u64().unwrap(), 100);
    }

    #[test]
    fn test_analyze_crash_input_binary() {
        let data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01, 0x02, 0x03];
        let analysis = FuzzDriverTool::analyze_crash_input(&data);
        assert_eq!(analysis["length"].as_u64().unwrap(), 6);
        assert_eq!(analysis["null_bytes"].as_u64().unwrap(), 1);
        assert_eq!(analysis["suggested_type"].as_str().unwrap(), "binary");
    }

    #[test]
    fn test_analyze_crash_input_repeated_bytes() {
        let data: Vec<u8> = vec![0xAA, 0xAA, 0xAA, 0xAA, 0xAA];
        let analysis = FuzzDriverTool::analyze_crash_input(&data);
        assert_eq!(analysis["repeated_adjacent_bytes"].as_u64().unwrap(), 4);
    }

    #[test]
    fn test_run_fuzz_basic() {
        let mut params = HashMap::new();
        params.insert("target_name".to_string(), serde_json::json!("fuzz_parse"));
        let result = FuzzDriverTool::run_fuzz(&params).unwrap();
        assert_eq!(result["target"], "fuzz_parse");
        assert!(result["command"].as_str().unwrap().contains("+nightly"));
        assert!(result["command"].as_str().unwrap().contains("fuzz run"));
        assert_eq!(result["jobs"].as_u64().unwrap(), 1);
        assert_eq!(result["max_len"].as_u64().unwrap(), 4096);
    }

    #[test]
    fn test_run_fuzz_with_options() {
        let mut params = HashMap::new();
        params.insert("target_name".to_string(), serde_json::json!("fuzz_decode"));
        params.insert("jobs".to_string(), serde_json::json!(4));
        params.insert("timeout".to_string(), serde_json::json!(7200));
        params.insert("max_len".to_string(), serde_json::json!(8192));
        let result = FuzzDriverTool::run_fuzz(&params).unwrap();
        assert!(result["command"].as_str().unwrap().contains("-j 4"));
        assert!(result["command"]
            .as_str()
            .unwrap()
            .contains("-max_len=8192"));
        assert_eq!(result["timeout_secs"].as_u64().unwrap(), 7200);
    }

    #[test]
    fn test_run_fuzz_missing_target() {
        let params = HashMap::new();
        let result = FuzzDriverTool::run_fuzz(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("target_name is required"));
    }

    #[test]
    fn test_fuzz_target_info_to_json() {
        let info = FuzzTargetInfo {
            name: "fuzz_parse".to_string(),
            file: "/path/to/fuzz_parse.rs".to_string(),
            corpus_count: 42,
            crash_count: 3,
        };
        let json = info.to_json();
        assert_eq!(json["name"], "fuzz_parse");
        assert_eq!(json["corpus_entries"], 42);
        assert_eq!(json["crash_count"], 3);
    }

    #[test]
    fn test_corpus_file_to_json() {
        let file = CorpusFile {
            name: "abc123".to_string(),
            size: 1234,
        };
        let json = file.to_json();
        assert_eq!(json["name"], "abc123");
        assert_eq!(json["size_bytes"], 1234);
    }

    #[test]
    fn test_fuzzable_func_to_json() {
        let func = FuzzableFunc {
            name: "parse_json".to_string(),
            file: "src/parser.rs".to_string(),
            line: 42,
            param_signature: "(data: &str)".to_string(),
            fuzz_priority: "high".to_string(),
        };
        let json = func.to_json();
        assert_eq!(json["name"], "parse_json");
        assert_eq!(json["line"], 42);
        assert_eq!(json["priority"], "high");
    }

    #[test]
    fn test_detect_crate_name() {
        let name = FuzzDriverTool::detect_crate_name(".");
        assert!(name.is_some());
        assert_eq!(name.unwrap(), "cargo-agent");
    }

    #[test]
    fn test_detect_crate_name_nonexistent() {
        let name = FuzzDriverTool::detect_crate_name("/nonexistent/path");
        assert!(name.is_none());
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = FuzzDriverTool;
        let params = HashMap::from([("action".to_string(), serde_json::json!("unknown_action"))]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action"));
    }

    #[tokio::test]
    async fn test_run_action() {
        let tool = FuzzDriverTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("run")),
            ("target_name".to_string(), serde_json::json!("fuzz_test")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "run");
        assert_eq!(result["target"], "fuzz_test");
    }

    #[tokio::test]
    async fn test_list_targets_no_dir() {
        let tool = FuzzDriverTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("list_targets")),
            (
                "path".to_string(),
                serde_json::json!("/tmp/nonexistent_fuzz_29"),
            ),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["action"], "list_targets");
        assert!(result["targets"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_corpus_missing_target() {
        let tool = FuzzDriverTool;
        let params = HashMap::from([("action".to_string(), serde_json::json!("corpus"))]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_crash_missing_file() {
        let tool = FuzzDriverTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("parse_crash")),
            (
                "crash_input".to_string(),
                serde_json::json!("/tmp/nonexistent_crash_29"),
            ),
        ]);
        let result = tool.execute(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_generate_target_in_temp_dir() {
        let tmp = std::env::temp_dir().join("fuzz_test_29");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Create a minimal Cargo.toml
        std::fs::write(
            tmp.join("Cargo.toml"),
            r#"[package]
name = "test-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        let tool = FuzzDriverTool;
        let params = HashMap::from([
            ("action".to_string(), serde_json::json!("generate_target")),
            (
                "path".to_string(),
                serde_json::json!(tmp.to_string_lossy().to_string()),
            ),
            (
                "target_name".to_string(),
                serde_json::json!("fuzz_parse_test"),
            ),
            ("input_type".to_string(), serde_json::json!("bytes")),
        ]);
        let result = tool.execute(&params).await.unwrap();
        assert_eq!(result["target_name"], "fuzz_parse_test");
        assert!(result["target_file"]
            .as_str()
            .unwrap()
            .contains("fuzz_parse_test.rs"));

        // Verify files were created
        assert!(tmp.join("fuzz/Cargo.toml").exists());
        assert!(tmp.join("fuzz/fuzz_targets/fuzz_parse_test.rs").exists());
        assert!(tmp.join("fuzz/corpus/fuzz_parse_test").exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
