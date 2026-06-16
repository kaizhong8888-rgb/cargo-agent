//! Skill system: domain-specific knowledge bundles that enhance agent capabilities.
//!
//! Each skill provides:
//! - Specialized instructions (injected into the conversation context)
//! - Optional reference files the agent can consult
//! - Keyword triggers for auto-activation
//!
//! Skills live in `~/.cargo-agent/skills/` as YAML files.
//!
//! # Example
//!
//! ```
//! use cargo_agent::skills::{Skill, SkillRegistry};
//!
//! let skill = Skill {
//!     name: "rust-helper".into(),
//!     description: "Rust programming helper".into(),
//!     always_active: true,
//!     keywords: vec!["rust".into(), "cargo".into()],
//!     system_instructions: "Help with Rust code.".into(),
//!     reference: "Use `cargo check` to verify.".into(),
//!     reference_files: vec![],
//!     category: "lang".into(),
//!     version: "1.0.0".into(),
//!     author: "cargo-agent".into(),
//!     created_at: String::new(),
//!     updated_at: String::new(),
//!     tags: vec!["rust".into()],
//!     priority: 10,
//! };
//!
//! let mut registry = SkillRegistry::new();
//! registry.register(skill);
//!
//! let ctx = registry.build_context_for("how do I use cargo?");
//! assert!(ctx.contains("rust-helper"));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single skill definition.
///
/// # Example
///
/// ```
/// use cargo_agent::skills::Skill;
///
/// let skill = Skill {
///     name: "testing-mastery".into(),
///     description: "Testing best practices".into(),
///     always_active: false,
///     keywords: vec!["test".into(), "unit test".into(), "coverage".into()],
///     system_instructions: "Follow testing best practices.".into(),
///     reference: "Use Triple-A pattern.".into(),
///     reference_files: vec![],
///     category: "testing".into(),
///     version: "1.0.0".into(),
///     author: "cargo-agent".into(),
///     created_at: String::new(),
///     updated_at: String::new(),
///     tags: vec!["test".into()],
///     priority: 5,
/// };
///
/// assert!(skill.matches_message("how to write unit tests?"));
/// assert!(!skill.matches_message("deploy to production"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique skill name (also the filename without .yaml).
    pub name: String,
    /// Short description of what the skill does.
    pub description: String,
    /// Whether this skill is always active.
    #[serde(default)]
    pub always_active: bool,
    /// Keywords that trigger auto-activation. If the user message contains
    /// any of these words/phrases, the skill's instructions are injected.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// The system instructions injected when this skill is active.
    #[serde(default)]
    pub system_instructions: String,
    /// Optional reference content (inline knowledge the agent can consult).
    #[serde(default)]
    pub reference: String,
    /// Optional file paths (relative to skill dir) with additional reference.
    #[serde(default)]
    pub reference_files: Vec<String>,
    // --- Structured metadata (frontmatter-style) ---
    /// Category for grouping (e.g. "web-framework", "database", "testing").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    /// Semantic version of the skill definition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Author/creator of the skill.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Creation timestamp (ISO 8601).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub created_at: String,
    /// Last update timestamp (ISO 8601).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub updated_at: String,
    /// Tags for fine-grained classification.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Priority level for ordering (1-10, higher = more important).
    #[serde(default)]
    pub priority: u8,
}

