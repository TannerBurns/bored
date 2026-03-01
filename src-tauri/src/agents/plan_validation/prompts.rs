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
}
