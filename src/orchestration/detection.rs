//! Question detection utilities for agent output parsing.
//!
//! This module provides robust question detection patterns to identify when
//! an agent is asking for user input, enabling the AI-as-Human proxy to
//! respond appropriately.
//!
//! ## Detection Patterns
//!
//! The module detects several types of questions:
//! - **Direct questions**: Lines ending with `?`
//! - **Numbered options**: Patterns like "1. Option A\n2. Option B"
//! - **Yes/No prompts**: "Do you want", "Would you like", etc.
//! - **Input prompts**: "Enter", "Please provide", "Type your", etc.
//!
//! ## Example
//!
//! ```
//! use zen::orchestration::detection::{is_question, extract_question, is_waiting_for_input};
//!
//! assert!(is_question("Which database should we use?"));
//! assert!(is_question("Choose an option:\n1. PostgreSQL\n2. MySQL"));
//! assert!(!is_question("Creating design document..."));
//!
//! let question = extract_question("I've analyzed the code.\nWhich approach?");
//! assert_eq!(question, Some("Which approach?".to_string()));
//!
//! assert!(is_waiting_for_input("Enter your name: "));
//! ```

use regex::Regex;
use std::sync::LazyLock;

/// Regex for detecting numbered option lists (1. 2. 3. or a. b. c. patterns)
static NUMBERED_OPTIONS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*[1-9a-zA-Z][.)]\s+\S").unwrap()
});

/// Regex for detecting multiple consecutive numbered options
static MULTIPLE_OPTIONS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*1[.)]\s+.+\n\s*2[.)]\s+").unwrap()
});

/// Common question phrase patterns (case-insensitive matching)
const QUESTION_PHRASES: &[&str] = &[
    "do you want",
    "would you like",
    "should i",
    "shall i",
    "can i",
    "may i",
    "please confirm",
    "please select",
    "please choose",
    "choose one",
    "choose an option",
    "select an option",
    "select one",
    "enter your",
    "type your",
    "provide your",
    "what is your",
    "what should",
    "which one",
    "which option",
    "how would you",
    "how should",
    "ready to proceed",
    "want me to",
    "like me to",
];

/// Prompt patterns that indicate waiting for input
const INPUT_PROMPT_PATTERNS: &[&str] = &[
    "enter ",
    "please provide",
    "please enter",
    "please input",
    "please type",
    "type your",
    "input your",
    "provide the",
    "specify the",
    "press enter",
    "press any key",
    "continue?",
    "proceed?",
    "(y/n)",
    "[y/n]",
    "(yes/no)",
    "[yes/no]",
];

/// Check if text contains a question that needs a response.
///
/// This function detects several types of questions:
/// - Direct questions ending with `?`
/// - Numbered/lettered option lists that imply a selection
/// - Common question phrases like "Do you want", "Would you like"
/// - Input prompts like "Enter your name:"
///
/// # Arguments
///
/// * `text` - The text to analyze for questions
///
/// # Returns
///
/// `true` if the text contains a question, `false` otherwise.
///
/// # Example
///
/// ```
/// use zen::orchestration::detection::is_question;
///
/// assert!(is_question("Which database should we use?"));
/// assert!(is_question("Choose an option:\n1. PostgreSQL\n2. MySQL"));
/// assert!(!is_question("Creating design document..."));
/// ```
pub fn is_question(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Check for direct questions (lines ending with ?)
    if has_question_mark(trimmed) {
        return true;
    }

    // Check for numbered/lettered options with a prompt
    if has_option_list_with_prompt(trimmed) {
        return true;
    }

    // Check for question phrases
    if has_question_phrase(trimmed) {
        return true;
    }

    // Check for input prompt patterns
    if has_input_prompt(trimmed) {
        return true;
    }

    false
}

/// Extract the question text from output containing context and a question.
///
/// This function attempts to extract just the question portion from text
/// that may contain surrounding context, status messages, or other content.
///
/// # Arguments
///
/// * `text` - The text to extract a question from
///
/// # Returns
///
/// `Some(question)` if a question was found, `None` otherwise.
///
/// # Example
///
/// ```
/// use zen::orchestration::detection::extract_question;
///
/// let q = extract_question("I've analyzed the code.\nWhich approach would you prefer?");
/// assert_eq!(q, Some("Which approach would you prefer?".to_string()));
///
/// let q = extract_question("Processing files...");
/// assert_eq!(q, None);
/// ```
pub fn extract_question(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // First, try to find a line ending with ?
    if let Some(question) = extract_question_mark_line(trimmed) {
        return Some(question);
    }

    // Check for option list with a prompt before it
    if let Some(prompt) = extract_option_prompt(trimmed) {
        return Some(prompt);
    }

    // Check for input prompt patterns at the end
    if let Some(prompt) = extract_input_prompt(trimmed) {
        return Some(prompt);
    }

    // Check for question phrases in the last few lines
    if let Some(question) = extract_question_phrase_line(trimmed) {
        return Some(question);
    }

    None
}