impl Skill {
    /// Load a skill from a YAML file.
    ///
    /// The file must contain valid YAML that matches the `Skill` struct.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cargo_agent::skills::Skill;
    /// use std::path::Path;
    ///
    /// // Given a file at ~/.cargo-agent/skills/my-skill.yaml:
    /// let skill = Skill::from_file(Path::new("/tmp/skill.yaml"));
    /// // Returns Ok(skill) on success
    /// ```
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let skill: Skill = serde_yaml::from_str(&content)?;
        Ok(skill)
    }

    /// Save this skill to its YAML file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cargo_agent::skills::Skill;
    /// use std::path::Path;
    ///
    /// let skill = Skill {
    ///     name: "my-skill".into(),
    ///     description: "My custom skill".into(),
    ///     always_active: false,
    ///     keywords: vec![],
    ///     system_instructions: "Be helpful.".into(),
    ///     reference: String::new(),
    ///     reference_files: vec![],
    ///     category: String::new(),
    ///     version: String::new(),
    ///     author: String::new(),
    ///     created_at: String::new(),
    ///     updated_at: String::new(),
    ///     tags: vec![],
    ///     priority: 0,
    /// };
    ///
    /// let path = skill.save_to(Path::new("/tmp/skills")).unwrap();
    /// assert!(path.exists());
    /// # std::fs::remove_file(path).ok();
    /// ```
    pub fn save_to(&self, dir: &Path) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let file_path = dir.join(format!("{}.yaml", self.name));
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&file_path, &content)?;
        Ok(file_path)
    }

    /// Check if this skill should be activated by the user message.
    ///
    /// A skill matches if **any** of its keywords appear in the message
    /// (case-insensitive).
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::Skill;
    ///
    /// let skill = Skill {
    ///     name: "database-design".into(),
    ///     description: "Database design help".into(),
    ///     always_active: false,
    ///     keywords: vec!["sql".into(), "database".into(), "table".into()],
    ///     system_instructions: String::new(),
    ///     reference: String::new(),
    ///     reference_files: vec![],
    ///     category: String::new(),
    ///     version: String::new(),
    ///     author: String::new(),
    ///     created_at: String::new(),
    ///     updated_at: String::new(),
    ///     tags: vec![],
    ///     priority: 0,
    /// };
    ///
    /// assert!(skill.matches_message("How do I design a SQL schema?"));
    /// assert!(skill.matches_message("database indexes"));
    /// assert!(!skill.matches_message("frontend design patterns"));
    /// ```
    pub fn matches_message(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        self.keywords.iter().any(|kw| lower.contains(kw))
    }

    /// Get the full instruction string combining system_instructions and reference.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::Skill;
    ///
    /// let skill = Skill {
    ///     name: "test".into(),
    ///     description: String::new(),
    ///     always_active: false,
    ///     keywords: vec![],
    ///     system_instructions: "Follow Rust idioms.".into(),
    ///     reference: "Use `Cow<str>`.".into(),
    ///     reference_files: vec![],
    ///     category: String::new(),
    ///     version: String::new(),
    ///     author: String::new(),
    ///     created_at: String::new(),
    ///     updated_at: String::new(),
    ///     tags: vec![],
    ///     priority: 0,
    /// };
    ///
    /// let ctx = skill.build_context();
    /// assert!(ctx.contains("Follow Rust idioms."));
    /// assert!(ctx.contains("Use `Cow<str>`."));
    /// ```
    pub fn build_context(&self) -> String {
        let mut parts = Vec::with_capacity(1 + self.reference_files.len());
        if !self.system_instructions.is_empty() {
            parts.push(self.system_instructions.clone());
        }
        if !self.reference.is_empty() {
            parts.push(format!("## Reference\n{}", self.reference));
        }
        for file in &self.reference_files {
            if let Ok(content) = std::fs::read_to_string(crate::constants::skills_dir().join(file))
            {
                parts.push(format!("## Reference: {}\n{}", file, content));
            }
        }
        parts.join("\n\n---\n\n")
    }

    /// Generate a frontmatter-style header string for display/debugging.
    ///
    /// This produces a human-readable YAML-like block showing all metadata fields.
    /// Fields with empty values are omitted for brevity.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::Skill;
    ///
    /// let skill = Skill {
    ///     name: "web-dev".into(),
    ///     description: "Web development helper".into(),
    ///     always_active: false,
    ///     keywords: vec!["axum".into()],
    ///     system_instructions: "Use Axum patterns.".into(),
    ///     reference: String::new(),
    ///     reference_files: vec![],
    ///     category: "web-framework".into(),
    ///     version: "1.0.0".into(),
    ///     author: "cargo-agent".into(),
    ///     created_at: "2024-01-01T00:00:00Z".into(),
    ///     updated_at: String::new(),
    ///     tags: vec!["http".into(), "rest".into()],
    ///     priority: 8,
    /// };
    ///
    /// let header = skill.frontmatter();
    /// assert!(header.contains("name: web-dev"));
    /// assert!(header.contains("category: web-framework"));
    /// assert!(header.contains("version: 1.0.0"));
    /// ```
    pub fn frontmatter(&self) -> String {
        let mut lines: Vec<String> = Vec::with_capacity(12);
        lines.push("---".into());
        lines.push(format!("name: {}", self.name));
        lines.push(format!(
            "description: {}",
            escape_yaml_scalar(&self.description)
        ));

        if !self.category.is_empty() {
            lines.push(format!("category: {}", self.category));
        }
        if !self.version.is_empty() {
            lines.push(format!("version: {}", self.version));
        }
        if !self.author.is_empty() {
            lines.push(format!("author: {}", self.author));
        }
        if !self.created_at.is_empty() {
            lines.push(format!("created_at: {}", self.created_at));
        }
        if !self.updated_at.is_empty() {
            lines.push(format!("updated_at: {}", self.updated_at));
        }
        if self.priority > 0 {
            lines.push(format!("priority: {}", self.priority));
        }
        if self.always_active {
            lines.push("always_active: true".into());
        }
        if !self.keywords.is_empty() {
            lines.push(format!("keywords: [{}]", self.keywords.join(", ")));
        }
        if !self.tags.is_empty() {
            lines.push(format!("tags: [{}]", self.tags.join(", ")));
        }
        lines.push("---".into());
        lines.join("\n")
    }

    /// Create a new skill with metadata auto-populated (timestamps, default author).
    pub fn new_with_metadata(
        name: String,
        description: String,
        category: String,
        keywords: Vec<String>,
        system_instructions: String,
        author: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: name.clone(),
            description,
            always_active: false,
            keywords,
            system_instructions,
            reference: String::new(),
            reference_files: vec![],
            category,
            version: "0.1.0".into(),
            author: author.unwrap_or_else(|| "cargo-agent".into()),
            created_at: now.clone(),
            updated_at: now,
            tags: vec![],
            priority: 5,
        }
    }
}

