//! AI-as-Human proxy for autonomous skill interaction.
//!
//! The `AIHumanProxy` answers skill clarification questions on behalf of
//! the user, enabling fully autonomous workflow execution. It uses the
//! original user intent and accumulated conversation context to generate
//! reasonable responses.
//!
//! ## Real vs Mock Responses
//!
//! The proxy supports two modes:
//! - **Mock mode** (default): Returns pattern-based responses for testing
//! - **Real mode**: Uses Claude headless to generate contextual responses
//!
//! To enable real mode, use `AIHumanProxy::with_claude()`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Tracks conversation context for consistent AI-as-Human responses.
///
/// This struct accumulates question-answer pairs and extracted decisions
/// to maintain consistency across the conversation. When recording Q&A pairs,
/// it automatically extracts key decisions (naming conventions, technology
/// choices, architectural patterns) for future reference.
///
/// # Example
///
/// ```
/// use zen::orchestration::ConversationContext;
///
/// let mut ctx = ConversationContext::new();
/// ctx.record("What should we name the module?", "user_auth");
/// assert_eq!(ctx.decisions().get("naming"), Some(&"user_auth".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct ConversationContext {
    /// History of question-answer pairs.
    qa_history: Vec<(String, String)>,
    /// Extracted key decisions for consistency.
    decisions: HashMap<String, String>,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self {
            qa_history: Vec::new(),
            decisions: HashMap::new(),
        }
    }
}

impl ConversationContext {
    /// Create a new empty conversation context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a question and its answer for future reference.
    ///
    /// This method also extracts key decisions from the Q&A pair based on
    /// question patterns. Recognized decision categories:
    ///
    /// - **naming**: Questions about names, naming conventions
    /// - **database**: Questions about database choices
    /// - **technology**: Questions about frameworks, libraries
    /// - **architecture**: Questions about patterns, approaches, structure
    ///
    /// # Arguments
    ///
    /// * `question` - The question that was asked
    /// * `answer` - The answer provided
    ///
    /// # Example
    ///
    /// ```
    /// use zen::orchestration::ConversationContext;
    ///
    /// let mut ctx = ConversationContext::new();
    /// ctx.record("Which database should we use?", "PostgreSQL");
    /// assert_eq!(ctx.decisions().get("database"), Some(&"PostgreSQL".to_string()));
    /// ```
    pub fn record(&mut self, question: &str, answer: &str) {
        self.qa_history.push((question.to_string(), answer.to_string()));

        // Extract key decisions based on question patterns
        let question_lower = question.to_lowercase();

        // Naming decisions
        if question_lower.contains("name")
            || question_lower.contains("naming")
            || question_lower.contains("call it")
            || question_lower.contains("call the")
        {
            self.decisions.insert("naming".to_string(), answer.to_string());
        }

        // Database decisions
        if question_lower.contains("database") || question_lower.contains("db") {
            self.decisions.insert("database".to_string(), answer.to_string());
        }

        // Technology/library decisions
        if question_lower.contains("framework")
            || question_lower.contains("library")
            || question_lower.contains("tool")
            || question_lower.contains("technology")
        {
            self.decisions.insert("technology".to_string(), answer.to_string());
        }

        // Architecture decisions
        if question_lower.contains("pattern")
            || question_lower.contains("architecture")
            || question_lower.contains("approach")
            || question_lower.contains("structure")
            || question_lower.contains("design")
        {
            self.decisions
                .insert("architecture".to_string(), answer.to_string());
        }
    }

