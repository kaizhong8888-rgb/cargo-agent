use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;

struct TextProcessor;

#[async_trait::async_trait]
impl Tool for TextProcessor {
    fn name(&self) -> &str {
        "text_processor"
    }

    fn description(&self) -> &str {
        "Advanced text processing: case conversion, counting, base64/url/html encoding, \
         UUID generation, regex matching/replacement, string padding/truncation, \
         word wrapping, slugification, random string generation, join/split, and more."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "Operation to perform: case, count, truncate, base64, url, html, \
                              reverse, trim, pad, repeat, uuid, random, wrap, indent, \
                              slugify, regex, join, split, length, shuffle, substring, \
                              replace, char_at, escape, unescape"
                    .into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "text".into(),
                description: "Input text to process (required for most actions except uuid/random)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "case_style".into(),
                description: "Target case style for 'case' action: snake, camel, pascal, kebab, \
                              screaming_snake, title, upper, lower".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "mode".into(),
                description: "Sub-mode for certain actions: \
                              base64→encode/decode, url→encode/decode, \
                              trim→left/right/both, pad→left/right/center, \
                              html→escape/unescape, regex→find/replace/count".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "width".into(),
                description: "Target width for truncate/pad/wrap actions (default: 80)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
            ToolParameter {
                name: "ellipsis".into(),
                description: "Ellipsis string for truncate action (default: '...')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "count".into(),
                description: "Repeat count for 'repeat' action, \
                              or random string length for 'random' action (default: 10)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
            ToolParameter {
                name: "separator".into(),
                description: "Separator string for join/split (default: ',')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "items".into(),
                description: "JSON array of strings for 'join' action, or string for 'split'".into(),
                required: false,
                parameter_type: "array".into(),
            },
            ToolParameter {
                name: "pattern".into(),
                description: "Regex pattern for 'regex' action".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "replacement".into(),
                description: "Replacement string for 'regex' replace mode (supports $1, $2, etc.)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "char_set".into(),
                description: "Character set for 'random' action: alphanumeric, alpha, numeric, \
                              hex, ascii (default: alphanumeric)".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "padding".into(),
                description: "Padding character for 'pad' action (default: ' ')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "start".into(),
                description: "Start index for substring action (default: 0)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
            ToolParameter {
                name: "end".into(),
                description: "End index for substring action (default: end of string)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
            ToolParameter {
                name: "indent_str".into(),
                description: "Indentation string for 'indent' action (default: '  ')".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "char_pos".into(),
                description: "Character position for char_at action (default: 0)".into(),
                required: false,
                parameter_type: "integer".into(),
            },
            ToolParameter {
                name: "old".into(),
                description: "Text to replace for 'replace' action".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "new".into(),
                description: "Replacement text for 'replace' action".into(),
                required: false,
                parameter_type: "string".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: 'action'".to_string())?;

        match action {
            "case" => action_case(params),
            "count" => action_count(params),
            "truncate" => action_truncate(params),
            "base64" => action_base64(params),
            "url" => action_url(params),
            "html" => action_html(params),
            "reverse" => action_reverse(params),
            "trim" => action_trim(params),
            "pad" => action_pad(params),
            "repeat" => action_repeat(params),
            "uuid" => action_uuid(),
            "random" => action_random(params),
            "wrap" => action_wrap(params),
            "indent" => action_indent(params),
            "slugify" => action_slugify(params),
            "regex" => action_regex(params),
            "join" => action_join(params),
            "split" => action_split(params),
            "length" => action_length(params),
            "shuffle" => action_shuffle(params),
            "substring" => action_substring(params),
            "replace" => action_simple_replace(params),
            "char_at" => action_char_at(params),
            "escape" => action_escape(params),
            "unescape" => action_unescape(params),
            _ => Err(format!(
                "Unknown action: '{}'. Available: case, count, truncate, base64, url, html, \
                 reverse, trim, pad, repeat, uuid, random, wrap, indent, slugify, regex, \
                 join, split, length, shuffle, substring, replace, char_at, escape, unescape",
                action
            )),
        }
    }
}

// ─── Helper ────────────────────────────────────────────

fn get_text(params: &HashMap<String, Value>) -> Result<String, String> {
    params
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing required parameter: 'text'".to_string())
}

fn get_int(params: &HashMap<String, Value>, key: &str, default: i64) -> i64 {
    params
        .get(key)
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

fn get_str<'a>(params: &'a HashMap<String, Value>, key: &str, default: &'a str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or(default)
}

/// Convert a string to the specified case style using the `heck` crate.
fn convert_case(text: &str, style: &str) -> Result<String, String> {
    match style {
        "snake" | "snake_case" => Ok(heck::AsSnakeCase(text).to_string()),
        "camel" | "camelCase" => Ok(heck::AsLowerCamelCase(text).to_string()),
        "pascal" | "PascalCase" => Ok(heck::AsPascalCase(text).to_string()),
        "kebab" | "kebab-case" => Ok(heck::AsKebabCase(text).to_string()),
        "screaming_snake" | "SCREAMING_SNAKE" | "SCREAMING_SNAKE_CASE" => {
            Ok(heck::AsShoutySnakeCase(text).to_string())
        }
        "title" | "Title Case" | "TitleCase" => Ok(heck::AsTitleCase(text).to_string()),
        "upper" | "UPPER" | "uppercase" => Ok(text.to_uppercase()),
        "lower" | "LOWER" | "lowercase" => Ok(text.to_lowercase()),
        "train" | "Train-Case" => Ok(heck::AsTrainCase(text).to_string()),
        "sentence" | "Sentence case" => {
            let mut s = text.to_lowercase();
            if let Some(c) = s.chars().next() {
                s.replace_range(..1, &c.to_uppercase().to_string());
            }
            Ok(s)
        }
        "alternating" | "aLtErNaTiNg" => {
            let mut result = String::with_capacity(text.len());
            for (i, c) in text.chars().enumerate() {
                if i % 2 == 0 {
                    result.extend(c.to_uppercase());
                } else {
                    result.extend(c.to_lowercase());
                }
            }
            Ok(result)
        }
        "inverse" => {
            let mut result = String::with_capacity(text.len());
            for c in text.chars() {
                if c.is_uppercase() {
                    result.extend(c.to_lowercase());
                } else {
                    result.extend(c.to_uppercase());
                }
            }
            Ok(result)
        }
        _ => Err(format!(
            "Unknown case style: '{}'. Available: snake, camel, pascal, kebab, \
             screaming_snake, title, upper, lower, train, sentence, alternating, inverse",
            style
        )),
    }
}

// ─── Action Implementations ────────────────────────────

fn action_case(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let style = get_str(params, "case_style", "snake");
    let result = convert_case(&text, style)?;
    Ok(serde_json::json!({
        "result": result,
        "original": text,
        "case_style": style
    }))
}

fn action_count(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let chars = text.chars().count();
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();
    let lines = text.lines().count();
    let bytes = text.len();

    // Count non-whitespace chars
    let non_ws_chars = text.chars().filter(|c| !c.is_whitespace()).count();

    // Count sentences (rough: . ! ? followed by space or end)
    let sentences = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .filter(|s| !s.trim().is_empty())
        .count();

    // Count paragraphs
    let paragraphs = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count();

    Ok(serde_json::json!({
        "chars": chars,
        "words": word_count,
        "lines": lines,
        "bytes": bytes,
        "non_whitespace_chars": non_ws_chars,
        "sentences": sentences,
        "paragraphs": paragraphs
    }))
}

fn action_truncate(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let width = get_int(params, "width", 80) as usize;
    let ellipsis = get_str(params, "ellipsis", "...");

    if text.chars().count() <= width {
        return Ok(serde_json::json!({
            "result": text,
            "truncated": false,
            "original_length": text.chars().count()
        }));
    }

    // Truncate at character boundary
    let truncated: String = text.chars().take(width).collect();
    let result = format!("{}{}", truncated.trim_end(), ellipsis);

    Ok(serde_json::json!({
        "result": result,
        "truncated": true,
        "original_length": text.chars().count(),
        "new_length": result.chars().count()
    }))
}

fn action_base64(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mode = get_str(params, "mode", "encode");
    let text = get_text(params)?;

    match mode {
        "encode" => {
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                text.as_bytes(),
            );
            Ok(serde_json::json!({
                "result": encoded,
                "mode": "encode",
                "original_bytes": text.len(),
                "encoded_length": encoded.len()
            }))
        }
        "decode" => {
            let decoded = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                text.trim(),
            )
            .map_err(|e| format!("Base64 decode error: {}", e))?;
            let result = String::from_utf8(decoded.clone())
                .map_err(|_| format!("Decoded bytes are not valid UTF-8. Use 'bytes' field for binary data."))?;
            Ok(serde_json::json!({
                "result": result,
                "mode": "decode",
                "decoded_bytes": decoded.len()
            }))
        }
        "encode_url" => {
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE,
                text.as_bytes(),
            );
            Ok(serde_json::json!({
                "result": encoded,
                "mode": "encode_url",
                "original_bytes": text.len()
            }))
        }
        "decode_url" => {
            let decoded = base64::Engine::decode(
                &base64::engine::general_purpose::URL_SAFE,
                text.trim(),
            )
            .map_err(|e| format!("Base64 URL decode error: {}", e))?;
            let result = String::from_utf8(decoded)
                .map_err(|_| "Decoded bytes are not valid UTF-8.".to_string())?;
            Ok(serde_json::json!({
                "result": result,
                "mode": "decode_url"
            }))
        }
        _ => Err(format!(
            "Unknown base64 mode: '{}'. Available: encode, decode, encode_url, decode_url",
            mode
        )),
    }
}

fn action_url(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mode = get_str(params, "mode", "encode");
    let text = get_text(params)?;

    match mode {
        "encode" => {
            let encoded = urlencoding::encode(&text);
            Ok(serde_json::json!({
                "result": encoded.to_string(),
                "mode": "encode"
            }))
        }
        "decode" => {
            let decoded = urlencoding::decode(&text)
                .map_err(|e| format!("URL decode error: {}", e))?;
            Ok(serde_json::json!({
                "result": decoded.to_string(),
                "mode": "decode"
            }))
        }
        _ => Err(format!(
            "Unknown URL mode: '{}'. Available: encode, decode",
            mode
        )),
    }
}

fn action_html(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mode = get_str(params, "mode", "escape");
    let text = get_text(params)?;

    match mode {
        "escape" => {
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#x27;");
            Ok(serde_json::json!({
                "result": escaped,
                "mode": "escape"
            }))
        }
        "unescape" => {
            let unescaped = text
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#x27;", "'")
                .replace("&#39;", "'");
            Ok(serde_json::json!({
                "result": unescaped,
                "mode": "unescape"
            }))
        }
        _ => Err(format!(
            "Unknown HTML mode: '{}'. Available: escape, unescape",
            mode
        )),
    }
}

fn action_reverse(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let reversed: String = text.chars().rev().collect();
    Ok(serde_json::json!({
        "result": reversed,
        "original_length": text.chars().count()
    }))
}

fn action_trim(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let mode = get_str(params, "mode", "both");

    let result = match mode {
        "left" | "l" | "start" => text.trim_start().to_string(),
        "right" | "r" | "end" => text.trim_end().to_string(),
        "both" | "b" | "all" | "" => text.trim().to_string(),
        _ => {
            // Custom characters to trim
            let trimmed_start = text.trim_start_matches(mode);
            let trimmed = trimmed_start.trim_end_matches(mode);
            trimmed.to_string()
        }
    };

    let trimmed_count = text.len() - result.len();
    Ok(serde_json::json!({
        "result": result,
        "trimmed_chars": trimmed_count,
        "mode": mode
    }))
}

fn action_pad(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let width = get_int(params, "width", 80) as usize;
    let mode = get_str(params, "mode", "right");
    let padding = get_str(params, "padding", " ");

    if width <= text.chars().count() {
        return Ok(serde_json::json!({
            "result": text,
            "padded": false,
            "chars_added": 0
        }));
    }

    let total_pad = width - text.chars().count();
    let result = match mode {
        "left" | "l" | "start" => {
            let pad_str = padding.repeat(total_pad / padding.len() + 1);
            format!("{}{}", &pad_str[..total_pad], text)
        }
        "right" | "r" | "end" | "" => {
            let pad_str = padding.repeat(total_pad / padding.len() + 1);
            format!("{}{}", text, &pad_str[..total_pad])
        }
        "center" | "c" | "both" => {
            let left_pad = total_pad / 2;
            let right_pad = total_pad - left_pad;
            let pad_left = padding.repeat(left_pad / padding.len() + 1);
            let pad_right = padding.repeat(right_pad / padding.len() + 1);
            format!(
                "{}{}{}",
                &pad_left[..left_pad],
                text,
                &pad_right[..right_pad]
            )
        }
        _ => {
            return Err(format!(
                "Unknown pad mode: '{}'. Available: left, right, center",
                mode
            ));
        }
    };

    Ok(serde_json::json!({
        "result": result,
        "padded": true,
        "chars_added": total_pad,
        "mode": mode
    }))
}

fn action_repeat(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let count = get_int(params, "count", 2) as usize;

    if count == 0 {
        return Ok(serde_json::json!({
            "result": "",
            "repetitions": 0
        }));
    }

    let result = text.repeat(count);
    Ok(serde_json::json!({
        "result": result,
        "repetitions": count,
        "total_length": result.chars().count()
    }))
}

fn action_uuid() -> Result<Value, String> {
    use uuid::Uuid;
    let id = Uuid::new_v4();
    Ok(serde_json::json!({
        "result": id.to_string(),
        "uuid_version": 4,
        "hyphenated": id.hyphenated().to_string(),
        "urn": id.urn().to_string(),
        "simple": id.simple().to_string()
    }))
}

fn action_random(params: &HashMap<String, Value>) -> Result<Value, String> {
    use rand::Rng;
    let length = get_int(params, "count", 16) as usize;
    let char_set = get_str(params, "char_set", "alphanumeric");

    let chars: &[u8] = match char_set {
        "alphanumeric" | "alnum" => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        "alpha" => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "numeric" | "digits" => b"0123456789",
        "hex" | "hexadecimal" => b"0123456789abcdef",
        "hex_upper" => b"0123456789ABCDEF",
        "ascii" => {
            let mut ascii_chars = Vec::with_capacity(95);
            for c in 32u8..=126u8 {
                ascii_chars.push(c);
            }
            let mut rng = rand::thread_rng();
            let result: String = (0..length).map(|_| rng.gen_range(32u8..=126u8) as char).collect();
            return Ok(serde_json::json!({
                "result": result,
                "length": length,
                "char_set": char_set
            }));
        }
        "lower" | "lowercase" => b"abcdefghijklmnopqrstuvwxyz",
        "upper" | "uppercase" => b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        _ => {
            // Custom character set from parameter
            let mut rng = rand::thread_rng();
            let bytes = char_set.as_bytes();
            let result: String = (0..length)
                .map(|_| {
                    let idx = rng.gen_range(0..bytes.len());
                    bytes[idx] as char
                })
                .collect();
            return Ok(serde_json::json!({
                "result": result,
                "length": length,
                "char_set": "custom"
            }));
        }
    };

    let mut rng = rand::thread_rng();
    let result: String = (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..chars.len());
            chars[idx] as char
        })
        .collect();

    Ok(serde_json::json!({
        "result": result,
        "length": length,
        "char_set": char_set
    }))
}

