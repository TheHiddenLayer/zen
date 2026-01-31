# Task: Implement AI-Driven Recovery

## Description
Implement the determine_recovery() method that uses AI judgment to decide the best recovery action for stuck or failing agents.

## Background
When an agent is stuck, there are multiple possible actions: restart, reassign, decompose the task, or escalate to user. AI analyzes the situation and makes an intelligent decision.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.4 determine_recovery, Section 7.2 Recovery Strategy)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to HealthMonitor:
   ```rust
   pub enum RecoveryAction {
       Restart,
       Reassign { to_agent: AgentId },
       Decompose { into_tasks: Vec<Task> },
       Escalate { message: String },
       Abort,
   }

   async fn determine_recovery(&self, agent: &AgentHandle) -> RecoveryAction
   ```
2. Use AI to analyze:
   - Recent agent output
   - Task description
   - Error patterns
   - Retry count
3. Execute recovery action
4. Track retries per task

## Dependencies
- HealthMonitor from task-01
- ClaudeHeadless for AI analysis
- AgentPool for recovery execution

## Implementation Approach
1. Define RecoveryAction enum
2. Implement determine_recovery() with AI prompt
3. Parse AI response into RecoveryAction
4. Implement execute_recovery() for each action type
5. Track retry counts in task state
6. Add tests with mock AI responses

## Acceptance Criteria

1. **Restart Decision**
   - Given agent stuck due to transient error
   - When determine_recovery() is called
   - Then RecoveryAction::Restart is returned

2. **Decompose Decision**
   - Given agent stuck on complex task
   - When AI analyzes situation
   - Then RecoveryAction::Decompose with smaller tasks is returned

3. **Escalate Decision**
   - Given max retries exceeded
   - When determine_recovery() is called
   - Then RecoveryAction::Escalate is returned

4. **Recovery Execution**
   - Given RecoveryAction::Restart
   - When execute_recovery() is called
   - Then agent is terminated and task is re-queued

5. **Retry Tracking**
   - Given task has been retried twice
   - When third failure occurs
   - Then max_retries is considered in decision

## Metadata
- **Complexity**: High
- **Labels**: Health, Recovery, AI, Decision
- **Required Skills**: Rust, AI integration, error recovery
