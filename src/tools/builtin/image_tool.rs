//! Image tool: analyze and manipulate images using the image crate.
//!
//! Supports: resize, format, info, convert, thumbnail.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use image::GenericImageView;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(ImageTool));
}

struct ImageTool;

#[async_trait::async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str {
        "image"
    }

    fn description(&self) -> &str {
        "Analyze and manipulate images. Actions: info (get dimensions, format, size), \
         resize (scale to dimensions), thumbnail (create small preview), convert (change format)."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".to_string(),
                parameter_type: "string".to_string(),
                description: "Action: info, resize, thumbnail, convert".to_string(),
                required: true,
            },
            ToolParameter {
                name: "path".to_string(),
                parameter_type: "string".to_string(),
                description: "Path to the image file".to_string(),
                required: true,
            },
            ToolParameter {
                name: "width".to_string(),
                parameter_type: "number".to_string(),
                description: "Target width (for resize/thumbnail)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "height".to_string(),
                parameter_type: "number".to_string(),
                description: "Target height (for resize/thumbnail)".to_string(),
                required: false,
            },
            ToolParameter {
                name: "output".to_string(),
                parameter_type: "string".to_string(),
                description: "Output file path (for resize/convert)".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("path is required".to_string())?;

        let img = image::open(path).map_err(|e| format!("Failed to load image: {e}"))?;
        let (width, height) = img.dimensions();

        match action {
            "info" => {
                let size_bytes = Path::new(path).metadata().map(|m| m.len()).unwrap_or(0);
                Ok(serde_json::json!({
                    "width": width,
                    "height": height,
                    "size_bytes": size_bytes,
                    "format": format_from_path(path).unwrap_or("unknown"),
                }))
            }
            "resize" | "thumbnail" => {
                let target_w = params
                    .get("width")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(width as u64) as u32;
                let target_h = params
                    .get("height")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(height as u64) as u32;
                let output = params
                    .get("output")
                    .and_then(|v| v.as_str())
                    .unwrap_or(path);

                let resized = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
                resized
                    .save(output)
                    .map_err(|e| format!("Failed to save image: {e}"))?;

                Ok(serde_json::json!({
                    "action": action,
                    "original": format!("{width}x{height}"),
                    "output": format!("{target_w}x{target_h}"),
                    "saved_to": output,
                }))
            }
            "convert" => {
                let output = params
                    .get("output")
                    .and_then(|v| v.as_str())
                    .ok_or("output is required for convert".to_string())?;

                img.save(output)
                    .map_err(|e| format!("Failed to save converted image: {e}"))?;

                Ok(serde_json::json!({
                    "action": "convert",
                    "saved_to": output,
                    "format": format_from_path(output).unwrap_or("unknown"),
                }))
            }
            _ => Err(format!(
                "Unknown action: {action}. Valid: info, resize, thumbnail, convert"
            )),
        }
    }
}

fn format_from_path(path: &str) -> Option<&'static str> {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "png" => "PNG",
            "jpg" | "jpeg" => "JPEG",
            "gif" => "GIF",
            "bmp" => "BMP",
            "webp" => "WebP",
            "tiff" | "tif" => "TIFF",
            _ => "unknown",
        })
}
