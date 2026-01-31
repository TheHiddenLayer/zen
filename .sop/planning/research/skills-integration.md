# Skills Integration Research

## Overview

This document provides research findings on how the agent-sop Skills system works and how Zen should integrate with it to provide Skills to agents.

## Executive Summary

Skills are structured markdown files (SKILL.md) that define agent workflows following a specific format. They are invoked through a Skill tool interface and can accept parameters. Claude Code discovers Skills from `~/.claude/skills/` directory and makes them available to agents through a tool interface.

## Skills Structure

### File Organization

Skills are organized in the `~/.claude/skills/` directory with the following structure:

```
~/.claude/skills/
├── code-assist/
│   └── SKILL.md
├── code-task-generator/
│   └── SKILL.md
├── codebase-summary/
│   └── SKILL.md
├── eval/
│   └── SKILL.md
└── pdd/
    └── SKILL.md
```

Each Skill is in its own directory containing a single `SKILL.md` file.

### SKILL.md Format

The SKILL.md file follows a specific structure:

#### 1. Front Matter (YAML)

```yaml
---
name: skill-name
description: Brief description of what the skill does
type: anthropic-skill
version: "1.0"
---
```

**Fields:**
- `name`: Identifier for the skill (kebab-case)
- `description`: One-line summary shown in skill listings
- `type`: Always "anthropic-skill" for this format
- `version`: Semantic version string

#### 2. Main Content Structure

After the front matter, the markdown document follows this structure:

1. **# Skill Name** - Top-level heading with the skill name
2. **## Overview** - High-level description of what the skill does
3. **## Parameters** - Input parameters the skill accepts
4. **## Steps** - Detailed step-by-step instructions
5. **## Examples** (optional) - Usage examples
6. **## Troubleshooting** (optional) - Common issues and solutions
7. **## Appendices** (optional) - Reference materials

### Parameters Section

Parameters are defined with:
- Parameter name (required/optional status)
- Default values
- Description
- **Constraints for parameter acquisition** - Special section with MUST/SHOULD/MAY requirements

Example:
```markdown
## Parameters

- **task_description** (required): A description of the task to be implemented
- **mode** (optional, default: "auto"): The interaction mode

**Constraints for parameter acquisition:**
- You MUST ask for all parameters upfront in a single prompt
- You MUST support multiple input methods (direct input, file path, URL)
```

### Steps Section

Steps are numbered sections with:
- Clear step titles (e.g., "### 1. Setup")
- Step description
- **Constraints** section with MUST/SHOULD/MAY requirements
- Sub-steps when needed (e.g., "#### 2.1 Analyze Requirements")

Example:
```markdown
### 1. Setup

Initialize the project environment and create necessary directory structures.

**Constraints:**
- You MUST validate and create the documentation directory structure
- You MUST discover existing instruction files
- You MUST NOT proceed if directory creation fails
```

### Constraint Language

Skills use a structured constraint language with specific keywords:

- **MUST**: Mandatory requirements that cannot be violated
- **MUST NOT**: Prohibited actions
- **SHOULD**: Strong recommendations that can be deviated from with justification
- **SHOULD NOT**: Actions to avoid unless there's good reason
- **MAY**: Optional features or alternatives

Constraints also include rationales using "because" clauses:
```markdown
- You MUST NOT commit changes until tests pass because committing broken code
  disrupts the development workflow
```

## How Skills Work in Claude Code

### Skill Discovery

Based on the Skill tool description in the system prompt, Claude Code:

1. Scans `~/.claude/skills/` directory for SKILL.md files
2. Reads the front matter (name, description, type, version)
3. Makes skills available through the Skill tool interface

### Skill Invocation

Skills are invoked through the `Skill` tool with these parameters:

```python
Skill(
    skill="skill-name",      # The skill name from front matter
    args="optional args"     # Optional arguments string
)
```

Example invocations:
- `Skill(skill="pdd")` - Invoke PDD skill with no args
- `Skill(skill="code-assist", args="-m 'auto'")` - Invoke with args
- `Skill(skill="codebase-summary", args="--consolidate")` - Invoke with flags

### User Interface

Users can reference skills by:
1. Slash commands: `/pdd`, `/code-assist`, etc.
2. Natural language: "run the pdd skill"
3. Direct tool invocation through the Skill tool

The system prompt tells Claude:
```
When users ask you to run a "slash command" or reference "/<something>"
(e.g., "/commit", "/review-pr"), they are referring to a skill. Use this
tool to invoke the corresponding skill.
```

### Skill Execution Flow

1. User requests a skill (via slash command or natural language)
2. Agent recognizes the skill request
3. Agent invokes Skill tool with skill name and optional args
4. Claude Code loads the SKILL.md file
5. The skill's instructions become part of the agent's context
6. Agent follows the Steps and Constraints defined in the skill
7. Agent completes the workflow and returns results

