//! Hash Tool: compute file and string checksums.
//!
//! Supports MD5, SHA-1, SHA-256, SHA-512, and BLAKE3.
//! Actions: file (compute hash of a file), string (compute hash of a string),
//! verify (compare hash against expected value), batch (compute hashes of multiple files).

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use md5::Md5 as Md5Hasher;
use serde_json::Value;
use sha1::Sha1 as Sha1Hasher;
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(HashTool));
}

struct HashTool;

#[async_trait::async_trait]
impl Tool for HashTool {
    fn name(&self) -> &str {
        "hash"
    }

    fn description(&self) -> &str {
        "Compute file and string checksums. Actions: file (compute hash of a file), \
         string (compute hash of a string), verify (compare hash against expected value), \
         batch (compute hashes of multiple files). \
         Supports algorithms: md5, sha1, sha256, sha512, blake3."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: file, string, verify, batch".to_string(),
                required: true,
            },
            ToolParameter {
                name: "algorithm".to_string(),
                parameter_type: "string".to_string(),
                description: "Hash algorithm: md5, sha1, sha256 (default), sha512, blake3".to_string(),
                required: false,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "File path (for file/batch action)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "paths".to_string(),
                parameter_type: "array".to_string(),
                description: "JSON array of file paths (for batch action)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "text".to_string(),
                parameter_type: "string".to_string(),
                description: "Text to hash (for string action)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "expected".to_string(),
                parameter_type: "string".to_string(),
                description: "Expected hash value to compare against (for verify action)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "file" => hash_file_action(params),
            "string" => hash_string_action(params),
            "verify" => verify_hash(params),
            "batch" => batch_hash(params),
            _ => Err(format!("Unknown action: {action}. Valid: file, string, verify, batch")),
        }
    }
}

/// Compute hash of a file.
fn hash_file_action(params: &HashMap<String, Value>) -> Result<Value, String> {
    let file_path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("'path' is required for file action")?;
    let algorithm = params
        .get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256");

    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    let (hash_hex, hash_bytes) = compute_file_hash(path, algorithm)?;

    let metadata = path.metadata().map_err(|e| format!("Failed to get file metadata: {e}"))?;

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "path": file_path,
        "hash": hash_hex,
        "hash_length_bytes": hash_bytes,
        "file_size": metadata.len(),
        "file_size_display": format_size(metadata.len()),
    }))
}

/// Compute hash of a string.
fn hash_string_action(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = params
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or("'text' is required for string action")?;
    let algorithm = params
        .get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256");

    let hash_hex = compute_string_hash(text, algorithm)?;

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "text": text,
        "hash": hash_hex,
    }))
}

/// Verify a file hash against an expected value.
fn verify_hash(params: &HashMap<String, Value>) -> Result<Value, String> {
    let file_path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("'path' is required for verify action")?;
    let expected = params
        .get("expected")
        .and_then(|v| v.as_str())
        .ok_or("'expected' is required for verify action")?;

    // Detect algorithm from expected hash length
    let algorithm = detect_algorithm_from_length(expected)?;

    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    let (actual_hash, _) = compute_file_hash(path, &algorithm)?;
    let is_match = actual_hash.eq_ignore_ascii_case(expected);

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "path": file_path,
        "expected": expected,
        "actual": actual_hash,
        "match": is_match,
    }))
}

/// Compute hashes of multiple files.
fn batch_hash(params: &HashMap<String, Value>) -> Result<Value, String> {
    let paths: Vec<String> = params
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .ok_or("'paths' (JSON array) is required for batch action")?;
    let algorithm = params
        .get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256");

    let mut results = Vec::with_capacity(paths.len());
    let mut errors = Vec::new();

    for file_path in &paths {
        let path = Path::new(file_path);
        if !path.exists() {
            errors.push(serde_json::json!({
                "path": file_path,
                "error": "File not found",
            }));
            continue;
        }
        if !path.is_file() {
            errors.push(serde_json::json!({
                "path": file_path,
                "error": "Not a file",
            }));
            continue;
        }

        match compute_file_hash(path, algorithm) {
            Ok((hash_hex, _)) => results.push(serde_json::json!({
                "path": file_path,
                "hash": hash_hex,
            })),
            Err(e) => errors.push(serde_json::json!({
                "path": file_path,
                "error": e,
            })),
        }
    }

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "total": paths.len(),
        "successful": results.len(),
        "failed": errors.len(),
        "results": results,
        "errors": if errors.is_empty() { Value::Null } else { Value::Array(errors) },
    }))
}

/// Detect hash algorithm from hash string length.
fn detect_algorithm_from_length(hash: &str) -> Result<&'static str, String> {
    let hex = hash.trim();
    match hex.len() {
        32 => Ok("md5"),
        40 => Ok("sha1"),
        64 => Ok("sha256"), // BLAKE3 also produces 64-char hex, default to SHA-256
        128 => Ok("sha512"),
        _ => Err(format!(
            "Cannot detect algorithm from hash length {}. Expected 32 (MD5), 40 (SHA1), 64 (SHA256/BLAKE3), or 128 (SHA512)",
            hex.len()
        )),
    }
}

/// Compute file hash with streaming (memory-efficient for large files).
fn compute_file_hash(path: &Path, algorithm: &str) -> Result<(String, usize), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file '{:?}': {e}", path))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    match algorithm.to_lowercase().as_str() {
        "md5" => {
            let mut hasher = Md5Hasher::new();
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
                if bytes_read == 0 { break; }
                hasher.update(&buffer[..bytes_read]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha1" => {
            let mut hasher = Sha1Hasher::new();
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
                if bytes_read == 0 { break; }
                hasher.update(&buffer[..bytes_read]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
                if bytes_read == 0 { break; }
                hasher.update(&buffer[..bytes_read]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
                if bytes_read == 0 { break; }
                hasher.update(&buffer[..bytes_read]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "blake3" => {
            let mut hasher = blake3::Hasher::new();
            loop {
                let bytes_read = reader.read(&mut buffer).map_err(|e| format!("Read error: {e}"))?;
                if bytes_read == 0 { break; }
                hasher.update(&buffer[..bytes_read]);
            }
            let result = hasher.finalize();
            Ok((result.to_hex().to_string(), result.as_bytes().len()))
        }
        _ => Err(format!("Unsupported algorithm: {algorithm}. Supported: md5, sha1, sha256, sha512, blake3")),
    }
}

/// Compute string hash.
fn compute_string_hash(text: &str, algorithm: &str) -> Result<String, String> {
    match algorithm.to_lowercase().as_str() {
        "md5" => {
            let mut hasher = Md5Hasher::new();
            hasher.update(text.as_bytes());
            let result = hasher.finalize();
            Ok(hex::encode(result))
        }
        "sha1" => {
            let mut hasher = Sha1Hasher::new();
            hasher.update(text.as_bytes());
            let result = hasher.finalize();
            Ok(hex::encode(result))
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            let result = hasher.finalize();
            Ok(hex::encode(result))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(text.as_bytes());
            let result = hasher.finalize();
            Ok(hex::encode(result))
        }
        "blake3" => {
            let mut hasher = blake3::Hasher::new();
            hasher.update(text.as_bytes());
            let result = hasher.finalize();
            Ok(result.to_hex().to_string())
        }
        _ => Err(format!("Unsupported algorithm: {algorithm}. Supported: md5, sha1, sha256, sha512, blake3")),
    }
}

/// Format bytes to human-readable size.
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}
