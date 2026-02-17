//! Shared model name mapping and defaults.
//!
//! Both Claude and Cursor need to map normalized model names (e.g. "opus-4.6")
//! to the format their CLI expects (e.g. "claude-opus-4-6"). This module
//! provides that mapping in one place.

/// Map a normalized model name to the CLI format used by agents.
pub fn map_model_name(model: &str) -> String {
    match model {
        "opus-4.6" => "claude-opus-4-6".to_string(),
        "opus-4.5" => "claude-opus-4-5".to_string(),
        "sonnet-4.5" => "claude-sonnet-4-5".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_models() {
        assert_eq!(map_model_name("opus-4.6"), "claude-opus-4-6");
        assert_eq!(map_model_name("opus-4.5"), "claude-opus-4-5");
        assert_eq!(map_model_name("sonnet-4.5"), "claude-sonnet-4-5");
    }

    #[test]
    fn passes_through_unknown_models() {
        assert_eq!(map_model_name("custom-model"), "custom-model");
        assert_eq!(map_model_name("claude-opus-4-6"), "claude-opus-4-6");
    }
}
