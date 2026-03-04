//! Prompt building for spec discovery conversations.

use crate::db::{ConversationMessage, ConversationRole};

/// Build the initial prompt for starting a spec discovery conversation
pub fn build_initial_prompt(user_input: &str) -> String {
    format!(
        r#"# Spec Discovery Session

You are helping create a **comprehensive, implementation-ready software specification** through an interactive conversation.
Your job is to:
1. **Explore the codebase deeply** to understand the existing architecture, patterns, conventions, and relevant code
2. **Share detailed observations** about what you found that's relevant to the user's request — include specific file paths, function names, type definitions, and patterns
3. **Ask precise, informed questions** based on both the user's request AND what you find in the code — avoid vague questions; ask about specific technical decisions
4. **Gather ALL context needed** so that the final spec is a complete, self-contained document that requires NO further clarification to implement

## CRITICAL: Capture Everything

The specification you produce will be used to generate implementation tickets. Each ticket will be handed to an AI coding agent that has **NO access to this conversation** — it will ONLY see the spec and the ticket description. Therefore:

- **Capture EVERY detail** discussed in the conversation — nothing should be left implicit
- **Include specific file paths**, module names, function signatures, type definitions, and database schemas discovered during exploration
- **Document ALL decisions** with their rationale — not just what was decided, but WHY
- **Describe exact integration points** — which existing functions to call, which types to use, which patterns to follow
- **Specify error handling approaches**, edge cases, and validation rules discussed
- **Note naming conventions**, code organization patterns, and architectural styles from the codebase
- If the user mentions something in passing, still capture it — assume nothing will be remembered outside this spec

## User's Initial Request
{}

## Your Task

### Step 1: Explore the Codebase Thoroughly
Explore the repository to understand:
- Project structure, directory organization, and module boundaries
- Existing patterns and conventions (state management, component structure, API patterns, error handling)
- Related existing code that this feature might integrate with or extend — read the actual implementations, not just file names
- Dependencies and tools already in use (versions matter)
- Any existing similar functionality that can serve as a template or reference implementation
- Database schemas, API contracts, and type definitions that are relevant
- Test patterns and infrastructure

### Step 2: Respond
Your FINAL text response MUST be ONLY a JSON code block with two or three fields.
The `observations` and `questions` values are **markdown strings** — write them exactly as you want them displayed, with full markdown formatting (headings, bullet lists, numbered lists, bold, etc.).

When you still have questions to ask, respond with:
```json
{{
  "spec_complete": false,
  "observations": "<markdown string>",
  "questions": "<markdown string>"
}}
```

**Observations should be DETAILED** — include specific file paths, code patterns, function names, and type definitions you discovered. Don't just say "the project uses React" — say "the project uses React 18 with TypeScript, Zustand for state management (stores in `src/stores/`), and TailwindCSS for styling. Components follow the pattern in `src/components/views/ExampleView.tsx` with..."

**Questions should be SPECIFIC and ACTIONABLE** — don't ask "how should we handle errors?" — ask "Should validation errors for the email field return a 422 with field-level error messages (like the existing `createUser` endpoint in `src/api/users.rs:45`), or should we use a different pattern?"

When you have enough information (usually 3-6 exchanges, or immediately if the request is very clear), respond with the COMPLETE spec:
```json
{{
  "spec_complete": true,
  "observations": "<markdown string — comprehensive final summary of ALL findings>",
  "structured_spec": {{
    "requirements": [
      "Requirement 1: <specific, self-contained requirement — include HTTP routes, field names, types, behavior, edge cases>",
      "Requirement 2: <another specific requirement — e.g. 'GET /health returns 200 JSON with name and version fields'>",
      "Requirement 3: <add as many items as needed to cover EVERY requirement completely>"
    ],
    "decisions": [
      "Decision 1: <WHAT was decided> — <WHY this was chosen over alternatives> — <HOW it affects implementation>",
      "Decision 2: <detailed decision with rationale and implementation impact>"
    ],
    "constraints": [
      "Constraint 1: <specific constraint with context — e.g., 'Must use the existing `AuthMiddleware` in `src/middleware/auth.rs` for all new endpoints because...'>",
      "Constraint 2: <detailed constraint>"
    ],
    "technical_notes": [
      "Create <path/to/new/file.ext> — <purpose and key implementation details, e.g. 'entry point with graceful shutdown via http.Server'>",
      "Modify <path/to/existing/file.ext> — <exactly what to change and why>",
      "Follow pattern in <path/to/reference/file.ext> — <which aspects of the pattern to replicate>",
      "Run: <shell command needed for setup, e.g. 'go mod init github.com/org/repo && go get github.com/gin-gonic/gin'>",
      "Integration point: call <ExistingFunction> from <src/module.ts> to <achieve goal>",
      "Add as many notes as needed — one concrete, actionable item per entry"
    ]
  }}
}}
```

**IMPORTANT — JSON format rules:**
- `requirements` and `technical_notes` MUST be JSON arrays of strings, not prose paragraphs
- Each array item must be a plain string — do NOT embed markdown code fences (` ``` `) inside array values
- Reference specific file paths, function names, and types by name within plain strings
- `observations` and `questions` are markdown strings (free-form prose is fine there)
- Your response must contain ONLY the JSON code block. No text before or after it.

Start by exploring the codebase, then respond with the JSON block."#,
        user_input
    )
}

/// Build a prompt for continuing a spec discovery conversation
pub fn build_conversation_prompt(user_input: &str, messages: &[ConversationMessage]) -> String {
    let mut conversation_history = String::new();

    for msg in messages {
        let role_label = match msg.role {
            ConversationRole::User => "User",
            ConversationRole::Assistant => "Assistant",
            ConversationRole::System => "System",
        };
        conversation_history.push_str(&format!("\n{}: {}\n", role_label, msg.content));
    }

    format!(
        r#"# Spec Discovery Session (Continued)

You are helping create a **comprehensive, implementation-ready software specification** through interactive conversation.
You have access to explore the codebase to inform your responses.

## CRITICAL: Capture Everything

The specification you produce will be used to generate implementation tickets. Each ticket will be handed to an AI coding agent that has **NO access to this conversation** — it will ONLY see the spec and the ticket description. Therefore, when you write the final spec:

- **Capture EVERY detail** from the entire conversation — nothing should be left implicit
- **Include specific file paths**, function signatures, type definitions, and code patterns from the codebase
- **Document ALL decisions** with full rationale — not just what, but WHY and HOW it affects implementation
- **Describe exact integration points** — which existing functions to call, which types to reuse, which patterns to follow

## User's Initial Request
{}

## Conversation History
{}

## Your Task
1. Consider the user's latest response and incorporate ALL information they've provided throughout the conversation
2. If needed, explore more of the codebase to inform your response — read actual implementations, not just file names
3. Respond with structured JSON — either asking follow-up questions or completing the spec

## Response Format
Your FINAL text response MUST be ONLY a JSON code block with two or three fields.
The `observations` and `questions` values are **markdown strings** — write them exactly as you want them displayed, with full markdown formatting.

When you still have questions:
```json
{{
  "spec_complete": false,
  "observations": "<markdown string with DETAILED insights — include specific file paths, code patterns, and function names>",
  "questions": "<markdown string with SPECIFIC, ACTIONABLE numbered questions — reference code you found, propose options with trade-offs>"
}}
```

When you have enough information (you understand scope, integration points, technical decisions, and constraints):
```json
{{
  "spec_complete": true,
  "observations": "<markdown string — comprehensive final summary of ALL findings from exploration and conversation>",
  "structured_spec": {{
    "requirements": [
      "Requirement 1: <specific, self-contained requirement — include HTTP routes, field names, types, behavior, edge cases>",
      "Requirement 2: <another specific requirement — e.g. 'GET /health returns 200 JSON with name and version fields'>",
      "Requirement 3: <add as many items as needed to cover EVERY requirement completely>"
    ],
    "decisions": [
      "Decision 1: <WHAT was decided> — <WHY this was chosen> — <HOW it affects implementation>",
      "Decision 2: <detailed decision with full rationale and implementation impact>"
    ],
    "constraints": [
      "Constraint 1: <specific constraint with context and the codebase evidence for it>",
      "Constraint 2: <detailed constraint with implementation implications>"
    ],
    "technical_notes": [
      "Create <path/to/new/file.ext> — <purpose and key implementation details>",
      "Modify <path/to/existing/file.ext> — <exactly what to change and why>",
      "Follow pattern in <path/to/reference/file.ext> — <which aspects of the pattern to replicate>",
      "Integration point: call <ExistingFunction> from <src/module.ts> to <achieve goal>",
      "Add as many notes as needed — one concrete, actionable item per entry"
    ]
  }}
}}
```

**IMPORTANT — JSON format rules:**
- `requirements` and `technical_notes` MUST be JSON arrays of strings, not prose paragraphs
- Each array item must be a plain string — do NOT embed markdown code fences (` ``` `) inside array values
- Reference specific file paths, function names, and types by name within plain strings
- `observations` and `questions` are markdown strings (free-form prose is fine there)
- Your response must contain ONLY the JSON code block. No text before or after it.

Continue based on the user's latest response."#,
        user_input, conversation_history
    )
}

