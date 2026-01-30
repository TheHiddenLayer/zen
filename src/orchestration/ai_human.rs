//! AI-as-Human proxy for autonomous skill interaction.
//!
//! The `AIHumanProxy` answers skill clarification questions on behalf of
//! the user, enabling fully autonomous workflow execution. It uses the
//! original user intent and accumulated conversation context to generate
//! reasonable responses.

use std::sync::{Arc, RwLock};

/// Tracks conversation context for consistent AI-as-Human responses.
///
/// This struct accumulates question-answer pairs and extracted decisions
/// to maintain consistency across the conversation. The full implementation
/// is in Task 3.2; this provides the minimal interface needed by AIHumanProxy.
#[derive(Debug, Default, Clone)]
pub struct ConversationContext {
    /// History of question-answer pairs.
    qa_history: Vec<(String, String)>,
}

impl ConversationContext {
    /// Create a new empty conversation context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a question and its answer for future reference.
    pub fn record(&mut self, question: &str, answer: &str) {
        self.qa_history.push((question.to_string(), answer.to_string()));
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
}

/// AI-as-Human proxy for answering skill clarification questions.
///
/// Skills like /pdd ask clarifying questions one at a time. Instead of
/// requiring human interaction, the AIHumanProxy answers these questions
/// based on the original user intent and accumulated context.
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
}

impl AIHumanProxy {
    /// Default model for fast responses.
    const DEFAULT_MODEL: &'static str = "haiku";

    /// Create a new AI-as-Human proxy from a user prompt.
    ///
    /// The proxy stores the original prompt to use as context when
    /// answering questions. It uses the "haiku" model by default
    /// for fast responses.
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
}
