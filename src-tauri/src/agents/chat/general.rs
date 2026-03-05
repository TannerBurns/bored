use crate::db::models::{ChatMessage, ChatMessageRole};

use super::config::ChatAgentError;
use super::ChatAgent;

pub fn build_general_prompt(messages: &[ChatMessage]) -> String {
    let mut prompt = String::from("# Chat Conversation\n\nContinue this conversation. Respond naturally and helpfully.\n\n## Conversation History\n");

    for msg in messages {
        let role_label = match msg.role {
            ChatMessageRole::User => "User",
            ChatMessageRole::Assistant => "Assistant",
            ChatMessageRole::System => "System",
        };
        prompt.push_str(&format!("\n{}: {}\n", role_label, msg.content));
    }

    prompt.push_str("\n## Your Task\n\nRespond to the user's latest message. Be helpful, concise, and accurate.\n");
    prompt
}

impl ChatAgent {
    pub(super) async fn run_general(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatMessage, ChatAgentError> {
        let prompt = build_general_prompt(&messages);
        let (response, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let message = self.save_assistant_message(&response, None).await?;
        self.persist_log_events(&ts_lines, &message.id);
        self.extract_and_store_cost(&stdout, Some(&message.id)).await?;

        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_general_prompt_empty_messages() {
        let prompt = build_general_prompt(&[]);
        assert!(prompt.contains("Chat Conversation"));
        assert!(prompt.contains("Your Task"));
    }

    #[test]
    fn build_general_prompt_includes_messages() {
        let messages = vec![
            ChatMessage {
                id: "1".to_string(),
                chat_id: "c1".to_string(),
                role: ChatMessageRole::User,
                content: "Hello".to_string(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "2".to_string(),
                chat_id: "c1".to_string(),
                role: ChatMessageRole::Assistant,
                content: "Hi there".to_string(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
        ];

        let prompt = build_general_prompt(&messages);
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.contains("Assistant: Hi there"));
    }

    #[test]
    fn build_general_prompt_includes_system_role() {
        let messages = vec![ChatMessage {
            id: "1".to_string(),
            chat_id: "c1".to_string(),
            role: ChatMessageRole::System,
            content: "You are helpful".to_string(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }];

        let prompt = build_general_prompt(&messages);
        assert!(prompt.contains("System: You are helpful"));
    }
}
