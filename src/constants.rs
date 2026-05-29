use once_cell::sync::Lazy;

pub const AGENT_NAME: &str = "cargo-agent";

pub static CARGO_HOME: Lazy<String> = Lazy::new(|| {
    std::env::var("CARGO_AGENT_HOME")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_else(|| "~".to_string())
});

pub static AGENT_DIR: Lazy<String> = Lazy::new(|| format!("{}/.cargo-agent", *CARGO_HOME));

/// Get the path to the memories storage directory.
///
/// Returns `{AGENT_DIR}/memories`.
///
/// # Example
///
/// ```
/// use cargo_agent::constants::memories_dir;
///
/// let dir = memories_dir();
/// assert!(dir.ends_with("/.cargo-agent/memories"));
/// ```
pub fn memories_dir() -> String {
    format!("{}/memories", *AGENT_DIR)
}

/// Get the path to the skills directory.
///
/// Returns `{AGENT_DIR}/skills`.
///
/// # Example
///
/// ```
/// use cargo_agent::constants::skills_dir;
/// use std::path::Path;
///
/// let dir = skills_dir();
/// assert!(dir.to_string_lossy().ends_with(".cargo-agent/skills"));
/// ```
pub fn skills_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(&*AGENT_DIR).join("skills")
}

/// Get the path to the config file.
///
/// Returns `{AGENT_DIR}/config.yaml`.
pub fn config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(&*AGENT_DIR).join("config.yaml")
}

/// Get the agent data directory as a PathBuf.
///
/// Returns `~/.cargo-agent`.
pub fn agent_dir_path() -> std::path::PathBuf {
    std::path::PathBuf::from(&*AGENT_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memories_dir_format() {
        let dir = memories_dir();
        assert!(dir.contains(".cargo-agent/memories"));
    }

    #[test]
    fn test_skills_dir_format() {
        let dir = skills_dir();
        assert!(dir.to_string_lossy().contains(".cargo-agent"));
        assert!(dir.to_string_lossy().contains("skills"));
    }

    #[test]
    fn test_config_path_format() {
        let path = config_path();
        assert!(path.to_string_lossy().contains(".cargo-agent"));
        assert!(path.to_string_lossy().contains("config.yaml"));
    }

    #[test]
    fn test_agent_name() {
        assert_eq!(AGENT_NAME, "cargo-agent");
    }
}
