//! Individual check functions for code review.
//!
//! Each `check_*` function scans a file's content for a specific category of
//! issues and pushes findings into the provided `Vec<ReviewIssue>`.

use super::config::{ActiveChecks, ReviewIssue, Severity, Thresholds};
use super::patterns::*;
use regex::Regex;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Calculate the 1-based line number for a byte position in content.
#[inline]
pub(super) fn line_at(content: &str, pos: usize) -> usize {
    content[..pos].matches('\n').count() + 1
}

/// Check if a position in the file is within test code.
pub(super) fn is_in_test_code(content: &str, pos: usize) -> bool {
    let before = &content[..pos];
    let lines_before: Vec<&str> = before.lines().collect();
    let mut test_block_depth: i32 = 0;

    for line in lines_before.iter().rev().take(100) {
        let trimmed = line.trim();
        if trimmed == "#[test]" || trimmed == "#[cfg(test)]" || trimmed.starts_with("#[cfg(test") {
            return true;
        }
        let opens = trimmed.chars().filter(|c| *c == '{').count() as i32;
        let closes = trimmed.chars().filter(|c| *c == '}').count() as i32;
        let was_negative = test_block_depth < 0;
        test_block_depth += opens;
        test_block_depth -= closes;
        if !was_negative && test_block_depth < 0 {
            return false;
        }
        if trimmed.contains("mod tests") && trimmed.ends_with('{') {
            return true;
        }
    }
    false
}

/// Extract the variable name from context around an .unwrap() call.
pub(super) fn extract_var_name(context: &str) -> String {
    if let Some(c) = RE_UNWRAP.captures(context) {
        c.get(1).map(|m| m.as_str().to_string()).unwrap_or_else(|| "value".to_string())
    } else {
        "value".to_string()
    }
}

/// Find the byte offset of a given 1-based line number in content.
pub(super) fn find_line_start(content: &str, line_num: usize) -> usize {
    if line_num <= 1 { return 0; }
    let mut pos = 0;
    for _ in 1..line_num {
        if let Some(nl) = content[pos..].find('\n') {
            pos += nl + 1;
        } else {
            break;
        }
    }
    pos
}

/// Parse inline ignore directives from content.
pub(super) fn parse_ignore_directives(content: &str) -> Vec<(usize, String)> {
    let mut ignores: Vec<(usize, String)> = Vec::new();
    for caps in RE_IGNORE_DIRECTIVE.captures_iter(content) {
        let Some(cap) = caps.get(0) else { continue; };
        let line_num = line_at(content, cap.start());
        let checks_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        for check in checks_str.split(',') {
            let c = check.trim().to_lowercase();
            if c == "all" {
                ignores.push((line_num, "all".to_string()));
            } else if !c.is_empty() {
                ignores.push((line_num, c));
            }
        }
    }
    ignores
}

/// Check if an issue at a given line should be ignored.
#[inline]
pub(super) fn is_ignored(ignores: &[(usize, String)], line: usize, check: &str) -> bool {
    ignores.iter().any(|(l, c)| *l == line && (c == "all" || c == check))
}

// ── Check: Unsafe Code ───────────────────────────────────────────────────────

pub(super) fn check_unsafe_code(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_UNSAFE_BLOCK.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe block detected. Review for memory safety invariants.".to_string(),
            recommendation: Some(
                "Minimize unsafe code. Document safety invariants with // SAFETY: comments. \
                 Consider safe abstractions like std::cell::UnsafeCell or pin::Pin."
                    .to_string(),
            ),
        });
    }
    for caps in RE_UNSAFE_FN.captures_iter(content) {
        let Some(m) = caps.get(0) else { continue; };
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe function declaration. All callers must uphold safety preconditions.".to_string(),
            recommendation: Some(
                "Document safety preconditions in doc comments (# Safety section). \
                 Keep unsafe functions small and focused."
                    .to_string(),
            ),
        });
    }
    for caps in RE_UNSAFE_TRAIT.captures_iter(content) {
        let Some(m) = caps.get(0) else { continue; };
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Unsafe trait declaration. All implementors must uphold safety contracts.".to_string(),
            recommendation: Some("Prefer safe traits with internal unsafe impls. If necessary, document all safety invariants.".to_string()),
        });
    }
    for m in RE_PTR_DEREF.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Raw pointer dereference. Only valid inside unsafe blocks.".to_string(),
            recommendation: Some("Ensure pointer is aligned, non-null, and points to valid memory. Use .as_ref() / .as_mut() instead.".to_string()),
        });
    }
    for m in RE_TRANSMUTE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "unsafe".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "mem::transmute used. Type layout assumptions are fragile.".to_string(),
            recommendation: Some("Use safe alternatives: bytemuck, From/Into, or TryFrom. If transmute is necessary, add a SAFETY comment.".to_string()),
        });
    }
}

// ── Check: Error Handling ────────────────────────────────────────────────────

