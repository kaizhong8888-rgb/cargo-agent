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
    /// };
    ///
    /// assert!(skill.matches_message("How do I design a SQL schema?"));
    /// assert!(skill.matches_message("database indexes"));
    /// assert!(!skill.matches_message("frontend design patterns"));
    /// ```
    pub fn matches_message(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        self.keywords.iter().any(|kw| {
            let kw_lower = kw.to_lowercase();
            lower.contains(&kw_lower)
        })
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
    /// };
    ///
    /// let ctx = skill.build_context();
    /// assert!(ctx.contains("Follow Rust idioms."));
    /// assert!(ctx.contains("Use `Cow<str>`."));
    /// ```
    pub fn build_context(&self) -> String {
        let mut parts = Vec::new();
        if !self.system_instructions.is_empty() {
            parts.push(self.system_instructions.clone());
        }
        if !self.reference.is_empty() {
            parts.push(format!("## Reference\n{}", self.reference));
        }
        for file in &self.reference_files {
            if let Ok(content) = std::fs::read_to_string(
                crate::constants::skills_dir().join(file),
            ) {
                parts.push(format!("## Reference: {}\n{}", file, content));
            }
        }
        parts.join("\n\n---\n\n")
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
            if path.extension().is_some_and(|ext| ext == "yaml") ||
               path.extension().is_some_and(|ext| ext == "yml") {
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
    /// });
    ///
    /// registry.register(Skill {
    ///     name: "on-demand".into(),
    ///     description: String::new(), always_active: false,
    ///     keywords: vec![], system_instructions: String::new(),
    ///     reference: String::new(), reference_files: vec![],
    /// });
    ///
    /// assert_eq!(registry.active_skills().len(), 1);
    /// assert_eq!(registry.active_skills()[0].name, "always-on");
    /// ```
    pub fn active_skills(&self) -> Vec<&Skill> {
        self.skills.values()
            .filter(|s| s.always_active)
            .collect()
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
    /// });
    ///
    /// assert_eq!(registry.matching_skills("I need Rust help").len(), 1);
    /// assert!(registry.matching_skills("I need Python help").is_empty());
    /// ```
    pub fn matching_skills(&self, message: &str) -> Vec<&Skill> {
        self.skills.values()
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
    /// });
    ///
    /// let list = registry.list();
    /// assert_eq!(list.len(), 1);
    /// assert_eq!(list[0].0, "alpha");
    /// assert!(list[0].2); // always_active
    /// ```
    pub fn list(&self) -> Vec<(String, String, bool)> {
        self.skills.values()
            .map(|s| (s.name.clone(), s.description.clone(), s.always_active))
            .collect()
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
    /// });
    ///
    /// let ctx = registry.build_context_for("hello");
    /// assert!(ctx.contains("base"));
    /// assert!(ctx.contains("Be concise."));
    /// ```
    pub fn build_context_for(&self, message: &str) -> String {
        let mut parts = Vec::new();

        // Always-active skills first
        for skill in self.active_skills() {
            parts.push(format!("## Skill: {}\n{}", skill.name, skill.build_context()));
        }

        // Keyword-matched skills
        for skill in self.matching_skills(message) {
            parts.push(format!("## Skill: {}\n{}", skill.name, skill.build_context()));
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

        std::fs::remove_dir_all(&dir).ok();
    }
}
