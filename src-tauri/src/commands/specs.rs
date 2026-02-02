//! Tauri commands for spec (planning) operations

use std::sync::Arc;
use std::path::PathBuf;
use serde::Deserialize;
use tauri::State;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{Database, Spec, CreateSpec, UpdateSpec, SpecStatus, Exploration, SpecProgress};
use crate::agents::planner::{PlannerAgent, PlannerConfig};
use crate::agents::{AgentKind, ClaudeApiConfig};
use crate::commands::claude::ClaudeApiSettingsState;
use crate::lifecycle::epic::{on_epic_moved_to_ready, check_spec_completion_by_id};

/// Input for creating a spec
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpecInput {
    pub board_id: String,
    pub target_board_id: Option<String>,
    pub project_id: String,
    pub name: String,
    pub user_input: String,
    pub agent_pref: Option<String>,
    pub model: Option<String>,
}

#[tauri::command]
pub async fn create_spec(
    input: CreateSpecInput,
    db: State<'_, Arc<Database>>,
) -> Result<Spec, String> {
    tracing::info!("Creating spec '{}' for board {} (target: {:?}) project {}", 
        input.name, input.board_id, input.target_board_id, input.project_id);
    
    db.create_spec(&CreateSpec {
        board_id: input.board_id,
        target_board_id: input.target_board_id,
        project_id: input.project_id,
        name: input.name,
        user_input: input.user_input,
        agent_pref: input.agent_pref,
        model: input.model,
        settings: serde_json::json!({}),
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_specs(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Spec>, String> {
    db.get_specs(&board_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_specs(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Spec>, String> {
    db.get_all_specs().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_spec(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Spec, String> {
    db.get_spec(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_spec(
    id: String,
    name: Option<String>,
    user_input: Option<String>,
    agent_pref: Option<String>,
    model: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Spec, String> {
    tracing::info!("Updating spec {}", id);
    
    db.update_spec(&id, &UpdateSpec {
        name,
        user_input,
        status: None,
        agent_pref,
        model,
        exploration_log: None,
        plan_markdown: None,
        plan_json: None,
        settings: None,
    }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_spec(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
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
    let count = db.delete_spec_with_tickets(&id).map_err(|e| e.to_string())?;
    tracing::info!("Deleted spec {} and {} tickets", id, count);
    Ok(count)
}

#[tauri::command]
pub async fn set_spec_status(
    id: String,
    status: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let status = SpecStatus::parse(&status)
        .ok_or_else(|| format!("Invalid status: {}", status))?;
    
    db.set_spec_status(&id, status).map_err(|e| e.to_string())
}

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
    
    db.append_spec_exploration(&id, &exploration).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_spec_plan(
    id: String,
    markdown: String,
    json: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_spec_plan(&id, &markdown, json.as_ref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn approve_plan(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Approving plan for spec {}", id);
    
    // Check that spec exists and is in awaiting_approval status
    let spec = db.get_spec(&id).map_err(|e| e.to_string())?;
    
    if spec.status != SpecStatus::AwaitingApproval {
        return Err(format!(
            "Cannot approve plan: spec is in '{}' status, expected 'awaiting_approval'",
            spec.status.as_str()
        ));
    }
    
    db.set_spec_status(&id, SpecStatus::Approved).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_spec_tickets(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::Ticket>, String> {
    db.get_spec_tickets(&id).map_err(|e| e.to_string())
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
    api_url: State<'_, String>,
    api_token: State<'_, String>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<String, String> {
    tracing::info!("Starting planner for spec {}", input.spec_id);
    
    // Get spec and its associated project
    let spec = db.get_spec(&input.spec_id).map_err(|e| e.to_string())?;
    let project = db.get_project(&spec.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", spec.project_id))?;
    
    // Determine agent kind from parameter, spec preference, or default
    let agent_kind = match input.agent_kind.as_deref() {
        Some("cursor") => AgentKind::Cursor,
        Some("claude") => AgentKind::Claude,
        _ => {
            // Use spec's agent_pref or default to Claude
            match spec.agent_pref.as_deref() {
                Some("cursor") => AgentKind::Cursor,
                Some("claude") => AgentKind::Claude,
                _ => AgentKind::Claude,
            }
        }
    };
    
    // Get Claude API config if using Claude agent
    let claude_api_config = (agent_kind == AgentKind::Claude)
        .then(|| ClaudeApiConfig::from(claude_api_state.get()));
    
    let config = PlannerConfig {
        spec_id: input.spec_id.clone(),
        max_explorations: input.max_explorations.unwrap_or(10),
        auto_approve: input.auto_approve.unwrap_or(false),
        model: input.model.or(spec.model),
        agent_kind,
        repo_path: PathBuf::from(&project.path),
        api_url: api_url.inner().clone(),
        api_token: api_token.inner().clone(),
        claude_api_config,
        timeout_secs: input.timeout_minutes.map(|m| m as u64 * 60).unwrap_or(300),
        max_retries: input.max_retries.unwrap_or(2),
    };
    
    let agent = PlannerAgent::with_events(
        db.inner().clone(),
        config,
        event_tx.inner().clone(),
    );
    
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
    api_url: State<'_, String>,
    api_token: State<'_, String>,
    claude_api_state: State<'_, ClaudeApiSettingsState>,
) -> Result<Vec<String>, String> {
    tracing::info!("Executing plan for spec {}", spec_id);
    
    // Get spec and its associated project
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let project = db.get_project(&spec.project_id)
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
        api_url: api_url.inner().clone(),
        api_token: api_token.inner().clone(),
        claude_api_config,
        timeout_secs: 300, // Not used for execution
        max_retries: 0,    // Not used for execution
    };
    
    let agent = PlannerAgent::with_events(
        db.inner().clone(),
        config,
        event_tx.inner().clone(),
    );
    
    let result = agent.execute_plan().await.map_err(|e| e.to_string())?;
    
    Ok(result.epic_ids)
}

/// Start work on a spec's epics - moves root epics (no dependencies) to Ready
#[tauri::command]
pub async fn start_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<Vec<String>, String> {
    tracing::info!("Starting work for spec {}", spec_id);
    
    // Get spec and validate state
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    
    // Must be in Executed or Halted status (epics created but work not started/was stopped)
    // Also allow from Completed status if not all epics are actually done (handles edge case from old code)
    let can_start = spec.status == SpecStatus::Executed 
        || spec.status == SpecStatus::Halted
        || (spec.status == SpecStatus::Completed 
            && !db.are_all_spec_epics_done(&spec_id).unwrap_or(true));
    
    if !can_start {
        return Err(format!(
            "Cannot start work: spec is in '{}' status, expected 'executed' or 'halted'",
            spec.status.as_str()
        ));
    }
    
    // Get root epics (no dependencies)
    let root_epics = db.get_spec_root_epics(&spec_id)
        .map_err(|e| e.to_string())?;
    
    if root_epics.is_empty() {
        return Err("No epics found for this spec".to_string());
    }
    
    // Use target_board_id if set, otherwise fall back to board_id
    let target_board_id = spec.target_board_id.as_ref()
        .unwrap_or(&spec.board_id);
    
    // Find the Ready column for the target board
    let ready_column = db.find_column_by_name(target_board_id, "Ready")
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
    
    // Update spec status to Working and set work_started_at timestamp (for ETA calculation)
    db.start_spec_work(&spec_id)
        .map_err(|e| e.to_string())?;
    
    // Check if all epics are already done (edge case: all work completed before start)
    // This handles scenarios where epics were moved to Done manually or through other paths
    if let Err(e) = check_spec_completion_by_id(&db.inner().clone(), &spec_id) {
        tracing::warn!("Failed to check spec completion after start: {}", e);
    }
    
    // Broadcast update
    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });
    
    Ok(started_epic_ids)
}

/// Get progress stats for a spec's epics
#[tauri::command]
pub async fn get_spec_progress(
    spec_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<SpecProgress, String> {
    db.get_spec_progress(&spec_id).map_err(|e| e.to_string())
}

/// Pause work on a spec - also pauses all currently running tickets
#[tauri::command]
pub async fn pause_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, crate::commands::runs::RunningAgents>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Pausing work for spec {}", spec_id);
    
    // First, find all running tickets in this spec and pause them
    let tickets = db.get_spec_tickets(&spec_id).map_err(|e| e.to_string())?;
    
    for ticket in tickets {
        if let Some(ref run_id) = ticket.locked_by_run_id {
            tracing::info!(
                "Pausing running ticket {} (run {}) as part of spec {} pause",
                ticket.id, run_id, spec_id
            );
            
            // Determine the current stage from the run's sub-runs
            // Use graceful degradation: if we can't get the stage, default to "plan"
            let current_stage = db.get_current_run_stage(run_id)
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
            if let Err(e) = db.update_run_status(run_id, crate::db::RunStatus::Paused, None, Some("Paused via spec pause")) {
                tracing::warn!("Failed to update run {} status to paused: {}", run_id, e);
            }
            
            // Cancel via handle if available
            {
                let handles = running_agents.handles.lock().expect("running agents mutex poisoned");
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
    
    // Now update the spec status
    db.pause_spec_work(&spec_id).map_err(|e| e.to_string())?;
    
    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });
    
    Ok(())
}

/// Resume work on a paused spec - also moves paused tickets to Ready for pickup
#[tauri::command]
pub async fn resume_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    running_agents: State<'_, crate::commands::runs::RunningAgents>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Resuming work for spec {}", spec_id);
    
    // Get the spec to find its board
    let spec = db.get_spec(&spec_id).map_err(|e| e.to_string())?;
    let board_id = spec.target_board_id.as_ref().unwrap_or(&spec.board_id);
    
    // Find the Ready column for this board
    let columns = db.get_columns(board_id).map_err(|e| e.to_string())?;
    let ready_column = columns.iter()
        .find(|c| c.name == "Ready")
        .ok_or_else(|| "Ready column not found".to_string())?;
    
    // Get all tickets in the spec that have pause state (paused_run_id set means they were paused mid-run)
    let tickets = db.get_spec_tickets(&spec_id).map_err(|e| e.to_string())?;
    
    for ticket in tickets {
        if ticket.paused_run_id.is_some() {
            tracing::info!(
                "Moving paused ticket {} to Ready for resume (run: {:?}, stage: {:?})",
                ticket.id, ticket.paused_run_id, ticket.paused_at_stage
            );
            
            // Remove old cancelled handle so the run can be resumed
            if let Some(ref run_id) = ticket.paused_run_id {
                let mut handles = running_agents.handles.lock().expect("running agents mutex poisoned");
                if handles.remove(run_id).is_some() {
                    tracing::info!("Removed old cancelled handle for run {} to allow resume", run_id);
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
    
    // Now update the spec status and clear paused_at from tickets
    db.resume_spec_work(&spec_id).map_err(|e| e.to_string())?;
    
    let _ = event_tx.send(LiveEvent::SpecUpdated {
        spec_id: spec_id.clone(),
    });
    
    Ok(())
}

/// Halt work on a spec - stops and resets to Halted status
#[tauri::command]
pub async fn halt_spec_work(
    spec_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<(), String> {
    tracing::info!("Halting work for spec {}", spec_id);
    
    db.halt_spec_work(&spec_id).map_err(|e| e.to_string())?;
    
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
