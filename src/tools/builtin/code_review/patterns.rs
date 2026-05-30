//! Pre-compiled regex patterns for all code review checks.
//!
//! All patterns are compiled once at startup using `once_cell::sync::Lazy`
//! for optimal performance. These are `pub(super)` — only used by `checks.rs`.

use once_cell::sync::Lazy;
use regex::Regex;

// ── Unsafe code patterns ──────────────────────────────────────────────────────

pub(super) static RE_UNSAFE_BLOCK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"unsafe\s*\{").expect("invalid regex: unsafe block"));
pub(super) static RE_UNSAFE_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*unsafe\s+(?:extern\s+)?fn\s+").expect("invalid regex: unsafe fn")
});
pub(super) static RE_UNSAFE_TRAIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*unsafe\s+trait\s+").expect("invalid regex: unsafe trait"));
pub(super) static RE_PTR_DEREF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\*\s*(?:const|mut)\s+").expect("invalid regex: ptr deref"));
pub(super) static RE_TRANSMUTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:std::)?mem::transmute\b").expect("invalid regex: transmute"));

// ── Error handling patterns ────────────────────────────────────────────────────

pub(super) static RE_UNWRAP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\w+)\.unwrap\s*\(\s*\)").expect("invalid regex: unwrap"));
pub(super) static RE_EXPECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\w+)\.expect\s*\(").expect("invalid regex: expect"));
pub(super) static RE_PANIC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*panic!\s*\(").expect("invalid regex: panic"));
pub(super) static RE_IGNORE_RESULT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*let\s+_\s*=\s*.+;$").expect("invalid regex: ignore result"));
pub(super) static RE_WRITELN_RESULT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:write!|writeln!)\s*\([^)]*\)\s*;").expect("invalid regex: writeln result")
});

// ── Performance patterns ───────────────────────────────────────────────────────

pub(super) static RE_CLONE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\w+)\.clone\s*\(\s*\)").expect("invalid regex: clone"));
pub(super) static RE_BOX_NEW: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Box::new\s*\(").expect("invalid regex: box new"));
pub(super) static RE_VEC_CAPACITY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Vec::with_capacity\s*\(\s*(\d+)\s*\)").expect("invalid regex: vec capacity")
});
pub(super) static RE_COLLECT_VEC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.collect::<Vec<_>>\s*\(\)").expect("invalid regex: collect vec"));

// ── Style patterns ─────────────────────────────────────────────────────────────

pub(super) static RE_FN_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)")
        .expect("invalid regex: fn start")
});
pub(super) static RE_NON_SNAKE_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Z][a-zA-Z0-9_]*)")
        .expect("invalid regex: non-snake fn")
});
pub(super) static RE_TODO_COMMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?m)^\s*//\s*(TODO|FIXME|HACK|XXX|BUG|WORKAROUND)")
        .expect("invalid regex: todo comment")
});

// ── Safety patterns ────────────────────────────────────────────────────────────

pub(super) static RE_MAYBE_UNINIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"MaybeUninit").expect("invalid regex: maybe uninit"));
pub(super) static RE_PTR_OFFSET: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.offset\s*\(").expect("invalid regex: ptr offset"));

// ── Correctness patterns ───────────────────────────────────────────────────────

pub(super) static RE_TODO_MACRO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*todo!\s*\(").expect("invalid regex: todo macro"));
pub(super) static RE_UNIMPLEMENTED: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"unimplemented!\s*\(").expect("invalid regex: unimplemented"));
pub(super) static RE_UNREACHABLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"unreachable!\s*\(").expect("invalid regex: unreachable"));
pub(super) static RE_MATCH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*match\s+").expect("invalid regex: match"));
pub(super) static RE_DEREF_IMPL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*impl\s+(?:<[^>]*>\s+)?Deref\s+for\s+").expect("invalid regex: deref impl")
});

// ── Concurrency patterns ───────────────────────────────────────────────────────

