//! Parse the output of `cursor agent --list-models` into structured model data.

use std::process::Command;

/// A single model entry from the Cursor CLI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorModelInfo {
    pub id: String,
    pub label: String,
    pub is_default: bool,
    pub is_current: bool,
}

/// The full parsed result of `cursor agent --list-models`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorModelList {
    pub models: Vec<CursorModelInfo>,
    pub current_model: Option<String>,
    pub default_model: Option<String>,
}

/// Run `cursor agent --list-models` and parse the output.
pub fn list_models() -> Result<CursorModelList, String> {
    let output = Command::new("cursor")
        .args(["agent", "--list-models"])
        .output()
        .map_err(|e| format!("Failed to run `cursor agent --list-models`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`cursor agent --list-models` exited with {}: {}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_list_models_output(&stdout)
}

/// Parse the raw text output into a [`CursorModelList`].
///
/// Each model line has the format: `<id> - <label>  (default)  (current)`
/// where the `(default)` and `(current)` markers are optional.
pub fn parse_list_models_output(output: &str) -> Result<CursorModelList, String> {
    let mut models = Vec::new();
    let mut current_model: Option<String> = None;
    let mut default_model: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Skip header and footer lines
        if line.starts_with("Available models")
            || line.starts_with("Tip:")
            || line.starts_with("---")
        {
            continue;
        }

        let Some((id, rest)) = line.split_once(" - ") else {
            continue;
        };

        let id = id.trim().to_string();
        let is_default = rest.contains("(default)");
        let is_current = rest.contains("(current)");

        let label = rest
            .replace("(default)", "")
            .replace("(current)", "")
            .trim()
            .to_string();

        if is_default {
            default_model = Some(id.clone());
        }
        if is_current {
            current_model = Some(id.clone());
        }

        models.push(CursorModelInfo {
            id,
            label,
            is_default,
            is_current,
        });
    }

    if models.is_empty() {
        return Err("No models found in `cursor agent --list-models` output".to_string());
    }

    Ok(CursorModelList {
        models,
        current_model,
        default_model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = r#"Available models

auto - Auto
composer-1.5 - Composer 1.5
composer-1 - Composer 1
gpt-5.4 - GPT-5.4
gpt-5.3-codex - GPT-5.3 Codex
opus-4.6-thinking - Claude 4.6 Opus (Thinking)  (default)
opus-4.6 - Claude 4.6 Opus
sonnet-4.5-thinking - Claude 4.5 Sonnet (Thinking)  (current)
sonnet-4.5 - Claude 4.5 Sonnet
gemini-3-flash - Gemini 3 Flash
grok - Grok

Tip: use --model <id> (or /model <id> in interactive mode) to switch.
"#;

    #[test]
    fn parse_sample_output() {
        let result = parse_list_models_output(SAMPLE_OUTPUT).unwrap();
        assert_eq!(result.models.len(), 11);
        assert_eq!(result.current_model.as_deref(), Some("sonnet-4.5-thinking"));
        assert_eq!(result.default_model.as_deref(), Some("opus-4.6-thinking"));
    }

    #[test]
    fn parse_identifies_flags() {
        let result = parse_list_models_output(SAMPLE_OUTPUT).unwrap();
        let opus = result.models.iter().find(|m| m.id == "opus-4.6-thinking").unwrap();
        assert!(opus.is_default);
        assert!(!opus.is_current);
        assert_eq!(opus.label, "Claude 4.6 Opus (Thinking)");

        let sonnet = result.models.iter().find(|m| m.id == "sonnet-4.5-thinking").unwrap();
        assert!(!sonnet.is_default);
        assert!(sonnet.is_current);
    }

    #[test]
    fn parse_strips_markers_from_label() {
        let result = parse_list_models_output(SAMPLE_OUTPUT).unwrap();
        let auto = result.models.iter().find(|m| m.id == "auto").unwrap();
        assert_eq!(auto.label, "Auto");
        assert!(!auto.is_default);
        assert!(!auto.is_current);
    }

    #[test]
    fn parse_empty_output_is_error() {
        let result = parse_list_models_output("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_header_only_is_error() {
        let result = parse_list_models_output("Available models\n\n");
        assert!(result.is_err());
    }

    #[test]
    fn parse_no_current_or_default() {
        let output = "auto - Auto\ngrok - Grok\n";
        let result = parse_list_models_output(output).unwrap();
        assert_eq!(result.models.len(), 2);
        assert!(result.current_model.is_none());
        assert!(result.default_model.is_none());
    }

    #[test]
    fn parse_both_flags_on_same_model() {
        let output = "opus-4.6 - Opus 4.6  (default)  (current)\n";
        let result = parse_list_models_output(output).unwrap();
        assert_eq!(result.models.len(), 1);
        assert_eq!(result.current_model.as_deref(), Some("opus-4.6"));
        assert_eq!(result.default_model.as_deref(), Some("opus-4.6"));
        assert!(result.models[0].is_default);
        assert!(result.models[0].is_current);
    }
}
