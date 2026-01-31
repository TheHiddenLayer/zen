# Task: Create User Documentation

## Description
Create comprehensive user documentation including installation, quick start, CLI reference, and troubleshooting guides.

## Background
Good documentation enables users to successfully use Zen without needing to read the code. It covers common workflows and helps users troubleshoot issues.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 2.4 User Workflow)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create/update `docs/user-guide.md`:
   - Installation instructions
   - Quick start (first workflow)
   - Workflow phases explained
   - CLI command reference
   - Configuration options
   - Troubleshooting guide
2. Update `README.md`:
   - Project description
   - Key features
   - Installation
   - Basic usage example
   - Links to detailed docs

## Dependencies
- All implemented features
- Existing documentation files

## Implementation Approach
1. Write installation section (prerequisites, install)
2. Write quick start with example workflow
3. Document each CLI command with examples
4. Explain 5 workflow phases
5. Document configuration options (zen.toml)
6. Create troubleshooting section
7. Update README with overview
8. Test all examples work

## Acceptance Criteria

1. **Quick Start Works**
   - Given new user follows quick start
   - When they run example command
   - Then first workflow completes

2. **CLI Reference Complete**
   - Given any CLI command
   - When user looks it up
   - Then usage and examples are documented

3. **Configuration Documented**
   - Given zen.toml options
   - When user needs to configure
   - Then all options are explained

4. **Troubleshooting Helpful**
   - Given common error scenarios
   - When user encounters them
   - Then solutions are documented

5. **README Overview**
   - Given someone views README
   - When they read it
   - Then they understand what Zen does

## Metadata
- **Complexity**: Medium
- **Labels**: Documentation, User Guide, README
- **Required Skills**: Technical writing, markdown
