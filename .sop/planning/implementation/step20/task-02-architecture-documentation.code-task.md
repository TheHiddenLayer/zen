# Task: Create Architecture Documentation

## Description
Create architecture documentation for contributors including system overview, component descriptions, data flow diagrams, and development setup.

## Background
Architecture documentation helps new contributors understand the system design and make informed decisions when extending or modifying Zen.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (all sections)

**Note:** You MUST read the detailed design document - this task is about translating it into developer-focused documentation.

## Technical Requirements
1. Update `docs/architecture.md`:
   - System overview diagram (mermaid)
   - Component descriptions
   - Data flow explanation
   - Thread model
   - State management (TEA pattern)
2. Create `docs/skills-integration.md`:
   - How Zen orchestrates Skills
   - AI-as-Human pattern explained
   - Adding new skills
3. Create/update `CONTRIBUTING.md`:
   - Development setup
   - Code style guide
   - Testing requirements
   - PR process

## Dependencies
- Detailed design document
- All implemented components

## Implementation Approach
1. Create/update architecture overview with diagrams
2. Document each major component
3. Explain TEA pattern and state flow
4. Document Skills orchestration
5. Explain AI-as-Human pattern
6. Write development setup guide
7. Document code style and conventions
8. Create PR checklist

## Acceptance Criteria

1. **Architecture Diagram**
   - Given docs/architecture.md
   - When developer reads it
   - Then system structure is clear

2. **Component Descriptions**
   - Given each major component
   - When looking for docs
   - Then purpose and interfaces are documented

3. **Skills Integration Guide**
   - Given developer wants to add skill
   - When reading skills-integration.md
   - Then process is clear

4. **Development Setup**
   - Given new contributor
   - When following CONTRIBUTING.md
   - Then they can build and test locally

5. **Code Style Guide**
   - Given code review
   - When checking style
   - Then conventions are documented

## Metadata
- **Complexity**: Medium
- **Labels**: Documentation, Architecture, Contributing
- **Required Skills**: Technical writing, system design, mermaid
