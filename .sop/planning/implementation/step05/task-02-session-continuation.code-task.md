# Task: Implement Claude Session Continuation

## Description
Add session continuation support to ClaudeHeadless, enabling multi-turn conversations by reusing session IDs across executions.

## Background
Claude Code maintains conversation context via session IDs. By passing `--session-id` to subsequent calls, we can continue a conversation. This is essential for the AIHumanProxy to maintain context when answering multiple questions.

## Reference Documentation
**Required:**
- Research: .sop/planning/research/claude-code-integration.md (session management section)

**Note:** You MUST read the Claude Code integration research for session continuation details.

## Technical Requirements
1. Add to ClaudeHeadless:
   - `continue_session(&self, session_id: &str, prompt: &str, cwd: &Path) -> Result<ClaudeResponse>`
2. Store session_id from responses for reuse
3. Implement session-aware execution that automatically continues
4. Integrate with AIHumanProxy for real Claude responses

## Dependencies
- ClaudeHeadless from task-01
- AIHumanProxy from Step 3

## Implementation Approach
1. Implement continue_session() with --session-id flag
2. Create SessionManager helper to track active sessions
3. Update AIHumanProxy.answer_question() to use real Claude calls
4. Add integration with haiku model for fast responses
5. Add tests for multi-turn conversations

## Acceptance Criteria

1. **Session Continuation**
   - Given a previous execution returned session_id="abc123"
   - When `continue_session("abc123", prompt, cwd)` is called
   - Then Claude continues the previous conversation context

2. **Context Preservation**
   - Given a multi-turn conversation about database choice
   - When continuing with follow-up questions
   - Then Claude remembers the previous context

3. **AIHumanProxy Integration**
   - Given AIHumanProxy with real Claude backend
   - When answer_question() is called
   - Then Claude generates contextual answers (not mocks)

4. **Fast Model Usage**
   - Given AIHumanProxy configured for haiku
   - When generating answers
   - Then responses are fast (< 2 seconds typical)

5. **Session Cleanup**
   - Given a workflow completes
   - When cleanup is triggered
   - Then session tracking is cleared

## Metadata
- **Complexity**: Medium
- **Labels**: Claude, Session, Integration, Multi-turn
- **Required Skills**: Rust, Claude CLI, session management