fn action_wrap(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let width = get_int(params, "width", 80) as usize;

    // Simple word-wrap algorithm
    let mut result = String::new();
    let mut line_len = 0;

    for word in text.split_whitespace() {
        if line_len + word.len() + 1 > width && line_len > 0 {
            result.push('\n');
            line_len = 0;
        }
        if line_len > 0 {
            result.push(' ');
            line_len += 1;
        }
        result.push_str(word);
        line_len += word.len();
    }

    let lines = result.lines().count();
    Ok(serde_json::json!({
        "result": result,
        "width": width,
        "lines": lines,
        "original_length": text.len()
    }))
}

fn action_indent(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let indent_str = get_str(params, "indent_str", "  ");
    let count = get_int(params, "count", 1) as usize;

    let prefix = indent_str.repeat(count);
    let result: String = text
        .lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(serde_json::json!({
        "result": result,
        "indent": indent_str,
        "level": count
    }))
}

fn action_slugify(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;

    // Convert to lowercase, replace non-alphanumeric with hyphens
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    Ok(serde_json::json!({
        "result": slug,
        "original": text,
        "slug_length": slug.len()
    }))
}

fn action_regex(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let pattern = get_str(params, "pattern", "");
    let mode = get_str(params, "mode", "find");

    if pattern.is_empty() {
        return Err("Missing required parameter: 'pattern' (regex pattern)".to_string());
    }

    let re = regex::Regex::new(pattern)
        .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?;

    match mode {
        "find" | "match" => {
            let matches: Vec<serde_json::Value> = re
                .find_iter(&text)
                .map(|m| {
                    serde_json::json!({
                        "start": m.start(),
                        "end": m.end(),
                        "text": m.as_str()
                    })
                })
                .collect();

            let count = matches.len();
            Ok(serde_json::json!({
                "matches": matches,
                "count": count,
                "pattern": pattern,
                "mode": "find"
            }))
        }
        "captures" => {
            let captures_list: Vec<serde_json::Value> = re
                .captures_iter(&text)
                .map(|caps| {
                    let groups: Vec<serde_json::Value> = caps
                        .iter()
                        .enumerate()
                        .map(|(i, m)| match m {
                            Some(mat) => serde_json::json!({
                                "index": i,
                                "start": mat.start(),
                                "end": mat.end(),
                                "text": mat.as_str()
                            }),
                            None => serde_json::json!(null),
                        })
                        .collect();
                    serde_json::json!({
                        "full_match": caps.get(0).map(|m| m.as_str()).unwrap_or(""),
                        "groups": groups
                    })
                })
                .collect();

            let count = captures_list.len();
            Ok(serde_json::json!({
                "captures": captures_list,
                "count": count,
                "pattern": pattern,
                "mode": "captures"
            }))
        }
        "replace" => {
            let replacement = get_str(params, "replacement", "");
            let result = re.replace_all(&text, replacement);
            let count = re.find_iter(&text).count();
            Ok(serde_json::json!({
                "result": result.to_string(),
                "replaced_count": count,
                "pattern": pattern,
                "replacement": replacement,
                "mode": "replace"
            }))
        }
        "count" => {
            let count = re.find_iter(&text).count();
            Ok(serde_json::json!({
                "count": count,
                "pattern": pattern,
                "mode": "count"
            }))
        }
        "test" | "is_match" => {
            let is_match = re.is_match(&text);
            Ok(serde_json::json!({
                "is_match": is_match,
                "pattern": pattern,
                "mode": "test"
            }))
        }
        "split" => {
            let parts: Vec<&str> = re.split(&text).collect();
            let count = parts.len();
            Ok(serde_json::json!({
                "parts": parts,
                "count": count,
                "pattern": pattern,
                "mode": "split"
            }))
        }
        _ => Err(format!(
            "Unknown regex mode: '{}'. Available: find, captures, replace, count, test, split",
            mode
        )),
    }
}

