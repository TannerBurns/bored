use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthorType {
    User,
    Agent,
    System,
}

impl AuthorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthorType::User => "user",
            AuthorType::Agent => "agent",
            AuthorType::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub ticket_id: String,
    pub author_type: AuthorType,
    pub body_md: String,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateComment {
    pub ticket_id: String,
    pub author_type: AuthorType,
    pub body_md: String,
    pub metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_type_as_str() {
        assert_eq!(AuthorType::User.as_str(), "user");
        assert_eq!(AuthorType::Agent.as_str(), "agent");
        assert_eq!(AuthorType::System.as_str(), "system");
    }

    #[test]
    fn author_type_serializes_to_lowercase() {
        assert_eq!(serde_json::to_string(&AuthorType::User).unwrap(), "\"user\"");
        assert_eq!(serde_json::to_string(&AuthorType::Agent).unwrap(), "\"agent\"");
        assert_eq!(serde_json::to_string(&AuthorType::System).unwrap(), "\"system\"");
    }

    #[test]
    fn author_type_deserializes_from_lowercase() {
        assert_eq!(serde_json::from_str::<AuthorType>("\"user\"").unwrap(), AuthorType::User);
        assert_eq!(serde_json::from_str::<AuthorType>("\"agent\"").unwrap(), AuthorType::Agent);
        assert_eq!(serde_json::from_str::<AuthorType>("\"system\"").unwrap(), AuthorType::System);
    }
}