## Existing Skills Analysis

### /pdd (Prompt-Driven Development)

**Purpose**: Transform rough ideas into detailed design documents with implementation plans

**Key Features:**
- Iterative requirements clarification (one question at a time)
- Research phase with user collaboration
- Design document generation with Mermaid diagrams
- Implementation plan with test-driven approach
- Structured artifacts in `.sop/planning/` directory

**Parameters:**
- `rough_idea` (required): Initial concept
- `project_dir` (optional, default: ".sop/planning")

**Workflow:** Requirements → Research → Design → Implementation Plan → Summary

### /code-task-generator

**Purpose**: Generate structured code task files from descriptions or PDD plans

**Key Features:**
- Auto-detects input type (description vs PDD plan)
- Creates `.code-task.md` files following Amazon format
- For PDD plans: processes steps one at a time
- Task approval workflow before generation
- Tracks PDD checklist status

**Parameters:**
- `input` (required): Task description or PDD plan path
- `step_number` (optional): For PDD plans, specific step to process
- `output_dir` (optional, default: ".sop/planning")
- `task_name` (optional): For descriptions, specific task name

**Workflow:** Detect Mode → Analyze → Structure → Plan → Generate → Report

### /code-assist

**Purpose**: Implement code tasks using test-driven development

**Key Features:**
- TDD workflow (RED → GREEN → REFACTOR)
- Interactive vs Auto modes
- Explores existing patterns before implementing
- Builds and tests before committing
- Maintains detailed progress documentation
- CODEASSIST.md integration for project-specific constraints

**Parameters:**
- `task_description` (required): Task to implement
- `mode` (optional, default: "auto"): "interactive" or "auto"
- `documentation_dir` (optional, default: ".sop/planning")
- `repo_root` (optional, default: current directory)
- `task_name` (optional): Short descriptive name

**Workflow:** Setup → Explore → Plan → Code (Tests → Implementation → Refactor) → Commit

### /codebase-summary

**Purpose**: Analyze codebase and generate comprehensive documentation

**Key Features:**
- Structured metadata files (architecture, components, interfaces, etc.)
- Mermaid diagrams for visualizations
- Knowledge base index (index.md) for AI assistants
- Optional consolidated output (AGENTS.md, README.md, etc.)
- Update mode for incremental updates
- Consistency and completeness checking

**Parameters:**
- `output_dir` (optional, default: ".sop/summary")
- `consolidate` (optional, default: false)
- `consolidate_target` (optional, default: "AGENTS.md")
- `consolidate_prompt` (optional): How to structure consolidated content
- `check_consistency` (optional, default: true)
- `check_completeness` (optional, default: true)
- `update_mode` (optional, default: false)
- `codebase_path` (optional, default: current directory)

**Workflow:** Setup → Analyze → Generate → Review → Consolidate → Summary

### /eval

**Purpose**: Conversational evaluation framework for AI agents using Strands Evals SDK

**Key Features:**
- Natural conversation-driven workflow
- Strands Evals SDK integration (Case, Experiment, Evaluators)
- Phase-based workflow (Plan → Data → Eval → Report)
- Flat eval/ directory structure (sibling to agent code)
- Real agent execution (no simulation)
- Evidence-based recommendations

**Parameters:**
- Skill has no explicit parameters, uses conversational flow
- User provides context through natural language
- Agent extracts requirements from conversation

**Workflow:** Planning → Test Data Generation → Implementation/Execution → Analysis/Reporting

## How Zen Should Integrate Skills

### 1. Skills Directory Structure

Zen should provide a mechanism for agents to access Skills from:

```
~/.zen/skills/           # Zen-provided skills
~/.claude/skills/        # User/system skills (existing)
<project>/.zen/skills/   # Project-specific skills
```

**Discovery Order:** Project → User → System → Zen default

### 2. Skill Definition in Zen

Zen should support creating Skills using the same format:

```python
# Example: Zen Skill definition
class ZenSkill:
    def __init__(self, name: str, skill_md: str):
        self.name = name
        self.skill_md = skill_md

    def to_file(self, output_dir: Path) -> Path:
        """Write skill to SKILL.md file in appropriate directory."""
        skill_dir = output_dir / self.name
        skill_dir.mkdir(parents=True, exist_ok=True)
        skill_file = skill_dir / "SKILL.md"
        skill_file.write_text(self.skill_md)
        return skill_file
```

### 3. Skill Registration

Zen should provide a way to register Skills so they're available to agents:

```python
# Option 1: Register skills at agent creation
agent = Agent(
    skills=["pdd", "code-assist", "zen/commit-workflow"],
    skill_paths=["~/.zen/skills/", "~/.claude/skills/"]
)

# Option 2: Dynamic skill loading
agent.register_skill("custom-skill", skill_md_content)

# Option 3: Skill discovery from directories
agent.discover_skills([
    Path.home() / ".zen/skills",
    Path.home() / ".claude/skills"
])
```