    /// Generate a summary of the conversation context.
    ///
    /// Returns an empty string if no history exists, otherwise returns
    /// a formatted summary of all Q&A pairs.
    pub fn summary(&self) -> String {
        if self.qa_history.is_empty() {
            String::new()
        } else {
            self.qa_history
                .iter()
                .map(|(q, a)| format!("Q: {}\nA: {}", q, a))
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }

    /// Check if the context is empty.
    pub fn is_empty(&self) -> bool {
        self.qa_history.is_empty()
    }

    /// Get the number of recorded Q&A pairs.
    pub fn len(&self) -> usize {
        self.qa_history.len()
    }

    /// Get the extracted decisions from the conversation.
    ///
    /// Returns a reference to the decisions HashMap containing key-value
    /// pairs for recognized decision categories (naming, database,
    /// technology, architecture).
    ///
    /// # Example
    ///
    /// ```
    /// use zen::orchestration::ConversationContext;
    ///
    /// let mut ctx = ConversationContext::new();
    /// ctx.record("What pattern should we use?", "Repository pattern");
    /// assert_eq!(ctx.decisions().get("architecture"), Some(&"Repository pattern".to_string()));
    /// ```
    pub fn decisions(&self) -> &HashMap<String, String> {
        &self.decisions
    }
}

/// Configuration for real Claude backend in AIHumanProxy.
///
/// When configured, the proxy will use Claude headless to generate
/// contextual responses instead of mock pattern matching.
#[derive(Debug, Clone)]
pub struct ClaudeBackendConfig {
    /// Path to the Claude binary.
    pub binary: PathBuf,
    /// Working directory for Claude execution.
    pub cwd: PathBuf,
    /// Session ID for continuation (if any).
    pub session_id: Option<String>,
}

/// AI-as-Human proxy for answering skill clarification questions.
///
/// Skills like /pdd ask clarifying questions one at a time. Instead of
/// requiring human interaction, the AIHumanProxy answers these questions
/// based on the original user intent and accumulated context.
///
/// # Modes
///
/// - **Mock mode** (default): Returns pattern-based responses for testing
/// - **Real mode**: Uses Claude headless to generate contextual responses
///
/// # Example
///
/// ```
/// use zen::orchestration::AIHumanProxy;
///
/// let proxy = AIHumanProxy::new("build user authentication");
/// assert!(!proxy.needs_escalation("Should we add tests?"));
/// ```
#[derive(Debug, Clone)]
pub struct AIHumanProxy {
    /// Original user intent/prompt.
    original_prompt: String,
    /// Context accumulated from conversation.
    context: Arc<RwLock<ConversationContext>>,
    /// Model to use for generating answers (e.g., "haiku" for fast responses).
    model: String,
    /// Optional Claude backend configuration for real responses.
    claude_config: Option<ClaudeBackendConfig>,
}

impl AIHumanProxy {
    /// Default model for fast responses.
    const DEFAULT_MODEL: &'static str = "haiku";

    /// Create a new AI-as-Human proxy from a user prompt.
    ///
    /// The proxy stores the original prompt to use as context when
    /// answering questions. It uses the "haiku" model by default
    /// for fast responses. By default, this creates a mock proxy
    /// that returns pattern-based responses.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The original user request/intent
    ///
    /// # Example
    ///
    /// ```
    /// use zen::orchestration::AIHumanProxy;
    ///
    /// let proxy = AIHumanProxy::new("build user authentication");
    /// ```
    pub fn new(prompt: &str) -> Self {
        Self {
            original_prompt: prompt.to_string(),
            context: Arc::new(RwLock::new(ConversationContext::new())),
            model: Self::DEFAULT_MODEL.to_string(),
            claude_config: None,
        }
    }

    /// Create an AI-as-Human proxy with real Claude backend.
    ///
    /// When configured with a Claude backend, the proxy will use
    /// Claude headless to generate contextual responses instead
    /// of mock pattern matching.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The original user request/intent
    /// * `config` - Claude backend configuration
    ///
    /// # Example
    ///
    /// ```ignore
    /// use zen::orchestration::{AIHumanProxy, ClaudeBackendConfig};
    /// use std::path::PathBuf;
    ///
    /// let config = ClaudeBackendConfig {
    ///     binary: PathBuf::from("/usr/local/bin/claude"),
    ///     cwd: PathBuf::from("."),
    ///     session_id: None,
    /// };
    /// let proxy = AIHumanProxy::with_claude("build authentication", config);
    /// ```
    pub fn with_claude(prompt: &str, config: ClaudeBackendConfig) -> Self {
        Self {
            original_prompt: prompt.to_string(),
            context: Arc::new(RwLock::new(ConversationContext::new())),
            model: Self::DEFAULT_MODEL.to_string(),
            claude_config: Some(config),
        }
    }

    /// Check if the proxy is configured with a real Claude backend.
    pub fn has_claude_backend(&self) -> bool {
        self.claude_config.is_some()
    }

    /// Get the Claude backend configuration, if any.
    pub fn claude_config(&self) -> Option<&ClaudeBackendConfig> {
        self.claude_config.as_ref()
    }

    /// Update the session ID for Claude continuation.
    ///
    /// This should be called after each Claude interaction to enable
    /// session continuation for the next question.
    pub fn set_session_id(&mut self, session_id: String) {
        if let Some(ref mut config) = self.claude_config {
            config.session_id = Some(session_id);
        }
    }

