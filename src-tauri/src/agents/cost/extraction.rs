//! Claude Code stream-json cost extraction.

use std::collections::HashMap;

use super::{normalize_model_name, ModelCostData, RunCostData};

/// Extract cost/usage data from Claude Code's stream-json output.
///
/// The `result` type JSON line contains usage and modelUsage fields:
/// ```json
/// {"type":"result","result":"text...","usage":{"input_tokens":1234,"output_tokens":5678,
///   "cache_read_input_tokens":900,"cache_creation_input_tokens":200,"total_cost_usd":0.12},
///  "modelUsage":{"claude-opus-4-6":{"inputTokens":1234,"outputTokens":5678,"costUSD":0.12}}}
/// ```
pub fn extract_cost_from_stream_json(stream_output: &str) -> Option<RunCostData> {
    for line in stream_output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                return parse_cost_from_result_json(&json);
            }
        }
    }
    None
}

/// Parse cost data from a `result` type JSON object.
fn parse_cost_from_result_json(json: &serde_json::Value) -> Option<RunCostData> {
    let usage = json.get("usage")?;

    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_creation_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_cost_usd = usage
        .get("total_cost_usd")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut model_usage = HashMap::new();
    if let Some(model_usage_json) = json.get("modelUsage").and_then(|v| v.as_object()) {
        for (model_name, model_data) in model_usage_json {
            let data = ModelCostData {
                input_tokens: model_data
                    .get("inputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                output_tokens: model_data
                    .get("outputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cache_read_tokens: model_data
                    .get("cacheReadInputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cache_creation_tokens: model_data
                    .get("cacheCreationInputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cost_usd: model_data
                    .get("costUSD")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            };
            // Normalize model name so that e.g. "claude-opus-4-6" and "opus-4.6"
            // are merged under the same key.
            let canonical = normalize_model_name(model_name);
            let entry = model_usage.entry(canonical).or_insert_with(ModelCostData::default);
            entry.input_tokens += data.input_tokens;
            entry.output_tokens += data.output_tokens;
            entry.cache_read_tokens += data.cache_read_tokens;
            entry.cache_creation_tokens += data.cache_creation_tokens;
            entry.cost_usd += data.cost_usd;
        }
    }

    // Always derive total_cost_usd from the sum of per-model costs.
    // This is the single source of truth — it guarantees the total shown
    // in the UI always equals the sum of the "By model" breakdown.
    let total_cost_usd: f64 = if model_usage.is_empty() {
        // No model breakdown available; fall back to the API-level total
        // so we don't lose cost data entirely.
        total_cost_usd
    } else {
        model_usage.values().map(|d| d.cost_usd).sum()
    };

    if input_tokens == 0
        && output_tokens == 0
        && cache_read_tokens == 0
        && cache_creation_tokens == 0
        && total_cost_usd == 0.0
    {
        return None;
    }

    Some(RunCostData {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        total_cost_usd,
        model_usage,
        is_estimated: false,
    })
}
