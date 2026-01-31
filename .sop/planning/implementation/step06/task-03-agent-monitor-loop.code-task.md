# Task: Implement Agent Output Monitor Loop

## Description
Create the shared monitor_agent_output() method that watches an agent's output, detects questions, and answers them via AIHumanProxy. This is the core interaction pattern for all skill phases.

## Background
When running any skill (/pdd, /code-assist, etc.), the agent outputs text that may include questions. The monitor loop captures this output, detects question patterns, and uses AIHumanProxy to answer, enabling autonomous operation.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 monitor_agent_output code)

**Note:** You MUST read the detailed design document before beginning implementation. The exact loop pattern is shown in Section 4.2.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn monitor_agent_output(&self, agent: &AgentHandle) -> Result<SkillResult> {
       loop {
           match agent.read_output().await? {
               AgentOutput::Question(q) => {
                   let answer = self.ai_human.answer_question(&q).await;
                   agent.send(&answer).await?;
               }
               AgentOutput::Completed => break,
               AgentOutput::Error(e) => return Err(e.into()),
               _ => continue,
           }
       }
   }
   ```
2. Define `SkillResult` struct for skill completion data
3. Implement output pattern detection for questions
4. Add polling interval configuration
5. Handle timeout for unresponsive agents

## Dependencies
- AgentHandle from Step 4
- AIHumanProxy from Step 3
- AgentOutput enum from Step 4

## Implementation Approach
1. Define SkillResult struct
2. Implement monitor loop with polling interval
3. Add question detection patterns (? at end, "Please choose", etc.)
4. Integrate AIHumanProxy for answers
5. Add completion detection (skill finished output)
6. Add timeout handling
7. Add tests with mock agent output

## Acceptance Criteria

1. **Question Detection and Response**
   - Given agent outputs "Which database should we use?"
   - When monitor loop processes it
   - Then AIHumanProxy generates answer and sends to agent

2. **Completion Detection**
   - Given agent outputs skill completion marker
   - When monitor loop processes it
   - Then loop exits with SkillResult

3. **Error Handling**
   - Given agent outputs an error
   - When monitor loop processes it
   - Then error is returned, not swallowed

4. **Timeout Protection**
   - Given agent produces no output for timeout period
   - When timeout is exceeded
   - Then timeout error is returned

5. **Continuous Monitoring**
   - Given agent outputs multiple questions
   - When each question appears
   - Then each is answered and loop continues

## Metadata
- **Complexity**: Medium
- **Labels**: Orchestration, Monitoring, AI, Loop
- **Required Skills**: Rust, async loops, pattern matching