### 4. Skill Tool Integration

Zen agents should have access to a Skill tool similar to Claude Code's:

```python
class SkillTool(Tool):
    name = "Skill"
    description = "Execute a skill (workflow/SOP) by name"

    def execute(self, skill: str, args: Optional[str] = None) -> str:
        """
        Load and execute a skill by injecting its content into agent context.

        Args:
            skill: Name of the skill to execute
            args: Optional arguments to pass to the skill
        """
        # 1. Discover skill from registered paths
        skill_path = self._find_skill(skill)

        # 2. Load SKILL.md content
        skill_content = self._load_skill(skill_path)

        # 3. Parse front matter and validate
        metadata = self._parse_front_matter(skill_content)

        # 4. Inject skill instructions into agent context
        return self._inject_skill(skill_content, args)
```

### 5. Skill Composition

Zen should support composing skills together:

```markdown
---
name: full-feature-workflow
description: Complete workflow from idea to implementation
type: anthropic-skill
version: "1.0"
skills:
  - pdd
  - code-task-generator
  - code-assist
---

# Full Feature Workflow

## Steps

### 1. Requirements and Design
Execute the PDD skill to create a detailed design.

**Constraints:**
- You MUST invoke Skill(skill="pdd") to gather requirements
- You MUST wait for PDD completion before proceeding

### 2. Generate Implementation Tasks
Convert PDD plan into code tasks.

**Constraints:**
- You MUST invoke Skill(skill="code-task-generator", args="<pdd-plan-path>")

### 3. Implement Tasks
Execute code-assist for each generated task.

**Constraints:**
- You MUST invoke Skill(skill="code-assist") for each task in sequence
```

### 6. Skill Inheritance and Extension

Zen should allow extending existing skills:

```markdown
---
name: zen-code-assist
description: Code assist with Zen-specific enhancements
type: anthropic-skill
version: "1.0"
extends: code-assist
---

# Zen Code Assist

Extends the standard code-assist skill with Zen-specific features.

## Additional Steps

### 0. Zen Context Setup
Before standard code-assist steps, set up Zen environment.

**Constraints:**
- You MUST check for .zen/ directory structure
- You MUST register implementation with Zen state management
- You MUST use Zen's GitNotes for storing task metadata
```

### 7. Zen-Provided Skills

Zen should provide core skills for its workflows:

1. **zen-task-create**: Create and register tasks in Zen
2. **zen-task-execute**: Execute tasks with Zen state management
3. **zen-commit-workflow**: Commit with Zen metadata
4. **zen-branch-workflow**: Branch management with Zen conventions
5. **zen-review**: Code review workflow
6. **zen-release**: Release management workflow

### 8. Skill Context and State

Skills should have access to Zen context:

```python
# Skills can access Zen state through context
class ZenSkillContext:
    """Context provided to skills running in Zen."""

    def __init__(self):
        self.zen_root: Path           # Zen root directory
        self.task_manager: TaskManager  # Access to task state
        self.git_manager: GitManager    # Git operations
        self.notes_manager: NotesManager # Git notes access
        self.current_task: Optional[Task] # Active task context

    def get_state(self, key: str) -> Any:
        """Get Zen state value."""
        ...

    def set_state(self, key: str, value: Any) -> None:
        """Set Zen state value."""
        ...
```

## Key Design Principles

### 1. Markdown-First

Skills are markdown documents that serve as:
- Human-readable documentation
- Machine-executable instructions
- Self-contained workflows

### 2. Constraint-Driven

Skills use structured constraints (MUST/SHOULD/MAY) to guide agent behavior with clear requirements and rationales.

### 3. Modular and Composable

Skills can:
- Be invoked independently
- Compose with other skills
- Extend existing skills
- Be project-specific or global

### 4. Context-Aware

Skills have access to:
- File system (through agent tools)
- Git state (through agent tools)
- Project structure
- Previous skill outputs
- User preferences

### 5. Progressive Disclosure

Skills use:
- Examples for common cases
- Troubleshooting for edge cases
- Appendices for reference material
- Clear step-by-step instructions

## Implementation Recommendations for Zen

### Priority 1: Basic Integration

1. **Skill Discovery**: Scan `~/.zen/skills/` and `~/.claude/skills/`
2. **Skill Tool**: Implement basic Skill tool for agent access
3. **SKILL.md Parser**: Parse front matter and markdown structure
4. **Context Injection**: Inject skill instructions into agent context

### Priority 2: Zen-Specific Skills

1. Create core Zen workflow skills:
   - zen-task-create
   - zen-task-execute
   - zen-commit-workflow

2. Document Zen skills following SKILL.md format