fn action_join(params: &HashMap<String, Value>) -> Result<Value, String> {
    let separator = get_str(params, "separator", ",");

    // Try to get items from JSON array parameter
    if let Some(items) = params.get("items") {
        if let Some(arr) = items.as_array() {
            let strings: Vec<&str> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            if strings.len() != arr.len() {
                return Err("All items in the array must be strings".to_string());
            }
            let result = strings.join(separator);
            return Ok(serde_json::json!({
                "result": result,
                "items_count": strings.len(),
                "separator": separator
            }));
        }
    }

    // Fallback: try to parse text as JSON array
    let text = get_text(params)?;
    let parsed: Vec<String> = serde_json::from_str(&text)
        .map_err(|_| {
            "Could not parse 'text' as JSON array. Provide 'items' parameter as JSON array of strings."
                .to_string()
        })?;

    let result = parsed.join(separator);
    Ok(serde_json::json!({
        "result": result,
        "items_count": parsed.len(),
        "separator": separator
    }))
}

fn action_split(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let separator = get_str(params, "separator", ",");

    let parts: Vec<&str> = text.split(separator).collect();
    let trimmed: Vec<String> = parts.iter().map(|s| s.trim().to_string()).collect();

    Ok(serde_json::json!({
        "parts": trimmed,
        "count": trimmed.len(),
        "separator": separator
    }))
}

