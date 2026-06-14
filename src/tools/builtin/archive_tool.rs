//! Archive Tool: compress and decompress files.
//!
//! Supports ZIP, TAR, TAR.GZ (tgz), and TAR.BZ2 formats.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ArchiveTool));
}

struct ArchiveTool;

#[async_trait::async_trait]
impl Tool for ArchiveTool {
    fn name(&self) -> &str {
        "archive"
    }

    fn description(&self) -> &str {
        "Compress and decompress files and directories. \
         Actions: compress (create archive), decompress (extract archive), list (list archive contents). \
         Supports formats: zip, tar, tar.gz/tgz, tar.bz2."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: compress, decompress, list".to_string(),
                required: true,
            },
            ToolParameter {
                name: "format".to_string(),
                parameter_type: "string".to_string(),
                description: "Archive format: zip, tar, tar.gz (or tgz), tar.bz2 (default: zip)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "source".to_string(),
                parameter_type: "string".to_string(),
                description: "Source file or directory to compress (for compress), or archive file path (for decompress/list)".to_string(),
                required: true,
            },
            ToolParameter {
                name: "destination".to_string(),
                parameter_type: "string".to_string(),
                description: "Destination archive path (for compress) or destination directory (for decompress). Default: auto-generated.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "compression_level".to_string(),
                parameter_type: "number".to_string(),
                description: "Compression level 0-9 (default: 6). 0=fastest/storage, 9=best compression.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "include".to_string(),
                parameter_type: "array".to_string(),
                description: "JSON array of glob patterns to include (e.g. '[\"*.txt\", \"*.rs\"]'). Default: all files.".to_string(),
                required: false,
            },
            ToolParameter {
                name: "exclude".to_string(),
                parameter_type: "array".to_string(),
                description: "JSON array of glob patterns to exclude (e.g. '[\"*.log\", \"target/*\"]').".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "compress" => compress_archive(params).await,
            "decompress" => decompress_archive(params).await,
            "list" => list_archive(params).await,
            _ => Err(format!(
                "Unknown action: {action}. Valid: compress, decompress, list"
            )),
        }
    }
}

/// Compress files/directories into an archive.
async fn compress_archive(params: &HashMap<String, Value>) -> Result<Value, String> {
    let format = params
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("zip")
        .to_lowercase();
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or("source is required")?;
    let compression_level = params
        .get("compression_level")
        .and_then(|v| v.as_u64())
        .unwrap_or(6) as u32;
    let include_patterns: Option<Vec<&str>> = params
        .get("include")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect());
    let exclude_patterns: Option<Vec<&str>> = params
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect());

    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(format!("Source '{source}' does not exist"));
    }

    // Auto-generate destination if not provided
    let destination = if let Some(dest) = params.get("destination").and_then(|v| v.as_str()) {
        dest.to_string()
    } else {
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("archive");
        let ext = match format.as_str() {
            "zip" => "zip",
            "tar" => "tar",
            "tar.gz" | "tgz" => "tar.gz",
            "tar.bz2" => "tar.bz2",
            _ => "zip",
        };
        format!("{stem}.{ext}")
    };

    match format.as_str() {
        "zip" => create_zip(
            source_path,
            &destination,
            compression_level,
            &include_patterns,
            &exclude_patterns,
        )?,
        "tar" => create_tar(
            source_path,
            &destination,
            false,
            &include_patterns,
            &exclude_patterns,
        )?,
        "tar.gz" | "tgz" => create_tar(
            source_path,
            &destination,
            true,
            &include_patterns,
            &exclude_patterns,
        )?,
        "tar.bz2" => {
            return Err("tar.bz2 format requires bzip2 library, use tar.gz instead".to_string());
        }
        _ => {
            return Err(format!(
                "Unsupported format: {format}. Supported: zip, tar, tar.gz, tgz"
            ))
        }
    }

    // Get file size info
    let metadata = std::fs::metadata(&destination)
        .map_err(|e| format!("Failed to get archive metadata: {e}"))?;

    let source_size = if source_path.is_dir() {
        dir_size(source_path)
    } else {
        source_path.metadata().map(|m| m.len()).unwrap_or(0)
    };

    Ok(serde_json::json!({
        "success": true,
        "message": format!("Archive created successfully"),
        "archive": destination,
        "format": format,
        "size_bytes": metadata.len(),
        "size_display": format_size(metadata.len()),
        "source_size_bytes": source_size,
        "compression_ratio": if source_size > 0 {
            format!("{:.2}%", (metadata.len() as f64 / source_size as f64) * 100.0)
        } else {
            "N/A".to_string()
        }
    }))
}

