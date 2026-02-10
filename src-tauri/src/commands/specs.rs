//! Tauri commands for spec (planning) operations

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::broadcast;

use crate::agents::planner::{PlannerAgent, PlannerConfig};
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::api::state::LiveEvent;
use crate::commands::claude::ClaudeApiSettingsState;
use crate::commands::ApiConnState;
use crate::db::{
    CreateSpec, Database, Exploration, Spec, SpecProgress, SpecVersion, SpecVersionStatus,
    SpecWithVersion, UpdateSpec,
};
use crate::lifecycle::epic::{check_spec_completion_by_id, on_epic_moved_to_ready};

/// Input for creating a spec
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpecInput {
    pub board_id: String,
    pub target_board_id: Option<String>,
    pub project_id: String,
    pub name: String,
    pub user_input: String,
    pub model: Option<String>,
}

#[tauri::command]
pub async fn create_spec(
    input: CreateSpecInput,
    db: State<'_, Arc<Database>>,
) -> Result<Spec, String> {
    tracing::info!(
        "Creating spec '{}' for board {} (target: {:?}) project {}",
        input.name,
        input.board_id,
        input.target_board_id,
        input.project_id
    );

    db.create_spec(&CreateSpec {
        board_id: input.board_id,
        target_board_id: input.target_board_id,
        project_id: input.project_id,
        name: input.name,
        user_input: input.user_input,
        model: input.model,
        settings: serde_json::json!({}),
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_specs(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Spec>, String> {
    db.get_specs(&board_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_specs(db: State<'_, Arc<Database>>) -> Result<Vec<Spec>, String> {
    db.get_all_specs().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_spec(id: String, db: State<'_, Arc<Database>>) -> Result<Spec, String> {
    db.get_spec(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_spec(
    id: String,
    name: Option<String>,
    user_input: Option<String>,
    model: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Spec, String> {
    tracing::info!("Updating spec {}", id);

    db.update_spec(
        &id,
        &UpdateSpec {
            name,
            user_input,
            model,
            settings: None,
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_spec(id: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    tracing::info!("Deleting spec {}", id);
    db.delete_spec(&id).map_err(|e| e.to_string())
}

/// Delete a spec and all its associated tickets (epics, child tickets, and their data)
#[tauri::command]
pub async fn delete_spec_with_tickets(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<usize, String> {
    tracing::info!("Deleting spec {} with all tickets", id);
    let count = db
        .delete_spec_with_tickets(&id)
        .map_err(|e| e.to_string())?;
    tracing::info!("Deleted spec {} and {} tickets", id, count);
    Ok(count)
}

/// Reset plan execution - delete all tickets for a spec version and reset status to approved
/// This allows the user to re-execute the plan to recreate tickets
#[tauri::command]
pub async fn reset_plan_execution(
    spec_id: String,
    version_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<usize, String> {
    tracing::info!("Resetting plan execution for spec {} version {}", spec_id, version_id);

    // Get the specific version
    let version = db
        .get_spec_version(&version_id)
        .map_err(|e| e.to_string())?;

    // Verify version belongs to the spec
    if version.spec_id != spec_id {
        return Err("Version does not belong to this spec".to_string());
    }

    // Only allow reset from awaiting_approval, approved, executed, working, paused, halted, or completed states
    let can_reset = matches!(
        version.status,
        SpecVersionStatus::AwaitingApproval
            | SpecVersionStatus::Approved
            | SpecVersionStatus::Executed
            | SpecVersionStatus::Working
            | SpecVersionStatus::Paused
            | SpecVersionStatus::Halted
            | SpecVersionStatus::Completed
    );

    if !can_reset {
        return Err(format!(
            "Cannot reset: spec version is in '{}' status.",
            version.status.as_str()
        ));
    }

    // Delete all tickets for this version
    let deleted_count = db
        .delete_spec_version_tickets(&version_id)
        .map_err(|e| e.to_string())?;

    tracing::info!(
        "Deleted {} tickets for spec version {}",
        deleted_count,
        version_id
    );

    // Reset status to approved so user can execute again
    db.set_spec_version_status(&version_id, SpecVersionStatus::Approved)
        .map_err(|e| e.to_string())?;

    // Emit update event
    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(deleted_count)
}

/// Set status on the latest spec version
#[tauri::command]
pub async fn set_spec_status(
    id: String,
    status: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let status =
        SpecVersionStatus::parse(&status).ok_or_else(|| format!("Invalid status: {}", status))?;

    // Get the latest version for this spec
    let version = db
        .get_latest_spec_version(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.set_spec_version_status(&version.id, status)
        .map_err(|e| e.to_string())
}

/// Set status on a specific spec version
#[tauri::command]
pub async fn set_spec_version_status(
    version_id: String,
    status: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let status =
        SpecVersionStatus::parse(&status).ok_or_else(|| format!("Invalid status: {}", status))?;

    db.set_spec_version_status(&version_id, status)
        .map_err(|e| e.to_string())
}

/// Append exploration to the latest spec version
#[tauri::command]
pub async fn append_spec_exploration(
    id: String,
    query: String,
    response: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let exploration = Exploration {
        query,
        response,
        timestamp: chrono::Utc::now(),
    };

    // Get the latest version for this spec
    let version = db
        .get_latest_spec_version(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.append_spec_version_exploration(&version.id, &exploration)
        .map_err(|e| e.to_string())
}

/// Append exploration to a specific spec version
#[tauri::command]
pub async fn append_spec_version_exploration(
    version_id: String,
    query: String,
    response: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let exploration = Exploration {
        query,
        response,
        timestamp: chrono::Utc::now(),
    };

    db.append_spec_version_exploration(&version_id, &exploration)
        .map_err(|e| e.to_string())
}

/// Set plan on the latest spec version
#[tauri::command]
pub async fn set_spec_plan(
    id: String,
    markdown: String,
    json: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    // Get the latest version for this spec
    let version = db
        .get_latest_spec_version(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.set_spec_version_plan(&version.id, &markdown, json.as_ref())
        .map_err(|e| e.to_string())
}

/// Set plan on a specific spec version
#[tauri::command]
pub async fn set_spec_version_plan(
    version_id: String,
    markdown: String,
    json: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_spec_version_plan(&version_id, &markdown, json.as_ref())
        .map_err(|e| e.to_string())
}

/// Approve plan for the latest spec version
#[tauri::command]
pub async fn approve_plan(id: String, db: State<'_, Arc<Database>>) -> Result<(), String> {
    tracing::info!("Approving plan for spec {}", id);

    // Get the latest version
    let version = db
        .get_latest_spec_version(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    if version.status != SpecVersionStatus::AwaitingApproval {
        return Err(format!(
            "Cannot approve plan: spec version is in '{}' status, expected 'awaiting_approval'",
            version.status.as_str()
        ));
    }

    db.set_spec_version_status(&version.id, SpecVersionStatus::Approved)
        .map_err(|e| e.to_string())
}

/// Get tickets for the latest spec version
#[tauri::command]
pub async fn get_spec_tickets(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::Ticket>, String> {
    // Get the latest version
    let version = db
        .get_latest_spec_version(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.get_spec_version_tickets(&version.id)
        .map_err(|e| e.to_string())
}

/// Get tickets for a specific spec version
#[tauri::command]
pub async fn get_spec_version_tickets(
    version_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::Ticket>, String> {
    db.get_spec_version_tickets(&version_id)
        .map_err(|e| e.to_string())
}

/// Get all versions for a spec
#[tauri::command]
pub async fn get_spec_versions(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<SpecVersion>, String> {
    db.get_spec_versions(&spec_id).map_err(|e| e.to_string())
}

/// Get the latest version for a spec
#[tauri::command]
pub async fn get_latest_spec_version(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Option<SpecVersion>, String> {
    db.get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())
}

/// Get a spec with its latest version
#[tauri::command]
pub async fn get_spec_with_version(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<SpecWithVersion, String> {
    db.get_spec_with_version(&id).map_err(|e| e.to_string())
}

/// Get specs with their latest versions for a board
#[tauri::command]
pub async fn get_specs_with_versions(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<SpecWithVersion>, String> {
    db.get_specs_with_versions(&board_id)
        .map_err(|e| e.to_string())
}

/// Get all specs with their latest versions
#[tauri::command]
pub async fn get_all_specs_with_versions(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<SpecWithVersion>, String> {
    db.get_all_specs_with_versions()
        .map_err(|e| e.to_string())
}

/// Create a new version for a spec (for iterating after previous version)
#[tauri::command]
pub async fn create_new_spec_version(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<SpecVersion, String> {
    tracing::info!("Creating new version for spec {}", spec_id);

    let version = db
        .create_new_spec_version(&spec_id)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(version)
}

/// Input for starting the planner
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPlannerInput {
    pub spec_id: String,
    pub agent_kind: Option<String>,
    pub max_explorations: Option<usize>,
    pub auto_approve: Option<bool>,
    pub model: Option<String>,
    pub timeout_minutes: Option<u32>,
    pub max_retries: Option<u32>,
}

/// Start the planner agent for a spec
#[tauri::command]
pub async fn start_planner(
    input: StartPlannerInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<String, String> {
    tracing::info!("Starting planner for spec {}", input.spec_id);

    // Get spec and its associated project
    let spec = db.get_spec(&input.spec_id).map_err(|e| e.to_string())?;
    let project = db
        .get_project(&spec.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", spec.project_id))?;

    // Determine agent kind from parameter or default to Claude
    let agent_kind = match input.agent_kind.as_deref() {
        Some("cursor") => AgentKind::Cursor,
        Some("claude") => AgentKind::Claude,
        _ => AgentKind::Claude,
    };

    // Get Claude API config if using Claude agent
    let claude_api_config =
        (agent_kind == AgentKind::Claude).then(|| ClaudeApiConfig::from(claude_api_state.get()));

    let config = PlannerConfig {
        spec_id: input.spec_id.clone(),
        max_explorations: input.max_explorations.unwrap_or(10),
        auto_approve: input.auto_approve.unwrap_or(false),
        model: input.model.or(spec.model),
        agent_kind,
        repo_path: PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config,
        timeout_secs: input.timeout_minutes.map(|m| m as u64 * 60).unwrap_or(300),
        max_retries: input.max_retries.unwrap_or(2),
    };

    let agent = PlannerAgent::with_events(db.inner().clone(), config, event_tx.inner().clone());

    let result = agent.run().await.map_err(|e| e.to_string())?;

    Ok(format!(
        "Planner completed with status: {:?}, epics: {}, tickets: {}",
        result.status,
        result.epic_ids.len(),
        result.ticket_ids.len()
    ))
}

/// Execute an approved plan
#[tauri::command]
pub async fn execute_plan(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_conn: State<'_, ApiConnState>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<Vec<String>, String> {
    tracing::info!("Executing plan for spec {}", spec_id);

    // Get spec and its associated project
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let project = db
        .get_project(&spec.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", spec.project_id))?;

    // Get Claude API config (execute_plan doesn't run agents but we include for consistency)
    let claude_api_config = Some(ClaudeApiConfig::from(claude_api_state.get()));

    let config = PlannerConfig {
        spec_id: spec_id.clone(),
        max_explorations: 0, // Not used for execution
        auto_approve: false,
        model: None,
        agent_kind: AgentKind::Claude, // Not used for execution
        repo_path: PathBuf::from(&project.path),
        api_url: api_conn.url.clone(),
        api_token: api_conn.token.clone(),
        claude_api_config,
        timeout_secs: 300, // Not used for execution
        max_retries: 0,    // Not used for execution
    };

    let agent = PlannerAgent::with_events(db.inner().clone(), config, event_tx.inner().clone());

    let result = agent.execute_plan().await.map_err(|e| e.to_string())?;

    Ok(result.epic_ids)
}

/// Start work on a spec version's epics - moves root epics (no dependencies) to Ready
#[tauri::command]
pub async fn start_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<Vec<String>, String> {
    tracing::info!("Starting work for spec {}", spec_id);

    // Get spec and its latest version
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    // Must be in Executed or Halted status (epics created but work not started/was stopped)
    // Also allow from Completed status if not all epics are actually done (handles edge case from old code)
    let can_start = version.status == SpecVersionStatus::Executed
        || version.status == SpecVersionStatus::Halted
        || (version.status == SpecVersionStatus::Completed
            && !db
                .are_all_spec_version_epics_done(&version.id)
                .unwrap_or(true));

    if !can_start {
        return Err(format!(
            "Cannot start work: spec version is in '{}' status, expected 'executed' or 'halted'",
            version.status.as_str()
        ));
    }

    // Get root epics (no dependencies) for the latest version
    let root_epics = db
        .get_spec_version_root_epics(&version.id)
        .map_err(|e| e.to_string())?;

    if root_epics.is_empty() {
        return Err("No epics found for this spec".to_string());
    }

    // Use target_board_id if set, otherwise fall back to board_id
    let target_board_id = spec.target_board_id.as_ref().unwrap_or(&spec.board_id);

    // Find the Ready column for the target board
    let ready_column = db
        .find_column_by_name(target_board_id, "Ready")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Ready column not found on target board".to_string())?;

    let mut started_epic_ids = Vec::new();

    for epic in &root_epics {
        // Move epic to Ready
        db.move_ticket(&epic.id, &ready_column.id)
            .map_err(|e| e.to_string())?;

        // Trigger on_epic_moved_to_ready to advance its first child
        let updated_epic = db.get_ticket(&epic.id).map_err(|e| e.to_string())?;
        if let Err(e) = on_epic_moved_to_ready(&db.inner().clone(), &updated_epic) {
            tracing::warn!("Failed to advance epic {} first child: {}", epic.id, e);
        }

        started_epic_ids.push(epic.id.clone());

        tracing::info!("Started epic {} for spec {}", epic.id, spec_id);
    }

    // Update spec version status to Working and set work_started_at timestamp (for ETA calculation)
    db.start_spec_version_work(&version.id)
        .map_err(|e| e.to_string())?;

    // Check if all epics are already done (edge case: all work completed before start)
    // This handles scenarios where epics were moved to Done manually or through other paths
    if let Err(e) = check_spec_completion_by_id(&db.inner().clone(), &version.id) {
        tracing::warn!("Failed to check spec completion after start: {}", e);
    }

    // Broadcast update
    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(started_epic_ids)
}

/// Get progress stats for the latest spec version's epics
#[tauri::command]
pub async fn get_spec_progress(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<SpecProgress, String> {
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.get_spec_version_progress(&version.id)
        .map_err(|e| e.to_string())
}

/// Get progress stats for a specific spec version's epics
#[tauri::command]
pub async fn get_version_progress(
    version_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<SpecProgress, String> {
    db.get_spec_version_progress(&version_id)
        .map_err(|e| e.to_string())
}

/// Pause work on a spec version - also pauses all currently running tickets
#[tauri::command]
pub async fn pause_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, crate::commands::runs::RunningAgents>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Pausing work for spec {}", spec_id);

    // Get latest version
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    // First, find all running tickets in this spec version and pause them
    let tickets = db
        .get_spec_version_tickets(&version.id)
        .map_err(|e| e.to_string())?;

    for ticket in tickets {
        if let Some(ref run_id) = ticket.locked_by_run_id {
            tracing::info!(
                "Pausing running ticket {} (run {}) as part of spec {} pause",
                ticket.id,
                run_id,
                spec_id
            );

            // Determine the current stage from the run's sub-runs
            // Use graceful degradation: if we can't get the stage, default to "plan"
            let current_stage = db
                .get_current_run_stage(run_id)
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to get current stage for run {}: {}", run_id, e);
                    None
                })
                .unwrap_or_else(|| "plan".to_string());

            // Set pause state on the ticket
            if let Err(e) = db.pause_ticket(&ticket.id, &current_stage, run_id) {
                tracing::warn!("Failed to set pause state on ticket {}: {}", ticket.id, e);
            }

            // Cancel the run with is_pause=true
            // Update run status to Paused
            if let Err(e) = db.update_run_status(
                run_id,
                crate::db::RunStatus::Paused,
                None,
                Some("Paused via spec pause"),
            ) {
                tracing::warn!("Failed to update run {} status to paused: {}", run_id, e);
            }

            // Cancel via handle if available
            {
                let handles = running_agents
                    .handles
                    .lock()
                    .expect("running agents mutex poisoned");
                if let Some(handle) = handles.get(run_id) {
                    handle.cancel();
                    tracing::info!("Cancelled handle for run {} (spec pause)", run_id);
                }
            }

            // Reset any in-progress tasks for this run
            if let Err(e) = db.reset_tasks_for_run(run_id) {
                tracing::warn!("Failed to reset tasks for run {}: {}", run_id, e);
            }
        }
    }

    // Now update the spec version status
    db.pause_spec_version_work(&version.id)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(())
}

/// Resume work on a paused spec version - also moves paused tickets to Ready for pickup
#[tauri::command]
pub async fn resume_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, crate::commands::runs::RunningAgents>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Resuming work for spec {}", spec_id);

    // Get latest version
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    // Get the spec to find its board
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let board_id = spec.target_board_id.as_ref().unwrap_or(&spec.board_id);

    // Find the Ready column for this board
    let columns = db.get_columns(board_id).map_err(|e| e.to_string())?;
    let ready_column = columns
        .iter()
        .find(|c| c.name == "Ready")
        .ok_or_else(|| "Ready column not found".to_string())?;

    // Get all tickets in the spec version that have pause state (paused_run_id set means they were paused mid-run)
    let tickets = db
        .get_spec_version_tickets(&version.id)
        .map_err(|e| e.to_string())?;

    for ticket in tickets {
        if ticket.paused_run_id.is_some() {
            tracing::info!(
                "Moving paused ticket {} to Ready for resume (run: {:?}, stage: {:?})",
                ticket.id,
                ticket.paused_run_id,
                ticket.paused_at_stage
            );

            // Remove old cancelled handle so the run can be resumed
            if let Some(ref run_id) = ticket.paused_run_id {
                let mut handles = running_agents
                    .handles
                    .lock()
                    .expect("running agents mutex poisoned");
                if handles.remove(run_id).is_some() {
                    tracing::info!(
                        "Removed old cancelled handle for run {} to allow resume",
                        run_id
                    );
                }
            }

            // Move ticket to Ready column so workers can pick it up
            if let Err(e) = db.move_ticket(&ticket.id, &ready_column.id) {
                tracing::warn!("Failed to move ticket {} to Ready: {}", ticket.id, e);
            }

            // Unlock the ticket so workers can pick it up
            if let Err(e) = db.unlock_ticket(&ticket.id) {
                tracing::warn!("Failed to unlock ticket {}: {}", ticket.id, e);
            }
        }
    }

    // Now update the spec version status and clear paused_at from tickets
    db.resume_spec_version_work(&version.id)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(())
}

/// Halt work on a spec version - stops all running agents, resets ticket state, and moves
/// non-Done tickets back to Backlog. This allows a clean restart via start_spec_work.
#[tauri::command]
pub async fn halt_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, crate::commands::runs::RunningAgents>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Halting work for spec {}", spec_id);

    // Get spec and latest version
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    // Use target_board_id if set, otherwise fall back to board_id
    let board_id = spec.target_board_id.as_ref().unwrap_or(&spec.board_id);

    // Find the Backlog column for this board
    let backlog_column = db
        .find_column_by_name(board_id, "Backlog")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Backlog column not found".to_string())?;

    // Get all tickets for this spec version
    let tickets = db
        .get_spec_version_tickets(&version.id)
        .map_err(|e| e.to_string())?;

    // Get columns to check which tickets are in Done
    let columns = db.get_columns(board_id).map_err(|e| e.to_string())?;
    let done_column_id = columns
        .iter()
        .find(|c| c.name == "Done")
        .map(|c| c.id.clone());

    for ticket in &tickets {
        // Cancel any running agent and reset run state
        if let Some(ref run_id) = ticket.locked_by_run_id {
            tracing::info!(
                "Aborting running ticket {} (run {}) as part of spec {} halt",
                ticket.id,
                run_id,
                spec_id
            );

            // Cancel via handle if available
            {
                let handles = running_agents
                    .handles
                    .lock()
                    .expect("running agents mutex poisoned");
                if let Some(handle) = handles.get(run_id) {
                    handle.cancel();
                    tracing::info!("Cancelled handle for run {} (spec halt)", run_id);
                }
            }

            // Update run status to Aborted
            if let Err(e) = db.update_run_status(
                run_id,
                crate::db::RunStatus::Aborted,
                None,
                Some("Aborted via spec halt"),
            ) {
                tracing::warn!("Failed to update run {} status to aborted: {}", run_id, e);
            }

            // Reset any in-progress tasks for this run
            if let Err(e) = db.reset_tasks_for_run(run_id) {
                tracing::warn!("Failed to reset tasks for run {}: {}", run_id, e);
            }

            // Unlock the ticket
            if let Err(e) = db.unlock_ticket(&ticket.id) {
                tracing::warn!("Failed to unlock ticket {}: {}", ticket.id, e);
            }
        }

        // Move non-Done tickets back to Backlog
        let is_done = done_column_id
            .as_ref()
            .map(|done_id| ticket.column_id == *done_id)
            .unwrap_or(false);

        if !is_done {
            if let Err(e) = db.move_ticket(&ticket.id, &backlog_column.id) {
                tracing::warn!("Failed to move ticket {} to Backlog: {}", ticket.id, e);
            } else {
                tracing::info!(
                    "Moved ticket {} to Backlog as part of spec {} halt",
                    ticket.id,
                    spec_id
                );
            }
        }
    }

    // Now update the spec version status and clear pause state
    db.halt_spec_version_work(&version.id)
        .map_err(|e| e.to_string())?;

    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });

    Ok(())
}

/// Get ETA information for a spec
#[tauri::command]
pub async fn get_spec_eta(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<crate::db::SpecEta, String> {
    crate::agents::eta::calculate_eta(&db.inner().clone(), &spec_id)
}

/// Get aggregated cost for a spec's latest version.
#[tauri::command]
pub async fn get_spec_cost(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<crate::agents::AggregatedCost, String> {
    let version = db
        .get_latest_spec_version(&spec_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No version found for spec".to_string())?;

    db.get_spec_version_cost(&version.id)
        .map_err(|e| e.to_string())
}

/// Get aggregated cost for a specific spec version.
#[tauri::command]
pub async fn get_spec_version_cost(
    version_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<crate::agents::AggregatedCost, String> {
    db.get_spec_version_cost(&version_id)
        .map_err(|e| e.to_string())
}
