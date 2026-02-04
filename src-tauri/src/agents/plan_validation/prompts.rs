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
}