    /// Clear the session ID (e.g., when a workflow completes).
    pub fn clear_session(&mut self) {
        if let Some(ref mut config) = self.claude_config {
            config.session_id = None;
        }
    }

    /// Get the original prompt.
    pub fn original_prompt(&self) -> &str {
        &self.original_prompt
    }

    /// Get the model being used.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Generate an answer to a skill's clarification question.
    ///
    /// This is a mock implementation that returns reasonable default
    /// answers based on common question patterns. The real implementation
    /// (in Step 5) will use Claude to generate contextual responses.
    ///
    /// # Arguments
    ///
    /// * `question` - The clarification question from the skill
    ///
    /// # Returns
    ///
    /// A reasonable default answer based on the question pattern.
    pub fn answer_question(&self, question: &str) -> String {
        let question_lower = question.to_lowercase();

        // Generate mock responses based on question patterns
        // Check more specific patterns first before general ones
        let answer = if question_lower.contains("database") {
            "Use PostgreSQL".to_string()
        } else if question_lower.contains("name") {
            "Use a descriptive name based on the functionality".to_string()
        } else if question_lower.contains("should we") || question_lower.contains("do you want") {
            "yes".to_string()
        } else if question_lower.contains("which") || question_lower.contains("what") {
            "Use the recommended default".to_string()
        } else {
            "Proceed with the recommended approach".to_string()
        };

        // Record the Q&A for context consistency
        if let Ok(mut context) = self.context.write() {
            context.record(question, &answer);
        }

        answer
    }

    /// Determine if a question needs human escalation.
    ///
    /// Some questions are too ambiguous or preference-based to be
    /// answered autonomously. This method detects such questions
    /// based on known patterns.
    ///
    /// # Arguments
    ///
    /// * `question` - The question to evaluate
    ///
    /// # Returns
    ///
    /// `true` if the question should be escalated to a human,
    /// `false` if the AI can handle it.
    ///
    /// # Example
    ///
    /// ```
    /// use zen::orchestration::AIHumanProxy;
    ///
    /// let proxy = AIHumanProxy::new("build auth");
    ///
    /// // Questions that need escalation
    /// assert!(proxy.needs_escalation("What is your personal preference?"));
    /// assert!(proxy.needs_escalation("Which style do you prefer?"));
    ///
    /// // Questions the AI can handle
    /// assert!(!proxy.needs_escalation("Should we add tests?"));
    /// ```
    pub fn needs_escalation(&self, question: &str) -> bool {
        let question_lower = question.to_lowercase();

        // Patterns that indicate subjective/preference questions
        const ESCALATION_PATTERNS: &[&str] = &[
            "which approach do you prefer",
            "what style do you want",
            "personal preference",
            "there are multiple valid",
            "which style do you prefer",
        ];

        ESCALATION_PATTERNS
            .iter()
            .any(|pattern| question_lower.contains(pattern))
    }