/// Prompt appended to a conversation when the agent returns only observations
/// (no questions), to trigger automatic spec completion.
pub const COMPLETION_PROMPT: &str = "Based on your observations and the conversation so far, you have enough information. \
    Please produce the final specification JSON block now. \
    The spec is the ONLY document implementing agents will see — capture EVERY detail from the conversation.\n\
    ```json\n{\n  \"spec_complete\": true,\n  \"observations\": \"<comprehensive final summary>\",\n  \"structured_spec\": {\n    \
    \"requirements\": [\"Requirement 1: <specific, self-contained requirement>\", \"Requirement 2: <another requirement>\"],\n    \
    \"decisions\": [\"Decision: WHAT — WHY — HOW it affects implementation\"],\n    \
    \"constraints\": [\"Constraint with context and codebase evidence\"],\n    \
    \"technical_notes\": [\"Create/Modify <path> — <details>\", \"Follow pattern in <path> — <what to replicate>\"]\n  }\n}\n```\n\
    IMPORTANT: requirements and technical_notes MUST be JSON arrays of strings, not single strings. \
    Each array item should be one concrete, actionable statement. Do NOT embed code fences inside array values.";

/// Format a slice of strings as a markdown bullet list (`- item\n- item`).
pub fn bullet_list(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("- {}", s))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn build_initial_prompt_includes_user_input() {
        let prompt = build_initial_prompt("Build a login page");
        assert!(prompt.contains("Build a login page"));
        assert!(prompt.contains("Spec Discovery Session"));
    }

    #[test]
    fn build_conversation_prompt_includes_history() {
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::User,
                content: "I want OAuth support".to_string(),
                created_at: Utc::now(),
            },
            ConversationMessage {
                id: "2".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::Assistant,
                content: "Which providers?".to_string(),
                created_at: Utc::now(),
            },
        ];

        let prompt = build_conversation_prompt("Build auth", &messages);
        assert!(prompt.contains("I want OAuth support"));
        assert!(prompt.contains("Which providers?"));
        assert!(prompt.contains("User:"));
        assert!(prompt.contains("Assistant:"));
    }

    #[test]
    fn build_conversation_prompt_handles_empty_history() {
        let prompt = build_conversation_prompt("Build auth", &[]);
        assert!(prompt.contains("Build auth"));
        assert!(prompt.contains("Conversation History"));
    }

    #[test]
    fn build_conversation_prompt_includes_system_messages() {
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::System,
                content: "Starting session...".to_string(),
                created_at: Utc::now(),
            },
            ConversationMessage {
                id: "2".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::User,
                content: "I want X".to_string(),
                created_at: Utc::now(),
            },
        ];

        let prompt = build_conversation_prompt("Build feature", &messages);
        assert!(prompt.contains("System:"));
        assert!(prompt.contains("Starting session..."));
        assert!(prompt.contains("User:"));
        assert!(prompt.contains("I want X"));
    }
}