pub(super) static RE_REFCELL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:RefCell|Cell)\s*<").expect("invalid regex: refcell"));
pub(super) static RE_STD_MUTEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"std::sync::Mutex").expect("invalid regex: std mutex"));
pub(super) static RE_UNSAFE_SEND_SYNC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"unsafe\s+impl\s+(Send|Sync)").expect("invalid regex: unsafe send/sync")
});
pub(super) static RE_STATIC_MUT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*static\s+mut\s+").expect("invalid regex: static mut"));
pub(super) static RE_ARC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Arc<").expect("invalid regex: arc"));

// ── Documentation patterns ─────────────────────────────────────────────────────

pub(super) static RE_PUB_FN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]")
        .expect("invalid regex: pub fn")
});

// ── Async patterns ─────────────────────────────────────────────────────────────

pub(super) static RE_ASYNC_FN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*(?:pub\s+)?async\s+fn\s+").expect("invalid regex: async fn"));
pub(super) static RE_AWAIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.await\b").expect("invalid regex: await"));
pub(super) static RE_STD_MUTEX_LOCK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"std::sync::Mutex[\s\S]*?\.lock\s*\(").expect("invalid regex: std mutex lock")
});
pub(super) static RE_BLOCKING_IO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(std::)?(fs|net|process|thread)::").expect("invalid regex: blocking io")
});

// ── Naming patterns ────────────────────────────────────────────────────────────

pub(super) static RE_LOWERCASE_STRUCT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?struct\s+([a-z][a-zA-Z0-9_]*)")
        .expect("invalid regex: lowercase struct")
});
pub(super) static RE_LOWERCASE_ENUM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?enum\s+([a-z][a-zA-Z0-9_]*)")
        .expect("invalid regex: lowercase enum")
});
pub(super) static RE_NON_SCREAMING_CONST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?const\s+([a-z][a-zA-Z0-9_]*)")
        .expect("invalid regex: non-screaming const")
});
pub(super) static RE_NON_SCREAMING_STATIC: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:unsafe\s+)?static\s+(?:mut\s+)?([a-z][a-zA-Z0-9_]*)")
        .expect("invalid regex: non-screaming static")
});

// ── Security patterns ──────────────────────────────────────────────────────────

/// SQL injection: format!(...) with SQL keyword in string + interpolated variable
pub(super) static RE_SQL_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r##"(?x)
        (?:format!|concat!|write!|writeln!)\s*\(
        [^)]*["']
        (?:SELECT|INSERT|UPDATE|DELETE|DROP|CREATE|ALTER|TRUNCATE|EXEC)\b
    "##,
    )
    .expect("invalid regex: SQL injection")
});

/// Hardcoded secrets: api_key/secret/password/token assigned a long string value
pub(super) static RE_HARDCODED_SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r##"(?i)(?x)
        (?:api[_-]?key|apikey|secret|password|token|auth|credential|private_key)
        \s*[=:]\s*["'][A-Za-z0-9_\-]{16,}["']
    "##,
    )
    .expect("invalid regex: hardcoded secret")
});

/// Private key / certificate content embedded in string
pub(super) static RE_PRIVATE_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"["']-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----"##)
        .expect("invalid regex: private key")
});

/// OpenAI API key pattern
pub(super) static RE_OPENAI_KEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r##"["']sk-[A-Za-z0-9]{32,}["']"##).expect("invalid regex: OpenAI key")
});

// ── Complexity patterns ────────────────────────────────────────────────────────

pub(super) static RE_FN_PARAMS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+\w+\s*\(([^)]*)\)")
        .expect("invalid regex: fn params")
});

// ── Testing patterns ───────────────────────────────────────────────────────────

pub(super) static RE_TEST_FN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*#\[test\]\s*$").expect("invalid regex: test fn"));
pub(super) static RE_ASSERT_MACRO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bassert(|_eq|_ne|_approx_eq)!").expect("invalid regex: assert macro")
});

// ── Debug residual patterns ────────────────────────────────────────────────────

pub(super) static RE_DBG_MACRO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^\s*(dbg!|eprintln!|println!)\s*\(").expect("invalid regex: dbg macro")
});

// ── Ignore system ──────────────────────────────────────────────────────────────

/// Matches inline ignore directives: `// code-review: ignore[check1,check2]`
pub(super) static RE_IGNORE_DIRECTIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"//\s*code-review:\s*ignore\s*\[([^\]]*)\]")
        .expect("invalid regex: ignore directive")
});
