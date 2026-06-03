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
                description: "Hash algorithm: md5, sha1, sha256 (default), sha512, blake3"
                    .to_string(),
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
                description: "Expected hash value to compare against (for verify action)"
                    .to_string(),
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
            _ => Err(format!(
                "Unknown action: {action}. Valid: file, string, verify, batch"
            )),
        }
    }
}

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
    let metadata = path
        .metadata()
        .map_err(|e| format!("Failed to get file metadata: {e}"))?;

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "path": file_path,
        "hash": hash_hex,
        "hash_length_bytes": hash_bytes,
        "file_size": metadata.len(),
        "file_size_display": format_size(metadata.len()),
    }))
}

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

fn verify_hash(params: &HashMap<String, Value>) -> Result<Value, String> {
    let file_path = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("'path' is required for verify action")?;
    let expected = params
        .get("expected")
        .and_then(|v| v.as_str())
        .ok_or("'expected' is required for verify action")?;

    let algorithm = detect_algorithm_from_length(expected)?;
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    let (actual_hash, _) = compute_file_hash(path, algorithm)?;
    let is_match = actual_hash.eq_ignore_ascii_case(expected);

    Ok(serde_json::json!({
        "algorithm": algorithm,
        "path": file_path,
        "expected": expected,
        "actual": actual_hash,
        "match": is_match,
    }))
}

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
            errors.push(serde_json::json!({"path": file_path, "error": "File not found"}));
            continue;
        }
        if !path.is_file() {
            errors.push(serde_json::json!({"path": file_path, "error": "Not a file"}));
            continue;
        }
        match compute_file_hash(path, algorithm) {
            Ok((hash_hex, _)) => {
                results.push(serde_json::json!({"path": file_path, "hash": hash_hex}))
            }
            Err(e) => errors.push(serde_json::json!({"path": file_path, "error": e})),
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

fn detect_algorithm_from_length(hash: &str) -> Result<&'static str, String> {
    let hex = hash.trim();
    match hex.len() {
        32 => Ok("md5"),
        40 => Ok("sha1"),
        64 => Ok("sha256"),
        128 => Ok("sha512"),
        _ => Err(format!(
            "Cannot detect algorithm from hash length {}. Expected 32 (MD5), 40 (SHA1), 64 (SHA256/BLAKE3), or 128 (SHA512)",
            hex.len()
        )),
    }
}

fn compute_file_hash(path: &Path, algorithm: &str) -> Result<(String, usize), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file '{:?}': {e}", path))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 8192];

    match algorithm.to_lowercase().as_str() {
        "md5" => {
            let mut hasher = Md5Hasher::new();
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha1" => {
            let mut hasher = Sha1Hasher::new();
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            Ok((hex::encode(result), result.len()))
        }
        "blake3" => {
            let mut hasher = blake3::Hasher::new();
            loop {
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| format!("Read error: {e}"))?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let result = hasher.finalize();
            Ok((result.to_hex().to_string(), result.as_bytes().len()))
        }
        _ => Err(format!(
            "Unsupported algorithm: {algorithm}. Supported: md5, sha1, sha256, sha512, blake3"
        )),
    }
}

fn compute_string_hash(text: &str, algorithm: &str) -> Result<String, String> {
    match algorithm.to_lowercase().as_str() {
        "md5" => {
            let mut hasher = Md5Hasher::new();
            hasher.update(text.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "sha1" => {
            let mut hasher = Sha1Hasher::new();
            hasher.update(text.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(text.as_bytes());
            Ok(hex::encode(hasher.finalize()))
        }
        "blake3" => {
            let mut hasher = blake3::Hasher::new();
            hasher.update(text.as_bytes());
            Ok(hasher.finalize().to_hex().to_string())
        }
        _ => Err(format!("Unsupported algorithm: {algorithm}")),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hello() {
        let h = compute_string_hash("hello", "sha256").unwrap();
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_md5_hello() {
        let h = compute_string_hash("hello", "md5").unwrap();
        assert_eq!(h, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_sha1_hello() {
        let h = compute_string_hash("hello", "sha1").unwrap();
        assert_eq!(h, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn test_sha512_length() {
        let h = compute_string_hash("hello", "sha512").unwrap();
        assert_eq!(h.len(), 128);
    }

    #[test]
    fn test_blake3_length() {
        let h = compute_string_hash("hello", "blake3").unwrap();
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn test_unsupported_algorithm() {
        assert!(compute_string_hash("hello", "md4").is_err());
    }

    #[test]
    fn test_detect_algorithm() {
        assert_eq!(
            detect_algorithm_from_length(&"a".repeat(32)).unwrap(),
            "md5"
        );
        assert_eq!(
            detect_algorithm_from_length(&"a".repeat(40)).unwrap(),
            "sha1"
        );
        assert_eq!(
            detect_algorithm_from_length(&"a".repeat(64)).unwrap(),
            "sha256"
        );
        assert_eq!(
            detect_algorithm_from_length(&"a".repeat(128)).unwrap(),
            "sha512"
        );
        assert!(detect_algorithm_from_length("abc").is_err());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    #[test]
    fn test_file_hash() {
        let tmp = std::env::temp_dir().join("hash_test.txt");
        std::fs::write(&tmp, "hello").unwrap();
        let (h, _) = compute_file_hash(&tmp, "sha256").unwrap();
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_file_not_found() {
        assert!(compute_file_hash(Path::new("/no/such/file"), "sha256").is_err());
    }

    #[test]
    fn test_hash_string_action() {
        let mut p = HashMap::new();
        p.insert("text".to_string(), Value::String("hello".to_string()));
        p.insert("algorithm".to_string(), Value::String("md5".to_string()));
        let r = hash_string_action(&p).unwrap();
        assert_eq!(
            r["hash"].as_str().unwrap(),
            "5d41402abc4b2a76b9719d911017c592"
        );
    }

    #[test]
    fn test_hash_string_action_missing_text() {
        assert!(hash_string_action(&HashMap::new()).is_err());
    }

    #[test]
    fn test_batch_hash() {
        let t1 = std::env::temp_dir().join("batch1.txt");
        let t2 = std::env::temp_dir().join("batch2.txt");
        std::fs::write(&t1, "hello").unwrap();
        std::fs::write(&t2, "world").unwrap();
        let mut p = HashMap::new();
        p.insert(
            "paths".to_string(),
            serde_json::json!([t1.to_str().unwrap(), t2.to_str().unwrap(), "/nonexistent"]),
        );
        let r = batch_hash(&p).unwrap();
        assert_eq!(r["total"].as_u64().unwrap(), 3);
        assert_eq!(r["successful"].as_u64().unwrap(), 2);
        assert_eq!(r["failed"].as_u64().unwrap(), 1);
        std::fs::remove_file(&t1).ok();
        std::fs::remove_file(&t2).ok();
    }

    #[test]
    fn test_hash_tool_metadata() {
        let t = HashTool;
        assert_eq!(t.name(), "hash");
        assert!(t.description().contains("checksum"));
        let params = t.parameters();
        assert_eq!(params.len(), 6);
        assert!(params.iter().any(|p| p.name == "action" && p.required));
    }
}