pub(super) fn check_error_handling(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_UNWRAP.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        let context = &content[m.start()..std::cmp::min(m.end() + 40, content.len())];
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!(".unwrap() call. Will panic if `{}` is None/Err.", extract_var_name(context)),
            recommendation: Some("Use `?` operator, .ok_or() with context, .unwrap_or_default(), or match.".to_string()),
        });
    }
    for m in RE_EXPECT.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".expect() call. Will panic on error.".to_string(),
            recommendation: Some("Use `?` operator or anyhow::Context for error propagation. Reserve .expect() for unrecoverable states only.".to_string()),
        });
    }
    for m in RE_PANIC.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "panic!() in production code. Return an error instead.".to_string(),
            recommendation: Some("Replace with return Err(...), anyhow::bail!(), or anyhow::ensure!().".to_string()),
        });
    }
    for m in RE_IGNORE_RESULT.find_iter(content) {
        let line = &content[m.start()..m.end()];
        if line.contains("write") || line.contains("read") || line.contains("send")
            || line.contains("save") || line.contains("remove") || line.contains("delete")
            || line.contains("create") || line.contains("insert") || line.contains("update")
        {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "error_handling".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Result ignored via `let _ = ...`. Errors silently discarded.".to_string(),
                recommendation: Some("Handle errors: if let Err(e) = ... { log::error!(...); } or .inspect_err(|e| ...).".to_string()),
            });
        }
    }
    for m in RE_WRITELN_RESULT.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "error_handling".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Result from write!/writeln! is ignored.".to_string(),
            recommendation: Some("Use .ok() to explicitly ignore, or add `?` to propagate.".to_string()),
        });
    }
}

// ── Check: Performance ───────────────────────────────────────────────────────

pub(super) fn check_performance(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_CLONE.find_iter(content) {
        if m.as_str().contains("Arc::clone") { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".clone() call. May cause unnecessary allocations.".to_string(),
            recommendation: Some("Consider borrowing instead. Use Cow or Arc for shared ownership.".to_string()),
        });
    }
    for m in RE_BOX_NEW.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Box::new() heap allocation.".to_string(),
            recommendation: Some("Only box when necessary: trait objects, recursive types, or large data across .await.".to_string()),
        });
    }
    for m in RE_VEC_CAPACITY.find_iter(content) {
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(cap) = digits.parse::<usize>() {
            if cap > 10_000 {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "performance".to_string(),
                    file: file.to_string(),
                    line: line_at(content, m.start()),
                    column: 1,
                    message: format!("Large Vec allocation ({cap} elements). May cause memory pressure."),
                    recommendation: Some("Consider streaming approach or Box<[T]> for fixed-size buffers.".to_string()),
                });
            }
        }
    }
    for m in RE_COLLECT_VEC.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "performance".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "collect::<Vec<_>>() intermediate allocation.".to_string(),
            recommendation: Some("Chain iterator adaptors directly: .map(), .filter(), .take().".to_string()),
        });
    }
}

// ── Check: Style ─────────────────────────────────────────────────────────────

pub(super) fn check_style(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>, thresholds: &Thresholds) {
    // Long functions
    let fn_positions: Vec<(usize, String)> = RE_FN_START.captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            Some((line_at(content, m0.start()), name))
        })
        .collect();

    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let end_line = if i + 1 < fn_positions.len() { fn_positions[i + 1].0 } else { lines.len() };
        let length = end_line - start_line;
        if length > thresholds.max_fn_length {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "style".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` is {length} lines (max: {}). Refactor.", thresholds.max_fn_length),
                recommendation: Some("Split into smaller functions. Apply single-responsibility principle.".to_string()),
            });
        } else if length > thresholds.max_fn_length / 2 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "style".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` is {length} lines."),
                recommendation: Some("Consider extracting helper functions for readability.".to_string()),
            });
        }
    }

    // Deep nesting
    let mut max_nesting_val = 0usize;
    let mut current_nesting = 0usize;
    let mut first_deep_line: Option<usize> = None;
    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("* ") { continue; }
        for ch in trimmed.chars() {
            match ch {
                '{' | '[' => {
                    current_nesting += 1;
                    if current_nesting > thresholds.max_nesting && first_deep_line.is_none() {
                        first_deep_line = Some(line_idx + 1);
                    }
                    max_nesting_val = max_nesting_val.max(current_nesting);
                }
                '}' | ']' => { current_nesting = current_nesting.saturating_sub(1); }
                _ => {}
            }
        }
    }
    if max_nesting_val > thresholds.max_nesting {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "style".to_string(),
            file: file.to_string(),
            line: first_deep_line.unwrap_or(1),
            column: 1,
            message: format!("Deep nesting (max depth: {max_nesting_val}, limit: {}). Refactor.", thresholds.max_nesting),
            recommendation: Some("Extract nested logic, use early returns, guard clauses, or iterator combinators.".to_string()),
        });
    }

    // Non-snake_case functions
    for caps in RE_NON_SNAKE_FN.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "style".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Function `{}` should be snake_case.", name.as_str()),
                recommendation: Some("Rename to snake_case. Use CamelCase for types, snake_case for functions.".to_string()),
            });
        }
    }

    // TODO/FIXME comments
    for caps in RE_TODO_COMMENT.captures_iter(content) {
        let tag = caps.get(1).map(|t| t.as_str()).unwrap_or("TODO");
        let Some(m0) = caps.get(0) else { continue; };
        let line_num = line_at(content, m0.start()) - 1;
        let s: &str = lines.get(line_num).map(|s| s.trim()).unwrap_or("");
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "style".to_string(),
            file: file.to_string(),
            line: line_num + 1,
            column: 1,
            message: format!("{tag} comment: \"{s}\""),
            recommendation: Some("Resolve before committing. Create tasks for each TODO.".to_string()),
        });
    }

    // Long lines
    for (idx, line) in lines.iter().enumerate() {
        if line.len() > thresholds.max_line_length {
            let trimmed = line.trim();
            if trimmed.starts_with("//") { continue; }
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "style".to_string(),
                file: file.to_string(),
                line: idx + 1,
                column: thresholds.max_line_length + 1,
                message: format!("Line is {} chars (max: {}).", line.len(), thresholds.max_line_length),
                recommendation: Some("Break long lines at logical points. Use rustfmt.".to_string()),
            });
        }
    }
}