fn action_length(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    Ok(serde_json::json!({
        "chars": text.chars().count(),
        "bytes": text.len(),
        "utf16_units": text.encode_utf16().count()
    }))
}

fn action_shuffle(params: &HashMap<String, Value>) -> Result<Value, String> {
    use rand::Rng;
    let text = get_text(params)?;
    let mut chars: Vec<char> = text.chars().collect();
    let mut rng = rand::thread_rng();

    // Fisher-Yates shuffle
    for i in (1..chars.len()).rev() {
        let j = rng.gen_range(0..=i);
        chars.swap(i, j);
    }

    let result: String = chars.into_iter().collect();
    Ok(serde_json::json!({
        "result": result,
        "original": text
    }))
}

fn action_substring(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let start = get_int(params, "start", 0) as usize;
    let chars: Vec<char> = text.chars().collect();

    let end = if let Some(v) = params.get("end").and_then(|v| v.as_i64()) {
        if v < 0 || v as usize > chars.len() {
            chars.len()
        } else {
            v as usize
        }
    } else {
        chars.len()
    };

    if start >= chars.len() {
        return Ok(serde_json::json!({
            "result": "",
            "start": start,
            "end": end,
            "original_length": chars.len()
        }));
    }

    let end = end.min(chars.len());
    let result: String = chars[start..end].iter().collect();

    Ok(serde_json::json!({
        "result": result,
        "start": start,
        "end": end,
        "original_length": chars.len(),
        "result_length": result.chars().count()
    }))
}

