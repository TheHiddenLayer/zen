# Task: Implement Question Detection in Agent Output

## Description
Implement robust question detection patterns to identify when an agent is asking for user input, enabling the AI-as-Human proxy to respond appropriately.

## Background
Skills like /pdd output questions in various formats. The system needs to detect when the agent is waiting for input vs. just outputting information. This is crucial for autonomous operation.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 1.6 AI-as-Human Pattern)
- Research: .sop/planning/research/skills-integration.md

**Note:** You MUST read both documents to understand question patterns in skills.

## Technical Requirements
1. Create question detection in `src/orchestration/detection.rs`:
   - `is_question(text: &str) -> bool`
   - `extract_question(text: &str) -> Option<String>`
   - `is_waiting_for_input(output: &str) -> bool`
2. Implement detection patterns:
   - Direct questions (ending with ?)
   - Numbered options ("1. Option A\n2. Option B")
   - Yes/No prompts
   - Input prompts ("Enter", "Please provide", "What is")
3. Detect waiting state (cursor position, prompt patterns)
4. Integrate with AgentOutput::Question variant

## Dependencies
- AgentHandle.read_output() from Step 4
- monitor_agent_output() from Step 6

## Implementation Approach
1. Define question pattern regexes
2. Implement is_question() with multiple patterns
3. Implement extract_question() to clean up question text
4. Implement is_waiting_for_input() for prompt detection
5. Update AgentOutput parsing to use these functions
6. Add comprehensive tests with real skill output samples

## Acceptance Criteria

1. **Direct Question Detection**
   - Given output "Which database should we use?"
   - When is_question() is called
   - Then true is returned

2. **Numbered Options Detection**
   - Given output "Choose an option:\n1. PostgreSQL\n2. MySQL"
   - When is_question() is called
   - Then true is returned (options indicate question)

3. **Non-Question Output**
   - Given output "Creating design document..."
   - When is_question() is called
   - Then false is returned

4. **Question Extraction**
   - Given output with surrounding context and a question
   - When extract_question() is called
   - Then just the question text is returned

5. **Waiting State Detection**
   - Given agent is waiting at a prompt
   - When is_waiting_for_input() is called
   - Then true is returned

## Metadata
- **Complexity**: Medium
- **Labels**: Detection, Parsing, AI, Skills
- **Required Skills**: Rust, regex, text parsing