// ── Check: Safety ────────────────────────────────────────────────────────────

pub(super) fn check_safety(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_TRANSMUTE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "mem::transmute: type sizes must match. Extremely unsafe.".to_string(),
            recommendation: Some("Use bytemuck::Pod for plain-data casts, or From/Into/TryFrom.".to_string()),
        });
    }
    for m in RE_MAYBE_UNINIT.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "MaybeUninit: incorrect use causes UB.".to_string(),
            recommendation: Some("Initialize all bytes before calling .assume_init(). Prefer safe init patterns.".to_string()),
        });
    }
    for m in RE_PTR_OFFSET.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "safety".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: ".offset() pointer arithmetic: out-of-bounds risk.".to_string(),
            recommendation: Some("Use .add()/.sub() (still unsafe but clearer). Ensure bounds checking.".to_string()),
        });
    }
}

// ── Check: Correctness ───────────────────────────────────────────────────────

pub(super) fn check_correctness(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_TODO_MACRO.find_iter(content) {
        let context = &content[m.start()..std::cmp::min(m.start() + 60, content.len())];
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("todo!(): \"{}\"", context.trim()),
            recommendation: Some("Implement the missing functionality. Use anyhow::bail!() for runtime errors.".to_string()),
        });
    }
    for m in RE_UNIMPLEMENTED.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "unimplemented!(): will panic at runtime.".to_string(),
            recommendation: Some("Implement the method body before committing.".to_string()),
        });
    }
    for m in RE_UNREACHABLE.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "unreachable!(): confirm this path is truly impossible.".to_string(),
            recommendation: Some("Only for logically impossible states. Consider if the type system can prove it.".to_string()),
        });
    }
    for m in RE_MATCH.find_iter(content) {
        let remaining = &content[m.end()..std::cmp::min(m.end() + 2000, content.len())];
        if !remaining.contains("_ =>") && !remaining.contains("other =>") && remaining.contains("::") {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "correctness".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Match without wildcard `_ =>` arm. Will fail on new enum variants.".to_string(),
                recommendation: Some("Add `_ => { ... }` arm. Or use #[non_exhaustive] on the enum.".to_string()),
            });
        }
    }
    for m in RE_DEREF_IMPL.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Info,
            check: "correctness".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Deref impl: coercions cause subtle bugs.".to_string(),
            recommendation: Some("Implement Deref only for smart pointers. Avoid Deref for method delegation (anti-pattern).".to_string()),
        });
    }
}

// ── Check: Concurrency ───────────────────────────────────────────────────────

pub(super) fn check_concurrency(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_REFCELL.find_iter(content) {
        let surrounding = &content[m.start().saturating_sub(30)..std::cmp::min(m.end() + 30, content.len())];
        if surrounding.contains("static") || surrounding.contains("lazy_static") || surrounding.contains("OnceCell") {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "concurrency".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "RefCell/Cell with shared/static state: not thread-safe.".to_string(),
                recommendation: Some("Use Mutex<T> or RwLock<T> for thread-safe interior mutability.".to_string()),
            });
        }
    }
    for m in RE_STD_MUTEX.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "std::sync::Mutex: may block thread in async context.".to_string(),
            recommendation: Some("Use tokio::sync::Mutex in async code if holding across .await points.".to_string()),
        });
    }
    for m in RE_UNSAFE_SEND_SYNC.find_iter(content) {
        let trait_name = if m.as_str().contains("Send") { "Send" } else { "Sync" };
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("Unsafe impl {trait_name}: incorrect guarantees => UB."),
            recommendation: Some("Only impl Send/Sync manually if certain of thread-safety. Use internal sync primitives.".to_string()),
        });
    }
    for m in RE_STATIC_MUT.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "concurrency".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "static mut: UB without sync. Use Mutex/RwLock/OnceLock.".to_string(),
            recommendation: Some("Use `static` with Mutex<T> for mutable global state, or OnceLock for lazy init.".to_string()),
        });
    }
    for m in RE_ARC.find_iter(content) {
        let context = &content[m.start()..std::cmp::min(m.start() + 50, content.len())];
        let non_send = ["Rc<", "RefCell<", "Cell<", "raw pointer", "*const", "*mut"];
        if non_send.iter().any(|t| context.contains(t)) {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "concurrency".to_string(),
                file: file.to_string(),
                line: line_at(content, m.start()),
                column: 1,
                message: "Arc wraps non-Send type: not thread-safe.".to_string(),
                recommendation: Some("Use Arc<Mutex<T>> for thread-safe shared ownership.".to_string()),
            });
        }
    }
}

// ── Check: Documentation ─────────────────────────────────────────────────────

