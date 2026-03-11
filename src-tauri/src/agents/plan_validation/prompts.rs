//! Prompt generation for plan validation.

/// Build the prompt for the validation agent to analyze a plan
pub fn build_plan_validation_prompt(plan: &str) -> String {
    format!(
        r#"Analyze this plan and determine if it requires user clarification before implementation.

## Plan
{plan}

## Decision Criteria
A plan needs clarification if it:
- Asks questions to the user
- Presents multiple options and asks which to choose
- States it cannot proceed without more information
- Expresses uncertainty about core requirements

A plan does NOT need clarification if it:
- Has a clear implementation path
- Makes reasonable assumptions (even if noted)
- Has complete, actionable steps

## Response Format
Respond with ONLY a JSON object:
{{"needs_clarification": true/false, "reason": "brief explanation"}}
"#
    )
}

/// Build the prompt for generating a user-facing clarification message
pub fn build_clarification_message_prompt(plan: &str) -> String {
    format!(
        r#"Based on this implementation plan, craft a clear message asking the user for the specific information needed to proceed.

## Plan
{plan}

## Instructions
- Extract the specific questions or ambiguities from the plan
- Write a concise, friendly message to the user
- Clearly state what information is needed
- If there are options, list them clearly
- Do NOT include implementation details - focus only on what the user needs to answer

Write ONLY the clarification message, no preamble.
"#
    )
}

/// Build the prompt for rewriting a task spec after the user answers clarification questions.
pub fn build_spec_rewrite_prompt(
    original_description: &str,
    clarification_questions: &str,
    user_answers: &str,
) -> String {
    format!(
        r#"You are rewriting a task specification. The original task was unclear in some areas, so the user was asked clarification questions and has now provided answers. Your job is to produce a single, clear, detailed task specification that incorporates all the information.

## Original Task Description
{original_description}

## Clarification Questions That Were Asked
{clarification_questions}

## User's Answers
{user_answers}

## Instructions
- Produce a single, self-contained task specification that merges the original description with the user's answers
- Preserve ALL original intent and requirements from the original description
- Incorporate the user's answers naturally into the specification — do not leave them as a separate Q&A section
- Resolve any ambiguities using the user's answers
- Be specific and actionable — the resulting spec should be ready for an engineer to implement without further questions
- Do NOT include meta-commentary about what changed or why
- Write ONLY the rewritten specification, no preamble or explanation
"#
    )
}

