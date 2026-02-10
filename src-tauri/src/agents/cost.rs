//! Cost extraction and estimation for agent runs.
//!
//! - Claude Code CLI: parses token usage and cost from stream-json `result` messages.
//! - Cursor CLI: estimates cost based on model pricing tables (no native cost data available).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cost and token usage data for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunCostData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    /// Per-model breakdown (model name -> cost data)
    #[serde(default)]
    pub model_usage: HashMap<String, ModelCostData>,
    /// Whether the cost is an estimate (true for Cursor) or authoritative (false for Claude)
    pub is_estimated: bool,
}

/// Per-model cost breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCostData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

/// Aggregated cost across multiple runs (for ticket or board summaries).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedCost {
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub run_count: u32,
    pub estimated_count: u32,
    /// Per-model totals across all runs
    #[serde(default)]
    pub model_totals: HashMap<String, ModelCostData>,
}

impl AggregatedCost {
    /// Add a run's cost data to the aggregate.
    pub fn add(&mut self, cost: &RunCostData) {
        self.total_cost_usd += cost.total_cost_usd;
        self.total_input_tokens += cost.input_tokens;
        self.total_output_tokens += cost.output_tokens;
        self.total_cache_read_tokens += cost.cache_read_tokens;
        self.total_cache_creation_tokens += cost.cache_creation_tokens;
        self.run_count += 1;
        if cost.is_estimated {
            self.estimated_count += 1;
        }

        for (model, data) in &cost.model_usage {
            let entry = self.model_totals.entry(model.clone()).or_default();
            entry.input_tokens += data.input_tokens;
            entry.output_tokens += data.output_tokens;
            entry.cache_read_tokens += data.cache_read_tokens;
            entry.cache_creation_tokens += data.cache_creation_tokens;
            entry.cost_usd += data.cost_usd;
        }
    }
}

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

    // Parse per-model breakdown from modelUsage
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
            model_usage.insert(model_name.clone(), data);
        }
    }

    // Only return if we found meaningful data
    if input_tokens == 0 && output_tokens == 0 && total_cost_usd == 0.0 {
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

/// Model pricing per million tokens (USD).
struct ModelPricing {
    input_per_mtok: f64,
    output_per_mtok: f64,
}

/// Get pricing for a model. Falls back to Sonnet pricing if unknown.
fn get_model_pricing(model: &str) -> ModelPricing {
    // Normalize model name for matching
    let normalized = model.to_lowercase().replace(['-', '_'], " ");

    if normalized.contains("opus") {
        // Claude Opus 4.x pricing
        ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
        }
    } else if normalized.contains("sonnet") {
        // Claude Sonnet 4.x pricing
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }
    } else if normalized.contains("haiku") {
        // Claude Haiku pricing
        ModelPricing {
            input_per_mtok: 0.25,
            output_per_mtok: 1.25,
        }
    } else {
        // Default to Sonnet pricing as a reasonable middle ground
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }
    }
}

/// Estimate cost for a Cursor run based on model and output size.
///
/// This is a rough estimate since Cursor does not expose token usage data.
/// Uses ~4 characters per token as an approximation.
pub fn estimate_cost(model: &str, output_chars: usize, duration_secs: f64) -> RunCostData {
    let pricing = get_model_pricing(model);

    // Rough token estimation: ~4 chars per token for English text
    let estimated_output_tokens = (output_chars as f64 / 4.0) as u64;

    // Estimate input tokens based on duration and typical throughput
    // Longer runs typically process more input context
    // Rough heuristic: ~500 tokens/second of input processing
    let estimated_input_tokens = (duration_secs * 500.0) as u64;

    let input_cost = estimated_input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
    let output_cost = estimated_output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
    let total_cost = input_cost + output_cost;

    let model_name = model.to_string();
    let mut model_usage = HashMap::new();
    model_usage.insert(
        model_name,
        ModelCostData {
            input_tokens: estimated_input_tokens,
            output_tokens: estimated_output_tokens,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: total_cost,
        },
    );

    RunCostData {
        input_tokens: estimated_input_tokens,
        output_tokens: estimated_output_tokens,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        total_cost_usd: total_cost,
        model_usage,
        is_estimated: true,
    }
}