pub(super) fn check_documentation(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    let pub_item_patterns: Vec<(&str, &str)> = vec![
        (r"(?m)^\s*pub\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)", "function"),
        (r"(?m)^\s*pub\s+struct\s+([a-zA-Z_][a-zA-Z0-9_]*)", "struct"),
        (r"(?m)^\s*pub\s+(?:enum|union)\s+([a-zA-Z_][a-zA-Z0-9_]*)", "enum"),
        (r"(?m)^\s*pub\s+trait\s+([a-zA-Z_][a-zA-Z0-9_]*)", "trait"),
        (r"(?m)^\s*pub\s+type\s+([a-zA-Z_][a-zA-Z0-9_]*)", "type alias"),
        (r"(?m)^\s*pub\s+const\s+([a-zA-Z_][a-zA-Z0-9_]*)", "constant"),
    ];
    for (pattern, item_type) in &pub_item_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for caps in re.captures_iter(content) {
                let m0 = match caps.get(0) { Some(m) => m, None => continue };
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                if *item_type == "function" && (name.starts_with("test_") || name == "new") { continue; }
                let before = &content[..m0.start()];
                let has_doc = before.lines().rev().take(5).any(|l| {
                    let t = l.trim();
                    t.starts_with("///") || t.starts_with("/**") || t.starts_with("* ")
                });
                if !has_doc {
                    issues.push(ReviewIssue {
                        severity: Severity::Warning,
                        check: "documentation".to_string(),
                        file: file.to_string(),
                        line: line_at(content, m0.start()),
                        column: 1,
                        message: format!("Public {item_type} `{name}` missing doc comments."),
                        recommendation: Some("Add /// comments: purpose, params, returns, panics, errors.".to_string()),
                    });
                }
            }
        }
    }
    // Functions with params but no parameter docs
    for caps in RE_PUB_FN.captures_iter(content) {
        let m0 = match caps.get(0) { Some(m) => m, None => continue };
        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let sig_end = content[m0.start()..].find('{').map(|i| m0.start() + i).unwrap_or(content.len());
        let sig = &content[m0.start()..sig_end];
        if sig.contains('(') && !sig.contains("()") {
            let before = &content[..m0.start()];
            let has_param_docs = before.lines().rev().take(15).any(|l| {
                let t = l.trim();
                (t.starts_with("///") && (t.contains("- ") || t.contains('`')))
                    || t.contains("# Parameters") || t.contains("# Arguments")
            });
            if !has_param_docs {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "documentation".to_string(),
                    file: file.to_string(),
                    line: line_at(content, m0.start()),
                    column: 1,
                    message: format!("Public fn `{name}` has undocmented parameters."),
                    recommendation: Some("Add `# Parameters` section: `name` - description.".to_string()),
                });
            }
        }
    }
}

// ── Check: Naming Conventions ────────────────────────────────────────────────

pub(super) fn check_naming(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // Use helper from config module (it's also exported there for tests)
    use super::config::to_camel_case;

    for caps in RE_LOWERCASE_STRUCT.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Struct `{}` should use CamelCase (e.g. `{}`).", name.as_str(), to_camel_case(name.as_str())),
                recommendation: Some("Rename to CamelCase: type names use PascalCase convention.".to_string()),
            });
        }
    }
    for caps in RE_LOWERCASE_ENUM.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Enum `{}` should use CamelCase (e.g. `{}`).", name.as_str(), to_camel_case(name.as_str())),
                recommendation: Some("Rename to CamelCase: type names use PascalCase convention.".to_string()),
            });
        }
    }
    for caps in RE_NON_SCREAMING_CONST.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let n = name.as_str();
            if n.chars().any(|c| c.is_uppercase()) && n.contains('_') { continue; }
            if n.len() <= 2 { continue; }
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Constant `{n}` should use SCREAMING_SNAKE_CASE."),
                recommendation: Some("Rust convention: const values use UPPER_SNAKE_CASE naming.".to_string()),
            });
        }
    }
    for caps in RE_NON_SCREAMING_STATIC.captures_iter(content) {
        if let Some(name) = caps.get(1) {
            let n = name.as_str();
            if n.chars().any(|c| c.is_uppercase()) && n.contains('_') { continue; }
            if n.len() <= 2 { continue; }
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "naming".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Static `{n}` should use SCREAMING_SNAKE_CASE."),
                recommendation: Some("Rust convention: static values use UPPER_SNAKE_CASE.".to_string()),
            });
        }
    }
}

// ── Check: Async Pitfalls ────────────────────────────────────────────────────

fn has_recent_async_fn(before: &str) -> bool {
    let recent: Vec<&str> = before.lines().rev().take(100).collect();
    for line in &recent {
        let t = line.trim();
        if t.starts_with("async fn") || t.starts_with("pub async fn") || t.starts_with("pub(crate) async fn") {
            return true;
        }
        if t == "}" { return false; }
    }
    false
}

pub(super) fn check_async(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    let is_async_file = RE_ASYNC_FN.find(content).is_some() || RE_AWAIT.find(content).is_some();
    if !is_async_file { return; }
    for m in RE_STD_MUTEX_LOCK.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "async".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "std::sync::Mutex::lock() in async code: will block the thread.".to_string(),
            recommendation: Some("Use tokio::sync::Mutex or futures::lock::Mutex for async code.".to_string()),
        });
    }
    if RE_ASYNC_FN.find(content).is_some() {
        for m in RE_BLOCKING_IO.find_iter(content) {
            let before_pos = &content[..m.start()];
            if has_recent_async_fn(before_pos) {
                issues.push(ReviewIssue {
                    severity: Severity::Info,
                    check: "async".to_string(),
                    file: file.to_string(),
                    line: line_at(content, m.start()),
                    column: 1,
                    message: "Potential blocking I/O in async context. Use async alternatives.".to_string(),
                    recommendation: Some("Use tokio::fs, tokio::net, or tokio::process instead of std. Or use spawn_blocking().".to_string()),
                });
            }
        }
    }
}

