//! Prompt templates for the planner agent.
//!
//! These prompts are used to guide AI agents (Claude or Cursor) through
//! codebase exploration and work plan generation.

/// Generate the exploration prompt for analyzing a codebase.
/// 
/// This prompt instructs the agent to operate in read-only mode and
/// gather information about the codebase structure, patterns, and
/// relevant files for the user's request.
pub fn generate_exploration_prompt(user_input: &str, iteration: usize) -> String {
    format!(r#"# Codebase Exploration Task (Iteration {iteration})

You are analyzing a codebase to understand how to implement the following request:

{user_input}

## CRITICAL: READ-ONLY MODE
- DO NOT create, modify, or delete any files
- DO NOT run any commands that make changes (no git commit, no file writes)
- DO NOT use any code generation or editing tools
- ONLY use read operations: reading files, searching code, listing directories

## Exploration Goals
Analyze the codebase to understand:
1. **Architecture Overview**: Overall project structure and technology stack
2. **Relevant Components**: Existing files, modules, and patterns related to the request
3. **Integration Points**: Where new code would need to connect with existing code
4. **Dependencies**: External libraries and internal dependencies involved
5. **Patterns to Follow**: Coding conventions, naming patterns, and architectural styles used

## What to Look For
- Configuration files (package.json, Cargo.toml, etc.)
- Existing similar features that can serve as templates
- Database schemas or data models
- API endpoints and their structure
- Test patterns and testing infrastructure
- Documentation and comments

## Output Format
Provide a structured analysis with:

### 1. Project Overview
Brief description of the project type and main technologies.

### 2. Relevant Files
List the key files that would be involved in implementing the request:
- File path and purpose
- What changes might be needed

### 3. Existing Patterns
Describe patterns from the codebase that should be followed:
- Code organization conventions
- Naming conventions
- Error handling patterns
- Testing approaches

### 4. Suggested Implementation Approach
High-level breakdown of how to implement the request based on your exploration.

### 5. Potential Challenges
Any complexity, edge cases, or considerations discovered during exploration.
"#, user_input = user_input, iteration = iteration)
}

/// Generate the planning prompt that produces structured JSON output.
/// 
/// This prompt takes the exploration context and user request to generate
/// a structured work plan with epics and tickets.
pub fn generate_planning_prompt(user_input: &str, exploration_context: &str) -> String {
    format!(r#"# Work Plan Generation

Based on your exploration of the codebase, create a structured work plan for the following request:

## User Request
{user_input}

## Exploration Context
{exploration_context}

## Output Requirements
You MUST output ONLY a valid JSON object with no additional text before or after.
The JSON must exactly match this schema (NOTE: use camelCase for field names):

```json
{{
  "overview": "Brief 1-2 sentence description of the implementation approach",
  "epics": [
    {{
      "title": "Epic Title (concise, descriptive)",
      "description": "What this epic accomplishes and its scope",
      "dependsOn": [],
      "tickets": [
        {{
          "title": "Ticket Title (action-oriented)",
          "description": "Detailed implementation task with context",
          "acceptanceCriteria": ["Criterion 1", "Criterion 2"]
        }}
      ]
    }},
    {{
      "title": "Second Epic Title",
      "description": "Description of second epic",
      "dependsOn": ["Epic Title"],
      "tickets": [...]
    }},
    {{
      "title": "Third Epic (depends on multiple)",
      "description": "This epic needs both previous epics",
      "dependsOn": ["Epic Title", "Second Epic Title"],
      "tickets": [...]
    }}
  ]
}}
```

**IMPORTANT**: `dependsOn` is an ARRAY of epic titles, not a single string:
- `[]` = root epic, no dependencies (can start immediately)
- `["Epic A"]` = depends on one epic
- `["Epic A", "Epic B"]` = depends on multiple epics (waits for ALL to complete)

## Planning Guidelines

### Greenfield vs Existing Codebase

**CRITICAL: Determine the project type from the exploration context:**

**Greenfield Project** (no existing codebase, building from scratch):
- There MUST be exactly ONE root epic: "Project Scaffolding" or similar foundation
- ALL other epics MUST depend on the scaffolding epic (directly or transitively)
- Nothing can run in parallel until the project structure exists
- Example: Scaffolding → (Backend + Frontend in parallel) → Integration

**Existing Codebase** (adding features to existing project):
- Multiple root epics are allowed if they touch independent areas
- True parallelism is possible when epics don't share code/files/APIs

### Dependency Rules (Strict)

For EACH epic, verify ALL of these conditions for parallelism:
1. Does NOT need files/folders created by another epic
2. Does NOT need types/interfaces defined by another epic
3. Does NOT need APIs/endpoints from another epic
4. Does NOT need database tables/schemas from another epic
5. CAN actually compile and run independently

If ANY condition fails → the epic MUST depend on the other epic.

**Common Dependency Patterns:**
- Backend API → Frontend that calls it (Frontend depends on Backend)
- Database schema → Code using that schema (Code depends on Schema)
- Shared types/interfaces → Components using them (Components depend on Types)
- Core library → Features using it (Features depend on Core)

### Multiple Dependencies

When an epic needs work from MULTIPLE other epics:
- List ALL dependencies: `"dependsOn": ["Epic A", "Epic B", "Epic C"]`
- The epic will only start when ALL dependencies are complete
- Example: Dashboard UI needs both "Backend API" AND "Frontend Core"

### Intermediate Consolidation Epics

When parallel work streams need to INTEGRATE before dependent work can continue:
1. Create an intermediate "Consolidate X and Y" epic
2. The consolidation epic depends on the parallel streams
3. Subsequent work depends on the consolidation epic

Example flow:
```
Scaffolding (root)
  ├── Backend API (depends on Scaffolding)
  ├── Frontend Core (depends on Scaffolding)
  └── Consolidate Backend + Frontend (depends on [Backend API, Frontend Core])
        └── Dashboard Feature (depends on Consolidate Backend + Frontend)
```

### Epic Structure
- Create 2-8 epics for a logical breakdown of work
- Each epic represents a coherent phase or component
- First epic in greenfield projects MUST be scaffolding/setup

### Ticket Guidelines
- Each epic should have 2-6 tickets
- Tickets should be atomic, implementable by a single developer
- Use action-oriented titles: "Add X", "Implement Y", "Create Z"
- Include enough detail in description for implementation
- Acceptance criteria should be specific and testable

### Final Consolidation Epic (Required)
Every plan MUST end with a "Consolidate Changes" epic that:
- Has a title starting with "Consolidate" (e.g., "Consolidate Changes")
- Depends on ALL leaf epics (epics that nothing else depends on)
- Has a single ticket: "Merge all epic branches into consolidation branch"
- Description should list all epics to merge

## Example: Greenfield Project

For a "Build a Tauri app with React frontend and Rust backend":

Epic 1: "Project Scaffolding" (dependsOn: [])  ← ONLY root epic
- Ticket: "Initialize Tauri project with React"
- Ticket: "Configure TypeScript and build tools"

Epic 2: "Backend Core" (dependsOn: ["Project Scaffolding"])
- Ticket: "Create Rust service module"
- Ticket: "Implement Tauri IPC commands"

Epic 3: "Frontend Core" (dependsOn: ["Project Scaffolding"])
- Ticket: "Set up React Router and layout"
- Ticket: "Create UI component library"

Epic 4: "Consolidate Backend and Frontend" (dependsOn: ["Backend Core", "Frontend Core"])
- Ticket: "Integrate frontend with backend APIs"
- Ticket: "Verify end-to-end functionality"

Epic 5: "Feature: Dashboard" (dependsOn: ["Consolidate Backend and Frontend"])
- Ticket: "Create dashboard component"
- Ticket: "Connect to backend data"

Epic 6: "Consolidate Changes" (dependsOn: ["Feature: Dashboard"])
- Ticket: "Merge all epic branches into consolidation branch"

Now generate the JSON work plan for the user's request. Output ONLY the JSON, no other text.
"#, user_input = user_input, exploration_context = exploration_context)
}

/// Generate markdown from the plan overview for display purposes.
pub fn format_plan_overview(overview: &str, epic_count: usize, ticket_count: usize) -> String {
    format!(
        "## Overview\n\n{}\n\n**Scope:** {} epic(s), {} ticket(s)\n",
        overview, epic_count, ticket_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exploration_prompt_contains_user_input() {
        let prompt = generate_exploration_prompt("Add dark mode support", 1);
        
        assert!(prompt.contains("Add dark mode support"));
        assert!(prompt.contains("Iteration 1"));
        assert!(prompt.contains("READ-ONLY MODE"));
        assert!(prompt.contains("DO NOT create, modify, or delete"));
    }

    #[test]
    fn test_exploration_prompt_iteration_number() {
        let prompt1 = generate_exploration_prompt("Test", 1);
        let prompt3 = generate_exploration_prompt("Test", 3);
        
        assert!(prompt1.contains("Iteration 1"));
        assert!(prompt3.contains("Iteration 3"));
    }

    #[test]
    fn test_planning_prompt_contains_context() {
        let prompt = generate_planning_prompt(
            "Add caching layer",
            "The codebase uses Redis for other features..."
        );
        
        assert!(prompt.contains("Add caching layer"));
        assert!(prompt.contains("Redis for other features"));
        assert!(prompt.contains("dependsOn"));
        assert!(prompt.contains("acceptanceCriteria"));
    }

    #[test]
    fn test_planning_prompt_has_json_schema() {
        let prompt = generate_planning_prompt("Test", "Context");
        
        assert!(prompt.contains("\"overview\""));
        assert!(prompt.contains("\"epics\""));
        assert!(prompt.contains("\"tickets\""));
        assert!(prompt.contains("\"title\""));
        assert!(prompt.contains("\"description\""));
    }

    #[test]
    fn test_format_plan_overview() {
        let overview = format_plan_overview("Implement feature X", 3, 12);
        
        assert!(overview.contains("Implement feature X"));
        assert!(overview.contains("3 epic(s)"));
        assert!(overview.contains("12 ticket(s)"));
    }
}