    /// Get access to the conversation context.
    ///
    /// This returns a clone of the Arc, allowing shared access
    /// to the context across threads.
    pub fn context(&self) -> Arc<RwLock<ConversationContext>> {
        Arc::clone(&self.context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ConversationContext tests

    #[test]
    fn test_conversation_context_new() {
        let ctx = ConversationContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);
    }

    #[test]
    fn test_conversation_context_default() {
        let ctx = ConversationContext::default();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_conversation_context_record() {
        let mut ctx = ConversationContext::new();
        ctx.record("What database?", "PostgreSQL");
        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_conversation_context_summary_empty() {
        let ctx = ConversationContext::new();
        assert_eq!(ctx.summary(), "");
    }

    #[test]
    fn test_conversation_context_summary_with_history() {
        let mut ctx = ConversationContext::new();
        ctx.record("Q1?", "A1");
        ctx.record("Q2?", "A2");
        let summary = ctx.summary();
        assert!(summary.contains("Q: Q1?"));
        assert!(summary.contains("A: A1"));
        assert!(summary.contains("Q: Q2?"));
        assert!(summary.contains("A: A2"));
    }

    #[test]
    fn test_conversation_context_clone() {
        let mut ctx = ConversationContext::new();
        ctx.record("Q?", "A");
        let cloned = ctx.clone();
        assert_eq!(ctx.len(), cloned.len());
    }

    // Decision extraction tests

    #[test]
    fn test_decisions_empty_initially() {
        let ctx = ConversationContext::new();
        assert!(ctx.decisions().is_empty());
    }

    #[test]
    fn test_decision_extraction_naming() {
        let mut ctx = ConversationContext::new();
        ctx.record("What should we name the module?", "user_auth");
        assert_eq!(
            ctx.decisions().get("naming"),
            Some(&"user_auth".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_naming_convention() {
        let mut ctx = ConversationContext::new();
        ctx.record("What naming convention should we use?", "snake_case");
        assert_eq!(
            ctx.decisions().get("naming"),
            Some(&"snake_case".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_call_it() {
        let mut ctx = ConversationContext::new();
        ctx.record("What should we call it?", "AuthService");
        assert_eq!(
            ctx.decisions().get("naming"),
            Some(&"AuthService".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_database() {
        let mut ctx = ConversationContext::new();
        ctx.record("Which database should we use?", "PostgreSQL");
        assert_eq!(
            ctx.decisions().get("database"),
            Some(&"PostgreSQL".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_db_shorthand() {
        let mut ctx = ConversationContext::new();
        ctx.record("What db do you want?", "MySQL");
        assert_eq!(ctx.decisions().get("database"), Some(&"MySQL".to_string()));
    }

    #[test]
    fn test_decision_extraction_framework() {
        let mut ctx = ConversationContext::new();
        ctx.record("Which framework should we use?", "Axum");
        assert_eq!(
            ctx.decisions().get("technology"),
            Some(&"Axum".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_library() {
        let mut ctx = ConversationContext::new();
        ctx.record("What library for JSON?", "serde_json");
        assert_eq!(
            ctx.decisions().get("technology"),
            Some(&"serde_json".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_pattern() {
        let mut ctx = ConversationContext::new();
        ctx.record("What pattern should we use?", "Repository pattern");
        assert_eq!(
            ctx.decisions().get("architecture"),
            Some(&"Repository pattern".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_architecture() {
        let mut ctx = ConversationContext::new();
        ctx.record("What architecture?", "Hexagonal");
        assert_eq!(
            ctx.decisions().get("architecture"),
            Some(&"Hexagonal".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_approach() {
        let mut ctx = ConversationContext::new();
        ctx.record("What approach should we take?", "TDD");
        assert_eq!(ctx.decisions().get("architecture"), Some(&"TDD".to_string()));
    }

    #[test]
    fn test_decision_extraction_design() {
        let mut ctx = ConversationContext::new();
        ctx.record("What design do you prefer?", "Event-driven");
        assert_eq!(
            ctx.decisions().get("architecture"),
            Some(&"Event-driven".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_multiple_decisions() {
        let mut ctx = ConversationContext::new();
        ctx.record("What should we name it?", "user_service");
        ctx.record("Which database?", "PostgreSQL");
        ctx.record("What framework?", "Actix");
        ctx.record("What pattern?", "MVC");

        assert_eq!(
            ctx.decisions().get("naming"),
            Some(&"user_service".to_string())
        );
        assert_eq!(
            ctx.decisions().get("database"),
            Some(&"PostgreSQL".to_string())
        );
        assert_eq!(
            ctx.decisions().get("technology"),
            Some(&"Actix".to_string())
        );
        assert_eq!(ctx.decisions().get("architecture"), Some(&"MVC".to_string()));
    }

    #[test]
    fn test_decision_extraction_override() {
        let mut ctx = ConversationContext::new();
        ctx.record("What name?", "old_name");
        ctx.record("What name should we use instead?", "new_name");
        // Latest decision should override
        assert_eq!(
            ctx.decisions().get("naming"),
            Some(&"new_name".to_string())
        );
    }

    #[test]
    fn test_decision_extraction_case_insensitive() {
        let mut ctx = ConversationContext::new();
        ctx.record("What DATABASE should we use?", "MongoDB");
        assert_eq!(
            ctx.decisions().get("database"),
            Some(&"MongoDB".to_string())
        );
    }

    #[test]
    fn test_no_decision_for_unrelated_question() {
        let mut ctx = ConversationContext::new();
        ctx.record("Should we add tests?", "yes");
        assert!(ctx.decisions().get("naming").is_none());
        assert!(ctx.decisions().get("database").is_none());
        assert!(ctx.decisions().get("technology").is_none());
        assert!(ctx.decisions().get("architecture").is_none());
    }

    #[test]
    fn test_summary_with_three_qa_pairs() {
        let mut ctx = ConversationContext::new();
        ctx.record("Q1?", "A1");
        ctx.record("Q2?", "A2");
        ctx.record("Q3?", "A3");
        let summary = ctx.summary();
        assert!(summary.contains("Q: Q1?"));
        assert!(summary.contains("A: A1"));
        assert!(summary.contains("Q: Q2?"));
        assert!(summary.contains("A: A2"));
        assert!(summary.contains("Q: Q3?"));
        assert!(summary.contains("A: A3"));
    }

    #[test]
    fn test_context_includes_previous_database_decision() {
        let mut ctx = ConversationContext::new();
        ctx.record("Which database?", "PostgreSQL");
        // When a follow-up question is asked, summary includes the previous decision
        let summary = ctx.summary();
        assert!(summary.contains("PostgreSQL"));
    }

    // AIHumanProxy creation tests

    #[test]
    fn test_ai_human_proxy_new_stores_prompt() {
        let proxy = AIHumanProxy::new("build user authentication");
        assert_eq!(proxy.original_prompt(), "build user authentication");
    }

    #[test]
    fn test_ai_human_proxy_default_model_is_haiku() {
        let proxy = AIHumanProxy::new("test prompt");
        assert_eq!(proxy.model(), "haiku");
    }

    #[test]
    fn test_ai_human_proxy_context_is_empty_initially() {
        let proxy = AIHumanProxy::new("test prompt");
        let context = proxy.context();
        let ctx = context.read().unwrap();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_ai_human_proxy_is_cloneable() {
        let proxy = AIHumanProxy::new("test prompt");
        let cloned = proxy.clone();
        assert_eq!(proxy.original_prompt(), cloned.original_prompt());
        assert_eq!(proxy.model(), cloned.model());
    }

    // Mock answer generation tests

    #[test]
    fn test_answer_question_returns_string() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Any question?");
        assert!(!answer.is_empty());
    }

    #[test]
    fn test_answer_question_for_database_question() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Which database should we use?");
        assert!(answer.contains("PostgreSQL"));
    }

    #[test]
    fn test_answer_question_for_yes_no_question() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Should we add tests?");
        assert_eq!(answer, "yes");
    }

    #[test]
    fn test_answer_question_for_do_you_want() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Do you want to add logging?");
        assert_eq!(answer, "yes");
    }

    #[test]
    fn test_answer_question_for_which_question() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Which framework is best?");
        assert!(answer.contains("recommended default"));
    }

    #[test]
    fn test_answer_question_for_what_question() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("What approach is best?");
        assert!(answer.contains("recommended default"));
    }

    #[test]
    fn test_answer_question_for_name_question() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("What name should we use?");
        assert!(answer.contains("descriptive name"));
    }

    #[test]
    fn test_answer_question_generic_fallback() {
        let proxy = AIHumanProxy::new("build auth");
        let answer = proxy.answer_question("Proceed?");
        assert!(answer.contains("recommended approach"));
    }

    #[test]
    fn test_answer_question_records_context() {
        let proxy = AIHumanProxy::new("build auth");
        proxy.answer_question("Database?");
        proxy.answer_question("Tests?");

        let context = proxy.context();
        let ctx = context.read().unwrap();
        assert_eq!(ctx.len(), 2);
    }

    // Escalation detection tests

    #[test]
    fn test_needs_escalation_personal_preference() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("What is your personal preference for this?"));
    }

    #[test]
    fn test_needs_escalation_style_preference() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("Which style do you prefer?"));
    }

    #[test]
    fn test_needs_escalation_multiple_valid() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("There are multiple valid approaches here"));
    }

    #[test]
    fn test_needs_escalation_which_approach() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("Which approach do you prefer?"));
    }

    #[test]
    fn test_needs_escalation_what_style_want() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("What style do you want?"));
    }

    #[test]
    fn test_no_escalation_should_we_add_tests() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(!proxy.needs_escalation("Should we add tests?"));
    }

    #[test]
    fn test_no_escalation_database_question() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(!proxy.needs_escalation("Which database should we use?"));
    }

    #[test]
    fn test_no_escalation_standard_questions() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(!proxy.needs_escalation("Do you want to add logging?"));
        assert!(!proxy.needs_escalation("Should we include documentation?"));
        assert!(!proxy.needs_escalation("What name should we use?"));
    }

    #[test]
    fn test_needs_escalation_case_insensitive() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(proxy.needs_escalation("PERSONAL PREFERENCE"));
        assert!(proxy.needs_escalation("Personal Preference"));
        assert!(proxy.needs_escalation("personal PREFERENCE"));
    }

    #[test]
    fn test_needs_escalation_partial_match() {
        let proxy = AIHumanProxy::new("build auth");
        // Should match because it contains the pattern
        assert!(proxy.needs_escalation("I know you have a personal preference for this"));
    }

    // ClaudeBackendConfig tests

    #[test]
    fn test_claude_backend_config_debug() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("."),
            session_id: Some("session_123".to_string()),
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("ClaudeBackendConfig"));
        assert!(debug.contains("session_123"));
    }

    #[test]
    fn test_claude_backend_config_clone() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("."),
            session_id: Some("session_123".to_string()),
        };
        let cloned = config.clone();
        assert_eq!(config.binary, cloned.binary);
        assert_eq!(config.session_id, cloned.session_id);
    }

    // AIHumanProxy Claude backend tests

    #[test]
    fn test_ai_human_proxy_no_claude_backend_by_default() {
        let proxy = AIHumanProxy::new("build auth");
        assert!(!proxy.has_claude_backend());
        assert!(proxy.claude_config().is_none());
    }

    #[test]
    fn test_ai_human_proxy_with_claude() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/usr/local/bin/claude"),
            cwd: PathBuf::from("/project"),
            session_id: None,
        };
        let proxy = AIHumanProxy::with_claude("build auth", config);
        assert!(proxy.has_claude_backend());
        assert!(proxy.claude_config().is_some());
    }

    #[test]
    fn test_ai_human_proxy_claude_config_values() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/path/to/claude"),
            cwd: PathBuf::from("/work/dir"),
            session_id: Some("existing_session".to_string()),
        };
        let proxy = AIHumanProxy::with_claude("build auth", config);

        let retrieved = proxy.claude_config().unwrap();
        assert_eq!(retrieved.binary, PathBuf::from("/path/to/claude"));
        assert_eq!(retrieved.cwd, PathBuf::from("/work/dir"));
        assert_eq!(
            retrieved.session_id,
            Some("existing_session".to_string())
        );
    }

    #[test]
    fn test_ai_human_proxy_set_session_id() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/path/to/claude"),
            cwd: PathBuf::from("."),
            session_id: None,
        };
        let mut proxy = AIHumanProxy::with_claude("build auth", config);

        // Initially no session
        assert!(proxy.claude_config().unwrap().session_id.is_none());

        // Set session ID
        proxy.set_session_id("new_session_123".to_string());
        assert_eq!(
            proxy.claude_config().unwrap().session_id,
            Some("new_session_123".to_string())
        );
    }