fn action_simple_replace(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let old = get_str(params, "old", "");
    let new = get_str(params, "new", "");

    if old.is_empty() {
        return Err("Missing required parameter: 'old' (text to replace)".to_string());
    }

    let result = text.replace(old, new);
    let count = text.matches(old).count();

    Ok(serde_json::json!({
        "result": result,
        "replaced_count": count,
        "old": old,
        "new": new
    }))
}

fn action_char_at(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let pos = get_int(params, "char_pos", 0) as usize;

    let chars: Vec<char> = text.chars().collect();
    if pos >= chars.len() {
        return Err(format!(
            "Position {} is out of bounds (string has {} characters)",
            pos,
            chars.len()
        ));
    }

    let ch = chars[pos];
    Ok(serde_json::json!({
        "char": ch.to_string(),
        "position": pos,
        "unicode_code_point": format!("U+{:04X}", ch as u32),
        "is_uppercase": ch.is_uppercase(),
        "is_lowercase": ch.is_lowercase(),
        "is_digit": ch.is_ascii_digit(),
        "is_whitespace": ch.is_whitespace(),
        "is_alphanumeric": ch.is_alphanumeric()
    }))
}

fn action_escape(params: &HashMap<String, Value>) -> Result<Value, String> {
    // Escapes special characters in a string (backslash escapes)
    let text = get_text(params)?;
    let result: String = text
        .chars()
        .flat_map(|c| -> Vec<char> {
            match c {
                '\n' => vec!['\\', 'n'],
                '\r' => vec!['\\', 'r'],
                '\t' => vec!['\\', 't'],
                '\\' => vec!['\\', '\\'],
                '"' => vec!['\\', '"'],
                '\'' => vec!['\\', '\''],
                '\0' => vec!['\\', '0'],
                _ if c.is_control() => {
                    let hex = format!("\\x{:02X}", c as u8);
                    hex.chars().collect()
                }
                _ => vec![c],
            }
        })
        .collect();

    Ok(serde_json::json!({
        "result": result,
        "original_length": text.len(),
        "escaped_length": result.len()
    }))
}