3. Register Zen skills automatically when Zen initializes

### Priority 3: Advanced Features

1. **Skill Composition**: Support invoking skills from within skills
2. **Skill Extension**: Allow extending/overriding existing skills
3. **Skill State**: Provide ZenSkillContext to running skills
4. **Skill Validation**: Validate skill format and constraints
5. **Skill Templates**: Provide templates for creating custom skills

### Priority 4: Developer Experience

1. **Skill Debugging**: Add logging/tracing for skill execution
2. **Skill Testing**: Framework for testing skills
3. **Skill Documentation**: Auto-generate skill catalog
4. **Skill CLI**: Commands for managing skills (`zen skill list`, etc.)

## Technical Considerations

### 1. Skill Loading Performance

- Cache parsed SKILL.md files
- Lazy load skills only when invoked
- Pre-load frequently used skills

### 2. Skill Security

- Validate skill paths (prevent directory traversal)
- Sandbox skill execution if needed
- Limit file system access per skill

### 3. Skill Versioning

- Support semantic versioning
- Allow specifying version constraints
- Handle version compatibility

### 4. Skill Updates

- Support updating skills without breaking workflows
- Provide migration paths for major version changes
- Allow pinning to specific versions

## Example: Zen Task Workflow Skill

```markdown
---
name: zen-task-workflow
description: Complete Zen task workflow from creation to completion
type: anthropic-skill
version: "1.0"
---

# Zen Task Workflow

## Overview

This skill guides you through the complete Zen task workflow, from task creation through implementation to completion and commit.

## Parameters

- **task_description** (required): Description of the task to create and implement
- **task_type** (optional, default: "feature"): Type of task (feature, bugfix, refactor)
- **branch_name** (optional): Custom branch name (auto-generated if not provided)

**Constraints for parameter acquisition:**
- You MUST ask for task_description if not provided
- You SHOULD infer task_type from description if not specified
- You MUST generate branch_name following Zen conventions if not provided

## Steps

### 1. Create Zen Task

Create a new task in Zen's task management system.

**Constraints:**
- You MUST use Zen's GitNotes API to create task metadata
- You MUST store task under refs/notes/zen/tasks/<task-id>
- You MUST generate a unique task ID
- You MUST set initial task state to "created"
- You MUST record creation timestamp

### 2. Create Worktree

Create a git worktree for the task.

**Constraints:**
- You MUST use Zen's GitRefs API to create refs/zen/worktrees/<branch-name>
- You MUST create worktree in .zen/worktrees/<task-name>
- You MUST create branch from current branch or master
- You MUST link worktree to task ID in metadata

### 3. Execute Task

Implement the task using code-assist or manual implementation.

**Constraints:**
- You MUST update task state to "in_progress"
- You MAY invoke Skill(skill="code-assist") for implementation
- You MUST work within the worktree directory
- You MUST track progress in task metadata

### 4. Commit Changes

Commit the implementation with Zen metadata.

**Constraints:**
- You MUST verify all tests pass before committing
- You MUST use conventional commit format
- You MUST add Zen task ID to commit message footer
- You MUST update task state to "completed"

### 5. Update Task State

Mark the task as complete in Zen.

**Constraints:**
- You MUST update task metadata with completion timestamp
- You MUST update task state to "completed"
- You MUST record commit hash in task metadata
- You SHOULD suggest next steps (PR creation, merge, etc.)

## Examples

### Example Input

```
task_description: "Add email validation to user registration form"
task_type: "feature"
```

### Example Output

```
✅ Created Zen task: task-001-email-validation
✅ Created worktree: .zen/worktrees/feature-email-validation
✅ Implemented email validation with tests
✅ Committed changes: feat: add email validation to registration
✅ Updated task state: completed

Task Summary:
- Task ID: task-001-email-validation
- Branch: feature-email-validation
- Commit: a1b2c3d4
- Files changed: 3 files (+42, -5 lines)

Next steps:
1. Create pull request: zen pr create
2. Or merge to master: zen merge
```

## Troubleshooting

### Worktree Creation Fails

If worktree creation fails:
- You SHOULD check if a worktree with that name already exists
- You SHOULD suggest using a different branch name
- You MAY offer to remove the existing worktree if it's safe

### Commit Fails

If commit fails:
- You MUST ensure tests are passing
- You MUST ensure working directory is clean
- You SHOULD verify git configuration is correct
```

## Conclusion

Skills are a powerful mechanism for encoding workflows and SOPs that agents can follow. Zen should:

1. Support the existing SKILL.md format for compatibility
2. Provide Skills-aware agent initialization
3. Create Zen-specific skills for common workflows
4. Allow project-specific skill customization
5. Enable skill composition and extension

By integrating with the Skills system, Zen can provide structured, repeatable workflows while maintaining flexibility and extensibility.
