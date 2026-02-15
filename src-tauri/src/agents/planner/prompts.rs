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
    format!(
        r#"# Codebase Exploration Task (Iteration {iteration})

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
"#,
        user_input = user_input,
        iteration = iteration
    )
}

/// Generate the planning prompt that produces structured JSON output.
///
/// This prompt takes the exploration context and user request to generate
/// a structured work plan with epics and tickets.
pub fn generate_planning_prompt(user_input: &str, exploration_context: &str) -> String {
    format!(
        r###"# Work Plan Generation

Based on the detailed specification below, create a structured work plan with **implementation-ready tickets**.

## User Request & Specification
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
      "description": "What this epic accomplishes, its scope, and how it fits into the overall implementation",
      "dependsOn": [],
      "tickets": [
        {{
          "title": "Ticket Title (action-oriented)",
          "description": "DETAILED mini-spec for this ticket (see Ticket Description Requirements below)",
          "acceptanceCriteria": ["Criterion 1", "Criterion 2", "Criterion 3"],
          "branchName": "feat/epic-slug/ticket-slug"
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

## CRITICAL: Ticket Description Requirements

**Each ticket description is a MINI-SPEC.** The implementing agent will ONLY see the ticket title and description — it will NOT have access to the overall spec, the exploration context, or any conversation history. Therefore, each ticket description MUST be a **self-contained implementation document** that includes ALL of the following:

### What EVERY ticket description MUST contain:

1. **Objective**: A clear 1-2 sentence summary of what this ticket accomplishes and WHY it's needed in the larger context.

2. **Specific files to create or modify**: List EVERY file path that needs to be touched. For modifications, describe what changes are needed in each file. For new files, describe what they should contain.
   - Example: "Modify `src/stores/authStore.ts` to add a `refreshToken` field to the store state and a `setRefreshToken` action"
   - Example: "Create `src/components/auth/LoginForm.tsx` following the component pattern in `src/components/auth/SignupForm.tsx`"

3. **Implementation details**: Describe the actual logic, algorithms, data flow, or UI behavior to implement. Don't just say "implement the feature" — describe HOW.
   - What functions/methods to create and what they should do
   - What types/interfaces to define or use (include the shape if they're new)
   - What API endpoints to call or create (include request/response shapes)
   - What database queries or migrations to write

4. **Patterns and conventions to follow**: Reference specific existing code as templates.
   - Example: "Follow the same pattern as the `useTicketSync` hook in `src/hooks/useTicketSync.ts` for the new `useEpicSync` hook"
   - Example: "Use the same error handling pattern as `src-tauri/src/commands/tickets.rs` — return `Result<T, String>` with `.map_err(|e| e.to_string())`"

5. **Integration points**: How this ticket's work connects to existing code and to work from other tickets.
   - Which existing functions/modules to import and use
   - Which store actions to dispatch
   - Which events to emit or listen for

6. **Edge cases and error handling**: Specific scenarios to handle.
   - What happens on invalid input?
   - What happens when the network fails?
   - What are the boundary conditions?

7. **Testing notes** (when applicable): What should be tested and how.

8. **Branch context**: Every ticket description MUST include a `## Branch` section that states:
   - The branch name this ticket will be implemented on (MUST match the ticket's `branchName` field exactly)
   - For tickets that build on previous work in the same epic: the branch of the preceding ticket so the implementing agent knows its base
   - For the final consolidation merge ticket: list ALL branch names from all epics that need to be merged, in order
   - Example: "## Branch\nThis ticket is implemented on branch `feat/auth-core/add-refresh-token`.\nThis branch is based on the previous ticket's branch `feat/auth-core/setup-auth-store`."

### Ticket description formatting:
Use markdown within the description string for readability. Structure with headers, bullet points, and code references. Aim for 200-500 words per ticket description — be thorough.

### Example of a GOOD ticket description:

"## Objective\nAdd a refresh token mechanism to the auth store so user sessions persist across browser refreshes.\n\n## Files to Modify\n- `src/stores/authStore.ts`: Add `refreshToken: string | null` to the store state, add `setRefreshToken(token: string)` and `clearAuth()` actions\n- `src/utils/api.ts`: Update the `fetchWithAuth()` helper to check token expiry and auto-refresh using the stored refresh token\n- `src/components/App.tsx`: Add a `useEffect` that calls `refreshSession()` on mount to restore the session from the stored refresh token\n\n## Implementation Details\n1. In `authStore.ts`, extend the `AuthState` interface to include `refreshToken`. The `setRefreshToken` action should persist the token to `localStorage` under the key `app_refresh_token`. The `clearAuth` action should remove both tokens from the store and localStorage.\n2. In `api.ts`, the `fetchWithAuth()` function currently reads the access token from the store. Add a check: if the access token is expired (decode the JWT and check `exp`), call `POST /api/auth/refresh` with the refresh token to get a new access token before proceeding with the original request.\n3. In `App.tsx`, on initial mount, check localStorage for a refresh token. If found, call the refresh endpoint to validate it and restore the session.\n\n## Patterns to Follow\n- Follow the existing Zustand store pattern in `src/stores/authStore.ts` — use `set()` for state updates, `get()` for reading state\n- Follow the API call pattern in `src/utils/api.ts` — all API calls go through `fetchWithAuth()`\n\n## Error Handling\n- If the refresh token is expired or invalid, clear all auth state and redirect to login\n- If the refresh endpoint returns a network error, retry once after 2 seconds, then clear auth\n- Never expose the refresh token in URL parameters or logs\n\n## Branch\nThis ticket is implemented on branch `feat/auth-core/add-refresh-token`.\nThis branch is based on the previous ticket's branch `feat/auth-core/setup-auth-store`."

### Example of a BAD ticket description (DO NOT DO THIS):
"Implement refresh token support for authentication. Update the auth store and API utilities to handle token refresh."

This is BAD because it tells the implementer NOTHING about which files to touch, what the code should look like, or how it integrates with the existing codebase.

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
- Epic descriptions should summarize the scope and list what the tickets within will accomplish

### Ticket Guidelines
- Each epic should have 2-6 tickets
- Tickets should be atomic, implementable by a single AI coding agent
- Use action-oriented titles: "Add X", "Implement Y", "Create Z"
- **Each ticket description MUST be a comprehensive mini-spec** (see Ticket Description Requirements above)
- Acceptance criteria should be specific, testable, and verifiable by looking at code
- Include at least 3 acceptance criteria per ticket

### Branch Naming Rules (Required)

Every ticket MUST have a `branchName` field. Branch names are determined at planning time so the full branching strategy is known upfront and each implementing agent knows exactly what branch to work on.

**Branch name format**: `<type>/<epic-slug>/<ticket-slug>`

**Type prefixes** (choose based on the nature of the work):
- `feat/` — New features or functionality
- `fix/` — Bug fixes
- `chore/` — Maintenance tasks, dependency updates, config changes
- `refactor/` — Code restructuring without changing behavior
- `docs/` — Documentation only changes
- `test/` — Adding or updating tests

**Slug rules**:
- Lowercase, hyphen-separated, 2-5 words
- Epic slug should be a short identifier for the epic (e.g., `backend-api`, `frontend-core`, `auth-system`)
- Ticket slug should describe the specific work (e.g., `add-user-endpoints`, `setup-router`)
- All tickets within the same epic MUST share the same epic slug

**Examples**:
- `feat/backend-api/add-user-endpoints`
- `feat/backend-api/add-auth-middleware`
- `feat/frontend-core/setup-router`
- `chore/consolidate/merge-all-branches`

### Final Consolidation Epic (Required)
Every plan MUST end with a "Consolidate Changes" epic that:
- Has a title starting with "Consolidate" (e.g., "Consolidate Changes")
- Depends on ALL leaf epics (epics that nothing else depends on)
- Has a single ticket: "Merge all epic branches into consolidation branch"
- The ticket description MUST list ALL branch names from all epics to merge (in its `## Branch` section)
- The ticket's `branchName` should use the `chore/consolidate/` prefix (e.g., `chore/consolidate/merge-all-branches`)

## Example: Greenfield Project

For a "Build a Tauri app with React frontend and Rust backend":

Epic 1: "Project Scaffolding" (dependsOn: [])  ← ONLY root epic
- Ticket: "Initialize Tauri project with React" → branchName: `feat/scaffolding/init-tauri-react`
- Ticket: "Configure TypeScript and build tools" → branchName: `chore/scaffolding/configure-ts-build`

Epic 2: "Backend Core" (dependsOn: ["Project Scaffolding"])
- Ticket: "Create Rust service module" → branchName: `feat/backend-core/create-rust-service`
- Ticket: "Implement Tauri IPC commands" → branchName: `feat/backend-core/implement-ipc-commands`

Epic 3: "Frontend Core" (dependsOn: ["Project Scaffolding"])
- Ticket: "Set up React Router and layout" → branchName: `feat/frontend-core/setup-router-layout`
- Ticket: "Create UI component library" → branchName: `feat/frontend-core/create-ui-components`

Epic 4: "Consolidate Backend and Frontend" (dependsOn: ["Backend Core", "Frontend Core"])
- Ticket: "Integrate frontend with backend APIs" → branchName: `feat/integrate-be-fe/connect-apis`
- Ticket: "Verify end-to-end functionality" → branchName: `test/integrate-be-fe/verify-e2e`

Epic 5: "Feature: Dashboard" (dependsOn: ["Consolidate Backend and Frontend"])
- Ticket: "Create dashboard component" → branchName: `feat/dashboard/create-component`
- Ticket: "Connect to backend data" → branchName: `feat/dashboard/connect-backend-data`

Epic 6: "Consolidate Changes" (dependsOn: ["Feature: Dashboard"])
- Ticket: "Merge all epic branches into consolidation branch" → branchName: `chore/consolidate/merge-all-branches`

Now generate the JSON work plan for the user's request. Remember: every ticket description must be a detailed mini-spec. Output ONLY the JSON, no other text.
"###,
        user_input = user_input,
        exploration_context = exploration_context
    )
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
            "The codebase uses Redis for other features...",
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
        assert!(prompt.contains("\"branchName\""));
    }

    #[test]
    fn test_planning_prompt_requires_mini_spec_tickets() {
        let prompt = generate_planning_prompt("Test", "Context");

        // Verify the prompt instructs agents to write detailed mini-spec tickets
        assert!(prompt.contains("MINI-SPEC"));
        assert!(prompt.contains("self-contained implementation document"));
        assert!(prompt.contains("Specific files to create or modify"));
        assert!(prompt.contains("Implementation details"));
        assert!(prompt.contains("Patterns and conventions to follow"));
        assert!(prompt.contains("Integration points"));
        assert!(prompt.contains("Edge cases and error handling"));
        assert!(prompt.contains("200-500 words per ticket"));
        assert!(prompt.contains("Branch context"));
    }

    #[test]
    fn test_planning_prompt_has_branch_naming_rules() {
        let prompt = generate_planning_prompt("Test", "Context");

        assert!(prompt.contains("Branch Naming Rules"));
        assert!(prompt.contains("branchName"));
        assert!(prompt.contains("feat/"));
        assert!(prompt.contains("fix/"));
        assert!(prompt.contains("chore/"));
        assert!(prompt.contains("refactor/"));
        assert!(prompt.contains("<type>/<epic-slug>/<ticket-slug>"));
        assert!(prompt.contains("chore/consolidate/"));
    }

    #[test]
    fn test_planning_prompt_example_includes_branch_names() {
        let prompt = generate_planning_prompt("Test", "Context");

        assert!(prompt.contains("feat/scaffolding/init-tauri-react"));
        assert!(prompt.contains("feat/backend-core/create-rust-service"));
        assert!(prompt.contains("feat/frontend-core/setup-router-layout"));
        assert!(prompt.contains("chore/consolidate/merge-all-branches"));
    }

    #[test]
    fn test_format_plan_overview() {
        let overview = format_plan_overview("Implement feature X", 3, 12);

        assert!(overview.contains("Implement feature X"));
        assert!(overview.contains("3 epic(s)"));
        assert!(overview.contains("12 ticket(s)"));
    }
}