fn action_unescape(params: &HashMap<String, Value>) -> Result<Value, String> {
    let text = get_text(params)?;
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some('x') => {
                    let hex: String = chars.by_ref().take(2).collect();
                    if let Ok(code) = u8::from_str_radix(&hex, 16) {
                        result.push(code as char);
                    } else {
                        result.push('x');
                        result.push_str(&hex);
                    }
                }
                Some('u') => {
                    if chars.peek() == Some(&'{') {
                        chars.next(); // skip '{'
                        let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            }
                        }
                    } else {
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                result.push(ch);
                            }
                        }
                    }
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    Ok(serde_json::json!({
        "result": result,
        "original_length": text.len(),
        "unescaped_length": result.len()
    }))
}

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(TextProcessor));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_conversions() {
        assert_eq!(convert_case("hello world", "snake").unwrap(), "hello_world");
        assert_eq!(convert_case("hello_world", "camel").unwrap(), "helloWorld");
        assert_eq!(convert_case("hello world", "pascal").unwrap(), "HelloWorld");
        assert_eq!(convert_case("hello world", "kebab").unwrap(), "hello-world");
        assert_eq!(convert_case("hello world", "upper").unwrap(), "HELLO WORLD");
        assert_eq!(convert_case("HELLO WORLD", "lower").unwrap(), "hello world");
        assert!(convert_case("hello", "unknown").is_err());
    }

    #[test]
    fn test_count_all() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("count".into()));
        params.insert("text".into(), Value::String("Hello\nWorld!\n\nTest".into()));
        let result = action_count(&params).unwrap();
        assert_eq!(result["chars"], 18);
        assert_eq!(result["words"], 3);
        assert_eq!(result["lines"], 4);
        assert_eq!(result["bytes"], 18);
    }

    #[test]
    fn test_truncate_short() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Hi".into()));
        params.insert("width".into(), Value::Number(10.into()));
        let result = action_truncate(&params).unwrap();
        assert_eq!(result["result"], "Hi");
        assert!(!result["truncated"].as_bool().unwrap());
    }

    #[test]
    fn test_truncate_long() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Hello, World! This is a test.".into()));
        params.insert("width".into(), Value::Number(10.into()));
        let result = action_truncate(&params).unwrap();
        assert!(result["result"].as_str().unwrap().contains("..."));
        assert!(result["truncated"].as_bool().unwrap());
    }

    #[test]
    fn test_base64_roundtrip() {
        let text = "Hello, World!";

        let mut enc_params = HashMap::new();
        enc_params.insert("action".into(), Value::String("base64".into()));
        enc_params.insert("text".into(), Value::String(text.into()));
        enc_params.insert("mode".into(), Value::String("encode".into()));
        let encoded = action_base64(&enc_params).unwrap();
        let encoded_str = encoded["result"].as_str().unwrap().to_string();

        let mut dec_params = HashMap::new();
        dec_params.insert("action".into(), Value::String("base64".into()));
        dec_params.insert("text".into(), Value::String(encoded_str));
        dec_params.insert("mode".into(), Value::String("decode".into()));
        let decoded = action_base64(&dec_params).unwrap();
        assert_eq!(decoded["result"], text);
    }

    #[test]
    fn test_url_roundtrip() {
        let text = "hello world & more=foo";

        let mut enc_params = HashMap::new();
        enc_params.insert("action".into(), Value::String("url".into()));
        enc_params.insert("text".into(), Value::String(text.into()));
        enc_params.insert("mode".into(), Value::String("encode".into()));
        let encoded = action_url(&enc_params).unwrap();

        let mut dec_params = HashMap::new();
        dec_params.insert("action".into(), Value::String("url".into()));
        dec_params.insert("text".into(), encoded["result"].as_str().unwrap().into());
        dec_params.insert("mode".into(), Value::String("decode".into()));
        let decoded = action_url(&dec_params).unwrap();
        assert_eq!(decoded["result"], text);
    }

    #[test]
    fn test_html_escape_roundtrip() {
        let text = "<script>alert('xss')</script>";

        let mut esc_params = HashMap::new();
        esc_params.insert("action".into(), Value::String("html".into()));
        esc_params.insert("text".into(), Value::String(text.into()));
        esc_params.insert("mode".into(), Value::String("escape".into()));
        let escaped = action_html(&esc_params).unwrap();

        let mut unesc_params = HashMap::new();
        unesc_params.insert("action".into(), Value::String("html".into()));
        unesc_params.insert("text".into(), escaped["result"].as_str().unwrap().into());
        unesc_params.insert("mode".into(), Value::String("unescape".into()));
        let unescaped = action_html(&unesc_params).unwrap();
        assert_eq!(unescaped["result"], text);
    }

    #[test]
    fn test_reverse() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("reverse".into()));
        params.insert("text".into(), Value::String("hello".into()));
        let result = action_reverse(&params).unwrap();
        assert_eq!(result["result"], "olleh");
    }

    #[test]
    fn test_trim() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("  hello  ".into()));
        let result = action_trim(&params).unwrap();
        assert_eq!(result["result"], "hello");
    }

    #[test]
    fn test_repeat() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("ab".into()));
        params.insert("count".into(), Value::Number(3.into()));
        let result = action_repeat(&params).unwrap();
        assert_eq!(result["result"], "ababab");
    }

    #[test]
    fn test_uuid_format() {
        let result = action_uuid().unwrap();
        let uuid_str = result["result"].as_str().unwrap();
        assert_eq!(uuid_str.len(), 36);
        assert_eq!(uuid_str.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn test_random_length() {
        let mut params = HashMap::new();
        params.insert("count".into(), Value::Number(32.into()));
        let result = action_random(&params).unwrap();
        assert_eq!(result["result"].as_str().unwrap().len(), 32);
    }

    #[test]
    fn test_slugify() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Hello World! This is a Test.".into()));
        let result = action_slugify(&params).unwrap();
        assert_eq!(result["result"], "hello-world-this-is-a-test");
    }

    #[test]
    fn test_regex_find() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello 123 world 456".into()));
        params.insert("pattern".into(), Value::String(r"\d+".into()));
        params.insert("mode".into(), Value::String("find".into()));
        let result = action_regex(&params).unwrap();
        assert_eq!(result["count"], 2);
        assert_eq!(result["matches"][0]["text"], "123");
        assert_eq!(result["matches"][1]["text"], "456");
    }

    #[test]
    fn test_regex_replace() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello 123 world".into()));
        params.insert("pattern".into(), Value::String(r"\d+".into()));
        params.insert("replacement".into(), Value::String("NUM".into()));
        params.insert("mode".into(), Value::String("replace".into()));
        let result = action_regex(&params).unwrap();
        assert_eq!(result["result"], "hello NUM world");
    }

    #[test]
    fn test_regex_test() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello@example.com".into()));
        params.insert("pattern".into(), Value::String(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$".into()));
        params.insert("mode".into(), Value::String("test".into()));
        let result = action_regex(&params).unwrap();
        assert!(result["is_match"].as_bool().unwrap());
    }

    #[test]
    fn test_join() {
        let mut params = HashMap::new();
        params.insert("items".into(), serde_json::json!(["a", "b", "c"]));
        params.insert("separator".into(), Value::String(",".into()));
        let result = action_join(&params).unwrap();
        assert_eq!(result["result"], "a,b,c");
    }

    #[test]
    fn test_split() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("a,b,c".into()));
        params.insert("separator".into(), Value::String(",".into()));
        let result = action_split(&params).unwrap();
        assert_eq!(result["parts"].as_array().unwrap().len(), 3);
        assert_eq!(result["parts"][0], "a");
    }

    #[test]
    fn test_length() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Hello!".into()));
        let result = action_length(&params).unwrap();
        assert_eq!(result["chars"], 6);
        assert_eq!(result["bytes"], 6);
    }

    #[test]
    fn test_shuffle() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("abcd".into()));
        let result = action_shuffle(&params).unwrap();
        // Shuffled string should have same characters
        let mut sorted_result: Vec<char> = result["result"].as_str().unwrap().chars().collect();
        sorted_result.sort();
        let sorted_result: String = sorted_result.into_iter().collect();
        assert_eq!(sorted_result, "abcd");
    }

    #[test]
    fn test_substring() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Hello World".into()));
        params.insert("start".into(), Value::Number(0.into()));
        params.insert("end".into(), Value::Number(5.into()));
        let result = action_substring(&params).unwrap();
        assert_eq!(result["result"], "Hello");
    }

    #[test]
    fn test_replace_action() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello world world".into()));
        params.insert("old".into(), Value::String("world".into()));
        params.insert("new".into(), Value::String("there".into()));
        let result = action_simple_replace(&params).unwrap();
        assert_eq!(result["result"], "hello there there");
        assert_eq!(result["replaced_count"], 2);
    }

    #[test]
    fn test_char_at() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("Rust".into()));
        params.insert("char_pos".into(), Value::Number(0.into()));
        let result = action_char_at(&params).unwrap();
        assert_eq!(result["char"], "R");
        assert!(result["is_uppercase"].as_bool().unwrap());
    }

    #[test]
    fn test_pad() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hi".into()));
        params.insert("width".into(), Value::Number(5.into()));
        params.insert("mode".into(), Value::String("right".into()));
        let result = action_pad(&params).unwrap();
        assert_eq!(result["result"], "hi   ");
    }

    #[test]
    fn test_wrap() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello world foo bar baz".into()));
        params.insert("width".into(), Value::Number(10.into()));
        let result = action_wrap(&params).unwrap();
        assert!(result["lines"].as_i64().unwrap() >= 2);
    }

    #[test]
    fn test_indent() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello\nworld".into()));
        params.insert("indent_str".into(), Value::String(">>".into()));
        let result = action_indent(&params).unwrap();
        assert_eq!(result["result"], ">>hello\n>>world");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        let text = "hello\nworld\t\"quoted\"";

        let mut esc_params = HashMap::new();
        esc_params.insert("text".into(), Value::String(text.into()));
        let escaped = action_escape(&esc_params).unwrap();

        let mut unesc_params = HashMap::new();
        unesc_params.insert("text".into(), escaped["result"].as_str().unwrap().into());
        let unescaped = action_unescape(&unesc_params).unwrap();
        assert_eq!(unescaped["result"], text);
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("nonexistent".into()));
        let result = TextProcessor.execute(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_text() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("reverse".into()));
        let result = TextProcessor.execute(&params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_title_case() {
        assert_eq!(convert_case("hello_world", "title").unwrap(), "Hello World");
    }

    #[test]
    fn test_sentence_case() {
        assert_eq!(convert_case("HELLO WORLD", "sentence").unwrap(), "Hello world");
    }

    #[test]
    fn test_train_case() {
        assert_eq!(convert_case("helloWorld", "train").unwrap(), "Hello-World");
    }

    #[test]
    fn test_alternating_case() {
        let result = convert_case("hello", "alternating").unwrap();
        assert_eq!(result, "hElLo");
    }

    #[test]
    fn test_inverse_case() {
        assert_eq!(convert_case("Hello World", "inverse").unwrap(), "hELLO wORLD");
    }

    #[test]
    fn test_base64_url_safe() {
        let text = "hello??";

        let mut enc_params = HashMap::new();
        enc_params.insert("text".into(), Value::String(text.into()));
        enc_params.insert("mode".into(), Value::String("encode_url".into()));
        let encoded = action_base64(&enc_params).unwrap();
        // URL-safe base64 should not contain + or /
        let encoded_str = encoded["result"].as_str().unwrap();
        assert!(!encoded_str.contains('+'));
        assert!(!encoded_str.contains('/'));

        let mut dec_params = HashMap::new();
        dec_params.insert("text".into(), Value::String(encoded_str.into()));
        dec_params.insert("mode".into(), Value::String("decode_url".into()));
        let decoded = action_base64(&dec_params).unwrap();
        assert_eq!(decoded["result"], text);
    }

    #[test]
    fn test_regex_captures() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("John: 25, Jane: 30".into()));
        params.insert("pattern".into(), Value::String(r"(\w+): (\d+)".into()));
        params.insert("mode".into(), Value::String("captures".into()));
        let result = action_regex(&params).unwrap();
        assert_eq!(result["count"], 2);
    }

    #[test]
    fn test_regex_split() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("a,b;c".into()));
        params.insert("pattern".into(), Value::String(r"[,;]".into()));
        params.insert("mode".into(), Value::String("split".into()));
        let result = action_regex(&params).unwrap();
        assert_eq!(result["parts"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_trim_custom_chars() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("...hello...".into()));
        params.insert("mode".into(), Value::String(".".into()));
        let result = action_trim(&params).unwrap();
        assert_eq!(result["result"], "hello");
    }

    #[test]
    fn test_pad_center() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hi".into()));
        params.insert("width".into(), Value::Number(7.into()));
        params.insert("mode".into(), Value::String("center".into()));
        params.insert("padding".into(), Value::String("-".into()));
        let result = action_pad(&params).unwrap();
        assert_eq!(result["result"], "--hi---");
    }

    #[test]
    fn test_random_custom_charset() {
        let mut params = HashMap::new();
        params.insert("count".into(), Value::Number(10.into()));
        params.insert("char_set".into(), Value::String("ABC".into()));
        let result = action_random(&params).unwrap();
        let s = result["result"].as_str().unwrap();
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| c == 'A' || c == 'B' || c == 'C'));
    }

    #[test]
    fn test_char_at_out_of_bounds() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("ab".into()));
        params.insert("char_pos".into(), Value::Number(10.into()));
        let result = action_char_at(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_repeat_zero() {
        let mut params = HashMap::new();
        params.insert("text".into(), Value::String("hello".into()));
        params.insert("count".into(), Value::Number(0.into()));
        let result = action_repeat(&params).unwrap();
        assert_eq!(result["result"], "");
    }
}
