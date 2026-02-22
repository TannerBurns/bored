//! Default model constants used as fallbacks when no model is configured.

pub const DEFAULT_STAGE_MODEL: &str = "opus-4.6";
pub const DEFAULT_DIAGNOSTIC_MODEL: &str = "sonnet-4.6";

/// Single source of truth for model-name mappings.
/// Each entry: (friendly_name, cli_family_prefix, version).
pub const MODEL_ENTRIES: &[(&str, &str, &str)] = &[
    ("opus-4.6", "claude-opus", "4.6"),
    ("opus-4.5", "claude-opus", "4.5"),
    ("sonnet-4.6", "claude-sonnet", "4.6"),
    ("sonnet-4.5", "claude-sonnet", "4.5"),
];

/// Map a friendly model name to its Claude Code CLI identifier.
/// Claude Code uses dashes in versions: "opus-4.6" -> "claude-opus-4-6".
pub fn map_model_name(model: &str) -> String {
    for &(friendly, family, version) in MODEL_ENTRIES {
        if model == friendly {
            return format!("{}-{}", family, version.replace('.', "-"));
        }
    }
    model.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_opus_4_6() {
        assert_eq!(map_model_name("opus-4.6"), "claude-opus-4-6");
    }

    #[test]
    fn maps_opus_4_5() {
        assert_eq!(map_model_name("opus-4.5"), "claude-opus-4-5");
    }

    #[test]
    fn maps_sonnet_4_6() {
        assert_eq!(map_model_name("sonnet-4.6"), "claude-sonnet-4-6");
    }

    #[test]
    fn maps_sonnet_4_5() {
        assert_eq!(map_model_name("sonnet-4.5"), "claude-sonnet-4-5");
    }

    #[test]
    fn passes_through_unknown_model() {
        assert_eq!(map_model_name("gpt-4o"), "gpt-4o");
    }

    #[test]
    fn passes_through_already_mapped_name() {
        assert_eq!(map_model_name("claude-opus-4-6"), "claude-opus-4-6");
    }

    #[test]
    fn passes_through_empty_string() {
        assert_eq!(map_model_name(""), "");
    }
}
