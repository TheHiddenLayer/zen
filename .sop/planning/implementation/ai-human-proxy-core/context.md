# AIHumanProxy Core - Context Document

## Project Structure
- **Language:** Rust
- **Project Type:** Cargo workspace (library + binary)
- **Testing Framework:** Built-in #[test] and #[cfg(test)] modules
- **Key Dependencies:** tokio, serde, uuid, chrono

## Task Requirements

### Functional Requirements
1. Create `AIHumanProxy` struct that stores:
   - Original user prompt
   - Conversation context (via Arc<RwLock<ConversationContext>>)
   - Model identifier (default: "haiku")

2. Implement methods:
   - `new(prompt: &str) -> Self` - Constructor
   - `answer_question(&self, question: &str) -> String` - Mock response generation
   - `needs_escalation(&self, question: &str) -> bool` - Detect ambiguous questions

### Acceptance Criteria
1. Proxy creation stores original intent
2. Mock answer generation returns reasonable defaults
3. Escalation detection for phrases like "personal preference", "which style do you prefer"
4. Non-escalation for straightforward questions like "Should we add tests?"
5. Module compiles with `pub mod orchestration;` in lib.rs

## Existing Patterns

### Module Structure Pattern (from workflow/)
```
src/orchestration/
├── mod.rs      # Module exports
└── ai_human.rs # AIHumanProxy implementation
```

### Type Definition Pattern (from workflow/types.rs)
- Use serde derives for serialization
- Include comprehensive test modules within each file
- Use `#[cfg(test)] mod tests` pattern
- Document with `///` doc comments

### Test Pattern (from existing tests)
- Test functions prefixed with `test_`
- Descriptive test names using snake_case
- Comprehensive coverage of all public methods

## Dependencies
- `std::sync::{Arc, RwLock}` for thread-safe context sharing
- No additional crates needed (tokio already in Cargo.toml)

## Implementation Path
1. Create `src/orchestration/mod.rs` with exports
2. Create `src/orchestration/ai_human.rs` with:
   - `ConversationContext` struct (placeholder for Task 3.2)
   - `AIHumanProxy` struct
   - Constructor and methods
3. Add `pub mod orchestration;` to `src/lib.rs`
4. Add tests