/// Extract cost from agent output, trying Claude parsing first, then falling back to estimation.
pub fn extract_or_estimate_cost(
    stdout: &str,
    model: &str,
    duration_secs: f64,
    is_claude: bool,
) -> Option<RunCostData> {
    // For Claude, try to parse authoritative cost data from stream-json
    if is_claude {
        if let Some(cost) = extract_cost_from_stream_json(stdout) {
            return Some(cost);
        }
    }

    // Fall back to estimation based on output size
    let output_chars = stdout.len();
    if output_chars > 0 || duration_secs > 0.0 {
        Some(estimate_cost(model, output_chars, duration_secs))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cost_from_claude_result() {
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}
{"type":"result","result":"Hello world","usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":100,"total_cost_usd":0.0525},"modelUsage":{"claude-opus-4-6":{"inputTokens":1000,"outputTokens":500,"cacheReadInputTokens":200,"cacheCreationInputTokens":100,"costUSD":0.0525}}}"#;

        let cost = extract_cost_from_stream_json(stream_output).unwrap();
        assert_eq!(cost.input_tokens, 1000);
        assert_eq!(cost.output_tokens, 500);
        assert_eq!(cost.cache_read_tokens, 200);
        assert_eq!(cost.cache_creation_tokens, 100);
        assert!((cost.total_cost_usd - 0.0525).abs() < 0.0001);
        assert!(!cost.is_estimated);

        let model = cost.model_usage.get("claude-opus-4-6").unwrap();
        assert_eq!(model.input_tokens, 1000);
        assert_eq!(model.output_tokens, 500);
        assert!((model.cost_usd - 0.0525).abs() < 0.0001);
    }

    #[test]
    fn parse_cost_missing_usage_returns_none() {
        let stream_output = r#"{"type":"result","result":"Hello world"}"#;
        let cost = extract_cost_from_stream_json(stream_output);
        assert!(cost.is_none());
    }

    #[test]
    fn parse_cost_zero_tokens_returns_none() {
        let stream_output = r#"{"type":"result","result":"","usage":{"input_tokens":0,"output_tokens":0,"total_cost_usd":0.0}}"#;
        let cost = extract_cost_from_stream_json(stream_output);
        assert!(cost.is_none());
    }

    #[test]
    fn parse_cost_no_result_line() {
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"hello"}}}"#;
        let cost = extract_cost_from_stream_json(stream_output);
        assert!(cost.is_none());
    }

    #[test]
    fn parse_cost_partial_usage() {
        let stream_output =
            r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50}}"#;
        let cost = extract_cost_from_stream_json(stream_output).unwrap();
        assert_eq!(cost.input_tokens, 100);
        assert_eq!(cost.output_tokens, 50);
        assert_eq!(cost.cache_read_tokens, 0);
        assert_eq!(cost.total_cost_usd, 0.0);
        assert!(!cost.is_estimated);
    }

    #[test]
    fn estimate_cost_opus() {
        let cost = estimate_cost("claude-opus-4-6", 4000, 10.0);
        assert!(cost.is_estimated);
        assert!(cost.total_cost_usd > 0.0);
        assert_eq!(cost.output_tokens, 1000); // 4000 chars / 4
        assert!(cost.model_usage.contains_key("claude-opus-4-6"));
    }

    #[test]
    fn estimate_cost_sonnet() {
        let cost_opus = estimate_cost("opus-4.6", 4000, 10.0);
        let cost_sonnet = estimate_cost("sonnet-4.5", 4000, 10.0);
        // Opus should be more expensive than Sonnet
        assert!(cost_opus.total_cost_usd > cost_sonnet.total_cost_usd);
    }

    #[test]
    fn estimate_cost_zero_output() {
        let cost = estimate_cost("sonnet-4.5", 0, 5.0);
        assert!(cost.is_estimated);
        assert_eq!(cost.output_tokens, 0);
        // Should still have input tokens from duration
        assert!(cost.input_tokens > 0);
    }

    #[test]
    fn extract_or_estimate_prefers_parsed() {
        let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.01}}"#;
        let cost =
            extract_or_estimate_cost(stream_output, "opus-4.6", 10.0, true).unwrap();
        // Should use parsed data, not estimation
        assert!(!cost.is_estimated);
        assert_eq!(cost.input_tokens, 100);
    }

    #[test]
    fn extract_or_estimate_falls_back_for_cursor() {
        let plain_output = "This is plain text from Cursor agent";
        let cost = extract_or_estimate_cost(plain_output, "opus-4.6", 10.0, false).unwrap();
        assert!(cost.is_estimated);
        assert!(cost.total_cost_usd > 0.0);
    }

    #[test]
    fn aggregated_cost_add() {
        let mut agg = AggregatedCost::default();

        let cost1 = RunCostData {
            input_tokens: 100,
            output_tokens: 50,
            total_cost_usd: 0.01,
            is_estimated: false,
            ..Default::default()
        };
        let cost2 = RunCostData {
            input_tokens: 200,
            output_tokens: 100,
            total_cost_usd: 0.02,
            is_estimated: true,
            ..Default::default()
        };

        agg.add(&cost1);
        agg.add(&cost2);

        assert_eq!(agg.run_count, 2);
        assert_eq!(agg.estimated_count, 1);
        assert_eq!(agg.total_input_tokens, 300);
        assert_eq!(agg.total_output_tokens, 150);
        assert!((agg.total_cost_usd - 0.03).abs() < 0.0001);
    }

    #[test]
    fn cost_data_serialization_roundtrip() {
        let cost = RunCostData {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_creation_tokens: 100,
            total_cost_usd: 0.05,
            model_usage: HashMap::new(),
            is_estimated: false,
        };
        let json = serde_json::to_string(&cost).unwrap();
        let parsed: RunCostData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.input_tokens, 1000);
        assert_eq!(parsed.output_tokens, 500);
        assert!((parsed.total_cost_usd - 0.05).abs() < 0.001);
    }
}
