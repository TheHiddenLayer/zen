# Question Detection Plan

## Test Strategy

### Test Scenarios for `is_question()`

#### Direct Questions (ending with ?)
1. "Which database should we use?" → true
2. "What is your name?" → true
3. "Should we add tests?" → true
4. "Creating design document..." → false

#### Numbered Options Detection
5. "Choose an option:\n1. PostgreSQL\n2. MySQL" → true
6. "Select one:\n1. Option A\n2. Option B\n3. Option C" → true
7. "The following files were created:\n1. main.rs\n2. lib.rs" → false (informational list)

#### Yes/No Prompts
8. "Do you want to proceed?" → true
9. "Would you like to add tests?" → true
10. "Should I continue?" → true
11. "Shall we begin?" → true

#### Input Prompts
12. "Enter your name:" → true
13. "Please provide the file path" → true
14. "Type your response:" → true
15. "What is the project name?" → true

#### Negative Cases (Not Questions)
16. "Creating design document..." → false
17. "Task completed successfully" → false
18. "Processing files..." → false
19. "File saved to /path/to/file" → false

### Test Scenarios for `extract_question()`

1. Input: "I've analyzed the code.\nWhich approach would you prefer?"
   Output: Some("Which approach would you prefer?")

2. Input: "Here are the options:\n1. Option A\n2. Option B\nPlease choose one:"
   Output: Some("Please choose one:")

3. Input: "The implementation is complete. Would you like me to run tests?"
   Output: Some("Would you like me to run tests?")

4. Input: "Processing files..."
   Output: None

5. Input: "Multiple questions?\nWhich one?\nIs this a test?"
   Output: Some("Is this a test?") (last question)

### Test Scenarios for `is_waiting_for_input()`

1. Output ending with "> " → true (shell prompt)
2. Output ending with ":" → true (input prompt)
3. Output ending with "? " → true (question waiting)
4. Output with blank line at end → possibly waiting
5. Output with "Press Enter to continue" → true
6. Output with regular text ending → false

## Implementation Plan

### 1. Create detection.rs Module
- Add `src/orchestration/detection.rs`
- Export from `src/orchestration/mod.rs`

### 2. Implement Core Functions

#### `is_question(text: &str) -> bool`
```rust
// Detection rules in priority order:
// 1. Check for direct questions (lines ending with ?)
// 2. Check for numbered/lettered options patterns
// 3. Check for input prompt patterns
// 4. Check for question phrase patterns
```

#### `extract_question(text: &str) -> Option<String>`
```rust
// Strategy:
// 1. Split by lines, find last line with question indicator
// 2. If question mark found, extract that line
// 3. If numbered options, extract the prompt before options
// 4. Return cleaned, trimmed question text
```

#### `is_waiting_for_input(output: &str) -> bool`
```rust
// Check for:
// 1. Prompt characters at end (>, :, ?)
// 2. "Enter", "Press", "Type" at end
// 3. Numbered options with no completion marker
// 4. Blank trailing line (agent stopped outputting)
```

### 3. Integration with AgentOutput
Update `pool.rs` to call `detection::is_question()` from `contains_question_pattern()`.

### 4. Testing Approach
- Unit tests for each function
- Real skill output sample tests (from /pdd, /code-assist)
- Edge cases (empty strings, whitespace, unicode)