/// Decompress an archive.
async fn decompress_archive(params: &HashMap<String, Value>) -> Result<Value, String> {
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or("source is required")?;

    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(format!("Archive '{source}' does not exist"));
    }

    let destination = if let Some(dest) = params.get("destination").and_then(|v| v.as_str()) {
        dest.to_string()
    } else {
        // Default: extract to a directory named after the archive (without extension)
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted");
        // Handle .tar.gz double extension
        let stem = if source.ends_with(".tar.gz") || source.ends_with(".tgz") {
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("extracted")
                .to_string()
        } else {
            stem.to_string()
        };
        stem
    };

    // Detect format from extension
    let format = detect_format(source);

    match format.as_str() {
        "zip" => extract_zip(source_path, &destination)?,
        "tar" => extract_tar(source_path, &destination)?,
        "tar.gz" | "tgz" => extract_tar_gz(source_path, &destination)?,
        "tar.bz2" => {
            return Err("tar.bz2 extraction requires bzip2 library".to_string());
        }
        _ => return Err(format!("Unsupported archive format: {source}")),
    }

    // Count extracted files
    let file_count = count_files(Path::new(&destination));

    Ok(serde_json::json!({
        "success": true,
        "message": format!("Archive extracted successfully to '{destination}'"),
        "destination": destination,
        "source": source,
        "format": format,
        "extracted_files": file_count,
    }))
}

/// List contents of an archive.
async fn list_archive(params: &HashMap<String, Value>) -> Result<Value, String> {
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or("source is required")?;

    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(format!("Archive '{source}' does not exist"));
    }

    let format = detect_format(source);

    match format.as_str() {
        "zip" => list_zip(source_path),
        "tar" | "tar.gz" | "tgz" => list_tar(source_path, &format),
        _ => Err(format!("Unsupported archive format: {source}")),
    }
}

// ---- ZIP operations ----

fn create_zip(
    source: &Path,
    destination: &str,
    compression_level: u32,
    include: &Option<Vec<&str>>,
    exclude: &Option<Vec<&str>>,
) -> Result<(), String> {
    let file = std::fs::File::create(destination)
        .map_err(|e| format!("Failed to create zip file '{destination}': {e}"))?;

    let level = compression_level.min(9);
    let zip_level = match level {
        0 => zip::CompressionMethod::Stored,
        _ => zip::CompressionMethod::Deflated,
    };

    let mut zip_writer = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
        .compression_method(zip_level)
        .unix_permissions(0o644);

    let files = collect_files(source, include, exclude)?;

    for entry_path in &files {
        let relative = if source.is_dir() {
            entry_path
                .strip_prefix(source)
                .map_err(|e| format!("Path error: {e}"))?
                .to_str()
                .unwrap_or("")
        } else {
            entry_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
        };

        if relative.is_empty() {
            continue;
        }

        if entry_path.is_dir() {
            zip_writer
                .add_directory(relative, options)
                .map_err(|e| format!("Failed to add directory '{relative}': {e}"))?;
        } else {
            let mut f = std::fs::File::open(entry_path)
                .map_err(|e| format!("Failed to open '{relative}': {e}"))?;
            zip_writer
                .start_file(relative, options)
                .map_err(|e| format!("Failed to start file '{relative}': {e}"))?;
            std::io::copy(&mut f, &mut zip_writer)
                .map_err(|e| format!("Failed to write '{relative}': {e}"))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip: {e}"))?;

    Ok(())
}

fn extract_zip(source: &Path, destination: &str) -> Result<(), String> {
    let file = std::fs::File::open(source).map_err(|e| format!("Failed to open zip file: {e}"))?;

    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {e}"))?;

    std::fs::create_dir_all(destination)
        .map_err(|e| format!("Failed to create destination directory '{destination}': {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;

        let Some(outpath) = entry.enclosed_name().map(|p| {
            let dest = Path::new(destination);
            dest.join(p)
        }) else {
            continue;
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory '{:?}': {e}", outpath))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create parent directory '{:?}': {e}", parent)
                })?;
            }
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create file '{:?}': {e}", outpath))?;
            std::io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to extract file '{:?}': {e}", outpath))?;
        }
    }

    Ok(())
}

