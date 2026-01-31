# Task: Create ClaudeHeadless Executor

## Description
Create the ClaudeHeadless struct that executes Claude Code in headless mode with JSON output parsing, enabling programmatic interaction with the AI agent.

## Background
Claude Code supports headless execution via `claude -p "prompt" --output-format json`. This mode returns structured JSON responses that can be parsed programmatically. The ClaudeHeadless executor wraps this functionality for use by the AIHumanProxy and agent spawning.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 shows Claude headless usage)
- Research: .sop/planning/research/claude-code-integration.md

**Note:** You MUST read the Claude Code integration research for headless mode details.

## Technical Requirements
1. Create `src/orchestration/claude.rs` with `ClaudeHeadless`:
   ```rust
   pub struct ClaudeHeadless {
       binary: PathBuf,
       output_format: String,  // "json"
   }
   ```
2. Define response types:
   ```rust
   pub struct ClaudeResponse {
       pub session_id: Option<String>,
       pub result: ResultType,
       pub cost_usd: Option<f64>,
   }
   pub enum ResultType {
       Success { output: String },
       Error { message: String },
   }
   ```
3. Implement execution:
   - `execute(&self, prompt: &str, cwd: &Path) -> Result<ClaudeResponse>`
   - Parse JSON response structure
   - Handle non-zero exit codes

## Dependencies
- tokio::process for async command execution
- serde_json for response parsing
- which crate for binary detection

## Implementation Approach
1. Define ClaudeResponse and ResultType structs
2. Implement ClaudeHeadless::new() with binary detection
3. Implement execute() using tokio::process::Command
4. Parse stdout as JSON, handle stderr for errors
5. Add timeout handling for long operations
6. Add tests with mock responses

## Acceptance Criteria

1. **Binary Detection**
   - Given Claude Code is installed
   - When `ClaudeHeadless::new()` is called
   - Then the binary path is detected correctly

2. **Successful Execution**
   - Given a valid prompt
   - When `execute(prompt, cwd)` is called
   - Then ClaudeResponse with Success result is returned

3. **JSON Parsing**
   - Given Claude returns JSON output
   - When response is parsed
   - Then session_id, result, and cost are extracted

4. **Error Handling**
   - Given Claude returns an error
   - When execute() completes
   - Then ClaudeResponse with Error result is returned

5. **Timeout Handling**
   - Given a very long-running prompt
   - When timeout is exceeded
   - Then an appropriate timeout error is returned

## Metadata
- **Complexity**: Medium
- **Labels**: Claude, Integration, Headless, JSON
- **Required Skills**: Rust, tokio process, JSON parsing, CLI integration
