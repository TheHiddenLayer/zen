# Task: Create AIHumanProxy Core Structure

## Description
Create the AIHumanProxy struct that autonomously answers skill clarification questions on behalf of the user. This is a key innovation enabling fully autonomous workflow execution.

## Background
Skills like /pdd ask clarifying questions one at a time. Instead of requiring human interaction, the AIHumanProxy answers these questions based on the original user intent and accumulated context. It uses a fast model (haiku) for quick responses.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 1.6 AI-as-Human Pattern, Section 4.2 AIHumanProxy code)

**Note:** You MUST read the detailed design document before beginning implementation. Section 1.6 explains the pattern in detail.

## Technical Requirements
1. Create `src/orchestration/mod.rs` with module exports
2. Create `src/orchestration/ai_human.rs` with:
   ```rust
   pub struct AIHumanProxy {
       original_prompt: String,
       context: Arc<RwLock<ConversationContext>>,
       model: String,  // "haiku" for fast responses
   }
   ```
3. Implement `AIHumanProxy::new(prompt: &str) -> Self`
4. Implement `answer_question(&self, question: &str) -> String` (mock for now)
5. Implement `needs_escalation(&self, question: &str) -> bool`
6. Add `pub mod orchestration;` to `src/lib.rs`

## Dependencies
- tokio for async (already in Cargo.toml)
- Arc, RwLock from std::sync

## Implementation Approach
1. Create the orchestration module structure
2. Define AIHumanProxy with original prompt storage
3. Implement mock answer_question that returns reasonable defaults
4. Implement escalation detection for ambiguous patterns
5. Prepare for real Claude integration in Step 5
6. Add unit tests with mock responses

## Acceptance Criteria

1. **Proxy Creation**
   - Given a user prompt "build user authentication"
   - When `AIHumanProxy::new(prompt)` is called
   - Then a proxy is created storing the original intent

2. **Mock Answer Generation**
   - Given a question "Which database should we use?"
   - When `answer_question(question)` is called
   - Then a reasonable default answer is returned (mock)

3. **Escalation Detection**
   - Given a question containing "personal preference" or "which style do you prefer"
   - When `needs_escalation(question)` is called
   - Then true is returned indicating human input needed

4. **Non-Escalation Questions**
   - Given a straightforward question "Should we add tests?"
   - When `needs_escalation(question)` is called
   - Then false is returned (AI can handle this)

5. **Module Integration**
   - Given the orchestration module is complete
   - When `cargo build` is run
   - Then the project compiles with `pub mod orchestration;` in lib.rs

## Metadata
- **Complexity**: Medium
- **Labels**: AI, Orchestration, Innovation, Core
- **Required Skills**: Rust, async, Arc/RwLock, pattern matching