fn list_zip(source: &Path) -> Result<Value, String> {
    let file = std::fs::File::open(source).map_err(|e| format!("Failed to open zip file: {e}"))?;

    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {e}"))?;

    let mut entries = Vec::with_capacity(archive.len());
    let mut total_size = 0u64;
    let mut file_count = 0;
    let mut dir_count = 0;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;

        let name = entry.name().to_string();
        let size = entry.size();
        let is_dir = entry.is_dir();

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
            total_size += size;
        }

        entries.push(serde_json::json!({
            "name": name,
            "size": size,
            "size_display": format_size(size),
            "is_dir": is_dir,
            "compressed_size": entry.compressed_size(),
        }));
    }

    Ok(serde_json::json!({
        "archive": source.to_str().unwrap_or(""),
        "format": "zip",
        "total_entries": entries.len(),
        "file_count": file_count,
        "dir_count": dir_count,
        "total_size_bytes": total_size,
        "total_size_display": format_size(total_size),
        "entries": entries,
    }))
}

// ---- TAR operations ----

fn create_tar(
    source: &Path,
    destination: &str,
    gzip: bool,
    include: &Option<Vec<&str>>,
    exclude: &Option<Vec<&str>>,
) -> Result<(), String> {
    let file = std::fs::File::create(destination)
        .map_err(|e| format!("Failed to create tar file '{destination}': {e}"))?;

    let files = collect_files(source, include, exclude)?;

    if gzip {
        let encoder = GzEncoder::new(file, Compression::default());
        let mut tar_writer = tar::Builder::new(encoder);

        for entry_path in &files {
            let relative = if source.is_dir() {
                entry_path
                    .strip_prefix(source)
                    .map_err(|e| format!("Path error: {e}"))?
                    .to_str()
                    .unwrap_or("")
            } else {
                entry_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
            };

            if relative.is_empty() {
                continue;
            }

            if entry_path.is_file() {
                let mut f = std::fs::File::open(entry_path)
                    .map_err(|e| format!("Failed to open '{relative}': {e}"))?;
                tar_writer
                    .append_file(relative, &mut f)
                    .map_err(|e| format!("Failed to add file '{relative}': {e}"))?;
            }
        }

        let encoder = tar_writer
            .into_inner()
            .map_err(|e| format!("Failed to finalize tar: {e}"))?;
        encoder
            .finish()
            .map_err(|e| format!("Failed to finalize gzip: {e}"))?;
    } else {
        let mut tar_writer = tar::Builder::new(file);

        for entry_path in &files {
            let relative = if source.is_dir() {
                entry_path
                    .strip_prefix(source)
                    .map_err(|e| format!("Path error: {e}"))?
                    .to_str()
                    .unwrap_or("")
            } else {
                entry_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
            };

            if relative.is_empty() {
                continue;
            }

            if entry_path.is_file() {
                let mut f = std::fs::File::open(entry_path)
                    .map_err(|e| format!("Failed to open '{relative}': {e}"))?;
                tar_writer
                    .append_file(relative, &mut f)
                    .map_err(|e| format!("Failed to add file '{relative}': {e}"))?;
            }
        }

        tar_writer
            .into_inner()
            .map_err(|e| format!("Failed to finalize tar: {e}"))?;
    }

    Ok(())
}

fn extract_tar(source: &Path, destination: &str) -> Result<(), String> {
    let file = std::fs::File::open(source).map_err(|e| format!("Failed to open tar file: {e}"))?;

    let mut archive = tar::Archive::new(file);

    std::fs::create_dir_all(destination)
        .map_err(|e| format!("Failed to create destination directory '{destination}': {e}"))?;

    archive
        .unpack(destination)
        .map_err(|e| format!("Failed to extract tar: {e}"))?;

    Ok(())
}

fn extract_tar_gz(source: &Path, destination: &str) -> Result<(), String> {
    let file =
        std::fs::File::open(source).map_err(|e| format!("Failed to open tar.gz file: {e}"))?;

    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    std::fs::create_dir_all(destination)
        .map_err(|e| format!("Failed to create destination directory '{destination}': {e}"))?;

    archive
        .unpack(destination)
        .map_err(|e| format!("Failed to extract tar.gz: {e}"))?;

    Ok(())
}

fn list_tar(source: &Path, format: &str) -> Result<Value, String> {
    let file = std::fs::File::open(source).map_err(|e| format!("Failed to open tar file: {e}"))?;

    let archive: Box<dyn std::io::Read> = if format == "tar.gz" || format == "tgz" {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let mut tar_archive = tar::Archive::new(archive);
    let mut entries = Vec::new();
    let mut total_size = 0u64;
    let mut file_count = 0;
    let mut dir_count = 0;

    for entry in tar_archive
        .entries()
        .map_err(|e| format!("Failed to read tar entries: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read tar entry: {e}"))?;
        let path = entry
            .path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let size = entry.size();
        let is_dir = entry.header().entry_type().is_dir();

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
            total_size += size;
        }

        entries.push(serde_json::json!({
            "name": path,
            "size": size,
            "size_display": format_size(size),
            "is_dir": is_dir,
        }));
    }

    Ok(serde_json::json!({
        "archive": source.to_str().unwrap_or(""),
        "format": format,
        "total_entries": entries.len(),
        "file_count": file_count,
        "dir_count": dir_count,
        "total_size_bytes": total_size,
        "total_size_display": format_size(total_size),
        "entries": entries,
    }))
}

