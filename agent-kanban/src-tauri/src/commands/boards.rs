use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Board {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: now,
            updated_at: now,
        }
    }
}

#[tauri::command]
pub async fn get_boards() -> Result<Vec<Board>, String> {
    tracing::info!("Getting all boards");
    Ok(vec![])
}

#[tauri::command]
pub async fn create_board(name: String) -> Result<Board, String> {
    tracing::info!("Creating board: {}", name);
    Ok(Board::new(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_new_sets_name() {
        let board = Board::new("Test Board".to_string());
        assert_eq!(board.name, "Test Board");
    }

    #[test]
    fn board_new_generates_uuid() {
        let board = Board::new("Test".to_string());
        assert!(!board.id.is_empty());
        assert!(uuid::Uuid::parse_str(&board.id).is_ok());
    }

    #[test]
    fn board_new_sets_timestamps() {
        let before = Utc::now();
        let board = Board::new("Test".to_string());
        let after = Utc::now();
        
        assert!(board.created_at >= before && board.created_at <= after);
        assert_eq!(board.created_at, board.updated_at);
    }

    #[test]
    fn board_serializes_to_json() {
        let board = Board::new("Test".to_string());
        let json = serde_json::to_string(&board).unwrap();
        assert!(json.contains("\"name\":\"Test\""));
    }

    #[test]
    fn board_deserializes_from_json() {
        let json = r#"{"id":"123","name":"From JSON","created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z"}"#;
        let board: Board = serde_json::from_str(json).unwrap();
        assert_eq!(board.id, "123");
        assert_eq!(board.name, "From JSON");
    }
}