/// Build the prompt for the auto-clarification agent.
pub fn build_auto_clarification_prompt(
    plan: &str,
    clarification_reason: &str,
    ticket_description: &str,
    task_content: &str,
    completed_task_summaries: &str,
) -> String {
    format!(
        r#"You are an autonomous agent resolving a plan clarification. A previous validation step determined that this plan needs user clarification, but you must resolve it yourself.

## Ticket Description
{ticket_description}

## Current Task Content
{task_content}

## Plan That Triggered Clarification
{plan}

## Why Clarification Was Requested
{clarification_reason}

## Previously Completed Tasks
{completed_task_summaries}

## Your Job
Analyze the situation and decide how to proceed. You have three options:

1. **update_task** — If the clarification can be resolved by making reasonable decisions or the answers are evident from context, rewrite the task content to remove the ambiguity. Incorporate clear, specific decisions so the implementation can proceed without further questions.

2. **delete_task** — If the task is no longer needed (e.g., a previous task already accomplished what this task describes, or the task duplicates existing work), delete it.

3. **cannot_resolve** — If the clarification genuinely requires human judgment and you cannot make a reasonable decision, indicate that you cannot resolve it.

## Response Format
Respond with ONLY a JSON object in one of these forms:

For updating the task:
{{"action": "update_task", "updated_content": "the full rewritten task content", "reason": "brief explanation of what you decided"}}

For deleting the task:
{{"action": "delete_task", "reason": "brief explanation of why the task is no longer needed"}}

If you cannot resolve it:
{{"action": "cannot_resolve", "reason": "brief explanation of why human input is required"}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_plan_validation_prompt_contains_plan() {
        let plan = "1. Do step A\n2. Do step B";
        let prompt = build_plan_validation_prompt(plan);

        assert!(prompt.contains("1. Do step A"));
        assert!(prompt.contains("2. Do step B"));
        assert!(prompt.contains("Decision Criteria"));
        assert!(prompt.contains("needs_clarification"));
    }

    #[test]
    fn build_clarification_message_prompt_contains_plan() {
        let plan = "Should we use React or Vue?";
        let prompt = build_clarification_message_prompt(plan);

        assert!(prompt.contains("Should we use React or Vue?"));
        assert!(prompt.contains("clarification message"));
    }

    #[test]
    fn build_spec_rewrite_prompt_contains_all_sections() {
        let prompt = build_spec_rewrite_prompt(
            "Add dark mode toggle",
            "Should we use CSS variables or Tailwind?",
            "Use Tailwind dark: classes",
        );

        assert!(prompt.contains("Add dark mode toggle"));
        assert!(prompt.contains("Should we use CSS variables or Tailwind?"));
        assert!(prompt.contains("Use Tailwind dark: classes"));
        assert!(prompt.contains("Original Task Description"));
        assert!(prompt.contains("Clarification Questions That Were Asked"));
        assert!(prompt.contains("User's Answers"));
        assert!(prompt.contains("Instructions"));
    }

    #[test]
    fn build_spec_rewrite_prompt_handles_empty_inputs() {
        let prompt = build_spec_rewrite_prompt("", "", "");
        assert!(prompt.contains("Original Task Description"));
        assert!(prompt.contains("User's Answers"));
    }

    #[test]
    fn build_spec_rewrite_prompt_preserves_multiline_content() {
        let desc = "Step 1: Do A\nStep 2: Do B\nStep 3: Do C";
        let questions = "Which database?\nWhat auth method?";
        let answers = "PostgreSQL\nOAuth2 with Google";
        let prompt = build_spec_rewrite_prompt(desc, questions, answers);

        assert!(prompt.contains("Step 1: Do A\nStep 2: Do B\nStep 3: Do C"));
        assert!(prompt.contains("Which database?\nWhat auth method?"));
        assert!(prompt.contains("PostgreSQL\nOAuth2 with Google"));
    }

    #[test]
    fn build_spec_rewrite_prompt_includes_instructions() {
        let prompt = build_spec_rewrite_prompt("task", "question", "answer");
        assert!(prompt.contains("self-contained task specification"));
        assert!(prompt.contains("Preserve ALL original intent"));
        assert!(prompt.contains("no preamble or explanation"));
    }

    #[test]
    fn build_auto_clarification_prompt_contains_all_sections() {
        let prompt = build_auto_clarification_prompt(
            "1. Set up database\n2. Add auth",
            "Unclear which database to use",
            "Build a REST API",
            "Implement the database layer",
            "- [setup project] Created project structure",
        );

        assert!(prompt.contains("1. Set up database\n2. Add auth"));
        assert!(prompt.contains("Unclear which database to use"));
        assert!(prompt.contains("Build a REST API"));
        assert!(prompt.contains("Implement the database layer"));
        assert!(prompt.contains("Created project structure"));
        assert!(prompt.contains("Ticket Description"));
        assert!(prompt.contains("Current Task Content"));
        assert!(prompt.contains("Plan That Triggered Clarification"));
        assert!(prompt.contains("Why Clarification Was Requested"));
        assert!(prompt.contains("Previously Completed Tasks"));
    }

    #[test]
    fn build_auto_clarification_prompt_handles_empty_inputs() {
        let prompt = build_auto_clarification_prompt("", "", "", "", "");
        assert!(prompt.contains("Ticket Description"));
        assert!(prompt.contains("update_task"));
        assert!(prompt.contains("delete_task"));
        assert!(prompt.contains("cannot_resolve"));
    }

    #[test]
    fn build_auto_clarification_prompt_includes_response_format() {
        let prompt =
            build_auto_clarification_prompt("plan", "reason", "desc", "task", "completed");
        assert!(prompt.contains("update_task"));
        assert!(prompt.contains("delete_task"));
        assert!(prompt.contains("cannot_resolve"));
        assert!(prompt.contains("updated_content"));
        assert!(prompt.contains("Response Format"));
    }

    #[test]
    fn build_auto_clarification_prompt_preserves_multiline_content() {
        let plan = "Step 1: Do A\nStep 2: Do B\nStep 3: Do C";
        let completed = "- [task1] Done\n- [task2] Also done";
        let prompt =
            build_auto_clarification_prompt(plan, "ambiguous", "desc", "task content", completed);
        assert!(prompt.contains("Step 1: Do A\nStep 2: Do B\nStep 3: Do C"));
        assert!(prompt.contains("- [task1] Done\n- [task2] Also done"));
    }
}
