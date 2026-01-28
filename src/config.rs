use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::{zlog_debug, Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub trust: bool,
    pub worktree_dir: Option<String>,
    pub command: Option<String>,
}

impl Config {
    pub fn zen_dir() -> Result<PathBuf> {
        Ok(dirs::home_dir().ok_or(Error::NoHomeDir)?.join(".zen"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::zen_dir()?.join("zen.toml"))
    }

    pub fn state_path() -> Result<PathBuf> {
        Ok(Self::zen_dir()?.join("state.json"))
    }

    pub fn worktrees_dir() -> Result<PathBuf> {
        let config = Self::load()?;
        match config.worktree_dir {
            Some(dir) => Ok(expand_tilde(&dir)),
            None => Ok(Self::zen_dir()?.join("worktrees")),
        }
    }

    pub fn effective_command(&self) -> &str {
        self.command.as_deref().unwrap_or("claude")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        zlog_debug!("Config::load path={}", path.display());
        if !path.exists() {
            zlog_debug!("Config file not found, using defaults");
            return Ok(Self::default());
        }
        let config: Self = toml::from_str(&fs::read_to_string(&path)?)?;
        zlog_debug!(
            "Config loaded: trust={}, worktree_dir={:?}, command={:?}",
            config.trust,
            config.worktree_dir,
            config.command
        );
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let zen_dir = Self::zen_dir()?;
        zlog_debug!("Config::save zen_dir={}", zen_dir.display());
        if !zen_dir.exists() {
            zlog_debug!("Creating zen directory");
            fs::create_dir_all(&zen_dir)?;
        }
        let path = Self::config_path()?;
        fs::write(&path, toml::to_string_pretty(self)?)?;
        zlog_debug!("Config saved to {}", path.display());
        Ok(())
    }

    pub fn ensure_dirs() -> Result<()> {
        let zen_dir = Self::zen_dir()?;
        let worktrees_dir = Self::worktrees_dir()?;
        zlog_debug!(
            "Config::ensure_dirs zen={} worktrees={}",
            zen_dir.display(),
            worktrees_dir.display()
        );
        if !zen_dir.exists() {
            zlog_debug!("Creating zen directory: {}", zen_dir.display());
            fs::create_dir_all(&zen_dir)?;
        }
        if !worktrees_dir.exists() {
            zlog_debug!("Creating worktrees directory: {}", worktrees_dir.display());
            fs::create_dir_all(&worktrees_dir)?;
        }
        Ok(())
    }
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.trust);
        assert!(config.worktree_dir.is_none());
        assert!(config.command.is_none());
        assert_eq!(config.effective_command(), "claude");
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(expanded.ends_with("foo/bar"));
        assert!(!expanded.to_string_lossy().contains('~'));

        let absolute = expand_tilde("/absolute/path");
        assert_eq!(absolute, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config {
            trust: true,
            worktree_dir: Some("~/worktrees".to_string()),
            command: Some("claude --dangerously-skip-permissions".to_string()),
        };
        let toml = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert!(parsed.trust);
        assert_eq!(parsed.worktree_dir, Some("~/worktrees".to_string()));
        assert_eq!(
            parsed.command,
            Some("claude --dangerously-skip-permissions".to_string())
        );
    }
}
