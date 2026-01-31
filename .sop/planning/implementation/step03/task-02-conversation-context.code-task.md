# Task: Implement ConversationContext Tracking

## Description
Create the ConversationContext struct that tracks Q&A history and extracted decisions, enabling consistent AI-as-Human responses across a workflow.

## Background
When answering multiple questions during a skill execution, the AI-as-Human needs context from previous answers to maintain consistency. For example, if it chose "PostgreSQL" for the database question, it should reference that decision in subsequent answers about database setup.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 ConversationContext code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `ConversationContext` struct in `src/orchestration/ai_human.rs`:
   ```rust
   pub struct ConversationContext {
       qa_history: Vec<(String, String)>,
       decisions: HashMap<String, String>,
   }
   ```
2. Implement `record(&mut self, question: &str, answer: &str)`:
   - Add to qa_history
   - Extract key decisions (naming, technology choices, etc.)
3. Implement `summary(&self) -> String`:
   - Format Q&A history for prompt inclusion
4. Implement decision extraction heuristics:
   - Detect naming convention decisions
   - Detect technology/library choices
   - Detect architectural decisions

## Dependencies
- AIHumanProxy from task-01
- HashMap from std::collections

## Implementation Approach
1. Define ConversationContext struct
2. Implement record() with history tracking
3. Implement decision extraction using keyword matching
4. Implement summary() formatting for prompts
5. Integrate with AIHumanProxy.answer_question()
6. Add tests for context accumulation

## Acceptance Criteria

1. **Q&A Recording**
   - Given a question and answer pair
   - When `context.record(question, answer)` is called
   - Then the pair is stored in qa_history

2. **Decision Extraction**
   - Given Q: "What should we name the module?" A: "user_auth"
   - When recorded
   - Then decisions["naming"] contains "user_auth"

3. **Context Summary**
   - Given 3 recorded Q&A pairs
   - When `context.summary()` is called
   - Then a formatted string with all Q&A is returned

4. **Consistent Answers**
   - Given previous decision about database (PostgreSQL)
   - When a follow-up database question is asked
   - Then the summary includes the previous decision for context

5. **Empty Context**
   - Given a new ConversationContext
   - When `summary()` is called
   - Then an empty or minimal string is returned (not an error)

## Metadata
- **Complexity**: Low
- **Labels**: AI, Context, State Management
- **Required Skills**: Rust, HashMap, string processing