// ── Check: Security ──────────────────────────────────────────────────────────

pub(super) fn check_security(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_SQL_INJECTION.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Possible SQL injection: format!/concat! with SQL query containing interpolated variables.".to_string(),
            recommendation: Some("Use parameterized queries (sqlx::query! or diesel) instead of string formatting. Never concatenate user input into SQL strings.".to_string()),
        });
    }
    for m in RE_HARDCODED_SECRET.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Possible hardcoded secret (API key, password, or token).".to_string(),
            recommendation: Some("Use environment variables (std::env::var), .env files, or a secrets manager. Never commit secrets to version control.".to_string()),
        });
    }
    for m in RE_PRIVATE_KEY.find_iter(content) {
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "Private key content detected in source code!".to_string(),
            recommendation: Some("Remove private key from source. Use environment variables or a secrets manager. Rotate the compromised key immediately.".to_string()),
        });
    }
    for m in RE_OPENAI_KEY.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        issues.push(ReviewIssue {
            severity: Severity::Error,
            check: "security".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: "OpenAI API key detected in source code!".to_string(),
            recommendation: Some("Use environment variables (std::env::var(\"OPENAI_API_KEY\")) instead. Rotate the compromised key immediately.".to_string()),
        });
    }
}

// ── Check: Complexity ────────────────────────────────────────────────────────