/// Check if agent output indicates it's waiting for user input.
///
/// This detects patterns that suggest the agent has stopped and is
/// awaiting a response, such as:
/// - Prompt characters at the end (`:`, `>`, `?`)
/// - Explicit "Enter" or "Type" prompts
/// - Numbered options without a completion marker
///
/// # Arguments
///
/// * `output` - The agent output to analyze
///
/// # Returns
///
/// `true` if the agent appears to be waiting for input.
///
/// # Example
///
/// ```
/// use zen::orchestration::detection::is_waiting_for_input;
///
/// assert!(is_waiting_for_input("Enter your name: "));
/// assert!(is_waiting_for_input("Choose an option:\n1. Yes\n2. No"));
/// assert!(!is_waiting_for_input("Processing complete."));
/// ```
pub fn is_waiting_for_input(output: &str) -> bool {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return false;
    }

    // First check if it's a question at all
    if is_question(trimmed) {
        return true;
    }

    // Check for prompt endings
    let last_line = trimmed.lines().last().unwrap_or("");
    let last_trimmed = last_line.trim();

    // Ends with prompt characters (but not in the middle of a sentence)
    if last_trimmed.ends_with(':') && !last_trimmed.contains("...") {
        return true;
    }

    // Ends with > (shell-style prompt)
    if last_trimmed.ends_with('>') || last_trimmed.ends_with("> ") {
        return true;
    }

    // Ends with a question mark followed by space (awaiting response)
    if last_trimmed.ends_with("? ") {
        return true;
    }

    // Has option list at the end (likely waiting for selection)
    if has_trailing_option_list(trimmed) {
        return true;
    }

    false
}

// ============== Internal Helper Functions ==============

/// Check if any line ends with a question mark.
fn has_question_mark(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.ends_with('?')
    })
}

/// Check if text contains numbered options with a prompt.
fn has_option_list_with_prompt(text: &str) -> bool {
    // Must have at least 2 consecutive numbered options
    if !MULTIPLE_OPTIONS_RE.is_match(text) {
        return false;
    }

    // Check if there's a prompt-like line before or with the options
    let lower = text.to_lowercase();
    let has_prompt_context = lower.contains("choose")
        || lower.contains("select")
        || lower.contains("pick")
        || lower.contains("option")
        || lower.contains("which")
        || lower.contains("what");

    has_prompt_context
}

