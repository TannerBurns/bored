//! Prompt generation for agent workflows.

mod branch;
mod task;
mod ticket;
mod utils;
mod workflow;

pub use branch::{
    generate_branch_name_generation_prompt, generate_branch_prompt,
    generate_get_branch_name_prompt, parse_branch_name_from_output,
};
pub use task::{
    generate_task_implement_prompt, generate_task_plan_prompt, generate_task_prompt,
    generate_workspace_task_context,
};
pub use ticket::{
    build_code_review_ticket_context, generate_custom_prompt, generate_implement_prompt,
    generate_plan_decomposition_prompt, generate_plan_prompt, generate_system_prompt,
    generate_ticket_prompt, generate_ticket_prompt_full, generate_ticket_prompt_with_workflow,
    generate_todo_implement_prompt, generate_workspace_context,
};
pub use workflow::generate_command_prompt;
