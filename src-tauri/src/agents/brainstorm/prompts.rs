//! Prompt building for brainstorm conversations.

use crate::db::{ConversationMessage, ConversationRole};

/// Build the initial prompt for starting a brainstorm conversation
pub fn build_initial_prompt(user_input: &str) -> String {
    format!(
        r#"# Spec Discovery Session

You are helping create a detailed software specification through an interactive conversation.
Your job is to:
1. **Explore the codebase** to understand the existing architecture, patterns, and conventions
2. **Share observations** about what you found that's relevant to the user's request
3. **Ask informed questions** based on both the user's request AND what you find in the code
4. **Gather enough context** to create a comprehensive spec for implementation

## User's Initial Request
{}

## Your Task

### Step 1: Explore the Codebase
Explore the repository to understand:
- Project structure and organization
- Existing patterns and conventions (state management, component structure, API patterns)
- Related existing code that this feature might integrate with or extend
- Dependencies and tools already in use
- Any existing similar functionality

### Step 2: Respond with Observations and Questions
Your response MUST follow this format:

## Observations
Share what you discovered from exploring the codebase that's relevant to the request:
- Key architectural patterns you found
- Existing code/modules that relate to this feature
- Integration points discovered
- Potential approaches based on existing patterns

## Questions
Ask clarifying questions to refine the spec. Each question should:
- Be informed by what you found in the codebase
- Offer multiple-choice options when possible (A, B, C)
- Focus on scope, integration, and implementation decisions

If you have NO questions (you have enough information from your exploration and the user's request is clear), leave the Questions section empty and instead output a completion JSON block.

### When Complete
When you have enough information (usually 3-6 exchanges, or immediately if the request is clear and you have no questions), output:
```json
{{
  "spec_complete": true,
  "structured_spec": {{
    "requirements": "Clear summary of what needs to be built",
    "decisions": ["Decision 1 based on user input", "Decision 2 from discussion"],
    "constraints": ["Constraint from codebase", "Constraint from user"],
    "technical_notes": "Implementation approach based on codebase exploration - mention specific files, patterns, and integration points discovered"
  }}
}}
```

Start by exploring the codebase, then share your observations and ask your first question."#,
        user_input
    )
}

/// Build a prompt for continuing a brainstorm conversation
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

You are helping create a detailed software specification through interactive conversation.
You have access to explore the codebase to inform your responses.

## User's Initial Request
{}

## Conversation History
{}

## Your Task
1. Consider the user's latest response
2. If needed, explore more of the codebase to inform your response
3. Respond with observations and questions, OR signal completion if you have enough info

## Response Format
Your response MUST follow this format:

## Observations
Share any new insights from the user's response or additional codebase exploration:
- What you learned from the user's answer
- Any additional code/patterns discovered
- Updated understanding of requirements

## Questions
Ask follow-up questions if needed:
- Each question should be informed by the conversation and codebase
- Offer multiple-choice options when possible (A, B, C)
- Focus on remaining unknowns

If you have NO questions (you have enough information), leave the Questions section empty and output the completion JSON block instead.

### When to Complete
You have enough information when you understand:
- What the user wants to build (scope and features)
- How it fits with existing code (integration points)
- Key technical decisions (patterns to follow, reuse vs. new code)
- Any constraints or requirements

When ready to complete, output:
```json
{{
  "spec_complete": true,
  "structured_spec": {{
    "requirements": "Clear summary of what needs to be built",
    "decisions": ["Decision 1", "Decision 2"],
    "constraints": ["Constraint 1", "Constraint 2"],
    "technical_notes": "Implementation approach with specific files, patterns, and integration points from codebase exploration"
  }}
}}
```

Continue based on the user's latest response."#,
        user_input, conversation_history
    )
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