    #[test]
    fn test_ai_human_proxy_set_session_id_no_effect_without_backend() {
        let mut proxy = AIHumanProxy::new("build auth");
        // Should not panic, just no-op
        proxy.set_session_id("session_123".to_string());
        assert!(!proxy.has_claude_backend());
    }

    #[test]
    fn test_ai_human_proxy_clear_session() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/path/to/claude"),
            cwd: PathBuf::from("."),
            session_id: Some("existing_session".to_string()),
        };
        let mut proxy = AIHumanProxy::with_claude("build auth", config);

        // Verify session exists
        assert!(proxy.claude_config().unwrap().session_id.is_some());

        // Clear session
        proxy.clear_session();
        assert!(proxy.claude_config().unwrap().session_id.is_none());
    }

    #[test]
    fn test_ai_human_proxy_clear_session_no_effect_without_backend() {
        let mut proxy = AIHumanProxy::new("build auth");
        // Should not panic, just no-op
        proxy.clear_session();
        assert!(!proxy.has_claude_backend());
    }

    #[test]
    fn test_ai_human_proxy_with_claude_preserves_other_fields() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/path/to/claude"),
            cwd: PathBuf::from("."),
            session_id: None,
        };
        let proxy = AIHumanProxy::with_claude("my special prompt", config);

        assert_eq!(proxy.original_prompt(), "my special prompt");
        assert_eq!(proxy.model(), "haiku");
    }

    #[test]
    fn test_ai_human_proxy_with_claude_is_cloneable() {
        let config = ClaudeBackendConfig {
            binary: PathBuf::from("/path/to/claude"),
            cwd: PathBuf::from("."),
            session_id: Some("session_123".to_string()),
        };
        let proxy = AIHumanProxy::with_claude("build auth", config);
        let cloned = proxy.clone();

        assert_eq!(proxy.original_prompt(), cloned.original_prompt());
        assert!(cloned.has_claude_backend());
    }
}