// ---- Helper functions ----

/// Collect files to include in archive, respecting include/exclude patterns.
fn collect_files(
    source: &Path,
    include: &Option<Vec<&str>>,
    exclude: &Option<Vec<&str>>,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();

    if source.is_dir() {
        for entry in WalkDir::new(source).into_iter().filter_entry(|e| {
            // Skip hidden directories unless explicitly included
            let name = e.file_name().to_str().unwrap_or("");
            !name.starts_with('.') || name == "."
        }) {
            let entry = entry.map_err(|e| format!("Failed to walk directory: {e}"))?;
            let path = entry.path().to_path_buf();

            // Apply include/exclude patterns using simple substring matching
            let relative = path
                .strip_prefix(source)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if relative.is_empty() {
                files.push(path);
                continue;
            }

            // Apply exclude patterns
            if let Some(excludes) = exclude {
                let mut excluded = false;
                for pattern in excludes {
                    if matches_simple_pattern(&relative, pattern) {
                        excluded = true;
                        break;
                    }
                }
                if excluded {
                    continue;
                }
            }

            // Apply include patterns (if any, only include matching)
            if let Some(includes) = include {
                let mut included = false;
                for pattern in includes {
                    if matches_simple_pattern(&relative, pattern) {
                        included = true;
                        break;
                    }
                }
                if !included {
                    continue;
                }
            }

            files.push(path);
        }
    } else if source.is_file() {
        files.push(source.to_path_buf());
    }

    // Sort for reproducible archives
    files.sort();

    Ok(files)
}

/// Simple glob-like pattern matching (supports * and ?)
/// Uses native string matching instead of regex to avoid repeated compilation.
fn matches_simple_pattern(name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') && !pattern.contains('?') {
        return name == pattern;
    }

    // Common patterns handled natively for performance:
    // "*.ext" → ends_with
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return name.len() > suffix.len() && name.ends_with(suffix) && name.chars().nth(name.len() - suffix.len() - 1) == Some('.');
    }
    // "*.ext" without dot after star
    if let Some(suffix) = pattern.strip_prefix('*') {
        if !suffix.contains('*') {
            return name.ends_with(suffix);
        }
    }
    // "dir/*" → starts_with
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return name.starts_with(prefix) && (name.len() == prefix.len() || name.as_bytes().get(prefix.len()) == Some(&b'/'));
    }

    // General case: native glob matching without regex compilation
    glob_match(name.as_bytes(), pattern.as_bytes())
}

/// Native glob matcher - no heap allocation, no regex compilation.
fn glob_match(mut name: &[u8], mut pattern: &[u8]) -> bool {
    let mut star_name = name;
    let mut star_pat = pattern;

    loop {
        match (name.first(), pattern.first()) {
            // Both exhausted
            (None, None) => return true,
            // Pattern exhausted but name remains
            (Some(_), None) => return false,
            // Name exhausted but pattern remains - check if rest is all stars
            (None, Some(b'*')) => {
                pattern = &pattern[1..];
                continue;
            }
            (None, Some(_)) => return false,
            // Star in pattern - save backtrack point
            (_, Some(b'*')) => {
                star_pat = &pattern[1..];
                star_name = name;
                pattern = star_pat;
                name = star_name;
            }
            // '?' matches any single character
            (_, Some(b'?')) => {
                name = &name[1..];
                pattern = &pattern[1..];
            }
            // Exact character match
            (Some(n), Some(p)) if n == p => {
                name = &name[1..];
                pattern = &pattern[1..];
            }
            // Mismatch - backtrack to last star
            (_, _) => {
                if star_pat.is_empty() {
                    return false;
                }
                star_name = &star_name[1..];
                pattern = star_pat;
                name = star_name;
            }
        }
    }
}

/// Calculate directory size recursively.
fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Count files in a directory recursively.
fn count_files(path: &Path) -> usize {
    if path.is_dir() {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count()
    } else if path.is_file() {
        1
    } else {
        0
    }
}

/// Detect archive format from file extension.
fn detect_format(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.ends_with(".zip") {
        "zip".to_string()
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        "tar.gz".to_string()
    } else if lower.ends_with(".tar.bz2") {
        "tar.bz2".to_string()
    } else if lower.ends_with(".tar") {
        "tar".to_string()
    } else {
        "zip".to_string() // default
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
