# AIHumanProxy Core - Plan Document

## Test Strategy

### Test Scenarios

1. **Proxy Creation Tests**
   - `test_ai_human_proxy_new_stores_prompt` - Verify original prompt is stored
   - `test_ai_human_proxy_default_model_is_haiku` - Verify default model
   - `test_ai_human_proxy_context_is_empty_initially` - Verify empty context

2. **Mock Answer Generation Tests**
   - `test_answer_question_returns_string` - Basic return type
   - `test_answer_question_for_database_question` - Mock response for database questions
   - `test_answer_question_for_yes_no_question` - Mock "yes" for positive questions

3. **Escalation Detection Tests**
   - `test_needs_escalation_personal_preference` - "personal preference" triggers escalation
   - `test_needs_escalation_style_preference` - "which style do you prefer" triggers escalation
   - `test_needs_escalation_multiple_valid` - "there are multiple valid" triggers escalation
   - `test_needs_escalation_which_approach` - "which approach do you prefer" triggers escalation
   - `test_no_escalation_should_we_add_tests` - Standard question doesn't escalate
   - `test_no_escalation_database_question` - Technical question doesn't escalate
   - `test_needs_escalation_case_insensitive` - Case insensitive matching

4. **Clone Tests**
   - `test_ai_human_proxy_is_cloneable` - Verify Clone derives work

## Implementation Plan

### Step 1: Create Module Structure
- [x] Create `src/orchestration/mod.rs`
- [x] Create `src/orchestration/ai_human.rs`
- [x] Add `pub mod orchestration;` to `src/lib.rs`

### Step 2: Implement ConversationContext (Placeholder)
- [x] Define `ConversationContext` struct
- [x] Implement `Default` trait
- [x] Implement basic `summary()` method

### Step 3: Implement AIHumanProxy
- [x] Define struct with fields: original_prompt, context, model
- [x] Implement `new(prompt: &str) -> Self`
- [x] Implement `answer_question(&self, question: &str) -> String`
- [x] Implement `needs_escalation(&self, question: &str) -> bool`
- [x] Derive Clone

### Step 4: Add Tests
- [x] Implement all test scenarios
- [x] Verify all tests pass

### Step 5: Validate
- [x] Run `cargo build` to verify compilation
- [x] Run `cargo test orchestration` to verify tests pass