/// Escape a YAML scalar value for safe inline display.
fn escape_yaml_scalar(s: &str) -> String {
    if s.contains(':') || s.contains('#') || s.contains('\n') || s.contains('"') {
        // Wrap in double quotes and escape internal quotes
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// Registry that holds and manages skills.
///
/// # Example
///
/// ```
/// use cargo_agent::skills::{Skill, SkillRegistry};
///
/// let mut registry = SkillRegistry::new();
///
/// let skill = Skill {
///     name: "cli-architecture".into(),
///     description: "CLI design patterns".into(),
///     always_active: false,
///     keywords: vec!["cli".into(), "command".into()],
///     system_instructions: "Use clap for argument parsing.".into(),
///     reference: String::new(),
///     reference_files: vec![],
///     category: "cli".into(),
///     version: "1.0.0".into(),
///     author: "cargo-agent".into(),
///     created_at: String::new(),
///     updated_at: String::new(),
///     tags: vec![],
///     priority: 5,
/// };
///
/// registry.register(skill);
/// assert_eq!(registry.list().len(), 1);
/// assert_eq!(registry.matching_skills("build a CLI tool").len(), 1);
/// ```
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// Create a new, empty skill registry.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::SkillRegistry;
    ///
    /// let registry = SkillRegistry::new();
    /// assert!(registry.list().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load all skills from the skills directory.
    ///
    /// Loads all `.yaml` and `.yml` files from the given directory
    /// and parses them as `Skill` definitions.
    pub fn load_from_dir(dir: &Path) -> anyhow::Result<Self> {
        let mut registry = Self::new();
        if !dir.exists() {
            return Ok(registry);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "yaml")
                || path.extension().is_some_and(|ext| ext == "yml")
            {
                if let Ok(skill) = Skill::from_file(&path) {
                    registry.skills.insert(skill.name.clone(), skill);
                }
            }
        }

        Ok(registry)
    }

    /// Get always-active skills.
    ///
    /// Returns all skills where `always_active` is `true`.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    ///
    /// registry.register(Skill {
    ///     name: "always-on".into(),
    ///     description: String::new(), always_active: true,
    ///     keywords: vec![], system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// registry.register(Skill {
    ///     name: "on-demand".into(),
    ///     description: String::new(), always_active: false,
    ///     keywords: vec![], system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// assert_eq!(registry.active_skills().len(), 1);
    /// assert_eq!(registry.active_skills()[0].name, "always-on");
    /// ```
    pub fn active_skills(&self) -> Vec<&Skill> {
        self.skills.values().filter(|s| s.always_active).collect()
    }

    /// Find skills that match the user message by keyword.
    ///
    /// Only non-always-active skills are considered (always-active ones
    /// are always injected regardless of message content).
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    ///
    /// registry.register(Skill {
    ///     name: "rust-help".into(),
    ///     description: String::new(), always_active: false,
    ///     keywords: vec!["rust".into()],
    ///     system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// assert_eq!(registry.matching_skills("I need Rust help").len(), 1);
    /// assert!(registry.matching_skills("I need Python help").is_empty());
    /// ```
    pub fn matching_skills(&self, message: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| !s.always_active && s.matches_message(message))
            .collect()
    }

    /// Get a skill by name.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    /// registry.register(Skill {
    ///     name: "my-skill".into(),
    ///     description: "A skill".into(),
    ///     always_active: false, keywords: vec![],
    ///     system_instructions: "Instr.".into(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// assert!(registry.get("my-skill").is_some());
    /// assert!(registry.get("unknown").is_none());
    /// ```
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Register a skill.
    ///
    /// If a skill with the same name already exists, it is overwritten.
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Remove a skill by name.
    ///
    /// Returns `true` if a skill was removed, `false` if not found.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    /// registry.register(Skill {
    ///     name: "temp".into(), description: String::new(),
    ///     always_active: false, keywords: vec![],
    ///     system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// assert!(registry.remove("temp"));
    /// assert!(!registry.remove("temp"));
    /// ```
    pub fn remove(&mut self, name: &str) -> bool {
        self.skills.remove(name).is_some()
    }

    /// List all skill names, descriptions, and active status.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    /// registry.register(Skill {
    ///     name: "alpha".into(), description: "First skill".into(),
    ///     always_active: true, keywords: vec![],
    ///     system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// let list = registry.list();
    /// assert_eq!(list.len(), 1);
    /// assert_eq!(list[0].0, "alpha");
    /// assert!(list[0].2); // always_active
    /// ```
    pub fn list(&self) -> Vec<(String, String, bool)> {
        self.skills
            .values()
            .map(|s| (s.name.clone(), s.description.clone(), s.always_active))
            .collect()
    }

    /// List all skills with full metadata for display.
    pub fn list_with_metadata(&self) -> Vec<&Skill> {
        let mut skills: Vec<&Skill> = self.skills.values().collect();
        // Sort by priority (descending), then alphabetically
        skills.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.name.cmp(&b.name))
        });
        skills
    }

    /// Filter skills by category.
    pub fn by_category(&self, category: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.category.eq_ignore_ascii_case(category))
            .collect()
    }

    /// Filter skills by tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            .collect()
    }

    /// Get unique categories across all skills.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self
            .skills
            .values()
            .filter(|s| !s.category.is_empty())
            .map(|s| s.category.clone())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }

    /// Get summary statistics about the skill collection.
    pub fn stats(&self) -> SkillStats {
        let total = self.skills.len();
        let always_active = self.skills.values().filter(|s| s.always_active).count();
        let with_metadata = self
            .skills
            .values()
            .filter(|s| !s.category.is_empty() || !s.tags.is_empty())
            .count();
        let categories = self.categories();

        SkillStats {
            total,
            always_active,
            with_metadata,
            categories,
        }
    }

    /// Build the combined context string for active skills.
    ///
    /// Always-active skills are always included; then keyword-matched skills
    /// are appended. Returns an empty string if no skills are active.
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_agent::skills::{Skill, SkillRegistry};
    ///
    /// let mut registry = SkillRegistry::new();
    ///
    /// registry.register(Skill {
    ///     name: "base".into(), description: String::new(), always_active: true,
    ///     keywords: vec![], system_instructions: "Be concise.".into(),
    ///     reference: String::new(), reference_files: vec![],
    ///     category: String::new(), version: String::new(),
    ///     author: String::new(), created_at: String::new(),
    ///     updated_at: String::new(), tags: vec![], priority: 0,
    /// });
    ///
    /// let ctx = registry.build_context_for("hello");
    /// assert!(ctx.contains("base"));
    /// assert!(ctx.contains("Be concise."));
    /// ```
    pub fn build_context_for(&self, message: &str) -> String {
        let num_skills = self.active_skills().len() + self.matching_skills(message).len();
        let mut parts = Vec::with_capacity(num_skills);

        // Always-active skills first
        for skill in self.active_skills() {
            parts.push(format!(
                "## Skill: {}\n{}",
                skill.name,
                skill.build_context()
            ));
        }

        // Keyword-matched skills
        for skill in self.matching_skills(message) {
            parts.push(format!(
                "## Skill: {}\n{}",
                skill.name,
                skill.build_context()
            ));
        }

        if parts.is_empty() {
            return String::new();
        }

        parts.join("\n\n---\n\n")
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a skill collection.
#[derive(Debug, Clone)]
pub struct SkillStats {
    /// Total number of skills.
    pub total: usize,
    /// Number of always-active skills.
    pub always_active: usize,
    /// Number of skills with category or tags metadata.
    pub with_metadata: usize,
    /// Unique categories present in the collection.
    pub categories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_skill() -> Skill {
        Skill {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            always_active: false,
            keywords: vec!["rust".to_string(), "cargo".to_string()],
            system_instructions: "Follow Rust idioms.".to_string(),
            reference: "Use `Cow<str>` for owned/borrowed.".to_string(),
            reference_files: vec![],
            category: "testing".to_string(),
            version: "1.0.0".to_string(),
            author: "cargo-agent".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: String::new(),
            tags: vec!["test".to_string(), "rust".to_string()],
            priority: 5,
        }
    }

    #[test]
    fn test_matches_message_by_keyword() {
        let skill = test_skill();
        assert!(skill.matches_message("How do I write Rust code?"));
        assert!(skill.matches_message("cargo build failed"));
        assert!(!skill.matches_message("Hello world"));
    }

    #[test]
    fn test_build_context() {
        let skill = test_skill();
        let ctx = skill.build_context();
        assert!(ctx.contains("Follow Rust idioms."));
        assert!(ctx.contains("Use `Cow<str>`"));
    }

    #[test]
    fn test_skill_registry() {
        let mut reg = SkillRegistry::new();
        reg.register(test_skill());

        assert_eq!(reg.list().len(), 1);
        assert!(reg.active_skills().is_empty());
        assert_eq!(reg.matching_skills("cargo build").len(), 1);
        assert!(reg.matching_skills("hello").is_empty());
    }

    #[test]
    fn test_always_active_skill() {
        let mut reg = SkillRegistry::new();
        let mut skill = test_skill();
        skill.always_active = true;
        reg.register(skill);

        assert_eq!(reg.active_skills().len(), 1);
        assert!(reg.matching_skills("hello").is_empty()); // already active, not matched again
    }

    #[test]
    fn test_remove_skill() {
        let mut reg = SkillRegistry::new();
        reg.register(test_skill());
        assert!(reg.remove("test-skill"));
        assert!(!reg.remove("nonexistent"));
        assert!(reg.list().is_empty());
    }

    #[test]
    fn test_build_combined_context() {
        let mut reg = SkillRegistry::new();

        let mut always_skill = test_skill();
        always_skill.name = "always".to_string();
        always_skill.always_active = true;
        always_skill.keywords = vec![];
        reg.register(always_skill);

        let mut ondemand_skill = test_skill();
        ondemand_skill.name = "ondemand".to_string();
        ondemand_skill.always_active = false;
        reg.register(ondemand_skill);

        let ctx = reg.build_context_for("cargo is broken");
        assert!(ctx.contains("## Skill: always"));
        assert!(ctx.contains("## Skill: ondemand"));
    }

    #[test]
    fn test_skill_serialization_roundtrip() {
        let skill = test_skill();
        let dir = std::env::temp_dir().join("skill_test_roundtrip");
        let path = skill.save_to(&dir).unwrap();
        assert!(path.exists());

        let loaded = Skill::from_file(&path).unwrap();
        assert_eq!(loaded.name, skill.name);
        assert_eq!(loaded.description, skill.description);
        assert_eq!(loaded.keywords, skill.keywords);
        assert_eq!(loaded.category, skill.category);
        assert_eq!(loaded.version, skill.version);
        assert_eq!(loaded.tags, skill.tags);
        assert_eq!(loaded.priority, skill.priority);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_new_with_metadata() {
        let skill = Skill::new_with_metadata(
            "web-dev".to_string(),
            "Web development helper".to_string(),
            "web-framework".to_string(),
            vec!["axum".to_string()],
            "Use Axum patterns.".to_string(),
            None,
        );

        assert_eq!(skill.name, "web-dev");
        assert_eq!(skill.category, "web-framework");
        assert_eq!(skill.author, "cargo-agent");
        assert_eq!(skill.version, "0.1.0");
        assert!(!skill.created_at.is_empty());
        assert!(!skill.updated_at.is_empty());
        assert_eq!(skill.priority, 5);
    }

    #[test]
    fn test_frontmatter_output() {
        let skill = test_skill();
        let fm = skill.frontmatter();
        assert!(fm.starts_with("---\n"));
        assert!(fm.contains("name: test-skill"));
        assert!(fm.contains("category: testing"));
        assert!(fm.contains("version: 1.0.0"));
        assert!(fm.contains("priority: 5"));
        assert!(fm.contains("tags: [test, rust]"));
        assert!(fm.ends_with("---"));
    }

    #[test]
    fn test_frontmatter_empty_fields() {
        let skill = Skill {
            name: "minimal".to_string(),
            description: "Minimal skill".to_string(),
            always_active: false,
            keywords: vec![],
            system_instructions: String::new(),
            reference: String::new(),
            reference_files: vec![],
            category: String::new(),
            version: String::new(),
            author: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            tags: vec![],
            priority: 0,
        };
        let fm = skill.frontmatter();
        assert!(fm.contains("name: minimal"));
        assert!(!fm.contains("category:"));
        assert!(!fm.contains("version:"));
        assert!(!fm.contains("tags:"));
        assert!(!fm.contains("priority:"));
    }

    #[test]
    fn test_registry_by_category() {
        let mut reg = SkillRegistry::new();

        let mut skill1 = test_skill();
        skill1.name = "web1".to_string();
        skill1.category = "web-framework".to_string();
        reg.register(skill1);

        let mut skill2 = test_skill();
        skill2.name = "web2".to_string();
        skill2.category = "web-framework".to_string();
        reg.register(skill2);

        let mut skill3 = test_skill();
        skill3.name = "db1".to_string();
        skill3.category = "database".to_string();
        reg.register(skill3);

        assert_eq!(reg.by_category("web-framework").len(), 2);
        assert_eq!(reg.by_category("database").len(), 1);
        assert!(reg.by_category("nonexistent").is_empty());
    }

    #[test]
    fn test_registry_by_tag() {
        let mut reg = SkillRegistry::new();

        let mut skill1 = test_skill();
        skill1.name = "skill1".to_string();
        skill1.tags = vec!["http".to_string(), "rest".to_string()];
        reg.register(skill1);

        let mut skill2 = test_skill();
        skill2.name = "skill2".to_string();
        skill2.tags = vec!["http".to_string(), "graphql".to_string()];
        reg.register(skill2);

        let mut skill3 = test_skill();
        skill3.name = "skill3".to_string();
        skill3.tags = vec!["database".to_string()];
        reg.register(skill3);

        assert_eq!(reg.by_tag("http").len(), 2);
        assert_eq!(reg.by_tag("graphql").len(), 1);
        assert_eq!(reg.by_tag("database").len(), 1);
    }

    #[test]
    fn test_registry_categories() {
        let mut reg = SkillRegistry::new();

        let mut s1 = test_skill();
        s1.name = "s1".to_string();
        s1.category = "web".to_string();
        reg.register(s1);

        let mut s2 = test_skill();
        s2.name = "s2".to_string();
        s2.category = "database".to_string();
        reg.register(s2);

        let mut s3 = test_skill();
        s3.name = "s3".to_string();
        s3.category = "web".to_string(); // duplicate
        reg.register(s3);

        let cats = reg.categories();
        assert_eq!(cats, vec!["database", "web"]);
    }

    #[test]
    fn test_registry_stats() {
        let mut reg = SkillRegistry::new();

        let mut always_skill = test_skill();
        always_skill.name = "always".to_string();
        always_skill.always_active = true;
        always_skill.category = "core".to_string();
        reg.register(always_skill);

        let mut meta_skill = test_skill();
        meta_skill.name = "meta".to_string();
        meta_skill.tags = vec!["test".to_string()];
        reg.register(meta_skill);

        let mut bare_skill = test_skill();
        bare_skill.name = "bare".to_string();
        bare_skill.category = String::new();
        bare_skill.tags = vec![];
        reg.register(bare_skill);

        let stats = reg.stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.always_active, 1);
        assert_eq!(stats.with_metadata, 2); // always (has category) + meta (has tags)
        assert_eq!(stats.categories, vec!["core", "testing"]);
    }

    #[test]
    fn test_registry_list_with_metadata_sorted_by_priority() {
        let mut reg = SkillRegistry::new();

        let mut low = test_skill();
        low.name = "low".to_string();
        low.priority = 2;
        reg.register(low);

        let mut high = test_skill();
        high.name = "high".to_string();
        high.priority = 9;
        reg.register(high);

        let mut mid = test_skill();
        mid.name = "mid".to_string();
        mid.priority = 5;
        reg.register(mid);

        let list = reg.list_with_metadata();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "high");
        assert_eq!(list[1].name, "mid");
        assert_eq!(list[2].name, "low");
    }

    #[test]
    fn test_backward_compat_load_old_yaml() {
        // Simulate loading a YAML file without metadata fields (old format)
        let old_yaml = r#"
name: old-skill
description: An old skill
always_active: false
keywords:
  - old
  - legacy
system_instructions: "Legacy instructions"
reference: ""
reference_files: []
"#;
        let skill: Skill = serde_yaml::from_str(old_yaml).unwrap();
        assert_eq!(skill.name, "old-skill");
        assert_eq!(skill.category, "");
        assert_eq!(skill.version, "");
        assert_eq!(skill.priority, 0);
        assert!(skill.tags.is_empty());
    }
}
