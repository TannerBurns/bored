use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use std::sync::Arc;

use tokio::time::interval;

use crate::db::{Database, AgentEventPayload, EventType, AgentType, NormalizedEvent};

/// Get the default spool directory path
pub fn get_default_spool_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    let base_dir = dirs::home_dir()
        .map(|h| h.join("Library").join("Application Support").join("agent-kanban"))
        .unwrap_or_else(|| PathBuf::from("/tmp/agent-kanban"));

    // Use AppData\Roaming to match the JavaScript hook script
    // (dirs::data_dir() returns AppData\Local which causes a mismatch)
    #[cfg(target_os = "windows")]
    let base_dir = dirs::home_dir()
        .map(|h| h.join("AppData").join("Roaming").join("agent-kanban"))
        .unwrap_or_else(|| PathBuf::from("C:\\Temp\\agent-kanban"));

    #[cfg(target_os = "linux")]
    let base_dir = dirs::home_dir()
        .map(|h| h.join(".local").join("share").join("agent-kanban"))
        .unwrap_or_else(|| PathBuf::from("/tmp/agent-kanban"));

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let base_dir = PathBuf::from("/tmp/agent-kanban");

    base_dir.join("spool")
}

/// Process spooled events in the background
pub async fn start_spool_processor(db: Arc<Database>, spool_dir: PathBuf) {
    let mut ticker = interval(Duration::from_secs(30));
    
    tracing::info!("Starting spool processor, watching: {:?}", spool_dir);
    
    loop {
        ticker.tick().await;
        
        if let Err(e) = process_spool(&db, &spool_dir).await {
            tracing::error!("Spool processing error: {}", e);
        }
    }
}

async fn process_spool(db: &Database, spool_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !spool_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(spool_dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            match process_spool_file(db, &path) {
                Ok(()) => {
                    fs::remove_file(&path)?;
                    tracing::debug!("Processed spooled event: {:?}", path);
                }
                Err(e) => {
                    tracing::warn!("Failed to process spooled event {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(())
}

fn process_spool_file(db: &Database, path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path)?;
    let event: serde_json::Value = serde_json::from_str(&content)?;
    
    let run_id = event["runId"]
        .as_str()
        .ok_or("Missing runId")?
        .to_string();
    
    let ticket_id = event["ticketId"]
        .as_str()
        .ok_or("Missing ticketId")?
        .to_string();
    
    let agent_type = match event["agentType"].as_str() {
        Some("claude") => AgentType::Claude,
        _ => AgentType::Cursor,
    };
    
    let event_type_str = event["eventType"]
        .as_str()
        .ok_or("Missing eventType")?;
    
    let event_type = EventType::parse(event_type_str);
    
    let payload = AgentEventPayload {
        raw: event["payload"]["raw"].as_str().map(|s| s.to_string()),
        structured: event["payload"]["structured"].as_object().map(|o| {
            serde_json::Value::Object(o.clone())
        }),
    };
    
    let timestamp_str = event["timestamp"]
        .as_str()
        .ok_or("Missing timestamp")?;
    
    let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let normalized = NormalizedEvent {
        run_id,
        ticket_id,
        agent_type,
        event_type,
        payload,
        timestamp,
    };

    db.create_event(&normalized)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use crate::db::models::{CreateTicket, CreateRun, Priority};

    fn create_test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn create_spool_file(spool_dir: &std::path::Path, event_json: &str) -> PathBuf {
        let filename = format!("{}-test.json", chrono::Utc::now().timestamp_millis());
        let filepath = spool_dir.join(filename);
        let mut file = fs::File::create(&filepath).unwrap();
        file.write_all(event_json.as_bytes()).unwrap();
        filepath
    }

    #[test]
    fn test_get_default_spool_dir() {
        let spool_dir = get_default_spool_dir();
        assert!(spool_dir.to_string_lossy().contains("agent-kanban"));
        assert!(spool_dir.to_string_lossy().contains("spool"));
    }

    #[tokio::test]
    async fn test_process_empty_spool() {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join("spool");
        let db = create_test_db();
        
        // Should not error when spool dir doesn't exist
        let result = process_spool(&db, &spool_dir.to_path_buf()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_spool_file() {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join("spool");
        fs::create_dir_all(&spool_dir).unwrap();
        
        let db = create_test_db();
        
        // Create required board, ticket, and run
        let board = db.create_board("Test Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        
        let ticket = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: columns[0].id.clone(),
            title: "Test".to_string(),
            description_md: "".to_string(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: None,
            agent_pref: None,
        }).unwrap();
        
        let run = db.create_run(&CreateRun {
            ticket_id: ticket.id.clone(),
            agent_type: AgentType::Cursor,
            repo_path: "/tmp/test".to_string(),
        }).unwrap();
        
        // Create spool file
        let event_json = format!(r#"{{
            "runId": "{}",
            "ticketId": "{}",
            "agentType": "cursor",
            "eventType": "command_executed",
            "payload": {{
                "raw": "{{\"command\": \"ls\"}}",
                "structured": {{"command": "ls"}}
            }},
            "timestamp": "{}"
        }}"#, run.id, ticket.id, chrono::Utc::now().to_rfc3339());
        
        let file_path = create_spool_file(&spool_dir, &event_json);
        assert!(file_path.exists());
        
        // Process the spool
        process_spool(&db, &spool_dir).await.unwrap();
        
        // File should be deleted after processing
        assert!(!file_path.exists());
        
        // Event should be in the database
        let events = db.get_events(&run.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::CommandExecuted);
    }
}
