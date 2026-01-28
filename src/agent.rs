use crate::config::Config;

pub struct Agent {
    base_command: Vec<String>,
}

impl Agent {
    pub fn from_config(config: &Config) -> Self {
        Self {
            base_command: config
                .effective_command()
                .split_whitespace()
                .map(String::from)
                .collect(),
        }
    }

    fn is_claude(&self) -> bool {
        self.base_command
            .first()
            .map(|s| s.contains("claude"))
            .unwrap_or(true)
    }

    pub fn name(&self) -> &'static str {
        if self.is_claude() {
            "Claude"
        } else {
            "Unknown"
        }
    }

    pub fn binary(&self) -> &str {
        self.base_command
            .first()
            .map(|s| s.as_str())
            .unwrap_or("claude")
    }

    pub fn command(&self, prompt: Option<&str>) -> Vec<String> {
        let mut cmd = self.base_command.clone();
        if let Some(p) = prompt {
            cmd.push(p.to_string());
        }
        cmd
    }

    pub fn is_available(&self) -> bool {
        which::which(self.binary()).is_ok()
    }

    pub fn prompt_pattern(&self) -> Option<&'static str> {
        if self.is_claude() {
            Some("Do you want")
        } else {
            None
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent() {
        let agent = Agent::default();
        assert_eq!(agent.name(), "Claude");
        assert_eq!(agent.binary(), "claude");
        assert_eq!(agent.command(None), vec!["claude"]);
        assert_eq!(agent.command(Some("test")), vec!["claude", "test"]);
        assert_eq!(agent.prompt_pattern(), Some("Do you want"));
    }

    #[test]
    fn test_custom_command() {
        let config = Config {
            command: Some("claude --dangerously-skip-permissions".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Claude");
        assert_eq!(
            agent.command(Some("fix bug")),
            vec!["claude", "--dangerously-skip-permissions", "fix bug"]
        );
    }

    #[test]
    fn test_path_to_claude() {
        let config = Config {
            command: Some("/usr/bin/claude --flag".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Claude");
    }

    #[test]
    fn test_unknown_agent() {
        let config = Config {
            command: Some("aider --model gpt-4".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Unknown");
        assert_eq!(agent.prompt_pattern(), None);
    }
}
