//! Tauri commands for scratchpad (planner) operations

use std::sync::Arc;
use std::path::PathBuf;
use serde::Deserialize;
use tauri::State;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{Database, Scratchpad, CreateScratchpad, UpdateScratchpad, ScratchpadStatus, Exploration, ScratchpadProgress};
use crate::agents::planner::{PlannerAgent, PlannerConfig};
use crate::agents::AgentKind;
use crate::lifecycle::epic::on_epic_moved_to_ready;

/// Input for creating a scratchpad
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScratchpadInput {
    pub board_id: String,
    pub target_board_id: Option<String>,
    pub project_id: String,
    pub name: String,
    pub user_input: String,
    pub agent_pref: Option<String>,
    pub model: Option<String>,
}

#[tauri::command]
pub async fn create_scratchpad(
    input: CreateScratchpadInput,
    db: State<'_, Arc<Database>>,
) -> Result<Scratchpad, String> {
    tracing::info!("Creating scratchpad '{}' for board {} (target: {:?}) project {}", 
        input.name, input.board_id, input.target_board_id, input.project_id);
    
    db.create_scratchpad(&CreateScratchpad {
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
pub async fn get_scratchpads(
    board_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Scratchpad>, String> {
    db.get_scratchpads(&board_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_scratchpads(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<Scratchpad>, String> {
    db.get_all_scratchpads().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_scratchpad(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Scratchpad, String> {
    db.get_scratchpad(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_scratchpad(
    id: String,
    name: Option<String>,
    user_input: Option<String>,
    agent_pref: Option<String>,
    model: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Scratchpad, String> {
    tracing::info!("Updating scratchpad {}", id);
    
    db.update_scratchpad(&id, &UpdateScratchpad {
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
pub async fn delete_scratchpad(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Deleting scratchpad {}", id);
    db.delete_scratchpad(&id).map_err(|e| e.to_string())
}

/// Delete a scratchpad and all its associated tickets (epics, child tickets, and their data)
#[tauri::command]
pub async fn delete_scratchpad_with_tickets(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<usize, String> {
    tracing::info!("Deleting scratchpad {} with all tickets", id);
    let count = db.delete_scratchpad_with_tickets(&id).map_err(|e| e.to_string())?;
    tracing::info!("Deleted scratchpad {} and {} tickets", id, count);
    Ok(count)
}

#[tauri::command]
pub async fn set_scratchpad_status(
    id: String,
    status: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    let status = ScratchpadStatus::parse(&status)
        .ok_or_else(|| format!("Invalid status: {}", status))?;
    
    db.set_scratchpad_status(&id, status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn append_exploration(
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
    
    db.append_exploration(&id, &exploration).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_scratchpad_plan(
    id: String,
    markdown: String,
    json: Option<serde_json::Value>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_scratchpad_plan(&id, &markdown, json.as_ref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn approve_plan(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    tracing::info!("Approving plan for scratchpad {}", id);
    
    // Check that scratchpad exists and is in awaiting_approval status
    let scratchpad = db.get_scratchpad(&id).map_err(|e| e.to_string())?;
    
    if scratchpad.status != ScratchpadStatus::AwaitingApproval {
        return Err(format!(
            "Cannot approve plan: scratchpad is in '{}' status, expected 'awaiting_approval'",
            scratchpad.status.as_str()
        ));
    }
    
    db.set_scratchpad_status(&id, ScratchpadStatus::Approved).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_scratchpad_tickets(
    id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::Ticket>, String> {
    db.get_scratchpad_tickets(&id).map_err(|e| e.to_string())
}

/// Input for starting the planner
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPlannerInput {
    pub scratchpad_id: String,
    pub agent_kind: Option<String>,
    pub max_explorations: Option<usize>,
    pub auto_approve: Option<bool>,
    pub model: Option<String>,
}

/// Start the planner agent for a scratchpad
#[tauri::command]
pub async fn start_planner(
    input: StartPlannerInput,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_url: State<'_, String>,
    api_token: State<'_, String>,
) -> Result<String, String> {
    tracing::info!("Starting planner for scratchpad {}", input.scratchpad_id);
    
    // Get scratchpad and its associated project
    let scratchpad = db.get_scratchpad(&input.scratchpad_id).map_err(|e| e.to_string())?;
    let project = db.get_project(&scratchpad.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", scratchpad.project_id))?;
    
    // Determine agent kind from parameter, scratchpad preference, or default
    let agent_kind = match input.agent_kind.as_deref() {
        Some("cursor") => AgentKind::Cursor,
        Some("claude") => AgentKind::Claude,
        _ => {
            // Use scratchpad's agent_pref or default to Claude
            match scratchpad.agent_pref.as_deref() {
                Some("cursor") => AgentKind::Cursor,
                Some("claude") => AgentKind::Claude,
                _ => AgentKind::Claude,
            }
        }
    };
    
    let config = PlannerConfig {
        scratchpad_id: input.scratchpad_id.clone(),
        max_explorations: input.max_explorations.unwrap_or(10),
        auto_approve: input.auto_approve.unwrap_or(false),
        model: input.model.or(scratchpad.model),
        agent_kind,
        repo_path: PathBuf::from(&project.path),
        api_url: api_url.inner().clone(),
        api_token: api_token.inner().clone(),
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
    scratchpad_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    api_url: State<'_, String>,
    api_token: State<'_, String>,
) -> Result<Vec<String>, String> {
    tracing::info!("Executing plan for scratchpad {}", scratchpad_id);
    
    // Get scratchpad and its associated project
    let scratchpad = db.get_scratchpad(&scratchpad_id).map_err(|e| e.to_string())?;
    let project = db.get_project(&scratchpad.project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project '{}' not found", scratchpad.project_id))?;
    
    let config = PlannerConfig {
        scratchpad_id: scratchpad_id.clone(),
        max_explorations: 0, // Not used for execution
        auto_approve: false,
        model: None,
        agent_kind: AgentKind::Claude, // Not used for execution
        repo_path: PathBuf::from(&project.path),
        api_url: api_url.inner().clone(),
        api_token: api_token.inner().clone(),
    };
    
    let agent = PlannerAgent::with_events(
        db.inner().clone(),
        config,
        event_tx.inner().clone(),
    );
    
    let result = agent.execute_plan().await.map_err(|e| e.to_string())?;
    
    Ok(result.epic_ids)
}

/// Start work on a scratchpad's epics - moves root epics (no dependencies) to Ready
#[tauri::command]
pub async fn start_scratchpad_work(
    scratchpad_id: String,
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
) -> Result<Vec<String>, String> {
    tracing::info!("Starting work for scratchpad {}", scratchpad_id);
    
    // Get scratchpad and validate state
    let scratchpad = db.get_scratchpad(&scratchpad_id).map_err(|e| e.to_string())?;
    
    // Must be in Executed status (epics created but work not started)
    // Also allow from Completed status if not all epics are actually done (handles edge case from old code)
    let can_start = scratchpad.status == ScratchpadStatus::Executed 
        || (scratchpad.status == ScratchpadStatus::Completed 
            && !db.are_all_scratchpad_epics_done(&scratchpad_id).unwrap_or(true));
    
    if !can_start {
        return Err(format!(
            "Cannot start work: scratchpad is in '{}' status, expected 'executed'",
            scratchpad.status.as_str()
        ));
    }
    
    // Get root epics (no dependencies)
    let root_epics = db.get_scratchpad_root_epics(&scratchpad_id)
        .map_err(|e| e.to_string())?;
    
    if root_epics.is_empty() {
        return Err("No epics found for this scratchpad".to_string());
    }
    
    // Use target_board_id if set, otherwise fall back to board_id
    let target_board_id = scratchpad.target_board_id.as_ref()
        .unwrap_or(&scratchpad.board_id);
    
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
        
        tracing::info!("Started epic {} for scratchpad {}", epic.id, scratchpad_id);
    }
    
    // Update scratchpad status to Working
    db.set_scratchpad_status(&scratchpad_id, ScratchpadStatus::Working)
        .map_err(|e| e.to_string())?;
    
    // Broadcast update
    let _ = event_tx.send(LiveEvent::ScratchpadUpdated {
        scratchpad_id: scratchpad_id.clone(),
    });
    
    Ok(started_epic_ids)
}

/// Get progress stats for a scratchpad's epics
#[tauri::command]
pub async fn get_scratchpad_progress(
    scratchpad_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<ScratchpadProgress, String> {
    db.get_scratchpad_progress(&scratchpad_id).map_err(|e| e.to_string())
}
