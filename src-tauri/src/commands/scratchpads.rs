//! Tauri commands for scratchpad (planner) operations

use std::sync::Arc;
use tauri::State;
use crate::db::{Database, Scratchpad, CreateScratchpad, UpdateScratchpad, ScratchpadStatus, Exploration};
use crate::agents::planner::{PlannerAgent, PlannerConfig};

#[tauri::command]
pub async fn create_scratchpad(
    board_id: String,
    project_id: String,
    name: String,
    user_input: String,
    agent_pref: Option<String>,
    model: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<Scratchpad, String> {
    tracing::info!("Creating scratchpad '{}' for board {} project {}", name, board_id, project_id);
    
    db.create_scratchpad(&CreateScratchpad {
        board_id,
        project_id,
        name,
        user_input,
        agent_pref,
        model,
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

/// Start the planner agent for a scratchpad
#[tauri::command]
pub async fn start_planner(
    scratchpad_id: String,
    max_explorations: Option<usize>,
    auto_approve: Option<bool>,
    model: Option<String>,
    db: State<'_, Arc<Database>>,
) -> Result<String, String> {
    tracing::info!("Starting planner for scratchpad {}", scratchpad_id);
    
    let config = PlannerConfig {
        scratchpad_id: scratchpad_id.clone(),
        max_explorations: max_explorations.unwrap_or(10),
        auto_approve: auto_approve.unwrap_or(false),
        model,
    };
    
    let agent = PlannerAgent::new(db.inner().clone(), config);
    
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
) -> Result<Vec<String>, String> {
    tracing::info!("Executing plan for scratchpad {}", scratchpad_id);
    
    let config = PlannerConfig {
        scratchpad_id: scratchpad_id.clone(),
        max_explorations: 0, // Not used for execution
        auto_approve: false,
        model: None,
    };
    
    let agent = PlannerAgent::new(db.inner().clone(), config);
    
    let result = agent.execute_plan().await.map_err(|e| e.to_string())?;
    
    Ok(result.epic_ids)
}