pub(super) fn check_complexity(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    // Large file check
    if lines.len() > 1000 {
        issues.push(ReviewIssue {
            severity: Severity::Warning,
            check: "complexity".to_string(),
            file: file.to_string(),
            line: 1,
            column: 1,
            message: format!("File is {} lines (>1000). Consider splitting.", lines.len()),
            recommendation: Some("Split into smaller modules. Aim for <500 lines per file for maintainability.".to_string()),
        });
    }

    // Excessive function parameters
    for caps in RE_FN_PARAMS.captures_iter(content) {
        let params_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let param_count = params_str.split(',').map(|s| s.trim()).filter(|s| {
            !s.is_empty() && *s != "self" && *s != "&self" && *s != "&mut self" && !s.starts_with("self:")
        }).count();
        if param_count > 5 {
            let Some(m0) = caps.get(0) else { continue; };
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: line_at(content, m0.start()),
                column: 1,
                message: format!("Function has {param_count} parameters (max: 5). Consider refactoring."),
                recommendation: Some("Use a struct to group related parameters, or split the function.".to_string()),
            });
        }
    }

    // Cyclomatic complexity
    let fn_positions: Vec<(usize, String)> = RE_FN_START.captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            Some((line_at(content, m0.start()), name))
        })
        .collect();

    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let start_byte = find_line_start(content, *start_line);
        let end_byte = if i + 1 < fn_positions.len() {
            find_line_start(content, fn_positions[i + 1].0)
        } else { content.len() };
        if start_byte >= end_byte { continue; }
        let fn_body = &content[start_byte..end_byte];

        let mut complexity = 1;
        for m in fn_body.match_indices("if ") {
            let before = &fn_body[..m.0];
            let prev_ch = before.chars().last().unwrap_or(' ');
            if prev_ch == ' ' || prev_ch == '\t' || prev_ch == '\n' || prev_ch == '{' || prev_ch == ';' {
                complexity += 1;
            }
        }
        for _m in fn_body.match_indices("else if ") { complexity += 1; }
        complexity += fn_body.matches("=>").count();
        complexity += fn_body.matches("for ").count();
        complexity += fn_body.matches("while ").count();
        complexity += fn_body.matches("loop ").count();
        complexity += fn_body.matches("&&").count();
        complexity += fn_body.matches("||").count();

        if complexity > 15 {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` has high cyclomatic complexity (~{complexity}, max: 15). Refactor."),
                recommendation: Some("Extract conditions into helper functions, use early returns, or simplify match arms.".to_string()),
            });
        } else if complexity > 10 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "complexity".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Function `{name}` has moderate cyclomatic complexity (~{complexity})."),
                recommendation: Some("Consider extracting helper functions to improve readability.".to_string()),
            });
        }
    }
}

// ── Check: Testing Quality ───────────────────────────────────────────────────

pub(super) fn check_testing(content: &str, lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    let test_positions: Vec<usize> = RE_TEST_FN.find_iter(content)
        .map(|m| line_at(content, m.start()))
        .collect();
    for test_line in &test_positions {
        let fn_body_start = find_line_start(content, *test_line + 1);
        let search_start = content[fn_body_start..].find('\n')
            .map(|pos| fn_body_start + pos + 1).unwrap_or(fn_body_start);
        let next_test = content[search_start..].find("#[test]")
            .map(|pos| search_start + pos).unwrap_or(content.len());
        let test_body = &content[fn_body_start..next_test];
        let assert_count = RE_ASSERT_MACRO.find_iter(test_body).count();
        if assert_count == 0 {
            issues.push(ReviewIssue {
                severity: Severity::Warning,
                check: "testing".to_string(),
                file: file.to_string(),
                line: *test_line,
                column: 1,
                message: "Test function has no assertions. May not provide value.".to_string(),
                recommendation: Some("Add assertions (assert_eq!, assert_ne!, assert!) to verify expected behavior.".to_string()),
            });
        }
    }
    // Overly long test functions
    let fn_positions: Vec<(usize, String)> = RE_FN_START.captures_iter(content)
        .filter_map(|c| {
            let name = c.get(1).map(|m| m.as_str().to_string())?;
            let m0 = c.get(0)?;
            let line_num = line_at(content, m0.start());
            let before = &content[..m0.start()];
            if before.lines().rev().take(3).any(|l| l.trim() == "#[test]") {
                Some((line_num, name))
            } else { None }
        })
        .collect();
    for i in 0..fn_positions.len() {
        let (start_line, name) = &fn_positions[i];
        let end_line = if i + 1 < fn_positions.len() { fn_positions[i + 1].0 } else { lines.len() };
        let length = end_line - start_line;
        if length > 100 {
            issues.push(ReviewIssue {
                severity: Severity::Info,
                check: "testing".to_string(),
                file: file.to_string(),
                line: *start_line,
                column: 1,
                message: format!("Test function `{name}` is {length} lines. Consider splitting."),
                recommendation: Some("Split into multiple focused test cases, or use test parameterization.".to_string()),
            });
        }
    }
}

// ── Check: Debug Residuals ───────────────────────────────────────────────────

pub(super) fn check_debug(content: &str, _lines: &[&str], file: &str, issues: &mut Vec<ReviewIssue>) {
    for m in RE_DBG_MACRO.find_iter(content) {
        if is_in_test_code(content, m.start()) { continue; }
        let macro_name = if m.as_str().contains("dbg!") { "dbg!" }
            else if m.as_str().contains("eprintln!") { "eprintln!" }
            else { "println!" };

        if macro_name == "println!" {
            if file.ends_with("main.rs") || file.ends_with("bin/") { continue; }
            let before = &content[..m.start()];
            let recent_lines: Vec<&str> = before.lines().rev().take(20).collect();
            let in_display = recent_lines.iter().any(|l| {
                let t = l.trim();
                t.contains("impl") && (t.contains("Display") || t.contains("Debug") || t.contains("fmt::Formatter"))
            });
            if in_display { continue; }
        }
        issues.push(ReviewIssue {
            severity: if macro_name == "dbg!" { Severity::Warning } else { Severity::Info },
            check: "debug".to_string(),
            file: file.to_string(),
            line: line_at(content, m.start()),
            column: 1,
            message: format!("{macro_name}() call in production code. Debug residual."),
            recommendation: Some("Remove debug statements before committing. Use logging (log::info!) for persistent output.".to_string()),
        });
    }
}

// ── Apply all checks for a single file ────────────────────────────────────────

/// Run all enabled checks on file content. Returns the list of issues found.
/// The caller applies ignore directives and severity filtering.
pub(super) fn run_all_checks(
    content: &str,
    lines: &[&str],
    file: &str,
    active_checks: &ActiveChecks,
    thresholds: &Thresholds,
) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    if active_checks.unsafe_check {
        check_unsafe_code(content, lines, file, &mut issues);
    }
    if active_checks.error_handling {
        check_error_handling(content, lines, file, &mut issues);
    }
    if active_checks.performance {
        check_performance(content, lines, file, &mut issues);
    }
    if active_checks.style {
        check_style(content, lines, file, &mut issues, thresholds);
    }
    if active_checks.safety {
        check_safety(content, lines, file, &mut issues);
    }
    if active_checks.correctness {
        check_correctness(content, lines, file, &mut issues);
    }
    if active_checks.concurrency {
        check_concurrency(content, lines, file, &mut issues);
    }
    if active_checks.documentation {
        check_documentation(content, lines, file, &mut issues);
    }
    if active_checks.naming {
        check_naming(content, lines, file, &mut issues);
    }
    if active_checks.async_check {
        check_async(content, lines, file, &mut issues);
    }
    if active_checks.security {
        check_security(content, lines, file, &mut issues);
    }
    if active_checks.complexity {
        check_complexity(content, lines, file, &mut issues);
    }
    if active_checks.testing {
        check_testing(content, lines, file, &mut issues);
    }
    if active_checks.debug {
        check_debug(content, lines, file, &mut issues);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::{ActiveChecks, Thresholds};

    #[test]
    fn test_line_at() {
        let content = "line1\nline2\nline3\n";
        assert_eq!(line_at(content, 0), 1);
        assert_eq!(line_at(content, 5), 1);
        assert_eq!(line_at(content, 6), 2);
        assert_eq!(line_at(content, 12), 3);
        assert_eq!(line_at(content, 17), 3);
    }

    #[test]
    fn test_extract_var_name() {
        assert_eq!(extract_var_name("x.unwrap()"), "x");
        assert_eq!(extract_var_name("something_else.unwrap()"), "something_else");
    }

    #[test]
    fn test_is_in_test_code() {
        let code = r#"
fn main() {
    let x = Some(1);
    let y = x.unwrap();
}

#[test]
fn test_foo() {
    let x = Some(1);
    let y = x.unwrap();
}
"#;
        let main_unwrap_pos = code.find("x.unwrap()").unwrap();
        assert!(!is_in_test_code(code, main_unwrap_pos));

        let test_unwrap_pos = code.rfind("x.unwrap()").unwrap();
        assert!(is_in_test_code(code, test_unwrap_pos));
    }

    #[test]
    fn test_is_in_test_code_mod_tests() {
        let code = r#"
fn main() {
    let x = Some(1);
}

mod tests {
    fn helper() {
        let x = Some(1);
        x.unwrap();
    }
}
"#;
        let unwrap_pos = code.rfind("x.unwrap()").unwrap();
        assert!(is_in_test_code(code, unwrap_pos));
    }

    #[test]
    fn test_check_unsafe_code_basic() {
        let code = r#"
fn main() {
    let x = 5;
    let ptr = &x as *const i32;
    unsafe {
        println!("{}", *ptr);
    }
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_unsafe_code(code, &lines, "test.rs", &mut issues);
        assert!(!issues.is_empty(), "Should detect unsafe block and ptr deref");
        assert!(issues.iter().any(|i| i.check == "unsafe"));
    }

    #[test]
    fn test_check_error_handling_unwrap() {
        let code = r#"
fn main() {
    let x: Option<i32> = Some(5);
    let y = x.unwrap();
    let z = x.expect("should exist");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_error_handling(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("unwrap")));
        assert!(issues.iter().any(|i| i.message.contains("expect")));
    }

    #[test]
    fn test_check_naming_bad_struct() {
        let code = r#"
struct myStruct {
    field: i32,
}

enum myEnum {
    Variant,
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("myStruct")), "Should flag lowercase struct");
        assert!(issues.iter().any(|i| i.message.contains("myEnum")), "Should flag lowercase enum");
    }

    #[test]
    fn test_check_naming_good_struct() {
        let code = r#"
struct MyStruct {
    field: i32,
}

enum MyEnum {
    Variant,
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        let naming_issues: Vec<_> = issues.iter().filter(|i| i.check == "naming").collect();
        assert!(naming_issues.is_empty(), "Should not flag correct CamelCase: {:?}", naming_issues);
    }

    #[test]
    fn test_check_naming_constants() {
        let code = r#"
const max_size: usize = 100;
const MAX_SIZE: usize = 100;
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_naming(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("max_size")), "Should flag lowercase const");
        assert!(!issues.iter().any(|i| i.message.contains("MAX_SIZE")), "Should not flag SCREAMING const");
    }

    #[test]
    fn test_check_style_long_function() {
        let mut code = String::from("fn long_function() {\n");
        for _ in 0..120 { code.push_str("    let _ = 1;\n"); }
        code.push_str("}\n");
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        let thresholds = Thresholds { max_fn_length: 50, max_nesting: 8, max_line_length: 120 };
        check_style(&code, &lines, "test.rs", &mut issues, &thresholds);
        assert!(issues.iter().any(|i| i.check == "style" && i.message.contains("long_function")));
    }

    #[test]
    fn test_check_correctness_todo() {
        let code = r#"
fn main() {
    todo!("implement this");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_correctness(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("todo")));
    }

    #[test]
    fn test_check_performance_clone() {
        let code = r#"
fn main() {
    let s = String::from("hello");
    let s2 = s.clone();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_performance(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("clone")));
    }

    #[test]
    fn test_check_safety_maybe_uninit() {
        let code = r#"
use std::mem::MaybeUninit;
fn main() {
    let mut x = MaybeUninit::uninit();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_safety(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("MaybeUninit")));
    }

    #[test]
    fn test_check_concurrency_static_mut() {
        let code = r#"
static mut COUNTER: i32 = 0;
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_concurrency(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("static mut")));
    }

    #[test]
    fn test_check_documentation_missing() {
        let code = r#"
pub fn undocumented() -> i32 { 5 }

/// Documented function
pub fn documented() -> i32 { 5 }
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_documentation(code, &lines, "test.rs", &mut issues);
        let has_undocumented = issues.iter().any(|i| i.message.contains("`undocumented`"));
        let has_documented = issues.iter().any(|i| i.message.contains("`documented`"));
        assert!(has_undocumented, "Should flag undocumented fn, issues: {:?}", issues);
        assert!(!has_documented, "Should not flag documented fn, issues: {:?}", issues);
    }

    #[test]
    fn test_check_async_blocking_mutex() {
        let code = r#"
async fn async_fn() {
    let m = std::sync::Mutex::new(5);
    let _g = m.lock().unwrap();
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_async(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.check == "async"), "Should detect blocking mutex in async");
    }

    #[test]
    fn test_check_security_sql_injection() {
        let code = r#"
fn bad_query(user: &str) {
    let sql = format!("SELECT * FROM users WHERE name = '{}'", user);
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("SQL injection")), "Should detect SQL injection, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_hardcoded_secret() {
        let code = r#"
fn configure() {
    let api_key = "sk-abcdefghijklmnopqrstuvwxyz123456";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("hardcoded secret")), "Should detect hardcoded secret, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_private_key() {
        let code = r#"
fn main() {
    let key = "-----BEGIN RSA PRIVATE KEY-----abc123-----END RSA PRIVATE KEY-----";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("Private key")), "Should detect private key, issues: {:?}", issues);
    }

    #[test]
    fn test_check_security_openai_key() {
        let code = r#"
fn call_llm() {
    let key = "sk-abcdefghijklmnopqrstuvwxyz123456";
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_security(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("OpenAI API key")), "Should detect OpenAI key, issues: {:?}", issues);
    }

    #[test]
    fn test_parse_ignore_directives_single() {
        let code = "// code-review: ignore[unsafe]\nunsafe {}\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].0, 1);
        assert_eq!(ignores[0].1, "unsafe");
    }

    #[test]
    fn test_parse_ignore_directives_multiple() {
        let code = "// code-review: ignore[unsafe,style,error_handling]\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 3);
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "unsafe"));
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "style"));
        assert!(ignores.iter().any(|(l, c)| *l == 1 && c == "error_handling"));
    }

    #[test]
    fn test_parse_ignore_directives_all() {
        let code = "// code-review: ignore[all]\n";
        let ignores = parse_ignore_directives(code);
        assert_eq!(ignores.len(), 1);
        assert_eq!(ignores[0].1, "all");
    }

    #[test]
    fn test_is_ignored_exact() {
        let ignores = vec![(5, "unsafe".to_string()), (10, "style".to_string())];
        assert!(is_ignored(&ignores, 5, "unsafe"));
        assert!(!is_ignored(&ignores, 5, "style"));
        assert!(is_ignored(&ignores, 10, "style"));
        assert!(!is_ignored(&ignores, 10, "unsafe"));
        assert!(!is_ignored(&ignores, 3, "unsafe"));
    }

    #[test]
    fn test_is_ignored_all() {
        let ignores = vec![(5, "all".to_string())];
        assert!(is_ignored(&ignores, 5, "unsafe"));
        assert!(is_ignored(&ignores, 5, "style"));
        assert!(is_ignored(&ignores, 5, "error_handling"));
        assert!(!is_ignored(&ignores, 6, "unsafe"));
    }

    #[test]
    fn test_ignore_directive_suppresses_issue() {
        let code = "unsafe { let x = 1; } // code-review: ignore[unsafe]\n";
        let lines: Vec<&str> = code.lines().collect();
        let ignores = parse_ignore_directives(code);
        let mut issues = Vec::new();
        check_unsafe_code(code, &lines, "test.rs", &mut issues);
        issues.retain(|i| !is_ignored(&ignores, i.line, &i.check));
        assert!(issues.is_empty(), "Unsafe issue should be suppressed by ignore directive, got: {:?}", issues);
    }

    // ── Complexity tests ──────────────────────────────────────────────────────

    #[test]
    fn test_check_complexity_large_file() {
        let mut code = String::new();
        for i in 0..1200 { code.push_str(&format!("let x_{} = {};\n", i, i)); }
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(&code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains(">1000")), "Should flag large file, got: {:?}", issues);
    }

    #[test]
    fn test_check_complexity_excessive_params() {
        let code = r#"
fn bad_function(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("parameters")), "Should flag excessive params, got: {:?}", issues);
    }

    #[test]
    fn test_check_complexity_ok_params() {
        let code = r#"
fn good_function(a: i32, b: i32) {}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        let param_issues: Vec<_> = issues.iter().filter(|i| i.message.contains("parameters")).collect();
        assert!(param_issues.is_empty(), "Should not flag 2 params, got: {:?}", param_issues);
    }

    #[test]
    fn test_check_complexity_high_cyclomatic() {
        let code = r#"
fn complex_function(x: i32) -> i32 {
    let mut result = 0;
    if x > 0 && x < 100 {
        if x > 10 || x < 5 {
            if x > 20 {
                if x < 30 {
                    result = 1;
                }
            }
        }
    }
    match x {
        1 => result = 10,
        2 => result = 20,
        3 => result = 30,
        _ => result = 0,
    }
    for i in 0..x { result += i; }
    while result > 0 { result -= 1; }
    result
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_complexity(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("complexity")), "Should flag high cyclomatic complexity, got: {:?}", issues);
    }

    // ── Testing quality tests ─────────────────────────────────────────────────

    #[test]
    fn test_check_testing_no_assertions() {
        let code = r#"
#[test]
fn test_no_assert() {
    let x = 5;
    let y = 10;
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("no assertions")), "Should flag test without assertions, got: {:?}", issues);
    }

    #[test]
    fn test_check_testing_with_assertions() {
        let code = r#"
#[test]
fn test_with_assert() {
    let x = 5;
    assert_eq!(x, 5);
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(code, &lines, "test.rs", &mut issues);
        let no_assert_issues: Vec<_> = issues.iter().filter(|i| i.message.contains("no assertions")).collect();
        assert!(no_assert_issues.is_empty(), "Should not flag test with assertions, got: {:?}", no_assert_issues);
    }

    #[test]
    fn test_check_testing_long_test() {
        let mut code = String::from("#[test]\nfn test_long() {\n");
        for _ in 0..120 { code.push_str("    let _x = 1;\n"); }
        code.push_str("}\n");
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_testing(&code, &lines, "test.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("long")), "Should flag long test, got: {:?}", issues);
    }

    // ── Debug residual tests ──────────────────────────────────────────────────

    #[test]
    fn test_check_debug_dbg() {
        let code = r#"
fn calculate() -> i32 {
    let x = 5;
    dbg!(x);
    x + 1
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_debug(code, &lines, "src/lib.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("dbg!")), "Should flag dbg!(), got: {:?}", issues);
    }

    #[test]
    fn test_check_debug_eprintln() {
        let code = r#"
fn process() {
    eprintln!("processing...");
}
"#;
        let lines: Vec<&str> = code.lines().collect();
        let mut issues = Vec::new();
        check_debug(code, &lines, "src/lib.rs", &mut issues);
        assert!(issues.iter().any(|i| i.message.contains("eprintln!")), "Should flag eprintln!(), got: {:?}", issues);
    }
}