/// Check if text contains common question phrases.
fn has_question_phrase(text: &str) -> bool {
    let lower = text.to_lowercase();
    QUESTION_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// Check if text contains input prompt patterns.
fn has_input_prompt(text: &str) -> bool {
    let lower = text.to_lowercase();
    INPUT_PROMPT_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

/// Extract a line containing a question mark.
fn extract_question_mark_line(text: &str) -> Option<String> {
    // Find the last line ending with ? (most likely the actual question)
    text.lines()
        .rev()
        .find(|line| line.trim().ends_with('?'))
        .map(|line| line.trim().to_string())
}

/// Extract the prompt before an option list.
fn extract_option_prompt(text: &str) -> Option<String> {
    if !MULTIPLE_OPTIONS_RE.is_match(text) {
        return None;
    }

    // Find the line before the first numbered option
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("1.") || trimmed.starts_with("1)") {
            // Check the previous line for a prompt
            if i > 0 {
                let prev = lines[i - 1].trim();
                if !prev.is_empty() {
                    return Some(prev.to_string());
                }
            }
            break;
        }
    }

    // If no separate prompt line, check if first option line has a prompt prefix
    // e.g., "Choose: 1. Option A"
    None
}

/// Extract a line containing an input prompt pattern.
fn extract_input_prompt(text: &str) -> Option<String> {
    // Find lines containing input prompt patterns
    for line in text.lines().rev() {
        let line_lower = line.to_lowercase();
        for pattern in INPUT_PROMPT_PATTERNS {
            if line_lower.contains(pattern) {
                return Some(line.trim().to_string());
            }
        }
    }

    None
}

/// Extract a line containing a question phrase.
fn extract_question_phrase_line(text: &str) -> Option<String> {
    // Check the last few lines for question phrases
    let lines: Vec<&str> = text.lines().collect();
    let check_lines = if lines.len() > 5 {
        &lines[lines.len() - 5..]
    } else {
        &lines[..]
    };

    for line in check_lines.iter().rev() {
        let line_lower = line.to_lowercase();
        for phrase in QUESTION_PHRASES {
            if line_lower.contains(phrase) {
                return Some(line.trim().to_string());
            }
        }
    }

    None
}

/// Check if text ends with an option list (suggesting waiting for selection).
fn has_trailing_option_list(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return false;
    }

    // Check if last few lines are numbered options
    let last_lines = if lines.len() > 5 {
        &lines[lines.len() - 5..]
    } else {
        &lines[..]
    };

    let mut option_count = 0;
    for line in last_lines.iter().rev() {
        let trimmed = line.trim();
        if NUMBERED_OPTIONS_RE.is_match(trimmed) {
            option_count += 1;
        } else if !trimmed.is_empty() {
            break;
        }
    }

    option_count >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== is_question Tests ==========

    // Direct Question Detection
    #[test]
    fn test_is_question_direct_question_with_question_mark() {
        assert!(is_question("Which database should we use?"));
    }

    #[test]
    fn test_is_question_what_is_your_name() {
        assert!(is_question("What is your name?"));
    }

    #[test]
    fn test_is_question_should_we_add_tests() {
        assert!(is_question("Should we add tests?"));
    }

    #[test]
    fn test_is_question_multiline_with_question() {
        assert!(is_question("I've analyzed the code.\nWhich approach would you prefer?"));
    }

    // Numbered Options Detection
    #[test]
    fn test_is_question_numbered_options_choose() {
        assert!(is_question("Choose an option:\n1. PostgreSQL\n2. MySQL"));
    }

    #[test]
    fn test_is_question_numbered_options_select() {
        assert!(is_question("Select one:\n1. Option A\n2. Option B\n3. Option C"));
    }

    #[test]
    fn test_is_question_numbered_options_which() {
        assert!(is_question("Which do you prefer?\n1. Fast\n2. Reliable"));
    }

    // Yes/No Prompts
    #[test]
    fn test_is_question_do_you_want() {
        assert!(is_question("Do you want to proceed?"));
    }

    #[test]
    fn test_is_question_would_you_like() {
        assert!(is_question("Would you like to add tests?"));
    }

    #[test]
    fn test_is_question_should_i() {
        assert!(is_question("Should I continue with the implementation?"));
    }

    #[test]
    fn test_is_question_shall_we() {
        assert!(is_question("Shall we begin?"));
    }

    #[test]
    fn test_is_question_y_n_prompt() {
        assert!(is_question("Continue? (y/n)"));
    }

    #[test]
    fn test_is_question_yes_no_bracket() {
        assert!(is_question("Proceed with changes? [yes/no]"));
    }

    // Input Prompts
    #[test]
    fn test_is_question_enter_your_name() {
        assert!(is_question("Enter your name:"));
    }

    #[test]
    fn test_is_question_please_provide() {
        assert!(is_question("Please provide the file path"));
    }

    #[test]
    fn test_is_question_type_your_response() {
        assert!(is_question("Type your response:"));
    }

    #[test]
    fn test_is_question_what_is_project_name() {
        assert!(is_question("What is the project name?"));
    }

    #[test]
    fn test_is_question_press_enter() {
        assert!(is_question("Press Enter to continue"));
    }

    // Negative Cases (Not Questions)
    #[test]
    fn test_is_question_creating_document_is_not_question() {
        assert!(!is_question("Creating design document..."));
    }

    #[test]
    fn test_is_question_task_completed_is_not_question() {
        assert!(!is_question("Task completed successfully"));
    }

    #[test]
    fn test_is_question_processing_files_is_not_question() {
        assert!(!is_question("Processing files..."));
    }

    #[test]
    fn test_is_question_file_saved_is_not_question() {
        assert!(!is_question("File saved to /path/to/file"));
    }

    #[test]
    fn test_is_question_empty_string() {
        assert!(!is_question(""));
    }

    #[test]
    fn test_is_question_whitespace_only() {
        assert!(!is_question("   \n\t  "));
    }

    #[test]
    fn test_is_question_informational_list_is_not_question() {
        // Lists that are informational, not prompting for selection
        assert!(!is_question("The following files were created:\n- main.rs\n- lib.rs"));
    }

    // ========== extract_question Tests ==========

    #[test]
    fn test_extract_question_from_multiline() {
        let text = "I've analyzed the code.\nWhich approach would you prefer?";
        assert_eq!(
            extract_question(text),
            Some("Which approach would you prefer?".to_string())
        );
    }

    #[test]
    fn test_extract_question_from_options() {
        let text = "Here are the options:\n1. Option A\n2. Option B\nPlease choose one:";
        let result = extract_question(text);
        assert!(result.is_some());
        // Should extract the prompt, which could be "Please choose one:" or similar
    }

    #[test]
    fn test_extract_question_with_completion_and_question() {
        // In real agent output, the question is typically on its own line
        let text = "The implementation is complete.\nWould you like me to run tests?";
        assert_eq!(
            extract_question(text),
            Some("Would you like me to run tests?".to_string())
        );
    }

    #[test]
    fn test_extract_question_no_question() {
        assert_eq!(extract_question("Processing files..."), None);
    }

    #[test]
    fn test_extract_question_empty_string() {
        assert_eq!(extract_question(""), None);
    }

    #[test]
    fn test_extract_question_multiple_questions_returns_last() {
        let text = "First question?\nSecond question?\nThird question?";
        // Should return the last question (most recent/relevant)
        assert_eq!(
            extract_question(text),
            Some("Third question?".to_string())
        );
    }

    #[test]
    fn test_extract_question_from_option_list() {
        let text = "Choose an option:\n1. PostgreSQL\n2. MySQL\n3. SQLite";
        let result = extract_question(text);
        assert!(result.is_some());
        assert_eq!(result, Some("Choose an option:".to_string()));
    }

    #[test]
    fn test_extract_question_input_prompt() {
        let text = "Setting up the project.\nEnter your email address:";
        let result = extract_question(text);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Enter your email"));
    }

    // ========== is_waiting_for_input Tests ==========

    #[test]
    fn test_is_waiting_colon_prompt() {
        assert!(is_waiting_for_input("Enter your name:"));
    }

    #[test]
    fn test_is_waiting_colon_with_space() {
        assert!(is_waiting_for_input("Enter your name: "));
    }

    #[test]
    fn test_is_waiting_shell_prompt() {
        assert!(is_waiting_for_input("user@host $ >"));
    }

    #[test]
    fn test_is_waiting_question_with_space() {
        assert!(is_waiting_for_input("Continue? "));
    }

    #[test]
    fn test_is_waiting_option_list() {
        assert!(is_waiting_for_input("Choose one:\n1. Yes\n2. No"));
    }

    #[test]
    fn test_is_waiting_press_enter() {
        assert!(is_waiting_for_input("Press Enter to continue"));
    }

    #[test]
    fn test_is_waiting_not_waiting_complete() {
        assert!(!is_waiting_for_input("Processing complete."));
    }

    #[test]
    fn test_is_waiting_not_waiting_ellipsis() {
        // Ellipsis suggests ongoing work, not waiting
        assert!(!is_waiting_for_input("Processing..."));
    }

    #[test]
    fn test_is_waiting_empty_string() {
        assert!(!is_waiting_for_input(""));
    }

    #[test]
    fn test_is_waiting_question_mark_at_end() {
        assert!(is_waiting_for_input("Would you like to continue?"));
    }

    // ========== Edge Cases ==========

    #[test]
    fn test_is_question_case_insensitive() {
        assert!(is_question("DO YOU WANT to proceed?"));
        assert!(is_question("Would You Like to add tests?"));
        assert!(is_question("SHOULD I continue?"));
    }

    #[test]
    fn test_is_question_with_leading_whitespace() {
        assert!(is_question("   Which database?"));
    }

    #[test]
    fn test_is_question_with_trailing_whitespace() {
        assert!(is_question("Which database?   "));
    }

    #[test]
    fn test_extract_question_preserves_case() {
        let text = "Context here.\nWhich DATABASE Should We Use?";
        assert_eq!(
            extract_question(text),
            Some("Which DATABASE Should We Use?".to_string())
        );
    }

    // ========== Real-world Skill Output Samples ==========

    #[test]
    fn test_pdd_style_question() {
        let output = r#"I understand you want to build a user authentication system.

Let me ask a few clarifying questions:

What authentication method would you prefer?
1. Session-based authentication
2. JWT tokens
3. OAuth 2.0
"#;
        assert!(is_question(output));
        let extracted = extract_question(output);
        assert!(extracted.is_some());
    }

    #[test]
    fn test_code_assist_confirmation() {
        let output = r#"I've implemented the requested changes to the authentication module.

The following tests are now passing:
- test_login_success
- test_login_failure
- test_session_creation

Would you like me to commit these changes?"#;
        assert!(is_question(output));
    }

    #[test]
    fn test_not_a_question_status_update() {
        let output = r#"Running tests...
test_user_create ... ok
test_user_update ... ok
test_user_delete ... ok

All 3 tests passed."#;
        assert!(!is_question(output));
    }
}
